//! Owned track, chapter and playlist observations from the playback engine.
use crate::track_labels;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrackKind {
    Video,
    Audio,
    Sub,
}

impl TrackKind {
    pub fn property(self) -> &'static str {
        match self {
            Self::Video => "vid",
            Self::Audio => "aid",
            Self::Sub => "sid",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Track {
    pub id: i64,
    #[serde(rename = "type")]
    pub kind: TrackKind,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub lang: Option<String>,
    #[serde(default)]
    pub codec: Option<String>,
    #[serde(default)]
    pub selected: bool,
    #[serde(default, rename = "main-selection")]
    pub main_selection: Option<i64>,
    #[serde(default)]
    pub external: bool,
    #[serde(default)]
    pub albumart: bool,
    #[serde(default, rename = "external-filename")]
    pub external_filename: Option<String>,
    #[serde(default, rename = "default")]
    pub is_default: bool,
    #[serde(default)]
    pub forced: bool,
    #[serde(default, rename = "demux-channel-count")]
    pub channels: Option<i64>,
    #[serde(default, rename = "demux-channels")]
    pub channel_layout: Option<String>,
    #[serde(default, rename = "demux-samplerate")]
    pub sample_rate: Option<i64>,
    #[serde(default, rename = "demux-w")]
    pub width: Option<i64>,
    #[serde(default, rename = "demux-h")]
    pub height: Option<i64>,
    #[serde(default, rename = "demux-fps")]
    pub frame_rate: Option<f64>,
}

impl Track {
    pub fn is_primary_selected(&self) -> bool {
        // Both subtitle lanes can be decoded simultaneously. `selected` alone
        // cannot identify which lane should be highlighted in the primary list.
        self.selected && self.main_selection == Some(0)
    }

    pub fn display_name(&self) -> String {
        // MPVTrack.readableString excludes the language in the sidebar; the UI
        // displays it separately. Preserve title and codec/channel/rate metadata.
        let mut info = self.codec.iter().cloned().collect::<Vec<_>>();
        match self.kind {
            TrackKind::Video => {
                // Demux dimensions identify the track; a crop or rotation must
                // not rewrite its catalog description using output dimensions.
                if let Some((width, height)) = self.width.zip(self.height) {
                    info.push(format!("{width}×{height}"));
                }
                if let Some(rate) = self.frame_rate {
                    info.push(format!("{}fps", track_labels::number(rate)));
                }
            }
            TrackKind::Audio => {
                if let Some(channels) = self.channels {
                    info.push(format!("{channels}ch"));
                }
                if let Some(rate) = self.sample_rate {
                    info.push(format!("{}kHz", track_labels::number(rate as f64 / 1000.0)));
                }
            }
            TrackKind::Sub => {}
        }
        if self.is_default {
            info.push("Default".into());
        }
        let title = self.title.as_deref().unwrap_or("No Title");
        if info.is_empty() {
            title.into()
        } else if title.is_empty() {
            format!("({})", info.join(", "))
        } else {
            format!("{title} ({})", info.join(", "))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Chapter {
    pub time: f64,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PlaylistEntry {
    pub id: i64,
    pub filename: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub current: bool,
    #[serde(default)]
    pub playing: bool,
}
