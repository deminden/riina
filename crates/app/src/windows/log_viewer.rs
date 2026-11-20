//! A bounded, incrementally refreshed viewer for mpv's native log file.
use crate::{LogViewerRow, LogViewerWindow, PlayerWindow};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel, winit_030::WinitWindowAccessor};
use std::{
    cell::RefCell,
    collections::{BTreeSet, VecDeque},
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    rc::Rc,
    time::SystemTime,
};

const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;
const MAX_LOG_ENTRIES: usize = 8_000;
const MAX_LINE_BYTES: usize = 16 * 1024;
const MAX_BATCH_BYTES: usize = 2 * 1024 * 1024;
const LEVEL_OPTIONS: [&str; 8] = [
    "All Levels",
    "Fatal",
    "Error",
    "Warning",
    "Info",
    "Verbose",
    "Debug",
    "Trace",
];

#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    level: String,
    rank: u8,
    timestamp: String,
    subsystem: String,
    message: String,
    raw: String,
}

#[derive(Default)]
struct LogReader {
    path: PathBuf,
    offset: u64,
    created: Option<SystemTime>,
    modified: Option<SystemTime>,
    pending: Vec<u8>,
    entries: VecDeque<Entry>,
    available: bool,
}

impl LogReader {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            ..Self::default()
        }
    }

    fn refresh(&mut self) -> Result<bool, String> {
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
        if replaced || !self.available {
            self.clear_contents();
        }

        let mut start = if replaced || !self.available {
            length.saturating_sub(MAX_LOG_BYTES)
        } else {
            self.offset
        };
        let mut starts_mid_line = start > 0 && (replaced || !self.available);
        if length.saturating_sub(start) > MAX_BATCH_BYTES as u64 {
            start = length - MAX_BATCH_BYTES as u64;
            self.pending.clear();
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

        if starts_mid_line {
            if let Some(newline) = bytes.iter().position(|byte| *byte == b'\n') {
                bytes.drain(..=newline);
            } else {
                bytes.clear();
            }
        }
        self.consume(&bytes);
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

    fn consume(&mut self, bytes: &[u8]) {
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
            self.entries.push_back(parse_entry(raw));
        }
        while self.entries.len() > MAX_LOG_ENTRIES {
            self.entries.pop_front();
        }
    }
}

fn parse_entry(raw: String) -> Entry {
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
        "info" => 3,
        "status" => 4,
        "v" => 5,
        "vv" | "debug" => 6,
        _ => 7,
    }
}

fn level_label(level: &str) -> String {
    match level {
        "warn" => "warning".into(),
        "v" => "verbose".into(),
        "vv" => "verbose 2".into(),
        "vvv" => "verbose 3".into(),
        "vvvv" => "verbose 4".into(),
        "vvvvv" => "verbose 5".into(),
        "vvvvvv" => "verbose 6".into(),
        other => other.to_owned(),
    }
}

fn level_color(rank: u8) -> slint::Color {
    match rank {
        0 | 1 => slint::Color::from_rgb_u8(190, 44, 46),
        2 => slint::Color::from_rgb_u8(182, 112, 0),
        3 | 4 => slint::Color::from_rgb_u8(50, 106, 70),
        _ => slint::Color::from_rgb_u8(96, 96, 105),
    }
}

fn filter_entries<'a>(
    entries: &'a VecDeque<Entry>,
    query: &str,
    level_index: i32,
    subsystem: &str,
) -> Vec<&'a Entry> {
    let query = query.trim().to_lowercase();
    let max_rank = if level_index <= 0 {
        u8::MAX
    } else {
        level_index.saturating_sub(1) as u8
    };
    entries
        .iter()
        .filter(|entry| max_rank == u8::MAX || entry.rank <= max_rank)
        .filter(|entry| subsystem.is_empty() || entry.subsystem == subsystem)
        .filter(|entry| {
            query.is_empty()
                || entry.message.to_lowercase().contains(&query)
                || entry.subsystem.to_lowercase().contains(&query)
                || entry.level.to_lowercase().contains(&query)
        })
        .collect()
}

pub(crate) fn connect(
    app: &PlayerWindow,
    preferences: &crate::PreferencesWindow,
    log_path: PathBuf,
) {
    let slot = Rc::new(RefCell::new(None::<LogViewerWindow>));
    let reader = Rc::new(RefCell::new(LogReader::new(log_path.clone())));
    let subsystem_values = Rc::new(RefCell::new(vec![String::new()]));
    let timer = Rc::new(slint::Timer::default());
    let timer_slot = slot.clone();
    let timer_reader = reader.clone();
    let timer_subsystems = subsystem_values.clone();
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(500),
        move || {
            let Some(viewer) = timer_slot
                .borrow()
                .as_ref()
                .map(ComponentHandle::clone_strong)
            else {
                return;
            };
            if viewer.window().is_visible() {
                refresh_view(&viewer, &timer_reader, &timer_subsystems);
            }
        },
    );
    let keep_timer_alive = timer.clone();
    let show_slot = slot.clone();
    let show_reader = reader.clone();
    let show_subsystems = subsystem_values.clone();
    let owner = app.as_weak();
    preferences.on_advanced_open_log_viewer(move || {
        let _timer_lifetime = &keep_timer_alive;
        let Some(app) = owner.upgrade() else { return };
        let viewer = if let Some(viewer) = show_slot.borrow().as_ref() {
            viewer.clone_strong()
        } else {
            let viewer = match LogViewerWindow::new() {
                Ok(viewer) => viewer,
                Err(error) => {
                    app.set_status(format!("Could not open Log Viewer: {error}").into());
                    return;
                }
            };
            crate::appearance_preferences::install_system_typography(&viewer);
            connect_window(
                &app,
                &viewer,
                show_reader.clone(),
                show_subsystems.clone(),
                log_path.clone(),
            );
            viewer
                .window()
                .on_close_requested(|| slint::CloseRequestResponse::HideWindow);
            crate::platform::center_auxiliary_window(&viewer, &app);
            *show_slot.borrow_mut() = Some(viewer.clone_strong());
            viewer
        };
        refresh_view(&viewer, &show_reader, &show_subsystems);
        if let Err(error) = viewer.show() {
            app.set_status(format!("Could not show Log Viewer: {error}").into());
        } else {
            crate::platform::enable_log_viewer_acrylic(&viewer);
        }
    });
}

fn connect_window(
    app: &PlayerWindow,
    viewer: &LogViewerWindow,
    reader: Rc<RefCell<LogReader>>,
    subsystem_values: Rc<RefCell<Vec<String>>>,
    log_path: PathBuf,
) {
    let weak_app = app.as_weak();
    let weak_viewer = viewer.as_weak();
    let refresh_reader = reader.clone();
    let refresh_subsystems = subsystem_values.clone();
    viewer.on_refresh_logs(move || {
        if let Some(viewer) = weak_viewer.upgrade() {
            refresh_view(&viewer, &refresh_reader, &refresh_subsystems);
        }
    });

    let weak_viewer = viewer.as_weak();
    let filter_reader = reader.clone();
    let filter_subsystems = subsystem_values.clone();
    viewer.on_filters_changed(move |_query, _level_index, _subsystem_index| {
        if let Some(viewer) = weak_viewer.upgrade() {
            refresh_view(&viewer, &filter_reader, &filter_subsystems);
        }
    });

    let clipboard_app = weak_app.clone();
    viewer.on_copy_selected_log(move |text| {
        if let Err(error) = crate::platform::clipboard::copy_text(text.as_str())
            && let Some(app) = clipboard_app.upgrade()
        {
            app.set_status(format!("Could not copy log entry: {error}").into());
        }
    });

    let folder_app = weak_app.clone();
    let log_directory = log_path.parent().unwrap_or(Path::new(".")).to_path_buf();
    viewer.on_open_log_folder(move || {
        if let Err(error) = crate::platform::open_directory(&log_directory)
            && let Some(app) = folder_app.upgrade()
        {
            app.set_status(format!("Could not open the log directory: {error}").into());
        }
    });

    let save_app = weak_app;
    let save_reader = reader;
    let save_subsystems = subsystem_values;
    let save_viewer = viewer.as_weak();
    viewer.on_save_filtered_logs(move || {
        let (Some(app), Some(viewer)) = (save_app.upgrade(), save_viewer.upgrade()) else {
            return;
        };
        if app.get_dialog_open() {
            return;
        }
        let filtered = filtered_rows(&viewer, &save_reader, &save_subsystems);
        if filtered.is_empty() {
            viewer.set_status_text("There are no filtered log entries to save".into());
            return;
        }
        let content = filtered
            .iter()
            .map(|entry| entry.raw.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let app = app.as_weak();
        let viewer = viewer.as_weak();
        let _ = slint::spawn_local(async move {
            let (Some(app), Some(viewer)) = (app.upgrade(), viewer.upgrade()) else {
                return;
            };
            app.set_dialog_open(true);
            let result = async {
                let owner = viewer
                    .window()
                    .winit_window()
                    .await
                    .map_err(|error| error.to_string())?;
                let file = rfd::AsyncFileDialog::new()
                    .set_title("Save Filtered Logs")
                    .set_file_name("riina-filtered.log")
                    .set_parent(owner.as_ref())
                    .save_file()
                    .await;
                if let Some(file) = file {
                    file.write(content.as_bytes())
                        .await
                        .map_err(|error| error.to_string())?;
                    Ok::<bool, String>(true)
                } else {
                    Ok(false)
                }
            }
            .await;
            app.set_dialog_open(false);
            match result {
                Ok(true) => viewer.set_status_text("Filtered logs saved".into()),
                Ok(false) => {}
                Err(error) => {
                    viewer.set_status_text(format!("Could not save logs: {error}").into())
                }
            }
        });
    });
}

fn filtered_rows(
    viewer: &LogViewerWindow,
    reader: &Rc<RefCell<LogReader>>,
    subsystem_values: &Rc<RefCell<Vec<String>>>,
) -> Vec<Entry> {
    let subsystem = usize::try_from(viewer.get_subsystem_index())
        .ok()
        .and_then(|index| subsystem_values.borrow().get(index).cloned())
        .unwrap_or_default();
    let reader = reader.borrow();
    filter_entries(
        &reader.entries,
        viewer.get_search_text().as_str(),
        viewer.get_level_index(),
        &subsystem,
    )
    .into_iter()
    .cloned()
    .collect()
}

fn refresh_view(
    viewer: &LogViewerWindow,
    reader: &Rc<RefCell<LogReader>>,
    subsystem_values: &Rc<RefCell<Vec<String>>>,
) {
    let result = reader.borrow_mut().refresh();
    if let Err(error) = result {
        viewer.set_status_text(format!("Could not read log file: {error}").into());
        return;
    }
    let selected = usize::try_from(viewer.get_subsystem_index())
        .ok()
        .and_then(|index| subsystem_values.borrow().get(index).cloned())
        .unwrap_or_default();
    let (available, subsystem_options) = {
        let reader = reader.borrow();
        let subsystems = reader
            .entries
            .iter()
            .map(|entry| entry.subsystem.clone())
            .filter(|subsystem| !subsystem.is_empty())
            .collect::<BTreeSet<_>>();
        (reader.available, subsystems)
    };
    let mut choices = vec![String::new()];
    choices.extend(subsystem_options);
    let index = choices
        .iter()
        .position(|choice| *choice == selected)
        .unwrap_or(0);
    *subsystem_values.borrow_mut() = choices.clone();
    viewer.set_subsystem_options(ModelRc::new(VecModel::from(
        choices
            .iter()
            .enumerate()
            .map(|(index, choice)| {
                if index == 0 {
                    SharedString::from("All Subsystems")
                } else {
                    SharedString::from(choice.as_str())
                }
            })
            .collect::<Vec<_>>(),
    )));
    if viewer.get_subsystem_index() != index as i32 {
        viewer.set_subsystem_index(index as i32);
    }
    viewer.set_level_options(ModelRc::new(VecModel::from(
        LEVEL_OPTIONS
            .iter()
            .map(|label| SharedString::from(*label))
            .collect::<Vec<_>>(),
    )));

    let subsystem = choices.get(index).map(String::as_str).unwrap_or_default();
    let reader = reader.borrow();
    let rows = filter_entries(
        &reader.entries,
        viewer.get_search_text().as_str(),
        viewer.get_level_index(),
        subsystem,
    )
    .into_iter()
    .map(|entry| LogViewerRow {
        level: level_label(&entry.level).into(),
        timestamp: entry.timestamp.clone().into(),
        subsystem: entry.subsystem.clone().into(),
        message: entry.message.clone().into(),
        level_color: level_color(entry.rank),
        raw: entry.raw.clone().into(),
    })
    .collect::<Vec<_>>();
    viewer.set_rows(ModelRc::new(VecModel::from(rows)));
    viewer.set_selected_index(-1);
    let status = if available {
        format!(
            "{} entries  ·  {}",
            reader.entries.len(),
            reader.path.display()
        )
    } else {
        format!(
            "Log file not found. Enable logging and restart Riina.  ·  {}",
            reader.path.display()
        )
    };
    viewer.set_status_text(status.into());
}

#[cfg(test)]
mod tests {
    use super::{LogReader, filter_entries, parse_entry};
    use std::{collections::VecDeque, fs, io::Write, time::SystemTime};

    #[test]
    fn parses_mpv_timestamp_level_subsystem_and_message() {
        let entry = parse_entry("[  1.250][warn][demux] packet queue is full".into());
        assert_eq!(entry.level, "warn");
        assert_eq!(entry.rank, 2);
        assert_eq!(entry.timestamp, "1.250");
        assert_eq!(entry.subsystem, "demux");
        assert_eq!(entry.message, "packet queue is full");
    }

    #[test]
    fn parses_unstructured_lines_without_losing_the_original() {
        let entry = parse_entry("a plain diagnostic".into());
        assert_eq!(entry.message, "a plain diagnostic");
        assert_eq!(entry.raw, "a plain diagnostic");
        assert_eq!(entry.level, "info");
    }

    #[test]
    fn filters_are_case_insensitive_and_level_limits_include_more_severe_entries() {
        let entries = VecDeque::from([
            parse_entry("[0.1][error][core] Device failed".into()),
            parse_entry("[0.2][info][core] Device ready".into()),
            parse_entry("[0.3][v][demux] packet read".into()),
        ]);
        assert_eq!(filter_entries(&entries, "DEVICE", 5, "").len(), 2);
        assert_eq!(filter_entries(&entries, "packet", 0, "").len(), 1);
        assert_eq!(filter_entries(&entries, "device", 0, "core").len(), 2);
        assert_eq!(filter_entries(&entries, "device", 2, "core").len(), 1);
    }

    #[test]
    fn incremental_reader_keeps_partial_lines_until_the_next_append() {
        let path = temp_log_path();
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(b"[0.1][info][core] first\n[0.2][warn][core] par")
            .unwrap();
        drop(file);
        let mut reader = LogReader::new(path.clone());
        assert!(reader.refresh().unwrap());
        assert_eq!(reader.entries.len(), 1);
        let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(b"tial\n").unwrap();
        drop(file);
        assert!(reader.refresh().unwrap());
        assert_eq!(reader.entries.len(), 2);
        assert_eq!(reader.entries.back().unwrap().message, "partial");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn reader_resets_after_the_log_is_replaced_with_a_shorter_file() {
        let path = temp_log_path();
        fs::write(&path, b"[0.1][info][core] old line\n").unwrap();
        let mut reader = LogReader::new(path.clone());
        reader.refresh().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        fs::write(&path, b"[0.2][info][core] new\n").unwrap();
        assert!(reader.refresh().unwrap());
        assert_eq!(reader.entries.len(), 1);
        assert_eq!(reader.entries.front().unwrap().message, "new");
        let _ = fs::remove_file(path);
    }

    fn temp_log_path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "riina-log-viewer-{}-{}.log",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
}
