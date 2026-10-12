//! Safe staging for local IINA-compatible plugin archives.

use riina_core::plugin::Manifest;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use zip::ZipArchive;

const MAX_ENTRIES: usize = 10_000;
const MAX_UNPACKED_BYTES: u64 = 512 * 1024 * 1024;
static NEXT_STAGE: AtomicU64 = AtomicU64::new(0);

pub(crate) struct StagedPackage {
    stage_directory: PathBuf,
    package_directory: PathBuf,
    pub manifest: Manifest,
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
        let backup =
            plugins_directory.join(unique_stage_name().replace("plugin-stage", "plugin-backup"));
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
    fs::create_dir_all(plugins_directory).map_err(|error| error.to_string())?;
    let stage_directory = plugins_directory.join(unique_stage_name());
    fs::create_dir(&stage_directory).map_err(|error| error.to_string())?;
    let result = extract_validate(archive, &stage_directory).and_then(|package_directory| {
        Manifest::load_from_directory(&package_directory)
            .map(|manifest| (package_directory, manifest))
    });
    match result {
        Ok((package_directory, manifest)) => Ok(StagedPackage {
            stage_directory,
            package_directory,
            manifest,
        }),
        Err(error) => {
            let _ = fs::remove_dir_all(&stage_directory);
            Err(error)
        }
    }
}

pub(crate) fn stage_archive_bytes(
    bytes: Vec<u8>,
    plugins_directory: &Path,
) -> Result<StagedPackage, String> {
    fs::create_dir_all(plugins_directory).map_err(|error| error.to_string())?;
    let stage_directory = plugins_directory.join(unique_stage_name());
    fs::create_dir(&stage_directory).map_err(|error| error.to_string())?;
    let result = extract_validate(std::io::Cursor::new(bytes), &stage_directory).and_then(
        |package_directory| {
            Manifest::load_from_directory(&package_directory)
                .map(|manifest| (package_directory, manifest))
        },
    );
    match result {
        Ok((package_directory, manifest)) => Ok(StagedPackage {
            stage_directory,
            package_directory,
            manifest,
        }),
        Err(error) => {
            let _ = fs::remove_dir_all(&stage_directory);
            Err(error)
        }
    }
}

fn extract_validate<R: Read + Seek>(source: R, stage: &Path) -> Result<PathBuf, String> {
    let mut archive =
        ZipArchive::new(source).map_err(|error| format!("Invalid plugin archive: {error}"))?;
    if archive.is_empty() || archive.len() > MAX_ENTRIES {
        return Err("Plugin archive has an invalid number of entries".into());
    }
    let mut unpacked = 0_u64;
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
        unpacked = unpacked
            .checked_add(entry.size())
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
        let written = io::copy(&mut entry, &mut output).map_err(|error| error.to_string())?;
        output.flush().map_err(|error| error.to_string())?;
        if written != entry.size() {
            return Err(format!(
                "Plugin archive entry {} was truncated",
                entry.name()
            ));
        }
    }
    find_manifest_root(stage)
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

fn unique_stage_name() -> String {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(
        ".plugin-stage-{}-{stamp}-{}",
        std::process::id(),
        NEXT_STAGE.fetch_add(1, Ordering::Relaxed)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Cursor,
        sync::atomic::{AtomicU64, Ordering},
    };
    use zip::{ZipWriter, write::SimpleFileOptions};

    static NEXT_TEST: AtomicU64 = AtomicU64::new(0);

    fn temp_directory() -> PathBuf {
        let id = NEXT_TEST.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("riina-plugin-package-{}-{id}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        path
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
        let base = temp_directory();
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
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn archive_rejects_path_traversal_duplicate_manifests_and_missing_entry() {
        let base = temp_directory();
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
        fs::remove_dir_all(base).unwrap();
    }
}
