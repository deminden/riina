//! Permission-aware plugin paths with private data and temporary directories.

mod context;
mod encoding;

pub(crate) use context::PluginPlayerFileContext;
use encoding::decode_text;
use serde::Serialize;
use std::{
    collections::VecDeque,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
};

const MAX_FILE_BYTES: usize = 4 * 1024 * 1024;
const MAX_DOWNLOAD_BYTES: usize = 64 * 1024 * 1024;
const MAX_LIST_ENTRIES: usize = 4096;

pub(crate) struct PluginFileHandle {
    file: File,
    writable: bool,
}

impl PluginFileHandle {
    pub(crate) fn offset(&mut self) -> Result<u64, String> {
        self.file
            .stream_position()
            .map_err(|error| format!("Could not read file offset: {error}"))
    }

    pub(crate) fn seek_to(&mut self, offset: u64) -> Result<(), String> {
        self.file
            .seek(SeekFrom::Start(offset))
            .map(|_| ())
            .map_err(|error| format!("Could not seek in file: {error}"))
    }

    pub(crate) fn seek_to_end(&mut self) -> Result<(), String> {
        self.file
            .seek(SeekFrom::End(0))
            .map(|_| ())
            .map_err(|error| format!("Could not seek to file end: {error}"))
    }

    pub(crate) fn read(&mut self, length: usize) -> Result<Vec<u8>, String> {
        if length > MAX_FILE_BYTES {
            return Err("Handle reads are limited to 4 MiB per call".into());
        }
        let mut bytes = vec![0; length];
        let count = self
            .file
            .read(&mut bytes)
            .map_err(|error| format!("Could not read file: {error}"))?;
        bytes.truncate(count);
        Ok(bytes)
    }

    pub(crate) fn read_to_end(&mut self) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::new();
        (&mut self.file)
            .take((MAX_FILE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("Could not read file: {error}"))?;
        if bytes.len() > MAX_FILE_BYTES {
            return Err("Handle read-to-end is limited to 4 MiB".into());
        }
        Ok(bytes)
    }

    pub(crate) fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        if !self.writable {
            return Err("File handle is not open for writing".into());
        }
        if bytes.len() > MAX_FILE_BYTES {
            return Err("Handle writes are limited to 4 MiB per call".into());
        }
        self.file
            .write_all(bytes)
            .map_err(|error| format!("Could not write file: {error}"))
    }
}

#[derive(Clone)]
pub(crate) struct PluginFiles {
    data_root: PathBuf,
    temp_root: PathBuf,
    player_context: Arc<Mutex<PluginPlayerFileContext>>,
}

#[derive(Serialize)]
struct DirectoryEntry {
    filename: String,
    path: String,
    #[serde(rename = "isDir")]
    is_dir: bool,
}

impl PluginFiles {
    pub(crate) fn for_package(package_root: &Path, identifier: &str) -> Self {
        let plugins_root = package_root.parent().unwrap_or(package_root);
        let identifier = safe_identifier(identifier);
        Self {
            data_root: plugins_root.join(".data").join(&identifier),
            temp_root: std::env::temp_dir().join(format!("riina-{identifier}")),
            player_context: Arc::new(Mutex::new(PluginPlayerFileContext::default())),
        }
    }

    pub(crate) fn temporary(identifier: &str) -> Self {
        let root = std::env::temp_dir().join("riina-plugin-storage");
        let identifier = safe_identifier(identifier);
        Self {
            data_root: root.join(".data").join(&identifier),
            temp_root: root.join(".tmp").join(identifier),
            player_context: Arc::new(Mutex::new(PluginPlayerFileContext::default())),
        }
    }

    pub(crate) fn set_player_context(&self, context: PluginPlayerFileContext) {
        if let Ok(mut current) = self.player_context.lock() {
            *current = context;
        }
    }

    pub(crate) fn core_status_property(
        &self,
        name: &str,
    ) -> Result<Option<serde_json::Value>, String> {
        let context = self
            .player_context
            .lock()
            .map_err(|_| "Player file context was poisoned".to_owned())?;
        Ok(context.core_properties.get(name).cloned())
    }

    pub(crate) fn core_chapters(&self) -> Result<Vec<serde_json::Value>, String> {
        self.player_context
            .lock()
            .map(|context| context.chapters.clone())
            .map_err(|_| "Player file context was poisoned".to_owned())
    }

    pub(crate) fn core_recent_documents(&self) -> Result<Vec<serde_json::Value>, String> {
        self.player_context
            .lock()
            .map(|context| context.recent_documents.clone())
            .map_err(|_| "Player file context was poisoned".to_owned())
    }

    pub(crate) fn core_playback_history(&self) -> Result<Vec<serde_json::Value>, String> {
        self.player_context
            .lock()
            .map(|context| context.playback_history.clone())
            .map_err(|_| "Player file context was poisoned".to_owned())
    }

    pub(crate) fn core_version(&self) -> Result<serde_json::Value, String> {
        self.player_context
            .lock()
            .map(|context| context.core_version.clone())
            .map_err(|_| "Player file context was poisoned".to_owned())
    }

    pub(crate) fn core_window_property(
        &self,
        name: &str,
    ) -> Result<Option<serde_json::Value>, String> {
        let context = self
            .player_context
            .lock()
            .map_err(|_| "Player file context was poisoned".to_owned())?;
        Ok(context.core_window_properties.get(name).cloned())
    }

    pub(crate) fn exists(&self, path: &str, file_system_granted: bool) -> Result<bool, String> {
        let Some(path) = self.resolve(path, file_system_granted, false)? else {
            return Ok(false);
        };
        Ok(path.exists())
    }

    pub(crate) fn open_handle(
        &self,
        path: &str,
        mode: &str,
        file_system_granted: bool,
    ) -> Result<PluginFileHandle, String> {
        let writable = match mode {
            "read" => false,
            "write" => true,
            _ => return Err("File handle mode must be \"read\" or \"write\"".into()),
        };
        let path = self
            .resolve(path, file_system_granted, writable)?
            .ok_or_else(|| "File does not exist".to_owned())?;
        if !path.is_file() {
            return Err("Path is not a regular file".into());
        }
        let file = OpenOptions::new()
            .read(!writable)
            .write(writable)
            .open(path)
            .map_err(|error| format!("Could not open file handle: {error}"))?;
        Ok(PluginFileHandle { file, writable })
    }

    pub(crate) fn existing_path(
        &self,
        path: &str,
        file_system_granted: bool,
    ) -> Result<PathBuf, String> {
        self.resolve(path, file_system_granted, false)?
            .ok_or_else(|| "File does not exist".to_owned())
    }

    pub(crate) fn read_text(
        &self,
        path: &str,
        options: &str,
        file_system_granted: bool,
    ) -> Result<String, String> {
        let options: serde_json::Value = serde_json::from_str(options)
            .map_err(|error| format!("Invalid file options: {error}"))?;
        let encoding = options
            .get("encoding")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("utf8");
        let path = self
            .resolve(path, file_system_granted, false)?
            .ok_or_else(|| "File does not exist".to_owned())?;
        let file = File::open(path).map_err(|error| format!("Could not read file: {error}"))?;
        let metadata = file
            .metadata()
            .map_err(|error| format!("Could not inspect file: {error}"))?;
        if !metadata.is_file() {
            return Err("Path is not a regular file".into());
        }
        if metadata.len() > MAX_FILE_BYTES as u64 {
            return Err("File exceeds the 4 MiB read limit".into());
        }
        // A file can grow after metadata is read. Bound the actual read as well.
        let mut bytes = Vec::new();
        file.take((MAX_FILE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("Could not read file: {error}"))?;
        if bytes.len() > MAX_FILE_BYTES {
            return Err("File exceeds the 4 MiB read limit".into());
        }
        decode_text(&bytes, encoding)
    }

    pub(crate) fn write_text(
        &self,
        path: &str,
        content: &str,
        file_system_granted: bool,
    ) -> Result<(), String> {
        if content.len() > MAX_FILE_BYTES {
            return Err("File content exceeds the 4 MiB write limit".into());
        }
        let path = self
            .resolve(path, file_system_granted, true)?
            .ok_or_else(|| "File destination is invalid".to_owned())?;
        self.open_destination(&path)?
            .write_all(content.as_bytes())
            .map_err(|error| format!("Could not write file: {error}"))
    }

    pub(crate) fn write_download(
        &self,
        path: &str,
        content: &[u8],
        file_system_granted: bool,
    ) -> Result<(), String> {
        if content.len() > MAX_DOWNLOAD_BYTES {
            return Err("Download exceeds the 64 MiB limit".into());
        }
        let path = self
            .resolve(path, file_system_granted, true)?
            .ok_or_else(|| "Download destination is invalid".to_owned())?;
        self.open_destination(&path)?
            .write_all(content)
            .map_err(|error| format!("Could not save download: {error}"))
    }

    fn open_destination(&self, path: &Path) -> Result<File, String> {
        let mut options = OpenOptions::new();
        options.write(true);
        if self.is_private_path(path)? {
            options.create(true).truncate(true);
        } else {
            // Checking existence before writing permits another writer to create
            // the destination in between. The filesystem must enforce exclusivity.
            options.create_new(true);
        }
        options.open(path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                "Cannot overwrite a file outside plugin storage".into()
            } else {
                format!("Could not open file destination: {error}")
            }
        })
    }

    pub(crate) fn move_path(
        &self,
        source: &str,
        destination: &str,
        file_system_granted: bool,
    ) -> Result<(), String> {
        let source = self
            .resolve(source, file_system_granted, false)?
            .ok_or_else(|| "Move source does not exist".to_owned())?;
        let destination = self
            .resolve(destination, file_system_granted, true)?
            .ok_or_else(|| "Move destination is invalid".to_owned())?;
        if source == destination {
            return Err("Move source and destination are the same path".into());
        }
        if destination.exists() {
            return Err("Move destination already exists".into());
        }
        if source == self.canonical_root(&self.data_root)?
            || source == self.canonical_root(&self.temp_root)?
        {
            return Err("Cannot move a plugin storage root".into());
        }
        fs::rename(source, destination).map_err(|error| format!("Could not move file: {error}"))
    }

    pub(crate) fn trash(&self, path: &str, file_system_granted: bool) -> Result<(), String> {
        let path = self
            .resolve(path, file_system_granted, false)?
            .ok_or_else(|| "File does not exist".to_owned())?;
        if path == self.canonical_root(&self.data_root)?
            || path == self.canonical_root(&self.temp_root)?
        {
            return Err("Cannot trash a plugin storage root".into());
        }
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("Could not inspect file: {error}"))?;
        if metadata.file_type().is_symlink() {
            return Err("Cannot trash a symbolic link through the plugin file API".into());
        }
        trash::delete(&path).map_err(|error| format!("Could not move file to Trash: {error}"))
    }

    pub(crate) fn delete(&self, path: &str) -> Result<(), String> {
        let path = self
            .resolve(path, false, true)?
            .ok_or_else(|| "File does not exist".to_owned())?;
        if path == self.canonical_root(&self.data_root)?
            || path == self.canonical_root(&self.temp_root)?
        {
            return Err("Cannot delete a plugin storage root".into());
        }
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("Could not inspect file: {error}"))?;
        if metadata.file_type().is_symlink() {
            return Err("Cannot delete a symbolic link through the plugin file API".into());
        }
        if metadata.is_dir() {
            fs::remove_dir_all(path).map_err(|error| format!("Could not delete directory: {error}"))
        } else {
            fs::remove_file(path).map_err(|error| format!("Could not delete file: {error}"))
        }
    }

    pub(crate) fn list(
        &self,
        path: &str,
        options: &str,
        file_system_granted: bool,
    ) -> Result<String, String> {
        let options: serde_json::Value = serde_json::from_str(options)
            .map_err(|error| format!("Invalid file options: {error}"))?;
        let include_subdirectories = options
            .get("includeSubDir")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let root = self
            .resolve(path, file_system_granted, false)?
            .ok_or_else(|| "Directory does not exist".to_owned())?;
        if !root.is_dir() {
            return Err("Path is not a directory".into());
        }

        let mut pending = VecDeque::from([root.clone()]);
        let mut entries = Vec::new();
        while let Some(directory) = pending.pop_front() {
            for entry in fs::read_dir(&directory)
                .map_err(|error| format!("Could not list directory: {error}"))?
            {
                let entry =
                    entry.map_err(|error| format!("Could not read directory entry: {error}"))?;
                let entry_path = entry.path();
                let metadata = fs::symlink_metadata(&entry_path)
                    .map_err(|error| format!("Could not inspect directory entry: {error}"))?;
                if metadata.file_type().is_symlink() {
                    continue;
                }
                let is_dir = metadata.is_dir();
                let relative = entry_path
                    .strip_prefix(&root)
                    .map_err(|error| format!("Directory entry escaped its root: {error}"))?;
                entries.push(DirectoryEntry {
                    filename: entry.file_name().to_string_lossy().into_owned(),
                    path: relative.to_string_lossy().replace('\\', "/"),
                    is_dir,
                });
                if entries.len() > MAX_LIST_ENTRIES {
                    return Err("Directory listing exceeds the 4096 entry limit".into());
                }
                if is_dir && include_subdirectories {
                    pending.push_back(entry_path);
                }
            }
        }
        entries.sort_by(|left, right| left.path.cmp(&right.path));
        serde_json::to_string(&entries).map_err(|error| error.to_string())
    }

    fn resolve(
        &self,
        value: &str,
        file_system_granted: bool,
        for_write: bool,
    ) -> Result<Option<PathBuf>, String> {
        if let Some(relative) = value.strip_prefix("@current/") {
            if !file_system_granted {
                return Err("The file-system permission is required for @current paths".into());
            }
            let context = self
                .player_context
                .lock()
                .map_err(|_| "Player file context was poisoned".to_owned())?;
            let directory = context
                .current_directory
                .as_ref()
                .ok_or_else(|| "There is no current media file".to_owned())?;
            let path = directory.join(relative);
            if !path.is_absolute() {
                return Err("The current media directory is unavailable".into());
            }
            return self.resolve_external(&path, for_write);
        }
        if let Some((alias, id)) = track_alias(value) {
            let context = self
                .player_context
                .lock()
                .map_err(|_| "Player file context was poisoned".to_owned())?;
            let path = context
                .external_tracks
                .get(&format!("{alias}/{id}"))
                .ok_or_else(|| format!("No external track exists for {alias}/{id}"))?;
            return self.resolve_external(path, for_write);
        }
        if let Some(relative) = value.strip_prefix("@data/") {
            return self.resolve_local(&self.data_root, relative, for_write);
        }
        if let Some(relative) = value.strip_prefix("@tmp/") {
            return self.resolve_local(&self.temp_root, relative, for_write);
        }
        if value == "@data" || value == "@tmp" {
            return Err("Plugin paths must include a directory separator".into());
        }
        if !file_system_granted {
            return Err("The file-system permission is required for absolute paths".into());
        }

        let expanded = if let Some(relative) = value.strip_prefix("~/") {
            dirs::home_dir()
                .ok_or_else(|| "The home directory is unavailable".to_owned())?
                .join(relative)
        } else {
            PathBuf::from(value)
        };
        if !expanded.is_absolute() {
            return Err("File paths must be absolute or use @data/ and @tmp/".into());
        }
        self.resolve_external(&expanded, for_write)
    }

    fn resolve_local(
        &self,
        root: &Path,
        relative: &str,
        for_write: bool,
    ) -> Result<Option<PathBuf>, String> {
        let relative = normalize_private_relative(Path::new(relative))?;
        let root = self.canonical_root(root)?;
        let candidate = root.join(relative);
        self.validate_containment(&candidate, &root, for_write)
    }

    fn resolve_external(&self, path: &Path, for_write: bool) -> Result<Option<PathBuf>, String> {
        self.validate_containment(path, &PathBuf::new(), for_write)
    }

    fn validate_containment(
        &self,
        path: &Path,
        root: &Path,
        for_write: bool,
    ) -> Result<Option<PathBuf>, String> {
        if !root.as_os_str().is_empty()
            && fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink())
        {
            return Err("Symbolic links are not supported in plugin storage".into());
        }
        if path.exists() {
            let canonical = path
                .canonicalize()
                .map_err(|error| format!("Could not resolve file path: {error}"))?;
            if !root.as_os_str().is_empty() && !canonical.starts_with(root) {
                return Err("Plugin path resolves outside its storage directory".into());
            }
            return Ok(Some(canonical));
        }
        if !for_write {
            let mut ancestor = path;
            while !ancestor.exists() {
                let Some(parent) = ancestor.parent() else {
                    return Ok(None);
                };
                ancestor = parent;
            }
            let canonical_parent = ancestor
                .canonicalize()
                .map_err(|error| format!("Could not resolve file path: {error}"))?;
            if !root.as_os_str().is_empty() && !canonical_parent.starts_with(root) {
                return Err("Plugin path resolves outside its storage directory".into());
            }
            return Ok(None);
        }

        let parent = path
            .parent()
            .ok_or_else(|| "File path has no parent directory".to_owned())?;
        let canonical_parent = parent
            .canonicalize()
            .map_err(|error| format!("Could not resolve file parent: {error}"))?;
        if !root.as_os_str().is_empty() && !canonical_parent.starts_with(root) {
            return Err("Plugin path resolves outside its storage directory".into());
        }
        let name = path
            .file_name()
            .ok_or_else(|| "File path has no filename".to_owned())?;
        Ok(Some(canonical_parent.join(name)))
    }

    fn canonical_root(&self, root: &Path) -> Result<PathBuf, String> {
        fs::create_dir_all(root)
            .map_err(|error| format!("Could not prepare plugin storage: {error}"))?;
        root.canonicalize()
            .map_err(|error| format!("Could not resolve plugin storage: {error}"))
    }

    fn is_private_path(&self, path: &Path) -> Result<bool, String> {
        Ok(path.starts_with(self.canonical_root(&self.data_root)?)
            || path.starts_with(self.canonical_root(&self.temp_root)?))
    }
}

fn track_alias(value: &str) -> Option<(&'static str, i64)> {
    let (alias, id) = value.rsplit_once('/')?;
    let alias = match alias {
        "@video" => "@video",
        "@audio" => "@audio",
        "@sub" => "@sub",
        _ => return None,
    };
    Some((alias, id.parse().ok()?))
}

fn normalize_private_relative(relative: &Path) -> Result<PathBuf, String> {
    let mut normalized = PathBuf::new();
    for component in relative.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir if normalized.pop() => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err("Plugin path cannot traverse outside its storage directory".into());
            }
        }
    }
    Ok(normalized)
}

fn safe_identifier(identifier: &str) -> String {
    let stem = identifier.split('.').next().unwrap_or_default();
    let reserved = matches!(stem, "con" | "prn" | "aux" | "nul")
        || matches!(
            stem.strip_prefix("com")
                .or_else(|| stem.strip_prefix("lpt")),
            Some("1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        );
    if !identifier.is_empty()
        && identifier.len() <= 128
        && !identifier.ends_with('.')
        && !reserved
        && identifier.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'_')
        })
    {
        return identifier.to_owned();
    }
    // Truncation/replacement merges distinct plugins' private storage. Hash the
    // full identity when it cannot keep a portable filename. The prefix cannot
    // occur in a preserved identifier; uppercase names also need hashing on
    // case-insensitive filesystems. Keep ordinary existing storage paths stable.
    use sha2::{Digest, Sha256};
    format!("~{:x}", Sha256::digest(identifier.as_bytes()))
}

#[cfg(test)]
mod tests;
