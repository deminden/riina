//! Inspector fields that need a read from the active libmpv client.
use super::sample::Request;
use riina_mpv::{CommandSender, MpvPropertyFormat};
use riina_ui::InspectorTab;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

pub(super) struct Field {
    pub(super) label: &'static str,
    pub(super) property: &'static str,
}

pub(super) const FILE: &[Field] = &[
    Field {
        label: "Path",
        property: "path",
    },
    Field {
        label: "Size",
        property: "file-size",
    },
    Field {
        label: "Format",
        property: "file-format",
    },
    Field {
        label: "Chapters",
        property: "chapters",
    },
    Field {
        label: "Editions",
        property: "editions",
    },
    Field {
        label: "Title",
        property: "media-title",
    },
    Field {
        label: "Comment",
        property: "metadata/by-key/comment",
    },
];

pub(super) const VIDEO: &[Field] = &[
    Field {
        label: "Codec",
        property: "current-tracks/video/codec",
    },
    Field {
        label: "Codec description",
        property: "current-tracks/video/codec-desc",
    },
    Field {
        label: "Decoder",
        property: "hwdec-current",
    },
    Field {
        label: "Pixel format",
        property: "video-params/pixelformat",
    },
    Field {
        label: "Hardware pixel format",
        property: "video-params/hw-pixelformat",
    },
    Field {
        label: "Color matrix",
        property: "video-params/colormatrix",
    },
    Field {
        label: "Primaries",
        property: "video-params/primaries",
    },
    Field {
        label: "Transfer function",
        property: "video-params/gamma",
    },
    Field {
        label: "Signal peak",
        property: "video-params/sig-peak",
    },
    Field {
        label: "Output",
        property: "current-vo",
    },
    Field {
        label: "Frame rate",
        property: "container-fps",
    },
    Field {
        label: "Bit rate",
        property: "video-bitrate",
    },
];

pub(super) const AUDIO: &[Field] = &[
    Field {
        label: "Codec",
        property: "current-tracks/audio/codec-desc",
    },
    Field {
        label: "Format",
        property: "audio-params/format",
    },
    Field {
        label: "Output",
        property: "current-ao",
    },
    Field {
        label: "Channels",
        property: "audio-params/channels",
    },
    Field {
        label: "Sample rate",
        property: "audio-params/samplerate",
    },
    Field {
        label: "Bit rate",
        property: "audio-bitrate",
    },
];

pub(super) const STATUS: &[Field] = &[
    Field {
        label: "A/V sync",
        property: "avsync",
    },
    Field {
        label: "Total A/V sync correction",
        property: "total-avsync-change",
    },
    Field {
        label: "Dropped frames",
        property: "frame-drop-count",
    },
    Field {
        label: "Mistimed frames",
        property: "mistimed-frame-count",
    },
    Field {
        label: "Display refresh rate",
        property: "display-fps",
    },
    Field {
        label: "Estimated video frame rate",
        property: "estimated-vf-fps",
    },
    Field {
        label: "Estimated display refresh rate",
        property: "estimated-display-fps",
    },
];

pub(super) type Values = BTreeMap<String, Option<String>>;

pub(super) fn for_tab(tab: InspectorTab, track: Option<usize>, watches: &[String]) -> Vec<String> {
    let fields = |group: &[Field]| {
        group
            .iter()
            .map(|field| field.property.to_owned())
            .collect::<Vec<_>>()
    };
    match tab {
        InspectorTab::General => [fields(VIDEO), fields(AUDIO)].concat(),
        InspectorTab::Tracks => track.map_or_else(Vec::new, |index| {
            ["src-id", "decoder-desc"]
                .map(|key| format!("track-list/{index}/{key}"))
                .into()
        }),
        InspectorTab::File => [fields(FILE), vec!["filename".into()]].concat(),
        InspectorTab::Status => [fields(STATUS), watches.to_vec()].concat(),
    }
}

/// Read-only queries execute through the playback worker's acknowledged queue.
pub(super) fn read(
    sender: &CommandSender,
    properties: impl IntoIterator<Item = String>,
    request: &Request,
) -> Values {
    let deadline = Instant::now() + Duration::from_millis(750);
    let mut values = BTreeMap::new();
    for property in properties {
        if request.cancelled() || Instant::now() >= deadline {
            break;
        }
        values.entry(property).or_insert_with_key(|name| {
            sender
                .read_mpv_property(name, MpvPropertyFormat::Text, deadline)
                .ok()
                .and_then(|value| {
                    value
                        .as_str()
                        .filter(|text| !text.is_empty())
                        .map(str::to_owned)
                })
        });
    }
    values
}
