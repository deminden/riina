//! Compare the installed build to the selected Riina release channel.
use super::schedule::Channel;
use crate::{
    http::{RequestPolicy, agent_config},
    platform::update_installer::{Assets, Package},
};
use semver::Version;
use serde::Deserialize;
use std::{cmp::Ordering, time::Duration};

const RECENT: &str = "https://api.github.com/repos/deminden/riina/releases?per_page=30";
const TAG_PAGE: &str = "https://github.com/deminden/riina/releases/tag/";
const METADATA_LIMIT: u64 = 2 * 1024 * 1024;
const SOURCE_BASELINE: Version = Version::new(0, 5, 0);
const RETIRED_VERSIONS: [Version; 3] = [
    Version::new(4, 1, 0),
    Version::new(4, 2, 0),
    Version::new(4, 2, 1),
];

#[derive(Deserialize)]
struct Record {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    #[serde(default)]
    assets: Assets,
}

pub(super) struct Release {
    pub(super) version: Version,
    pub(super) url: String,
    pub(super) installer: Option<Package>,
}

pub(super) enum Outcome {
    Available(Release),
    Current(Version),
}

pub(super) fn check(channel: Channel) -> Result<Outcome, String> {
    let installed = Version::parse(env!("CARGO_PKG_VERSION")).map_err(|error| error.to_string())?;
    compare(&fetch_release_data(RECENT)?, &installed, channel)
}

fn fetch_release_data(url: &str) -> Result<Vec<u8>, String> {
    let mut response = agent_config(
        Duration::from_secs(12),
        RequestPolicy::HttpsWithoutRedirects,
    )
    .new_agent()
    .get(url)
    .header("Accept", "application/vnd.github+json")
    .call()
    .map_err(|error| {
        crate::http::format_api_error(
            "GitHub",
            "release check",
            error,
            &[
                (403, "GitHub denied the request. Try again later."),
                (429, "GitHub rate limit reached. Try again later."),
            ],
        )
    })?;
    response
        .body_mut()
        .with_config()
        .limit(METADATA_LIMIT)
        .read_to_vec()
        .map_err(|error| format!("Could not read release information: {error}"))
}

impl Record {
    fn release(self, channel: Channel) -> Option<Release> {
        let tag = self.tag_name;
        let version = Version::parse(tag.strip_prefix('v').unwrap_or(&tag)).ok()?;
        if self.draft
            || (channel == Channel::Stable && (self.prerelease || !version.pre.is_empty()))
        {
            return None;
        }
        // Validated SemVer limits the path segment. Only open the project release page.
        Some(Release {
            installer: Package::select(self.assets, &tag, &version),
            version,
            url: format!("{TAG_PAGE}{tag}"),
        })
    }
}

fn compare(data: &[u8], installed: &Version, channel: Channel) -> Result<Outcome, String> {
    let release = serde_json::from_slice::<Vec<Record>>(data)
        .map_err(|_| "GitHub returned invalid release information.".to_owned())?
        .into_iter()
        .filter_map(|record| record.release(channel))
        // 0.5.0 resumes the original version series after the 4.x releases.
        .filter(|release| {
            release.version.cmp_precedence(&SOURCE_BASELINE) != Ordering::Less
                && !RETIRED_VERSIONS.contains(&release.version)
        })
        .max_by(|left, right| left.version.cmp_precedence(&right.version))
        .ok_or("GitHub did not return a supported release for this channel.")?;
    Ok(
        if release.version.cmp_precedence(installed) == Ordering::Greater {
            Outcome::Available(release)
        } else {
            Outcome::Current(release.version)
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "windows")]
    #[test]
    #[ignore = "downloads the current published Windows installer without running it"]
    fn verifies_the_live_windows_installer() {
        let data =
            fetch_release_data("https://api.github.com/repos/deminden/riina/releases/latest")
                .unwrap();
        let release = serde_json::from_slice::<Record>(&data)
            .unwrap()
            .release(Channel::Stable)
            .unwrap();
        let downloaded = release
            .installer
            .expect("Published release has no verified installer offer")
            .download(&std::sync::atomic::AtomicBool::new(false))
            .unwrap();
        downloaded.lock_verified().unwrap();
    }

    fn record(tag: &str, draft: bool, prerelease: bool) -> serde_json::Value {
        serde_json::json!({
            "tag_name": tag,
            "draft": draft,
            "prerelease": prerelease,
            "html_url": "https://example.invalid/elsewhere"
        })
    }

    #[test]
    fn compares_version_precedence_and_only_links_to_the_project() {
        let installed = Version::parse("4.2.9").unwrap();
        let Outcome::Available(release) = compare(
            &serde_json::to_vec(&[record("v4.2.10", false, false)]).unwrap(),
            &installed,
            Channel::Stable,
        )
        .unwrap() else {
            panic!("4.2.10 should follow 4.2.9");
        };
        assert_eq!(release.version, Version::new(4, 2, 10));
        assert_eq!(release.url, format!("{TAG_PAGE}v4.2.10"));
        for tag in ["v4.2.8", "4.2.9+build.2"] {
            assert!(matches!(
                compare(
                    &serde_json::to_vec(&[record(tag, false, false)]).unwrap(),
                    &installed,
                    Channel::Stable
                ),
                Ok(Outcome::Current(_))
            ));
        }
    }

    #[test]
    fn rejects_drafts_prereleases_and_malformed_tags() {
        let installed = Version::new(4, 2, 1);
        for (tag, draft, beta) in [
            ("v5.0.0", true, false),
            ("v5.0.0", false, true),
            ("v5.0.0-beta.1", false, false),
            ("v5.0.0/other", false, false),
        ] {
            let data = serde_json::to_vec(&[record(tag, draft, beta)]).unwrap();
            assert!(compare(&data, &installed, Channel::Stable).is_err());
        }
    }

    #[test]
    fn selects_the_highest_published_version_across_stable_and_prereleases() {
        let recent = [
            record("v6.0.0", true, false),
            record("not-semver", false, true),
            record("v5.0.0-beta.2", false, true),
            record("v4.2.3", false, false),
            record("v5.0.0-beta.10", false, true),
        ];
        let data = serde_json::to_vec(&recent).unwrap();
        let Outcome::Available(release) =
            compare(&data, &Version::new(4, 2, 1), Channel::Prereleases).unwrap()
        else {
            panic!("Latest prerelease should be offered");
        };
        assert_eq!(release.version, Version::parse("5.0.0-beta.10").unwrap());
        assert_eq!(release.url, format!("{TAG_PAGE}v5.0.0-beta.10"));
        assert!(matches!(
            compare(&data, &Version::new(5, 0, 0), Channel::Prereleases),
            Ok(Outcome::Current(_))
        ));
    }

    #[test]
    fn reset_version_series_excludes_the_previous_windows_releases() {
        let data = serde_json::to_vec(&[
            record("v4.2.1", false, false),
            record("v4.2.0", false, false),
            record("v4.1.0", false, false),
            record("v0.5.0", false, false),
            record("v0.5.1-beta.1", false, true),
        ])
        .unwrap();
        assert!(matches!(
            compare(&data, &SOURCE_BASELINE, Channel::Stable),
            Ok(Outcome::Current(version)) if version == SOURCE_BASELINE
        ));
        assert!(matches!(
            compare(&data, &SOURCE_BASELINE, Channel::Prereleases),
            Ok(Outcome::Available(release))
                if release.version == Version::parse("0.5.1-beta.1").unwrap()
        ));
    }
}
