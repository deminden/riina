//! Project grouped History entries without doing filesystem work.
use super::{availability, search};
use crate::{
    PlaybackHistoryRow,
    platform::language::history_time::{HistoryTime, Style},
};
use chrono::{DateTime, Local, NaiveDate, TimeZone};
use riina_core::playback_history::Entry;
use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
};

pub(super) struct Projection {
    pub(super) rows: Vec<PlaybackHistoryRow>,
    pub(super) checks: Vec<PathBuf>,
}

pub(super) fn build_rows(
    entries: &[Entry],
    query: &str,
    search_mode: i32,
    group_mode: i32,
    collapsed_groups: &HashSet<String>,
    selected_paths: &HashSet<String>,
    files: &availability::Files,
) -> Projection {
    let group_by_location = group_mode == 1;
    let mut buckets = matching_groups(entries, query, search_mode, group_by_location)
        .into_iter()
        .collect::<Vec<_>>();
    if group_by_location {
        buckets.sort_by(|(left, _), (right, _)| match (left, right) {
            (Group::Location(left), Group::Location(right)) => {
                crate::platform::natural_compare(left, right).then_with(|| left.cmp(right))
            }
            _ => left.cmp(right),
        });
    } else {
        buckets.reverse();
    }

    let format = HistoryTime::current();
    let mut rows = Vec::new();
    let mut checks = Vec::new();
    for (group, mut entries) in buckets {
        entries.sort_by_key(|row| std::cmp::Reverse(row.entry.date_ms));
        let group_key = group.key();
        let collapsed = collapsed_groups.contains(&group_key);
        let label = group.label(
            entries.first().and_then(|row| row.played_at.as_ref()),
            &format,
        );
        rows.push(PlaybackHistoryRow {
            is_group: true,
            collapsed,
            group_key: group_key.into(),
            label: label.into(),
            ..PlaybackHistoryRow::default()
        });
        if collapsed {
            continue;
        }
        let style = if group_by_location {
            Style::DateTime
        } else {
            Style::Time
        };
        rows.extend(entries.into_iter().map(|row| {
            if let Some(path) = &row.media.local {
                checks.push(path.clone());
            }
            media_row(
                row,
                |date| format.format(date, style),
                selected_paths,
                files,
            )
        }));
    }
    Projection { rows, checks }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum Group {
    Day(Option<NaiveDate>),
    Location(String),
}

impl Group {
    fn key(&self) -> String {
        match self {
            Self::Day(Some(day)) => format!("day:{day}"),
            Self::Day(None) => "day:unknown".into(),
            Self::Location(location) => format!("location:{location}"),
        }
    }

    fn label(&self, played: Option<&DateTime<Local>>, format: &HistoryTime) -> String {
        match self {
            Self::Day(_) => played.map_or_else(
                || "Unknown Date".to_owned(),
                |date| format.format(date, Style::DayHeading),
            ),
            Self::Location(location) => location.clone(),
        }
    }
}

struct Record<'a> {
    entry: &'a Entry,
    media: MediaDescription,
    played_at: Option<DateTime<Local>>,
}

fn matching_groups<'a>(
    entries: &'a [Entry],
    query: &str,
    search_mode: i32,
    group_by_location: bool,
) -> BTreeMap<Group, Vec<Record<'a>>> {
    let query = search::Query::new(query);
    let mut groups = BTreeMap::<Group, Vec<Record<'_>>>::new();
    for entry in entries {
        let media = describe(&entry.path);
        let matches = match search_mode {
            0 => query.matches(&media.name),
            _ => query.matches_path(&entry.path, media.local.as_deref()),
        };
        if !matches {
            continue;
        }
        let played_at = i64::try_from(entry.date_ms)
            .ok()
            .and_then(|milliseconds| Local.timestamp_millis_opt(milliseconds).single());
        let key = if group_by_location {
            Group::Location(media.location.clone())
        } else {
            Group::Day(played_at.as_ref().map(DateTime::date_naive))
        };
        groups.entry(key).or_default().push(Record {
            entry,
            media,
            played_at,
        });
    }
    groups
}

fn media_row(
    record: Record<'_>,
    format: impl FnOnce(&DateTime<Local>) -> String,
    selected: &HashSet<String>,
    files: &availability::Files,
) -> PlaybackHistoryRow {
    let Record {
        entry,
        media,
        played_at,
    } = record;
    let state = media
        .local
        .as_ref()
        .and_then(|path| files.get(path))
        .copied();
    let available = state != Some(false) && !media.invalid_file_url;
    let fallback = if media.local.is_some() {
        &media.name
    } else {
        &entry.path
    };
    let show_progress = available && (media.local.is_none() || state == Some(true));
    let progress = entry.progress.filter(|_| show_progress);
    PlaybackHistoryRow {
        label: entry.title.as_ref().unwrap_or(fallback).as_str().into(),
        detail: entry
            .title
            .as_ref()
            .map_or("", |_| fallback.as_str())
            .into(),
        progress: progress_label(progress, entry.duration).into(),
        played_at: played_at.as_ref().map_or_else(String::new, format).into(),
        path: entry.path.clone().into(),
        progress_ratio: progress_ratio(progress, entry.duration),
        available,
        local_file: state == Some(true),
        selected: selected.contains(entry.path.as_str()),
        ..PlaybackHistoryRow::default()
    }
}

struct MediaDescription {
    name: String,
    location: String,
    local: Option<PathBuf>,
    invalid_file_url: bool,
}

fn describe(reference: &str) -> MediaDescription {
    if let Some(path) = path_from_reference(reference) {
        let name = path.file_name().map_or_else(
            || reference.to_owned(),
            |name| name.to_string_lossy().into_owned(),
        );
        let location = path.parent().map_or_else(
            || "".to_owned(),
            |parent| parent.to_string_lossy().into_owned(),
        );
        return MediaDescription {
            name,
            location,
            local: Some(path),
            invalid_file_url: false,
        };
    }
    if let Ok(url) = url::Url::parse(reference) {
        let location = url.host_str().unwrap_or("-").to_owned();
        let name = url
            .path_segments()
            .and_then(|mut segments| segments.next_back())
            .filter(|name| !name.is_empty())
            .map_or_else(
                || reference.to_owned(),
                |name| {
                    percent_encoding::percent_decode_str(name)
                        .decode_utf8_lossy()
                        .into_owned()
                },
            );
        return MediaDescription {
            name,
            location,
            local: None,
            invalid_file_url: url.scheme() == "file",
        };
    }
    MediaDescription {
        name: reference.to_owned(),
        location: String::new(),
        local: Some(PathBuf::from(reference)),
        invalid_file_url: false,
    }
}

pub(super) fn path_from_reference(reference: &str) -> Option<PathBuf> {
    let is_windows_drive = reference.as_bytes().get(1) == Some(&b':')
        && reference
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic);
    if is_windows_drive {
        return Some(PathBuf::from(reference));
    }
    match url::Url::parse(reference) {
        Ok(url) if url.scheme() == "file" => url
            .to_file_path()
            .ok()
            .or_else(|| crate::file_actions::local_media_path(reference)),
        Ok(_) => None,
        Err(_) => Some(PathBuf::from(reference)),
    }
}

fn progress_ratio(progress: Option<f64>, duration: f64) -> f32 {
    progress
        .filter(|_| duration.is_finite() && duration > 0.0)
        .map_or(0.0, |progress| (progress / duration).clamp(0.0, 1.0) as f32)
}

fn progress_label(progress: Option<f64>, duration: f64) -> String {
    let Some(progress) = progress.filter(|_| duration.is_finite() && duration > 0.0) else {
        return String::new();
    };
    format!("{} / {}", format_time(progress), format_time(duration))
}

fn format_time(seconds: f64) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return String::new();
    }
    let total = seconds.floor().min(u64::MAX as f64) as u64;
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let seconds = total % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}
