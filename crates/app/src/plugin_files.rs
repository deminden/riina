//! Permission-aware plugin paths with private data and temporary directories.

use serde::Serialize;
use std::{
    collections::{BTreeMap, VecDeque},
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PluginPlayerFileContext {
    media_generation: u64,
    media_ready: bool,
    current_directory: Option<PathBuf>,
    external_tracks: BTreeMap<String, PathBuf>,
    core_properties: BTreeMap<String, serde_json::Value>,
    observed_mpv_properties: BTreeMap<String, serde_json::Value>,
    chapters: Vec<serde_json::Value>,
    core_version: serde_json::Value,
    core_window_properties: BTreeMap<String, serde_json::Value>,
    recent_documents: Vec<serde_json::Value>,
    playback_history: Vec<serde_json::Value>,
}

impl Default for PluginPlayerFileContext {
    fn default() -> Self {
        Self::new(None, BTreeMap::new())
    }
}

impl PluginPlayerFileContext {
    pub(crate) fn new(
        current_directory: Option<PathBuf>,
        external_tracks: BTreeMap<String, PathBuf>,
    ) -> Self {
        Self {
            media_generation: 0,
            media_ready: false,
            current_directory,
            external_tracks,
            core_properties: BTreeMap::new(),
            observed_mpv_properties: BTreeMap::new(),
            chapters: Vec::new(),
            core_version: serde_json::Value::Null,
            core_window_properties: BTreeMap::from([
                ("loaded".into(), serde_json::Value::Bool(false)),
                ("frame".into(), serde_json::Value::Null),
                ("fullscreen".into(), serde_json::Value::Null),
                ("pip".into(), serde_json::Value::Null),
                ("ontop".into(), serde_json::Value::Null),
                ("visible".into(), serde_json::Value::Null),
                ("sidebar".into(), serde_json::Value::Null),
                ("screens".into(), serde_json::Value::Null),
                ("miniaturized".into(), serde_json::Value::Null),
            ]),
            recent_documents: Vec::new(),
            playback_history: Vec::new(),
        }
    }

    pub(crate) fn with_media_generation(mut self, generation: u64) -> Self {
        self.media_generation = generation;
        self
    }

    pub(crate) fn with_media_ready(mut self, ready: bool) -> Self {
        self.media_ready = ready;
        self
    }

    pub(crate) fn player_events_to(&self, next: &Self) -> Vec<(String, serde_json::Value)> {
        let mut events = Vec::new();
        if next.media_ready
            && (self.media_generation != next.media_generation || !self.media_ready)
            && let Some(url) = next
                .core_properties
                .get("url")
                .and_then(serde_json::Value::as_str)
        {
            events.push(("iina.file-loaded".into(), serde_json::json!(url)));
        }
        for (context_property, mpv_property) in [
            ("paused", "pause"),
            ("idle", "idle-active"),
            ("position", "time-pos"),
            ("duration", "duration"),
            ("speed", "speed"),
            ("videoWidth", "width"),
            ("videoHeight", "height"),
            ("title", "media-title"),
            ("audio.id", "aid"),
            ("audio.delay", "audio-delay"),
            ("audio.volume", "volume"),
            ("audio.muted", "mute"),
            ("subtitle.id", "sid"),
            ("subtitle.secondID", "secondary-sid"),
            ("subtitle.delay", "sub-delay"),
        ] {
            if let Some(value) = next.core_properties.get(context_property)
                && self.core_properties.get(context_property) != Some(value)
            {
                events.push((format!("mpv.{mpv_property}.changed"), value.clone()));
            }
        }
        for (property, value) in &next.observed_mpv_properties {
            if self.observed_mpv_properties.get(property) != Some(value) {
                events.push((format!("mpv.{property}.changed"), value.clone()));
            }
        }
        events
    }

    pub(crate) fn with_observed_mpv_properties(
        mut self,
        properties: BTreeMap<String, serde_json::Value>,
    ) -> Self {
        self.observed_mpv_properties = properties;
        self
    }

    pub(crate) fn with_core_snapshot(
        mut self,
        properties: BTreeMap<String, serde_json::Value>,
        chapters: Vec<serde_json::Value>,
    ) -> Self {
        self.core_properties = properties;
        self.chapters = chapters;
        self
    }

    pub(crate) fn with_recent_documents(
        mut self,
        recent_documents: Vec<serde_json::Value>,
    ) -> Self {
        self.recent_documents = recent_documents;
        self
    }

    pub(crate) fn with_playback_history(
        mut self,
        playback_history: Vec<serde_json::Value>,
    ) -> Self {
        self.playback_history = playback_history;
        self
    }

    pub(crate) fn with_core_version(mut self, version: serde_json::Value) -> Self {
        self.core_version = version;
        self
    }

    pub(crate) fn set_core_window_properties(
        &mut self,
        properties: BTreeMap<String, serde_json::Value>,
    ) {
        self.core_window_properties = properties;
    }
}

#[derive(Serialize)]
struct DirectoryEntry {
    filename: String,
    path: String,
    #[serde(rename = "isDir")]
    is_dir: bool,
}

fn decode_text(bytes: &[u8], name: &str) -> Result<String, String> {
    let label = name.trim().to_ascii_lowercase().replace('_', "-");
    let label = match label.as_str() {
        "utf8" => "utf-8",
        "utf16" | "unicode" => "utf-16",
        "utf16littleendian" => "utf-16le",
        "utf16bigendian" => "utf-16be",
        "utf32" => "utf-32",
        "utf32littleendian" => "utf-32le",
        "utf32bigendian" => "utf-32be",
        "japaneseuc" => "euc-jp",
        "shiftjis" => "shift_jis",
        "macosroman" => "macintosh",
        "big5-hkscs" | "big5-hkscs-1999" | "big5-e" => "big5",
        "iso-2022-jp-1" | "iso-2022-jp-2" | "iso-2022-jp-3" => "iso-2022-jp",
        "windowscp1250" => "windows-1250",
        "windowscp1251" => "windows-1251",
        "windowscp1252" => "windows-1252",
        "windowscp1253" => "windows-1253",
        "windowscp1254" => "windows-1254",
        "windowsarabic" => "windows-1256",
        "windowsbalticrim" => "windows-1257",
        "windowscyrillic" => "windows-1251",
        "windowsgreek" => "windows-1253",
        "windowshebrew" => "windows-1255",
        "windowslatin2" => "windows-1250",
        "windowslatin5" => "windows-1254",
        "windowsvietnamese" => "windows-1258",
        "isolatin5" => "iso-8859-9",
        "isolatinthai" => "windows-874",
        "cp949" => "euc-kr",
        "iso2022jp" | "iso-2022-jp" => "iso-2022-jp",
        "euc-cn" | "gbk-95" | "gb2312-80" => "gbk",
        "gb-18030-2000" => "gb18030",
        "latin1" | "isolatin1" | "iso-8859-1" => "x-riina-latin1",
        "latin2" | "isolatin2" => "iso-8859-2",
        "latin3" | "isolatin3" => "iso-8859-3",
        "latin4" | "isolatin4" => "iso-8859-4",
        "latin6" | "isolatin6" => "iso-8859-10",
        "latin7" | "isolatin7" => "iso-8859-13",
        "latin8" | "isolatin8" => "iso-8859-14",
        "latin9" | "isolatin9" => "iso-8859-15",
        "latin10" | "isolatin10" => "iso-8859-16",
        "isolatinarabic" => "iso-8859-6",
        "isolatincyrillic" => "iso-8859-5",
        "isolatingreek" => "iso-8859-7",
        "isolatinhebrew" => "iso-8859-8",
        other => other,
    };

    if label == "x-riina-latin1" {
        return Ok(bytes.iter().map(|byte| char::from(*byte)).collect());
    }
    if label == "ascii" {
        return bytes
            .iter()
            .copied()
            .map(|byte| {
                if byte.is_ascii() {
                    Ok(char::from(byte))
                } else {
                    Err("Input contains bytes outside ASCII".to_owned())
                }
            })
            .collect();
    }
    if matches!(label, "utf-32" | "utf-32le" | "utf-32be") {
        return decode_utf32(bytes, label);
    }

    let requested = if label == "utf-16" {
        match bytes {
            [0xff, 0xfe, ..] => "utf-16le",
            [0xfe, 0xff, ..] => "utf-16be",
            _ => "utf-16le",
        }
    } else {
        label
    };
    let encoding = encoding_rs::Encoding::for_label_no_replacement(requested.as_bytes())
        .ok_or_else(|| format!("Unknown or unsupported text encoding: {name}"))?;
    let bytes = if (encoding == encoding_rs::UTF_8 && bytes.starts_with(&[0xef, 0xbb, 0xbf]))
        || (encoding == encoding_rs::UTF_16LE && bytes.starts_with(&[0xff, 0xfe]))
        || (encoding == encoding_rs::UTF_16BE && bytes.starts_with(&[0xfe, 0xff]))
    {
        &bytes[if encoding == encoding_rs::UTF_8 { 3 } else { 2 }..]
    } else {
        bytes
    };
    encoding
        .decode_without_bom_handling_and_without_replacement(bytes)
        .map(|text| text.into_owned())
        .ok_or_else(|| format!("File contains invalid {name} text"))
}

fn decode_utf32(bytes: &[u8], label: &str) -> Result<String, String> {
    let (big_endian, bytes) = match label {
        "utf-32" if bytes.starts_with(&[0x00, 0x00, 0xfe, 0xff]) => (true, &bytes[4..]),
        "utf-32" if bytes.starts_with(&[0xff, 0xfe, 0x00, 0x00]) => (false, &bytes[4..]),
        "utf-32" | "utf-32le" => (
            false,
            bytes.strip_prefix(&[0xff, 0xfe, 0, 0]).unwrap_or(bytes),
        ),
        "utf-32be" => (
            true,
            bytes.strip_prefix(&[0, 0, 0xfe, 0xff]).unwrap_or(bytes),
        ),
        _ => unreachable!("UTF-32 labels are validated by the caller"),
    };
    if bytes.len() % 4 != 0 {
        return Err("UTF-32 input has an incomplete code point".into());
    }
    let mut text = String::with_capacity(bytes.len() / 2);
    for chunk in bytes.as_chunks::<4>().0 {
        let code_point = if big_endian {
            u32::from_be_bytes(*chunk)
        } else {
            u32::from_le_bytes(*chunk)
        };
        let character = char::from_u32(code_point)
            .ok_or_else(|| "UTF-32 input contains an invalid Unicode scalar".to_owned())?;
        text.push(character);
    }
    Ok(text)
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
        let metadata =
            fs::metadata(&path).map_err(|error| format!("Could not inspect file: {error}"))?;
        if !metadata.is_file() {
            return Err("Path is not a regular file".into());
        }
        if metadata.len() > MAX_FILE_BYTES as u64 {
            return Err("File exceeds the 4 MiB read limit".into());
        }
        let bytes = fs::read(path).map_err(|error| format!("Could not read file: {error}"))?;
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
        if !self.is_private_path(&path)? && path.exists() {
            return Err("Cannot overwrite a file outside plugin storage".into());
        }
        fs::write(path, content).map_err(|error| format!("Could not write file: {error}"))
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
        if !self.is_private_path(&path)? && path.exists() {
            return Err("Cannot overwrite a file outside plugin storage".into());
        }
        fs::write(path, content).map_err(|error| format!("Could not save download: {error}"))
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
    identifier
        .chars()
        .take(128)
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

    fn temporary_files() -> (PluginFiles, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "riina-plugin-files-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        (
            PluginFiles {
                data_root: root.join(".data"),
                temp_root: root.join(".tmp"),
                player_context: Arc::new(Mutex::new(PluginPlayerFileContext::default())),
            },
            root,
        )
    }

    #[test]
    fn player_context_reports_file_loaded_and_changed_mpv_properties() {
        let previous = PluginPlayerFileContext::default()
            .with_media_generation(4)
            .with_media_ready(true)
            .with_core_snapshot(
                BTreeMap::from([
                    ("url".into(), serde_json::json!("file:///media/old.mkv")),
                    ("paused".into(), serde_json::json!(false)),
                    ("position".into(), serde_json::json!(4.0)),
                ]),
                Vec::new(),
            )
            .with_observed_mpv_properties(BTreeMap::from([(
                "cache-buffering-state".into(),
                serde_json::json!({"total-bytes": 256}),
            )]));
        let next = PluginPlayerFileContext::default()
            .with_media_generation(5)
            .with_media_ready(true)
            .with_core_snapshot(
                BTreeMap::from([
                    ("url".into(), serde_json::json!("file:///media/new.mkv")),
                    ("paused".into(), serde_json::json!(true)),
                    ("position".into(), serde_json::json!(0.0)),
                ]),
                Vec::new(),
            )
            .with_observed_mpv_properties(BTreeMap::from([(
                "cache-buffering-state".into(),
                serde_json::json!({"total-bytes": 512}),
            )]));

        assert_eq!(
            previous.player_events_to(&next),
            [
                (
                    "iina.file-loaded".into(),
                    serde_json::json!("file:///media/new.mkv"),
                ),
                ("mpv.pause.changed".into(), serde_json::json!(true)),
                ("mpv.time-pos.changed".into(), serde_json::json!(0.0)),
                (
                    "mpv.cache-buffering-state.changed".into(),
                    serde_json::json!({"total-bytes": 512}),
                ),
            ]
        );

        let loading = PluginPlayerFileContext::default()
            .with_media_generation(5)
            .with_core_snapshot(
                BTreeMap::from([("url".into(), serde_json::json!("file:///media/new.mkv"))]),
                Vec::new(),
            );
        assert!(previous.player_events_to(&loading).is_empty());
        let loaded = loading.clone().with_media_ready(true);
        assert_eq!(
            loading.player_events_to(&loaded),
            [(
                "iina.file-loaded".into(),
                serde_json::json!("file:///media/new.mkv"),
            )]
        );
    }

    #[test]
    fn plugin_data_paths_support_bounded_text_and_listing() {
        let (files, root) = temporary_files();
        files
            .write_text("@data/state.json", "{\"ready\":true}", false)
            .unwrap();

        assert!(files.exists("@data/state.json", false).unwrap());
        assert_eq!(
            files.read_text("@data/state.json", "{}", false).unwrap(),
            "{\"ready\":true}"
        );
        let entries: serde_json::Value =
            serde_json::from_str(&files.list("@data/", "{}", false).unwrap()).unwrap();
        assert_eq!(entries[0]["filename"], "state.json");
        assert_eq!(entries[0]["path"], "state.json");

        files.delete("@data/state.json").unwrap();
        assert!(!files.exists("@data/state.json", false).unwrap());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn private_plugin_paths_reject_traversal_and_require_grant_for_absolute_paths() {
        let (files, root) = temporary_files();
        assert!(
            files
                .write_text("@data/../../outside.txt", "denied", false)
                .is_err()
        );
        fs::create_dir_all(&root).unwrap();
        let external = root.join("external.txt");
        assert!(
            files
                .write_text(external.to_str().unwrap(), "denied", false)
                .is_err()
        );

        files
            .write_text(external.to_str().unwrap(), "granted", true)
            .unwrap();
        assert_eq!(fs::read_to_string(&external).unwrap(), "granted");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn private_paths_normalize_dot_segments_without_escaping_the_plugin_root() {
        let (files, root) = temporary_files();
        files
            .write_text("@data/nested/../normalized.txt", "inside", false)
            .unwrap();
        assert_eq!(
            files
                .read_text("@data/./normalized.txt", "{}", false)
                .unwrap(),
            "inside"
        );
        assert!(
            files
                .write_text("@data/../outside.txt", "outside", false)
                .is_err()
        );
        assert!(
            files
                .write_text("@data/nested/../../outside.txt", "outside", false)
                .is_err()
        );
        assert!(!root.join("outside.txt").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn current_media_and_external_track_aliases_use_player_context() {
        let (files, root) = temporary_files();
        let media_dir = root.join("media");
        fs::create_dir_all(&media_dir).unwrap();
        fs::write(media_dir.join("captions.srt"), "subtitle").unwrap();
        let audio = root.join("external-audio.mka");
        fs::write(&audio, "audio").unwrap();
        files.set_player_context(PluginPlayerFileContext::new(
            Some(media_dir.clone()),
            BTreeMap::from([("@audio/7".into(), audio.clone())]),
        ));

        assert!(
            files
                .read_text("@current/captions.srt", "{}", false)
                .is_err()
        );
        assert_eq!(
            files
                .read_text("@current/captions.srt", "{}", true)
                .unwrap(),
            "subtitle"
        );
        assert_eq!(files.read_text("@audio/7", "{}", false).unwrap(), "audio");
        assert!(files.read_text("@sub/88", "{}", false).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn move_scopes_private_paths_and_requires_grant_for_external_paths() {
        let (files, root) = temporary_files();
        assert!(files.trash("@data/", false).is_err());
        files
            .write_text("@data/source.txt", "saved", false)
            .unwrap();
        files
            .move_path("@data/source.txt", "@data/destination.txt", false)
            .unwrap();
        assert!(!files.exists("@data/source.txt", false).unwrap());
        assert_eq!(
            files
                .read_text("@data/destination.txt", "{}", false)
                .unwrap(),
            "saved"
        );

        let external_source = root.join("outside.txt");
        let external_destination = root.join("moved.txt");
        fs::write(&external_source, "external").unwrap();
        assert!(
            files
                .trash(external_source.to_str().unwrap(), false)
                .is_err()
        );
        assert!(external_source.exists());
        assert!(
            files
                .move_path(
                    external_source.to_str().unwrap(),
                    external_destination.to_str().unwrap(),
                    false,
                )
                .is_err()
        );
        files
            .move_path(
                external_source.to_str().unwrap(),
                external_destination.to_str().unwrap(),
                true,
            )
            .unwrap();
        assert_eq!(
            fs::read_to_string(&external_destination).unwrap(),
            "external"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn writes_outside_plugin_storage_do_not_overwrite_existing_files() {
        let (files, root) = temporary_files();
        let external = root.join("external.txt");
        fs::create_dir_all(&root).unwrap();
        fs::write(&external, "keep").unwrap();
        assert!(
            files
                .write_text(external.to_str().unwrap(), "replace", true)
                .is_err()
        );
        assert_eq!(fs::read_to_string(&external).unwrap(), "keep");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn binary_handles_seek_read_and_write_with_bounded_operations() {
        let (files, root) = temporary_files();
        files.write_text("@data/binary.bin", "abcd", false).unwrap();

        let mut reader = files
            .open_handle("@data/binary.bin", "read", false)
            .unwrap();
        assert_eq!(reader.read(2).unwrap(), b"ab");
        assert_eq!(reader.offset().unwrap(), 2);
        reader.seek_to_end().unwrap();
        assert_eq!(reader.read_to_end().unwrap(), b"");
        assert!(reader.write(b"x").is_err());

        let mut writer = files
            .open_handle("@data/binary.bin", "write", false)
            .unwrap();
        writer.write(&[0, 255]).unwrap();
        writer.seek_to(2).unwrap();
        assert_eq!(writer.offset().unwrap(), 2);
        assert_eq!(
            fs::read(root.join(".data/binary.bin")).unwrap(),
            [0, 255, b'c', b'd']
        );
        assert!(
            files
                .open_handle("@data/binary.bin", "invalid", false)
                .is_err()
        );
        assert!(writer.read(1).is_err());

        let mut oversized_read = files
            .open_handle("@data/binary.bin", "read", false)
            .unwrap();
        assert!(oversized_read.read(MAX_FILE_BYTES + 1).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reveal_path_requires_an_existing_file_and_filesystem_grant_for_external_paths() {
        let (files, root) = temporary_files();
        files
            .write_text("@data/reveal.txt", "visible", false)
            .unwrap();
        assert!(files.existing_path("@data/reveal.txt", false).is_ok());

        let external = root.join("external.txt");
        fs::write(&external, "visible").unwrap();
        assert!(
            files
                .existing_path(external.to_str().unwrap(), false)
                .is_err()
        );
        assert_eq!(
            files
                .existing_path(external.to_str().unwrap(), true)
                .unwrap(),
            external.canonicalize().unwrap()
        );
        assert!(files.existing_path("@data/missing.txt", false).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn text_read_supports_iina_encoding_names_and_rejects_malformed_input() {
        let (files, root) = temporary_files();
        let path = root.join(".data/encoded.txt");
        fs::create_dir_all(path.parent().unwrap()).unwrap();

        fs::write(&path, [b'c', b'a', b'f', 0xe9, 0x80]).unwrap();
        assert_eq!(
            files
                .read_text(
                    "@data/encoded.txt",
                    r#"{"encoding":"windowsCP1252"}"#,
                    false
                )
                .unwrap(),
            "café€"
        );
        assert_eq!(
            files
                .read_text("@data/encoded.txt", r#"{"encoding":"isoLatin1"}"#, false)
                .unwrap(),
            "café\u{0080}"
        );

        for (name, bytes, expected) in [
            ("isoLatin5", &[0xd0][..], "Ğ"),
            ("isoLatinThai", &[0xa1][..], "ก"),
            ("KOI8_R", &[0xc1][..], "а"),
            ("KOI8_U", &[0xc1][..], "а"),
            ("big5_E", b"A", "A"),
            ("big5_HKSCS_1999", b"A", "A"),
            ("ISO_2022_JP_1", b"A", "A"),
            ("ISO_2022_JP_2", b"A", "A"),
            ("ISO_2022_JP_3", b"A", "A"),
        ] {
            fs::write(&path, bytes).unwrap();
            let options = serde_json::json!({ "encoding": name }).to_string();
            assert_eq!(
                files
                    .read_text("@data/encoded.txt", &options, false)
                    .unwrap(),
                expected,
                "Foundation encoding name {name}"
            );
        }

        fs::write(&path, [0xff, 0xfe, b'h', 0, b'i', 0]).unwrap();
        assert_eq!(
            files
                .read_text("@data/encoded.txt", r#"{"encoding":"unicode"}"#, false)
                .unwrap(),
            "hi"
        );

        fs::write(&path, [0xff, 0xfe, 0, 0, b'h', 0, 0, 0]).unwrap();
        assert_eq!(
            files
                .read_text("@data/encoded.txt", r#"{"encoding":"utf32"}"#, false)
                .unwrap(),
            "h"
        );

        fs::write(&path, [0x82, 0xa0]).unwrap();
        assert_eq!(
            files
                .read_text("@data/encoded.txt", r#"{"encoding":"shiftJIS"}"#, false)
                .unwrap(),
            "あ"
        );

        fs::write(&path, [0xc3, 0x28]).unwrap();
        assert!(
            files
                .read_text("@data/encoded.txt", "{}", false)
                .unwrap_err()
                .contains("invalid utf8")
        );
        assert!(
            files
                .read_text(
                    "@data/encoded.txt",
                    r#"{"encoding":"not-an-encoding"}"#,
                    false
                )
                .unwrap_err()
                .contains("Unknown or unsupported")
        );
        let _ = fs::remove_dir_all(root);
    }
}
