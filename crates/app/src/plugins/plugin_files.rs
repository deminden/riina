//! Permission-aware plugin paths with private data and temporary directories.

mod context;
mod encoding;

pub(crate) use context::{
    PlayerEvent, PluginPlayerFileContext, PluginPlaylist, WindowModes, WindowProperties,
};
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
    #[cfg(test)]
    _test_directory: Option<Arc<riina_test_support::TempDir>>,
}

#[derive(Serialize)]
struct DirectoryEntry {
    filename: String,
    path: String,
    #[serde(rename = "isDir")]
    is_dir: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PathAccess {
    Read,
    Write,
    RemoveEntry,
    NewEntry,
}

impl PluginFiles {
    pub(super) fn storage_identity(&self) -> Option<&str> {
        self.data_root.file_name()?.to_str()
    }

    pub(crate) fn for_package(package_root: &Path, identifier: &str) -> Self {
        let plugins_root = package_root.parent().unwrap_or(package_root);
        let identifier = safe_identifier(identifier);
        Self {
            data_root: plugins_root.join(".data").join(&identifier),
            temp_root: std::env::temp_dir().join(format!("riina-{identifier}")),
            player_context: Arc::new(Mutex::new(PluginPlayerFileContext::default())),
            #[cfg(test)]
            _test_directory: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn temporary(identifier: &str) -> Self {
        let root = Arc::new(riina_test_support::TempDir::new("plugin-storage").unwrap());
        let identifier = safe_identifier(identifier);
        Self {
            data_root: root.path().join(".data").join(&identifier),
            temp_root: root.path().join(".tmp").join(identifier),
            player_context: Arc::new(Mutex::new(PluginPlayerFileContext::default())),
            _test_directory: Some(root),
        }
    }

    pub(crate) fn set_player_context(&self, context: PluginPlayerFileContext) {
        // File permissions depend on this context. Keep a poisoned scope
        // unavailable rather than authorizing requests through a partial update.
        if let Ok(mut current) = self.player_context.lock() {
            *current = context;
        }
    }

    pub(crate) fn is_media_generation(&self, generation: u64) -> bool {
        self.player_context
            .lock()
            .is_ok_and(|context| context.is_media_generation(generation))
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

    pub(crate) fn playlist(&self) -> Result<PluginPlaylist, String> {
        self.player_context
            .lock()
            .map(|context| context.playlist.clone())
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
        Ok(context
            .window_modes
            .property(name)
            .or_else(|| context.core_window_properties.values.get(name).cloned()))
    }

    pub(crate) fn player_window_loaded(&self) -> Result<bool, String> {
        self.player_context
            .lock()
            .map(|context| context.core_window_properties.loaded())
            .map_err(|_| "Player file context was poisoned".to_owned())
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
        self.save_destination(&path, content.as_bytes())
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
        self.save_destination(&path, content)
    }

    fn save_destination(&self, path: &Path, bytes: &[u8]) -> Result<(), String> {
        let private = self.is_private_path(path)?;
        let parent = path.parent().ok_or("File destination has no parent")?;
        let mut staged = tempfile::Builder::new()
            .prefix(".riina-write-")
            .tempfile_in(parent)
            .map_err(|error| format!("Could not stage file contents: {error}"))?;
        staged
            .write_all(bytes)
            .and_then(|()| staged.as_file().sync_all())
            .map_err(|error| format!("Could not write file contents: {error}"))?;
        let result = if private {
            // Existing plugin readers keep their opened file when a new version is published.
            let pending = staged.into_temp_path();
            crate::platform::file_move::replace(&pending, path)
        } else {
            staged
                .persist_noclobber(path)
                .map(|_| ())
                .map_err(|failure| failure.error)
        };
        result.map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                "Cannot overwrite a file outside plugin storage".into()
            } else {
                format!("Could not publish file destination: {error}")
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
            .resolve_with(source, file_system_granted, PathAccess::RemoveEntry)?
            .ok_or_else(|| "Move source does not exist".to_owned())?;
        let destination = self
            .resolve_with(destination, file_system_granted, PathAccess::NewEntry)?
            .ok_or_else(|| "Move destination is invalid".to_owned())?;
        if source == destination {
            return Err("Move source and destination are the same path".into());
        }
        if source == self.canonical_root(&self.data_root)?
            || source == self.canonical_root(&self.temp_root)?
        {
            return Err("Cannot move a plugin storage root".into());
        }
        crate::platform::file_move::move_new(&source, &destination).map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                "Move destination already exists".into()
            } else {
                format!("Could not move file: {error}")
            }
        })
    }

    pub(crate) fn trash(&self, path: &str, file_system_granted: bool) -> Result<(), String> {
        let path = self
            .resolve_with(path, file_system_granted, PathAccess::RemoveEntry)?
            .ok_or_else(|| "File does not exist".to_owned())?;
        if path == self.canonical_root(&self.data_root)?
            || path == self.canonical_root(&self.temp_root)?
        {
            return Err("Cannot trash a plugin storage root".into());
        }
        trash::delete(&path).map_err(|error| format!("Could not move file to Trash: {error}"))
    }

    pub(crate) fn delete(&self, path: &str) -> Result<(), String> {
        let path = self
            .resolve_with(path, false, PathAccess::RemoveEntry)?
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
        let mut visited = 0;
        while let Some(directory) = pending.pop_front() {
            for entry in fs::read_dir(&directory)
                .map_err(|error| format!("Could not list directory: {error}"))?
            {
                let entry =
                    entry.map_err(|error| format!("Could not read directory entry: {error}"))?;
                visited += 1;
                if visited > MAX_LIST_ENTRIES {
                    return Err("Directory listing exceeds the 4096 entry limit".into());
                }
                let entry_path = entry.path();
                let kind = entry
                    .file_type()
                    .map_err(|error| format!("Could not inspect directory entry: {error}"))?;
                if kind.is_symlink() {
                    continue;
                }
                let is_dir = kind.is_dir();
                let relative = entry_path
                    .strip_prefix(&root)
                    .map_err(|error| format!("Directory entry escaped its root: {error}"))?;
                let mut path = String::new();
                for component in relative.components() {
                    path.push('/');
                    path.push_str(&component.as_os_str().to_string_lossy());
                }
                entries.push(DirectoryEntry {
                    filename: entry.file_name().to_string_lossy().into_owned(),
                    path,
                    is_dir,
                });
                if is_dir && include_subdirectories {
                    pending.push_back(entry_path);
                }
            }
        }
        entries.sort_by(|left, right| left.path.cmp(&right.path));
        serde_json::to_string(&entries).map_err(|error| error.to_string())
    }

    pub(super) fn resolve(
        &self,
        value: &str,
        file_system_granted: bool,
        for_write: bool,
    ) -> Result<Option<PathBuf>, String> {
        let access = if for_write {
            PathAccess::Write
        } else {
            PathAccess::Read
        };
        self.resolve_with(value, file_system_granted, access)
    }

    fn resolve_with(
        &self,
        value: &str,
        file_system_granted: bool,
        access: PathAccess,
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
            let path = directory.join(relative.trim_start_matches('/'));
            if !path.is_absolute() {
                return Err("The current media directory is unavailable".into());
            }
            return self.resolve_external(&path, access);
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
            return self.resolve_external(path, access);
        }
        if let Some(relative) = value.strip_prefix("@data/") {
            return self.resolve_local(&self.data_root, relative, access);
        }
        if let Some(relative) = value.strip_prefix("@tmp/") {
            return self.resolve_local(&self.temp_root, relative, access);
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
        self.resolve_external(&expanded, access)
    }

    fn resolve_local(
        &self,
        root: &Path,
        relative: &str,
        access: PathAccess,
    ) -> Result<Option<PathBuf>, String> {
        let relative = normalize_private_relative(Path::new(relative.trim_start_matches('/')))?;
        let root = self.canonical_root(root)?;
        let candidate = root.join(relative);
        self.validate_containment(&candidate, &root, access)
    }

    fn resolve_external(&self, path: &Path, access: PathAccess) -> Result<Option<PathBuf>, String> {
        self.validate_containment(path, Path::new(""), access)
    }

    fn validate_containment(
        &self,
        path: &Path,
        root: &Path,
        access: PathAccess,
    ) -> Result<Option<PathBuf>, String> {
        if !root.as_os_str().is_empty()
            && fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink())
        {
            return Err("Symbolic links are not supported in plugin storage".into());
        }
        if matches!(access, PathAccess::RemoveEntry | PathAccess::NewEntry) && path != root {
            // A move or Trash request acts on the named entry. Canonicalizing the
            // full path would instead move the target of an external symbolic link.
            let entry = Self::entry_in_canonical_parent(path, root)?;
            return if access == PathAccess::NewEntry {
                Ok(Some(entry))
            } else {
                match fs::symlink_metadata(&entry) {
                    Ok(_) => Ok(Some(entry)),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                    Err(error) => Err(format!("Could not inspect file: {error}")),
                }
            };
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
        if access != PathAccess::Write {
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

        Self::entry_in_canonical_parent(path, root).map(Some)
    }

    fn entry_in_canonical_parent(path: &Path, root: &Path) -> Result<PathBuf, String> {
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
        Ok(canonical_parent.join(name))
    }

    fn canonical_root(&self, root: &Path) -> Result<PathBuf, String> {
        fs::create_dir_all(root)
            .map_err(|error| format!("Could not prepare plugin storage: {error}"))?;
        root.canonicalize()
            .map_err(|error| format!("Could not resolve plugin storage: {error}"))
    }

    pub(super) fn is_private_path(&self, path: &Path) -> Result<bool, String> {
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

pub(crate) fn safe_identifier(identifier: &str) -> String {
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
    format!(
        "~{}",
        crate::hashing::hex(&Sha256::digest(identifier.as_bytes()))
    )
}

#[cfg(test)]
mod tests;
