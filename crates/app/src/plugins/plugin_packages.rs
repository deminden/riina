//! Safe staging for local IINA-compatible plugin archives.

use riina_core::plugin::Manifest;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use zip::ZipArchive;

const MAX_ENTRIES: usize = 10_000;
const MAX_UNPACKED_BYTES: u64 = 512 * 1024 * 1024;
const STALE_STAGE_AGE: Duration = Duration::from_secs(24 * 60 * 60);
const STAGE_PREFIX: &str = ".plugin-stage-";
const BACKUP_PREFIX: &str = ".plugin-backup-";
static NEXT_STAGE: AtomicU64 = AtomicU64::new(0);

pub(crate) struct StagedPackage {
    stage_directory: PathBuf,
    package_directory: PathBuf,
    _lease: StageLease,
    pub manifest: Manifest,
}

struct StageLease {
    path: PathBuf,
    file: Option<File>,
}

impl StageLease {
    fn acquire(stage: &Path) -> io::Result<Self> {
        let path = stage.with_extension("lease");
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        let lease = Self {
            path,
            file: Some(file),
        };
        if let Some(file) = &lease.file {
            file.lock()?;
        }
        Ok(lease)
    }
}

impl Drop for StageLease {
    fn drop(&mut self) {
        drop(self.file.take());
        let _ = fs::remove_file(&self.path);
    }
}

impl StagedPackage {
    /// Move the validated package into place only after the user accepts its permissions.
    pub fn install(self, plugins_directory: &Path) -> Result<PathBuf, String> {
        fs::create_dir_all(plugins_directory).map_err(|error| error.to_string())?;
        let destination =
            plugins_directory.join(format!("{}.iinaplugin", self.manifest.identifier));
        if destination.exists() {
            return Err(format!(
                "A plugin with identifier {} is already installed",
                self.manifest.identifier
            ));
        }
        fs::rename(&self.package_directory, &destination)
            .map_err(|error| format!("Could not install plugin package: {error}"))?;
        Ok(destination)
    }

    pub fn replace(self, plugins_directory: &Path) -> Result<PackageReplacement, String> {
        fs::create_dir_all(plugins_directory).map_err(|error| error.to_string())?;
        let destination =
            plugins_directory.join(format!("{}.iinaplugin", self.manifest.identifier));
        let metadata = fs::symlink_metadata(&destination)
            .map_err(|error| format!("Could not locate the installed plugin: {error}"))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err("The installed plugin path is not a regular directory".into());
        }
        let backup = plugins_directory.join(unique_directory_name(BACKUP_PREFIX));
        fs::rename(&destination, &backup)
            .map_err(|error| format!("Could not preserve the installed plugin: {error}"))?;
        if let Err(error) = fs::rename(&self.package_directory, &destination) {
            return match fs::rename(&backup, &destination) {
                Ok(()) => Err(format!("Could not install the plugin update: {error}")),
                Err(restore_error) => Err(format!(
                    "Could not install the plugin update: {error}. The previous package remains at {}; restore failed: {restore_error}",
                    backup.display()
                )),
            };
        }
        Ok(PackageReplacement {
            destination,
            backup: Some(backup),
        })
    }
}

pub(crate) struct PackageReplacement {
    destination: PathBuf,
    backup: Option<PathBuf>,
}

impl PackageReplacement {
    pub fn commit(mut self) -> Result<(), String> {
        let Some(backup) = self.backup.take() else {
            return Ok(());
        };
        fs::remove_dir_all(&backup)
            .map_err(|error| format!("Could not remove the previous plugin version: {error}"))?;
        Ok(())
    }

    pub fn rollback(mut self) -> Result<(), String> {
        let Some(backup) = self.backup.as_ref() else {
            return Ok(());
        };
        fs::remove_dir_all(&self.destination).map_err(|error| {
            format!("Could not remove the updated plugin during rollback: {error}")
        })?;
        fs::rename(backup, &self.destination)
            .map_err(|error| format!("Could not restore the previous plugin version: {error}"))?;
        self.backup = None;
        Ok(())
    }
}

impl Drop for PackageReplacement {
    fn drop(&mut self) {
        if let Some(backup) = self.backup.take() {
            let _ = fs::remove_dir_all(&self.destination);
            let _ = fs::rename(backup, &self.destination);
        }
    }
}

impl Drop for StagedPackage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.stage_directory);
    }
}

pub(crate) fn stage_archive(
    path: &Path,
    plugins_directory: &Path,
) -> Result<StagedPackage, String> {
    let metadata =
        fs::metadata(path).map_err(|error| format!("Could not read package: {error}"))?;
    if !metadata.is_file() {
        return Err("The plugin package must be a file".into());
    }
    let archive = File::open(path).map_err(|error| format!("Could not read package: {error}"))?;
    stage(archive, plugins_directory)
}

pub(crate) fn stage_archive_bytes(
    bytes: Vec<u8>,
    plugins_directory: &Path,
) -> Result<StagedPackage, String> {
    stage(io::Cursor::new(bytes), plugins_directory)
}

fn stage(source: impl Read + Seek, plugins_directory: &Path) -> Result<StagedPackage, String> {
    let locale = crate::platform::language::current_locale_identifier();
    fs::create_dir_all(plugins_directory).map_err(|error| error.to_string())?;
    let stage_directory = plugins_directory.join(unique_directory_name(STAGE_PREFIX));
    fs::create_dir(&stage_directory).map_err(|error| error.to_string())?;
    let lease = match StageLease::acquire(&stage_directory) {
        Ok(lease) => lease,
        Err(error) => {
            let _ = fs::remove_dir(&stage_directory);
            return Err(error.to_string());
        }
    };
    let result = extract_validate(source, &stage_directory).and_then(|package_directory| {
        Manifest::load_from_directory_with_locale(&package_directory, locale.as_deref())
            .map(|manifest| (package_directory, manifest))
    });
    match result {
        Ok((package_directory, manifest)) => Ok(StagedPackage {
            stage_directory,
            package_directory,
            _lease: lease,
            manifest,
        }),
        Err(error) => {
            let _ = fs::remove_dir_all(&stage_directory);
            Err(error)
        }
    }
}

pub(crate) fn sweep_staging(plugins_directory: &Path) {
    if let Err(error) = sweep(
        plugins_directory,
        SystemTime::now(),
        |directory, identifier| {
            crate::diagnostics::warn!(
                "Plugin backup {directory} has no installed package for {identifier}"
            );
        },
    ) && error.kind() != io::ErrorKind::NotFound
    {
        crate::diagnostics::warn!("Could not inspect plugin staging directories: {error}");
    }
}

fn sweep(
    plugins_directory: &Path,
    now: SystemTime,
    mut report_backup: impl FnMut(&str, &str),
) -> io::Result<()> {
    let root = plugins_directory.canonicalize()?;
    for item in fs::read_dir(&root)? {
        let item = item?;
        let name = item.file_name();
        let Some(name) = name.to_str() else { continue };
        if !item.file_type()?.is_dir() {
            continue;
        }
        let path = item.path();
        if generated_directory_name(name, STAGE_PREFIX) {
            let old = item
                .metadata()?
                .modified()
                .ok()
                .and_then(|modified| now.duration_since(modified).ok())
                .is_some_and(|age| age > STALE_STAGE_AGE);
            if old {
                sweep_stage(&root, &path)?;
            }
        } else if generated_directory_name(name, BACKUP_PREFIX)
            && let Ok(manifest) = Manifest::load_from_directory(&path)
            && !root
                .join(format!("{}.iinaplugin", manifest.identifier))
                .exists()
        {
            report_backup(name, &manifest.identifier);
        }
    }
    Ok(())
}

fn sweep_stage(root: &Path, path: &Path) -> io::Result<()> {
    if path.canonicalize()?.parent() != Some(root) {
        return Ok(());
    }
    let marker = path.with_extension("lease");
    let lease = match OpenOptions::new().write(true).open(&marker) {
        Ok(file) => {
            // Another running player may still be waiting for confirmation.
            if file.try_lock().is_err() {
                return Ok(());
            }
            Some(file)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    fs::remove_dir_all(path)?;
    if let Some(lease) = lease {
        drop(lease);
        let _ = fs::remove_file(marker);
    }
    Ok(())
}

fn generated_directory_name(name: &str, prefix: &str) -> bool {
    name.strip_prefix(prefix).is_some_and(|suffix| {
        let mut parts = suffix.split('-');
        parts
            .by_ref()
            .take(3)
            .filter(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
            .count()
            == 3
            && parts.next().is_none()
    })
}

fn extract_validate<R: Read + Seek>(source: R, stage: &Path) -> Result<PathBuf, String> {
    let mut archive =
        ZipArchive::new(source).map_err(|error| format!("Invalid plugin archive: {error}"))?;
    if archive.is_empty() || archive.len() > MAX_ENTRIES {
        return Err("Plugin archive has an invalid number of entries".into());
    }
    let mut declared_total = 0_u64;
    let mut written_total = 0_u64;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("Could not read plugin archive entry: {error}"))?;
        let enclosed = entry
            .enclosed_name()
            .ok_or_else(|| format!("Unsafe path in plugin archive: {}", entry.name()))?;
        if enclosed
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(format!("Unsafe path in plugin archive: {}", entry.name()));
        }
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(format!(
                "Plugin archive may not contain symlinks: {}",
                entry.name()
            ));
        }
        let declared = entry.size();
        declared_total = declared_total
            .checked_add(declared)
            .filter(|size| *size <= MAX_UNPACKED_BYTES)
            .ok_or_else(|| "Plugin archive expands beyond the 512 MiB limit".to_owned())?;
        let destination = stage.join(enclosed);
        if entry.is_dir() {
            fs::create_dir_all(&destination).map_err(|error| error.to_string())?;
            continue;
        }
        let parent = destination
            .parent()
            .ok_or_else(|| "Plugin archive entry has no parent directory".to_owned())?;
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
            .map_err(|error| format!("Duplicate or unwritable plugin entry: {error}"))?;
        let remaining = MAX_UNPACKED_BYTES - written_total;
        let written = copy_with_budget(&mut entry, &mut output, remaining)?;
        output.flush().map_err(|error| error.to_string())?;
        if written != declared {
            return Err(format!(
                "Plugin archive entry {} was truncated",
                entry.name()
            ));
        }
        written_total += written;
    }
    find_manifest_root(stage)
}

fn copy_with_budget(
    input: &mut impl Read,
    output: &mut impl Write,
    remaining: u64,
) -> Result<u64, String> {
    let written =
        io::copy(&mut input.take(remaining + 1), output).map_err(|error| error.to_string())?;
    if written > remaining {
        return Err("Plugin archive expands beyond the 512 MiB limit".into());
    }
    Ok(written)
}

fn find_manifest_root(stage: &Path) -> Result<PathBuf, String> {
    if stage.join("Info.json").is_file() {
        return Ok(stage.to_owned());
    }
    let mut candidates = fs::read_dir(stage)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir() && path.join("Info.json").is_file());
    let root = candidates
        .next()
        .ok_or_else(|| "Plugin archive does not contain Info.json".to_owned())?;
    if candidates.next().is_some() {
        return Err("Plugin archive contains multiple package manifests".into());
    }
    Ok(root)
}

fn unique_directory_name(prefix: &str) -> String {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(
        "{prefix}{}-{stamp}-{}",
        std::process::id(),
        NEXT_STAGE.fetch_add(1, Ordering::Relaxed)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use riina_test_support::TempDir;
    use std::io::Cursor;
    use zip::{ZipWriter, write::SimpleFileOptions};

    #[test]
    fn the_copy_budget_stops_reading_after_the_first_excess_byte() {
        let payload = [1, 2, 3, 4, 5, 6];
        let mut source = Cursor::new(payload);
        let mut output = Vec::new();
        assert!(copy_with_budget(&mut source, &mut output, 3).is_err());
        assert_eq!(source.position(), 4);
        assert_eq!(output, [1, 2, 3, 4]);

        let mut source = Cursor::new(payload);
        let mut output = Vec::new();
        assert_eq!(copy_with_budget(&mut source, &mut output, 6).unwrap(), 6);
        assert_eq!(output, payload);
    }

    #[test]
    fn the_sweep_preserves_recent_and_leased_staging_directories() {
        let directory = TempDir::new("plugin-package").unwrap();
        let root = directory.path();
        let live = root.join(unique_directory_name(STAGE_PREFIX));
        let abandoned = root.join(unique_directory_name(STAGE_PREFIX));
        let other = root.join(".plugin-stage-personal");
        for directory in [&live, &abandoned, &other] {
            fs::create_dir(directory).unwrap();
        }
        let marker = abandoned.with_extension("lease");
        fs::write(&marker, []).unwrap();
        let lease = StageLease::acquire(&live).unwrap();

        let now = SystemTime::now();
        sweep(root, now, |_, _| unreachable!()).unwrap();
        assert!(abandoned.exists());

        sweep(
            root,
            now + STALE_STAGE_AGE + Duration::from_secs(60),
            |_, _| unreachable!(),
        )
        .unwrap();
        assert!(live.exists());
        assert!(other.exists());
        assert!(!abandoned.exists());
        assert!(!marker.exists());
        drop(lease);
    }

    #[test]
    fn the_sweep_reports_an_orphaned_backup_and_leaves_it_available() {
        let directory = TempDir::new("plugin-package").unwrap();
        let root = directory.path();
        let backup = root.join(unique_directory_name(BACKUP_PREFIX));
        fs::create_dir(&backup).unwrap();
        fs::write(backup.join("Info.json"), INFO).unwrap();
        fs::write(backup.join("main.js"), "let test = true;").unwrap();
        let mut reported = Vec::new();
        sweep(root, SystemTime::now(), |name, identifier| {
            reported.push((name.to_owned(), identifier.to_owned()));
        })
        .unwrap();
        assert_eq!(
            reported,
            [(
                backup.file_name().unwrap().to_str().unwrap().into(),
                "org.test.plugin".into()
            )]
        );
        assert!(backup.is_dir());
        fs::create_dir(root.join("org.test.plugin.iinaplugin")).unwrap();
        sweep(root, SystemTime::now(), |_, _| unreachable!()).unwrap();
        assert!(backup.is_dir());
    }

    fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, contents) in entries {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(contents).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    const INFO: &[u8] = br#"{"name":"Test","identifier":"org.test.plugin","version":"1","author":{"name":"Tester"},"entry":"main.js","permissions":["video-overlay"]}"#;

    #[test]
    fn archive_is_staged_then_installed_only_after_validation() {
        let directory = TempDir::new("plugin-package").unwrap();
        let base = directory.path();
        let plugins = base.join("plugins");
        fs::create_dir_all(&plugins).unwrap();
        let archive_path = base.join("plugin.iinaplgz");
        fs::write(
            &archive_path,
            archive(&[
                ("Sample/Info.json", INFO),
                ("Sample/main.js", b"console.log('ready')"),
            ]),
        )
        .unwrap();
        let staged = stage_archive(&archive_path, &plugins).unwrap();
        assert_eq!(staged.manifest.identifier, "org.test.plugin");
        assert!(staged.package_directory.join("main.js").is_file());
        let installed = staged.install(&plugins).unwrap();
        assert!(installed.join("Info.json").is_file());
        assert_eq!(fs::read_dir(&plugins).unwrap().count(), 1);
    }

    #[test]
    fn archive_rejects_path_traversal_duplicate_manifests_and_missing_entry() {
        let directory = TempDir::new("plugin-package").unwrap();
        let base = directory.path();
        let plugins = base.join("plugins");
        fs::create_dir_all(&plugins).unwrap();
        for (index, entries) in [
            vec![
                ("../escape", b"bad".as_slice()),
                ("Info.json", INFO),
                ("main.js", b"x"),
            ],
            vec![
                ("a/Info.json", INFO),
                ("a/main.js", b"x"),
                ("b/Info.json", INFO),
            ],
            vec![("Info.json", INFO)],
        ]
        .into_iter()
        .enumerate()
        {
            let path = base.join(format!("bad-{index}.iinaplgz"));
            fs::write(&path, archive(&entries)).unwrap();
            assert!(stage_archive(&path, &plugins).is_err());
        }
        assert_eq!(fs::read_dir(&plugins).unwrap().count(), 0);
    }
}
