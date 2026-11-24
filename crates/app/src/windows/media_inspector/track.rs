//! Present the active catalog without changing the player's selected tracks.
use riina_core::{Track, TrackKind};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Identity {
    pub(super) kind: TrackKind,
    pub(super) id: i64,
}

impl From<&Track> for Identity {
    fn from(track: &Track) -> Self {
        Self {
            kind: track.kind,
            id: track.id,
        }
    }
}

pub(super) fn ordered(tracks: &[Track]) -> Vec<(usize, &Track)> {
    let mut displayed: Vec<_> = tracks.iter().enumerate().collect();
    displayed.sort_by_key(|(_, track)| match track.kind {
        TrackKind::Video => 0,
        TrackKind::Audio => 1,
        TrackKind::Sub => 2,
    });
    displayed
}

pub(super) fn label(track: &Track) -> String {
    let kind = match track.kind {
        TrackKind::Video => "Video",
        TrackKind::Audio => "Audio",
        TrackKind::Sub => "Subtitle",
    };
    format!("{kind} {} — {}", track.id, track.display_name())
}

pub(super) fn details(track: &Track) -> Vec<(&'static str, Option<String>)> {
    fn flag(value: bool) -> Option<String> {
        Some(if value { "Yes" } else { "No" }.into())
    }
    let language = track.lang.as_deref().map(|identifier| {
        let name =
            crate::platform::language::LanguageNames::current().display_name(Some(identifier));
        if name.is_empty() {
            identifier.into()
        } else {
            name
        }
    });
    vec![
        ("Track ID", Some(track.id.to_string())),
        ("Title", track.title.clone()),
        ("Language", language),
        ("Codec", track.codec.clone()),
        ("Default", flag(track.is_default)),
        ("Forced", flag(track.forced)),
        ("Selected", flag(track.selected)),
        ("External", flag(track.external)),
        ("External file", track.external_filename.clone()),
        (
            "Frame rate",
            track.frame_rate.map(|value| format!("{value:.3} fps")),
        ),
        (
            "Channels",
            track
                .channel_layout
                .clone()
                .or_else(|| track.channels.map(|value| value.to_string())),
        ),
        (
            "Sample rate",
            track.sample_rate.map(|value| format!("{value} Hz")),
        ),
    ]
}
