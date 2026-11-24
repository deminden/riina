//! Read complete records from the application and mpv logs in bounded batches.
use std::{
    collections::VecDeque,
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Clone, Copy)]
enum Source {
    Application,
    Engine,
}

pub(super) struct LogFiles {
    readers: [LogReader; 2],
    sequence: u64,
}

impl LogFiles {
    pub(super) fn new(preferences: &Path) -> Self {
        Self {
            readers: [
                LogReader::new(preferences.with_file_name("riina.log"), Source::Application),
                LogReader::new(
                    preferences.with_file_name("native-player.log"),
                    Source::Engine,
                ),
            ],
            sequence: 0,
        }
    }

    pub(super) fn refresh(&mut self) -> (bool, Option<String>) {
        let mut changed = false;
        let mut error = None;
        for reader in &mut self.readers {
            match reader.refresh(&mut self.sequence) {
                Ok(updated) => changed |= updated,
                Err(reason) => {
                    error = Some(format!(
                        "Could not read {}: {reason}",
                        reader.path.display()
                    ));
                }
            }
        }
        (changed, error)
    }

    pub(super) fn entries(&self) -> Vec<&Entry> {
        // mpv uses elapsed time and the application uses UTC. Preserve file order
        // and use the order in which records were read to combine the sources.
        let mut entries: Vec<_> = self
            .readers
            .iter()
            .flat_map(|reader| &reader.entries)
            .collect();
        entries.sort_unstable_by_key(|entry| entry.id);
        let extra = entries.len().saturating_sub(MAX_LOG_ENTRIES);
        entries.drain(..extra);
        entries
    }

    pub(super) fn available(&self) -> bool {
        self.readers.iter().any(|reader| reader.available)
    }
}

const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;
pub(super) const MAX_LOG_ENTRIES: usize = 8_000;
const MAX_LINE_BYTES: usize = 16 * 1024;
const MAX_BATCH_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Entry {
    pub(super) id: u64,
    pub(super) level: String,
    pub(super) rank: u8,
    pub(super) timestamp: String,
    pub(super) subsystem: String,
    pub(super) message: String,
    pub(super) raw: String,
}

struct LogReader {
    path: PathBuf,
    source: Source,
    offset: u64,
    created: Option<SystemTime>,
    modified: Option<SystemTime>,
    pending: Vec<u8>,
    entries: VecDeque<Entry>,
    available: bool,
}

impl LogReader {
    fn new(path: PathBuf, source: Source) -> Self {
        Self {
            path,
            source,
            offset: 0,
            created: None,
            modified: None,
            pending: Vec::new(),
            entries: VecDeque::new(),
            available: false,
        }
    }

    fn refresh(&mut self, sequence: &mut u64) -> Result<bool, String> {
        let metadata = match fs::metadata(&self.path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let changed = self.available || !self.entries.is_empty();
                self.reset();
                return Ok(changed);
            }
            Err(error) => return Err(error.to_string()),
        };
        let length = metadata.len();
        let created = metadata.created().ok();
        let modified = metadata.modified().ok();
        let replaced = self.available
            && (length < self.offset
                || created.is_some() && self.created.is_some() && created != self.created
                || length == self.offset && modified != self.modified);
        if self.available && !replaced && length == self.offset && modified == self.modified {
            return Ok(false);
        }
        let reopen = replaced || !self.available;
        let mut start = if reopen {
            length.saturating_sub(MAX_LOG_BYTES)
        } else {
            self.offset
        };
        let mut starts_mid_line = start > 0 && reopen;
        if length.saturating_sub(start) > MAX_BATCH_BYTES as u64 {
            start = length - MAX_BATCH_BYTES as u64;
            starts_mid_line = true;
        }
        let mut file = File::open(&self.path).map_err(|error| error.to_string())?;
        file.seek(SeekFrom::Start(start))
            .map_err(|error| error.to_string())?;
        let read_limit = length.saturating_sub(start).min(MAX_BATCH_BYTES as u64) as usize;
        let mut bytes = vec![0; read_limit];
        let mut read = 0;
        while read < bytes.len() {
            let count = file
                .read(&mut bytes[read..])
                .map_err(|error| error.to_string())?;
            if count == 0 {
                break;
            }
            read += count;
        }
        bytes.truncate(read);

        if reopen {
            self.clear_contents();
        }
        if starts_mid_line {
            self.pending.clear();
            if let Some(newline) = bytes.iter().position(|byte| *byte == b'\n') {
                bytes.drain(..=newline);
            } else {
                bytes.clear();
            }
        }
        self.consume(&bytes, sequence);
        self.offset = start.saturating_add(read as u64);
        self.created = created;
        self.modified = modified;
        self.available = true;
        Ok(true)
    }

    fn reset(&mut self) {
        self.clear_contents();
        self.offset = 0;
        self.created = None;
        self.modified = None;
        self.available = false;
    }

    fn clear_contents(&mut self) {
        self.pending.clear();
        self.entries.clear();
    }

    fn consume(&mut self, bytes: &[u8], sequence: &mut u64) {
        self.pending.extend_from_slice(bytes);
        let mut complete = Vec::new();
        let mut start = 0;
        for (index, byte) in self.pending.iter().enumerate() {
            if *byte == b'\n' {
                complete.push(self.pending[start..index].to_vec());
                start = index + 1;
            }
        }
        let remaining = self.pending[start..].to_vec();
        self.pending = remaining;
        if self.pending.len() > MAX_LINE_BYTES {
            self.pending.drain(..self.pending.len() - MAX_LINE_BYTES);
        }
        for mut line in complete {
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            let raw = String::from_utf8_lossy(&line).into_owned();
            let mut entry = match self.source {
                Source::Application => parse_application(raw),
                Source::Engine => parse_mpv(raw),
            };
            entry.id = *sequence;
            *sequence += 1;
            self.entries.push_back(entry);
        }
        while self.entries.len() > MAX_LOG_ENTRIES {
            self.entries.pop_front();
        }
    }
}

fn parse_mpv(raw: String) -> Entry {
    let mut rest = raw.as_str();
    let mut fields = Vec::with_capacity(3);
    for _ in 0..3 {
        let Some(after_open) = rest.strip_prefix('[') else {
            break;
        };
        let Some((field, tail)) = after_open.split_once(']') else {
            break;
        };
        fields.push(field.trim());
        rest = tail.trim_start();
    }

    let mut timestamp = String::new();
    let mut level = "info".to_owned();
    let mut subsystem = String::new();
    let mut cursor = 0;
    if fields
        .first()
        .is_some_and(|field| field.parse::<f64>().is_ok())
    {
        timestamp = fields[0].to_owned();
        cursor = 1;
    }
    if let Some(field) = fields.get(cursor)
        && is_level(field)
    {
        level = field.to_ascii_lowercase();
        cursor += 1;
    }
    if let Some(field) = fields.get(cursor) {
        subsystem = (*field).to_owned();
    }
    let rank = level_rank(&level);
    Entry {
        id: 0,
        level,
        rank,
        timestamp,
        subsystem,
        message: rest.to_owned(),
        raw,
    }
}

fn is_level(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "fatal"
            | "error"
            | "warn"
            | "warning"
            | "info"
            | "status"
            | "v"
            | "vv"
            | "vvv"
            | "vvvv"
            | "vvvvv"
            | "vvvvvv"
            | "debug"
            | "trace"
    )
}

fn level_rank(level: &str) -> u8 {
    match level.to_ascii_lowercase().as_str() {
        "fatal" => 0,
        "error" => 1,
        "warn" | "warning" => 2,
        "info" | "status" => 3,
        "v" => 4,
        "vv" | "debug" => 5,
        _ => 6,
    }
}

fn parse_application(raw: String) -> Entry {
    // The application writes an ISO timestamp followed by LEVEL [subsystem].
    let mut entry = parse_mpv(raw);
    let Some((time, rest)) = entry.raw.split_once(' ') else {
        return entry;
    };
    let Some((level, rest)) = rest.split_once(" [") else {
        return entry;
    };
    let Some((subsystem, message)) = rest.split_once("] ") else {
        return entry;
    };
    if !time.ends_with('Z') || !is_level(level) {
        return entry;
    }
    entry.timestamp = time.into();
    entry.level = level.to_ascii_lowercase();
    entry.rank = level_rank(level);
    entry.subsystem = subsystem.into();
    entry.message = message.into();
    entry
}
