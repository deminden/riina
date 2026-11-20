//! Owned plugin snapshots keep history persistence outside the JavaScript host.
use crate::{playback_history::playback_media_url, plugin_files::PluginPlayerFileContext};
use riina_core::{Phase, Playback, TrackKind};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Mutex,
};

pub(crate) fn observe_playback(
    playback: &Playback,
    recent_media: &Mutex<Vec<riina_core::RecentMedia>>,
    history: &Mutex<crate::playback_history::PlaybackHistoryState>,
    record_history: bool,
    session_id: u64,
) -> (PluginPlayerFileContext, Option<String>) {
    // History actions also refresh plugin snapshots. Release history before
    // acquiring recent media so those two paths cannot invert their locks.
    let (entries, error) = {
        let mut history = history
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        history.observe(session_id, playback, record_history);
        (history.entries().to_vec(), history.take_persistence_error())
    };
    let context = player_file_context(
        playback,
        &mut recent_media
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        &entries,
    );
    (context, error)
}

impl PluginPlayerFileContext {
    fn from_playback(playback: &Playback) -> Self {
        let current_media = playback
            .playlist
            .iter()
            .find(|entry| entry.current)
            .map(|entry| entry.filename.clone());
        let current_directory = current_media
            .as_deref()
            .and_then(|filename| Path::new(filename).parent())
            .filter(|path| path.is_absolute())
            .map(Path::to_path_buf);
        let external_tracks = playback
            .tracks
            .iter()
            .filter_map(|track| {
                let alias = match track.kind {
                    TrackKind::Video => "@video",
                    TrackKind::Audio => "@audio",
                    TrackKind::Sub => "@sub",
                };
                let path = track.external_filename.as_deref()?;
                Some((format!("{alias}/{}", track.id), PathBuf::from(path)))
            })
            .collect();
        let video = playback.video.as_ref();
        let is_network_resource = current_media
            .as_deref()
            .is_some_and(riina_core::media_reference::is_network_url_candidate);
        let properties = BTreeMap::from([
            ("paused".into(), serde_json::json!(playback.paused)),
            (
                "idle".into(),
                serde_json::json!(current_media.is_none() || playback.phase == Phase::Stopped),
            ),
            ("position".into(), finite_json(playback.position)),
            ("duration".into(), finite_json(playback.duration)),
            ("speed".into(), finite_json(Some(playback.speed))),
            (
                "videoWidth".into(),
                video.map_or(serde_json::Value::Null, |video| serde_json::json!(video.w)),
            ),
            (
                "videoHeight".into(),
                video.map_or(serde_json::Value::Null, |video| serde_json::json!(video.h)),
            ),
            (
                "isNetworkResource".into(),
                serde_json::json!(is_network_resource),
            ),
            (
                "url".into(),
                current_media
                    .as_ref()
                    .and_then(|value| playback_media_url(value))
                    .map_or(serde_json::Value::Null, |value| serde_json::json!(value)),
            ),
            ("title".into(), serde_json::json!(playback.title.as_str())),
            (
                "audio.id".into(),
                selected_track_id(playback, TrackKind::Audio, 0),
            ),
            (
                "audio.delay".into(),
                serde_json::json!(playback.audio_delay),
            ),
            ("audio.volume".into(), serde_json::json!(playback.volume)),
            ("audio.muted".into(), serde_json::json!(playback.muted)),
            (
                "audio.tracks".into(),
                serialized_tracks(playback, TrackKind::Audio),
            ),
            (
                "audio.currentTrack".into(),
                current_track(playback, TrackKind::Audio, 0),
            ),
            (
                "subtitle.id".into(),
                selected_track_id(playback, TrackKind::Sub, 0),
            ),
            (
                "subtitle.secondID".into(),
                selected_track_id(playback, TrackKind::Sub, 1),
            ),
            (
                "subtitle.delay".into(),
                serde_json::json!(playback.subtitle_delay),
            ),
            (
                "subtitle.tracks".into(),
                serialized_tracks(playback, TrackKind::Sub),
            ),
            (
                "subtitle.currentTrack".into(),
                current_track(playback, TrackKind::Sub, 0),
            ),
            (
                "video.id".into(),
                selected_track_id(playback, TrackKind::Video, 0),
            ),
            (
                "video.tracks".into(),
                serialized_tracks(playback, TrackKind::Video),
            ),
            (
                "video.currentTrack".into(),
                current_track(playback, TrackKind::Video, 0),
            ),
        ]);
        let chapters = playback
            .chapters
            .iter()
            .map(|chapter| {
                serde_json::json!({
                    "title": chapter.title,
                    "start": finite_json(Some(chapter.time)),
                })
            })
            .collect();
        Self::new(current_directory, external_tracks).with_core_snapshot(properties, chapters)
    }
}

fn selected_track_id(playback: &Playback, kind: TrackKind, lane: i64) -> serde_json::Value {
    selected_track(playback, kind, lane)
        .map_or(serde_json::Value::Null, |track| serde_json::json!(track.id))
}

fn current_track(playback: &Playback, kind: TrackKind, lane: i64) -> serde_json::Value {
    selected_track(playback, kind, lane).map_or(serde_json::Value::Null, track_to_json)
}

fn selected_track(playback: &Playback, kind: TrackKind, lane: i64) -> Option<&riina_core::Track> {
    playback.tracks.iter().find(|track| {
        track.kind == kind && track.selected && track.main_selection.unwrap_or(0) == lane
    })
}

fn serialized_tracks(playback: &Playback, kind: TrackKind) -> serde_json::Value {
    serde_json::Value::Array(
        playback
            .tracks
            .iter()
            .filter(|track| track.kind == kind)
            .map(track_to_json)
            .collect(),
    )
}

fn track_to_json(track: &riina_core::Track) -> serde_json::Value {
    serde_json::json!({
        "id": track.id,
        "title": track.title,
        "formattedTitle": formatted_track_title(track),
        "lang": track.lang,
        "codec": track.codec,
        "isDefault": track.is_default,
        "isForced": track.forced,
        "isSelected": track.selected,
        "isExternal": track.external,
        "demuxW": track.width,
        "demuxH": track.height,
        "demuxChannelCount": track.channels,
        "demuxChannels": track.channel_layout,
        "demuxSamplerate": track.sample_rate,
        "demuxFPS": track.frame_rate,
    })
}

fn formatted_track_title(track: &riina_core::Track) -> String {
    let language = track
        .lang
        .as_deref()
        .filter(|language| *language != "und")
        .map(|language| {
            crate::language_names::LanguageNames::current().display_name(Some(language))
        })
        .filter(|language| !language.is_empty())
        .map_or_else(String::new, |language| format!("[{language}] "));
    let mut details = track.codec.iter().cloned().collect::<Vec<_>>();
    match track.kind {
        TrackKind::Video => {
            if let Some((width, height)) = track.width.zip(track.height) {
                details.push(format!("{width}×{height}"));
            }
            if let Some(rate) = track.frame_rate {
                details.push(format!("{}fps", track_number(rate)));
            }
        }
        TrackKind::Audio => {
            if let Some(channels) = track.channels {
                details.push(format!("{channels}ch"));
            }
            if let Some(rate) = track.sample_rate {
                details.push(format!("{}kHz", track_number(rate as f64 / 1000.0)));
            }
        }
        TrackKind::Sub => {}
    }
    if track.is_default {
        details.push("Default".into());
    }
    let title = track.title.as_deref().unwrap_or("No Title");
    let details = if details.is_empty() {
        String::new()
    } else {
        format!(" ({})", details.join(", "))
    };
    format!("#{} {language}{title}{details}", track.id)
}

fn track_number(value: f64) -> String {
    let rounded = (value * 1000.0).round() / 1000.0;
    if rounded == 0.0 {
        "0".into()
    } else {
        rounded.to_string()
    }
}

fn finite_json(value: Option<f64>) -> serde_json::Value {
    value
        .filter(|value| value.is_finite())
        .map_or(serde_json::Value::Null, |value| serde_json::json!(value))
}

pub(crate) fn player_file_context(
    playback: &Playback,
    recent_media: &mut Vec<riina_core::RecentMedia>,
    history: &[riina_core::playback_history::Entry],
) -> PluginPlayerFileContext {
    if !matches!(playback.phase, Phase::Loading | Phase::Failed)
        && let Some(entry) = playback.playlist.iter().find(|entry| entry.current)
    {
        let position = playback
            .position
            .filter(|position| position.is_finite() && *position >= 0.0)
            .or_else(|| {
                recent_media
                    .iter()
                    .find(|media| media.path == entry.filename)
                    .map(|media| media.position)
            })
            .unwrap_or(0.0);
        let title = entry
            .title
            .as_ref()
            .filter(|title| !title.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| {
                if playback.title.trim().is_empty() {
                    Path::new(&entry.filename).file_name().map_or_else(
                        || entry.filename.clone(),
                        |name| name.to_string_lossy().into(),
                    )
                } else {
                    playback.title.clone()
                }
            });
        recent_media.retain(|media| media.path != entry.filename);
        recent_media.insert(
            0,
            riina_core::RecentMedia {
                path: entry.filename.clone(),
                title,
                position,
            },
        );
        recent_media.truncate(10);
    }
    PluginPlayerFileContext::from_playback(playback)
        .with_media_generation(playback.generation)
        .with_media_ready(matches!(
            playback.phase,
            Phase::Playing | Phase::Paused | Phase::Buffering | Phase::Seeking | Phase::Ended
        ))
        .with_observed_mpv_properties(playback.observed_mpv_properties.clone())
        .with_mpv_events(playback.mpv_events.clone())
        .with_core_version(version_snapshot(playback.mpv_version.as_deref()))
        .with_recent_documents(recent_document_values(recent_media))
        .with_playback_history(playback_history_values(history))
}

pub(crate) fn version_snapshot(mpv_version: Option<&str>) -> serde_json::Value {
    serde_json::json!({
        "iina": env!("CARGO_PKG_VERSION"),
        "build": option_env!("RIINA_BUILD_NUMBER").unwrap_or("development"),
        "mpv": mpv_version,
    })
}

pub(crate) fn recent_document_values(
    recent_media: &[riina_core::RecentMedia],
) -> Vec<serde_json::Value> {
    recent_media
        .iter()
        .filter_map(|media| {
            let url = playback_media_url(&media.path)?;
            let parsed = url::Url::parse(&url).ok()?;
            let name = media_url_name(&parsed)?;
            Some(serde_json::json!({"name": name, "url": url}))
        })
        .collect()
}

pub(crate) fn playback_history_values(
    entries: &[riina_core::playback_history::Entry],
) -> Vec<serde_json::Value> {
    entries
        .iter()
        .filter_map(|entry| {
            let url = playback_media_url(&entry.path)?;
            let parsed = url::Url::parse(&url).ok()?;
            let name = media_url_name(&parsed)?;
            Some(serde_json::json!({
                "name": name,
                "url": url,
                "date": entry.date_ms,
                "progress": finite_json(entry.progress),
                "duration": entry.duration,
            }))
        })
        .collect()
}

fn media_url_name(url: &url::Url) -> Option<String> {
    url.to_file_path()
        .ok()
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .or_else(|| {
            url.path_segments()
                .and_then(Iterator::last)
                .filter(|name| !name.is_empty())
                .map(str::to_owned)
        })
}

#[cfg(test)]
mod tests;
