use super::*;
use crate::{
    plugin_host::GlobalHost,
    plugin_packages::discover,
    preferences::plugin_preferences::{inventory::read_enabled, permissions::read_plugin_grants},
};
use std::{fs, io::Write, path::Path};
use zip::{ZipWriter, write::SimpleFileOptions};

const IDENTIFIER: &str = "org.test.archive";

struct Fixture {
    profile: Profile,
    archive: PathBuf,
    preferences: PathBuf,
    enabled: PathBuf,
    _sandbox: riina_test_support::TempDir,
}

impl Fixture {
    fn new(host: Option<GlobalHandle>) -> Self {
        let sandbox = riina_test_support::TempDir::new("local-package-review").unwrap();
        let root = sandbox.path();
        let preferences = root.join("preferences.json");
        let enabled = root.join("plugin-enabled.json");
        Self {
            profile: Profile::new(
                root.join("Plugins"),
                preferences.clone(),
                enabled.clone(),
                host,
            ),
            archive: root.join("sample.iinaplgz"),
            preferences,
            enabled,
            _sandbox: sandbox,
        }
    }

    fn review(&self, version: &str, permissions: &[&str]) -> Review {
        write_archive(&self.archive, version, permissions);
        self.profile.review(self.stage().unwrap()).unwrap()
    }

    fn stage(&self) -> Result<StagedPackage, String> {
        Source::Archive(self.archive.clone()).stage(&self.profile)
    }

    fn installed_version(&self) -> Option<String> {
        discover(self.profile.plugins(), None)
            .plugins
            .first()
            .map(|plugin| plugin.manifest.version.clone())
    }

    fn grants(&self) -> Option<Vec<String>> {
        read_plugin_grants(&self.preferences, IDENTIFIER).unwrap()
    }
}

fn write_archive(path: &Path, version: &str, permissions: &[&str]) {
    let mut archive = ZipWriter::new(fs::File::create(path).unwrap());
    archive
        .start_file("Info.json", SimpleFileOptions::default())
        .unwrap();
    serde_json::to_writer(
        &mut archive,
        &serde_json::json!({
            "name": "Review fixture",
            "identifier": IDENTIFIER,
            "version": version,
            "author": {"name": "Tester"},
            "entry": "main.js",
            "globalEntry": "main.js",
            "permissions": permissions,
        }),
    )
    .unwrap();
    archive
        .start_file("main.js", SimpleFileOptions::default())
        .unwrap();
    archive.write_all(b"void 0;").unwrap();
    archive.finish().unwrap();
}

#[test]
fn rejecting_and_discarding_archives_never_changes_the_installed_profile() {
    let fixture = Fixture::new(None);
    fs::write(&fixture.archive, b"invalid").unwrap();
    assert!(fixture.stage().err().unwrap().contains("Package rejected"));

    let review = fixture.review("1", &["video-overlay"]);
    assert!(!review.replaces());
    assert_eq!(review.manifest().permissions, ["video-overlay"]);
    drop(review);
    assert_eq!(fixture.installed_version(), None);
    assert_eq!(fixture.grants(), None);
    assert!(!fixture.enabled.exists());
}

#[test]
fn reviewed_install_and_replacement_enable_supported_packages() {
    let mut fixture = Fixture::new(None);
    let host = GlobalHost::start(&fixture.preferences).unwrap();
    fixture.profile = Profile::new(
        fixture.profile.plugins().to_owned(),
        fixture.preferences.clone(),
        fixture.enabled.clone(),
        Some(host.handle()),
    );
    let installed = fixture
        .profile
        .install(fixture.review("1", &["video-overlay"]))
        .unwrap();
    assert!(!installed.replaced);
    assert!(installed.enabled);
    assert!(read_enabled(&fixture.enabled).unwrap().contains(IDENTIFIER));
    assert_eq!(fixture.installed_version().as_deref(), Some("1"));
    assert_eq!(fixture.grants().unwrap(), ["video-overlay"]);

    let replacement = fixture.review("2", &["show-osd"]);
    assert!(replacement.replaces());
    assert_eq!(fixture.installed_version().as_deref(), Some("1"));
    let installed = fixture.profile.install(replacement).unwrap();
    assert!(installed.replaced);
    assert!(installed.enabled);
    assert_eq!(fixture.installed_version().as_deref(), Some("2"));
    assert_eq!(fixture.grants().unwrap(), ["show-osd"]);
}

#[test]
fn failed_permission_write_restores_the_previous_package() {
    let fixture = Fixture::new(None);
    let installed = fixture
        .profile
        .install(fixture.review("1", &["show-osd"]))
        .unwrap();
    assert!(!installed.enabled);
    let replacement = fixture.review("2", &["video-overlay"]);
    let blocked = fixture
        .preferences
        .with_file_name("plugin-permissions.json.lock");
    fs::remove_file(&blocked).unwrap();
    fs::create_dir(&blocked).unwrap();
    assert!(fixture.profile.install(replacement).is_err());
    assert_eq!(fixture.installed_version().as_deref(), Some("1"));
    assert_eq!(fixture.grants().unwrap(), ["show-osd"]);
    assert!(!read_enabled(&fixture.enabled).unwrap().contains(IDENTIFIER));
}

#[test]
fn stale_review_leaves_a_concurrently_replaced_package_intact() {
    let fixture = Fixture::new(None);
    fixture.profile.install(fixture.review("1", &[])).unwrap();
    let stale = fixture.review("2", &["show-osd"]);
    fixture
        .profile
        .install(fixture.review("3", &["video-overlay"]))
        .unwrap();
    assert!(
        fixture
            .profile
            .install(stale)
            .err()
            .unwrap()
            .contains("changed during review")
    );
    assert_eq!(fixture.installed_version().as_deref(), Some("3"));
    assert_eq!(fixture.grants().unwrap(), ["video-overlay"]);
}
