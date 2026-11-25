//! Copy native observations into owned playback state before the next event.
use crate::ffi::{self, Property, format};
use riina_core::{Phase, Playback};
use std::ffi::CStr;
#[cfg(test)]
use std::ffi::CString;

pub(super) fn decode_audio_devices(
    value: serde_json::Value,
) -> Result<Vec<riina_core::audio_preferences::OutputDevice>, String> {
    let devices: Vec<riina_core::audio_preferences::OutputDevice> =
        serde_json::from_value(value).map_err(|error| error.to_string())?;
    if devices.iter().any(|device| {
        device.name.is_empty() || device.name.contains('\0') || device.description.contains('\0')
    }) {
        return Err("mpv returned an invalid audio device entry".into());
    }
    Ok(devices)
}

#[derive(Debug)]
pub(super) enum ObservedPropertyError {
    UnexpectedFormat(std::ffi::c_int),
    Decode(ffi::NodeDecodeError),
    Serialize(serde_json::Error),
    PluginLimit,
}

impl std::fmt::Display for ObservedPropertyError {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnexpectedFormat(format) => {
                write!(
                    output,
                    "mpv returned format {format} instead of a generic node"
                )
            }
            Self::Decode(error) => error.fmt(output),
            Self::Serialize(error) => error.fmt(output),
            Self::PluginLimit => {
                output.write_str("mpv property value exceeds the 1 MiB plugin event limit")
            }
        }
    }
}

impl std::error::Error for ObservedPropertyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Decode(error) => Some(error),
            Self::Serialize(error) => Some(error),
            _ => None,
        }
    }
}

pub(super) unsafe fn observed_property_value(
    property: &Property,
) -> Result<serde_json::Value, ObservedPropertyError> {
    if property.data.is_null() {
        return Ok(serde_json::Value::Null);
    }
    if property.format != format::NODE {
        return Err(ObservedPropertyError::UnexpectedFormat(property.format));
    }
    // SAFETY: MPV_FORMAT_NODE selects the live node arm and the callback copies
    // it before the current mpv_wait_event result is replaced.
    let node = unsafe { &*property.data.cast::<ffi::Node>() };
    // SAFETY: the worker has not waited for another event, so node-owned data is live.
    let value = unsafe { node.copy_json() }.map_err(ObservedPropertyError::Decode)?;
    bounded_plugin_value(value)
}

pub(super) unsafe fn queried_property_value(
    property: &Property,
) -> Result<serde_json::Value, ObservedPropertyError> {
    use serde_json::Value;
    if property.data.is_null() {
        return Ok(Value::Null);
    }
    // SAFETY: the live reply owns a correctly tagged property value.
    let value = unsafe { property_value(property) }.map_err(ObservedPropertyError::Decode)?;
    bounded_plugin_value(match value {
        Some(PropertyValue::Node(value)) => value,
        Some(PropertyValue::Flag(flag)) => Value::Bool(flag),
        Some(PropertyValue::Double(value)) => Value::from(value),
        Some(PropertyValue::Integer(value)) => Value::from(value),
        Some(PropertyValue::Text(value)) => Value::String(value),
        None => Value::Null,
    })
}

fn bounded_plugin_value(
    value: serde_json::Value,
) -> Result<serde_json::Value, ObservedPropertyError> {
    let encoded = serde_json::to_vec(&value).map_err(ObservedPropertyError::Serialize)?;
    if encoded.len() > 1024 * 1024 {
        return Err(ObservedPropertyError::PluginLimit);
    }
    Ok(value)
}

fn metadata_value(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .as_object()?
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(key))?
        .1
        .as_str()
        .map(str::to_owned)
}

enum PropertyValue {
    Node(serde_json::Value),
    Flag(bool),
    Double(f64),
    Integer(i64),
    Text(String),
}

pub(super) struct ObservedProperty {
    pub(super) name: &'static CStr,
    pub(super) format: std::ffi::c_int,
    apply: fn(&mut Playback, PropertyValue) -> Result<(), String>,
    clear: fn(&mut Playback),
}

macro_rules! property {
    (@format Node) => { format::NODE };
    (@format Flag) => { format::FLAG };
    (@format Double) => { format::DOUBLE };
    (@format Integer) => { format::INT64 };
    (@format Text) => { format::STRING };
    (@clear) => { |_| {} };
    (@clear $clear:expr) => { $clear };
    ($name:literal, $kind:ident, |$state:ident, $value:ident| $apply:block $(, $clear:expr)?) => {
        ObservedProperty {
            name: $name,
            format: property!(@format $kind),
            apply: |$state, property| {
                if let PropertyValue::$kind($value) = property {
                    $apply
                }
                Ok(())
            },
            clear: property!(@clear $($clear)?),
        }
    };
}

pub(super) static OBSERVED: &[ObservedProperty] = &[
    property!(
        c"time-pos",
        Double,
        |state, value| {
            state.position = (value >= 0.0).then_some(value);
        },
        |state| {
            state.position = None;
        }
    ),
    property!(
        c"duration",
        Double,
        |state, value| {
            state.duration = (value >= 0.0).then_some(value);
        },
        |state| {
            state.duration = None;
        }
    ),
    property!(
        c"time-remaining",
        Double,
        |state, value| {
            state.remaining_time = value.is_finite().then_some(value.max(0.0));
        },
        |state| {
            state.remaining_time = None;
        }
    ),
    property!(
        c"playtime-remaining",
        Double,
        |state, value| {
            state.scaled_remaining_time = value.is_finite().then_some(value.max(0.0));
        },
        |state| {
            state.scaled_remaining_time = None;
        }
    ),
    property!(c"pause", Flag, |state, flag| {
        state.paused = flag;
    }),
    property!(c"paused-for-cache", Flag, |state, flag| {
        state.buffering = flag;
    }),
    property!(c"seeking", Flag, |state, flag| {
        state.seeking = flag;
    }),
    property!(c"seekable", Flag, |state, flag| {
        state.seekable = flag;
    }),
    property!(
        c"ignore-path-in-watch-later-config",
        Flag,
        |state, flag| {
            state.ignore_path_in_watch_later_config = Some(flag);
        },
        |state| {
            state.ignore_path_in_watch_later_config = None;
        }
    ),
    property!(
        c"save-position-on-quit",
        Flag,
        |state, flag| {
            state.save_position_on_quit = Some(flag);
        },
        |state| {
            state.save_position_on_quit = None;
        }
    ),
    property!(c"volume", Double, |state, value| {
        state.volume = value;
    }),
    property!(c"mute", Flag, |state, flag| {
        state.muted = flag;
    }),
    property!(c"speed", Double, |state, value| {
        state.speed = value;
    }),
    property!(c"media-title", Text, |state, value| {
        state.title = value;
    }),
    property!(
        c"mpv-version",
        Text,
        |state, value| {
            state.mpv_version = Some(value);
        },
        |state| {
            state.mpv_version = None;
        }
    ),
    property!(
        c"ffmpeg-version",
        Text,
        |state, value| {
            state.ffmpeg_version = Some(value);
        },
        |state| {
            state.ffmpeg_version = None;
        }
    ),
    property!(c"eof-reached", Flag, |state, flag| {
        if flag {
            state.phase = Phase::Ended;
        } else if state.phase == Phase::Ended {
            state.phase = Phase::Playing;
        }
    }),
    property!(
        c"current-ao",
        Text,
        |state, value| {
            state.audio_output = Some(value);
        },
        |state| {
            state.audio_output = None;
        }
    ),
    property!(
        c"audio-device",
        Text,
        |state, value| {
            state.audio_device = Some(value);
        },
        |state| {
            state.audio_device = None;
        }
    ),
    property!(
        c"hwdec-current",
        Text,
        |state, value| {
            state.hardware_decoder = Some(value);
        },
        |state| {
            state.hardware_decoder = None;
        }
    ),
    property!(
        c"hwdec",
        Text,
        |state, value| {
            state.hardware_decoding = Some(value);
        },
        |state| {
            state.hardware_decoding = None;
        }
    ),
    property!(
        c"vf",
        Node,
        |state, value| {
            state.video_filters = Some(serde_json::from_value(value).map_err(|e| e.to_string())?);
        },
        |state| {
            state.video_filters = None;
        }
    ),
    property!(
        c"af",
        Node,
        |state, value| {
            let raw_filters: Vec<serde_json::Value> =
                serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
            let filters: Vec<riina_core::crop::VideoFilter> =
                serde_json::from_value(value).map_err(|e| e.to_string())?;
            state.audio_equalizer = riina_core::audio_equalizer::observed_gains(&raw_filters);
            state.audio_filters = Some(filters);
        },
        |state| {
            state.audio_equalizer = None;
            state.audio_filters = None;
        }
    ),
    property!(
        c"track-list",
        Node,
        |state, value| {
            state.tracks = serde_json::from_value::<Vec<_>>(value)
                .map_err(|e| e.to_string())?
                .into();
            state.track_list_observed = true;
        },
        |state| {
            state.tracks = Default::default();
        }
    ),
    property!(
        c"chapter-list",
        Node,
        |state, value| {
            state.chapters = serde_json::from_value::<Vec<_>>(value)
                .map_err(|e| e.to_string())?
                .into();
        },
        |state| {
            state.chapters = Default::default();
        }
    ),
    property!(
        c"playlist",
        Node,
        |state, value| {
            state.playlist = serde_json::from_value::<Vec<_>>(value)
                .map_err(|e| e.to_string())?
                .into();
        },
        |state| {
            state.playlist = Default::default();
        }
    ),
    property!(
        c"audio-device-list",
        Node,
        |state, value| {
            state.audio_devices = decode_audio_devices(value)?.into();
        },
        |state| {
            state.audio_devices = Default::default();
        }
    ),
    property!(
        c"video-params",
        Node,
        |state, value| {
            state.video = video_parameters(value)?;
        },
        |state| {
            state.video = None;
        }
    ),
    property!(
        c"video-target-params",
        Node,
        |state, value| {
            state.video_target = video_parameters(value)?;
        },
        |state| {
            state.video_target = None;
        }
    ),
    property!(
        c"metadata",
        Node,
        |state, value| {
            state.artist = metadata_value(&value, "artist");
            state.album = metadata_value(&value, "album");
        },
        |state| {
            state.artist = None;
            state.album = None;
        }
    ),
    property!(
        c"video-aspect-override",
        Double,
        |state, value| {
            state.video_aspect_override = Some(value);
        },
        |state| {
            state.video_aspect_override = None;
        }
    ),
    property!(
        c"video-rotate",
        Integer,
        |state, value| {
            state.video_rotation = Some(value);
        },
        |state| {
            state.video_rotation = None;
        }
    ),
    property!(
        c"deinterlace",
        Text,
        |state, value| {
            state.deinterlace = match value.as_str() {
                "yes" => Some(true),
                "no" => Some(false),
                _ => None,
            }
        },
        |state| {
            state.deinterlace = None;
        }
    ),
    property!(
        c"deinterlace-active",
        Flag,
        |state, flag| {
            state.deinterlace_active = Some(flag);
        },
        |state| {
            state.deinterlace_active = None;
        }
    ),
    property!(
        c"video-frame-info",
        Node,
        |state, value| {
            state.video_frame_info =
                if value.is_null() || value.as_object().is_some_and(|map| map.is_empty()) {
                    None
                } else {
                    Some(serde_json::from_value(value).map_err(|e| e.to_string())?)
                };
        },
        |state| {
            state.video_frame_info = None;
        }
    ),
    property!(
        c"video-out-params",
        Node,
        |state, value| {
            state.video_output = video_parameters(value)?;
        },
        |state| {
            state.video_output = None;
        }
    ),
    property!(
        c"brightness",
        Integer,
        |state, value| {
            state.video_equalizer[riina_core::video_controls::Equalizer::Brightness.index()] =
                (-100..=100).contains(&value).then_some(value);
        },
        |state| {
            state.video_equalizer[riina_core::video_controls::Equalizer::Brightness.index()] = None;
        }
    ),
    property!(
        c"contrast",
        Integer,
        |state, value| {
            state.video_equalizer[riina_core::video_controls::Equalizer::Contrast.index()] =
                (-100..=100).contains(&value).then_some(value);
        },
        |state| {
            state.video_equalizer[riina_core::video_controls::Equalizer::Contrast.index()] = None;
        }
    ),
    property!(
        c"saturation",
        Integer,
        |state, value| {
            state.video_equalizer[riina_core::video_controls::Equalizer::Saturation.index()] =
                (-100..=100).contains(&value).then_some(value);
        },
        |state| {
            state.video_equalizer[riina_core::video_controls::Equalizer::Saturation.index()] = None;
        }
    ),
    property!(
        c"gamma",
        Integer,
        |state, value| {
            state.video_equalizer[riina_core::video_controls::Equalizer::Gamma.index()] =
                (-100..=100).contains(&value).then_some(value);
        },
        |state| {
            state.video_equalizer[riina_core::video_controls::Equalizer::Gamma.index()] = None;
        }
    ),
    property!(
        c"hue",
        Integer,
        |state, value| {
            state.video_equalizer[riina_core::video_controls::Equalizer::Hue.index()] =
                (-100..=100).contains(&value).then_some(value);
        },
        |state| {
            state.video_equalizer[riina_core::video_controls::Equalizer::Hue.index()] = None;
        }
    ),
    property!(c"audio-delay", Double, |state, value| {
        state.audio_delay = value;
    }),
    property!(c"sub-delay", Double, |state, value| {
        state.subtitle_delay = value;
    }),
    property!(c"sub-visibility", Flag, |state, flag| {
        state.subtitle_visible = flag;
    }),
    property!(
        c"sub-scale",
        Double,
        |state, value| {
            state.subtitle_scale = (0.0..=100.0).contains(&value).then_some(value);
        },
        |state| {
            state.subtitle_scale = None;
        }
    ),
    property!(
        c"sub-pos",
        Double,
        |state, value| {
            state.subtitle_position = (0.0..=150.0).contains(&value).then_some(value);
        },
        |state| {
            state.subtitle_position = None;
        }
    ),
    property!(
        c"sub-font-size",
        Double,
        |state, value| {
            state.subtitle_font_size = (value > 0.0).then_some(value);
        },
        |state| {
            state.subtitle_font_size = None;
        }
    ),
    property!(
        c"sub-outline-size",
        Double,
        |state, value| {
            state.subtitle_outline_size = (value >= 0.0).then_some(value);
        },
        |state| {
            state.subtitle_outline_size = None;
        }
    ),
    property!(
        c"sub-blur",
        Double,
        |state, value| {
            state.subtitle_blur = (0.0..=20.0).contains(&value).then_some(value);
        },
        |state| {
            state.subtitle_blur = None;
        }
    ),
    property!(
        c"sub-spacing",
        Double,
        |state, value| {
            state.subtitle_spacing = Some(value);
        },
        |state| {
            state.subtitle_spacing = None;
        }
    ),
    property!(
        c"sub-shadow-offset",
        Double,
        |state, value| {
            state.subtitle_shadow_offset = (value >= 0.0).then_some(value);
        },
        |state| {
            state.subtitle_shadow_offset = None;
        }
    ),
    property!(
        c"sub-color",
        Text,
        |state, value| {
            state.subtitle_text_color = riina_core::subtitle_color::SubtitleColor::from_mpv(&value);
        },
        |state| {
            state.subtitle_text_color = None;
        }
    ),
    property!(
        c"sub-outline-color",
        Text,
        |state, value| {
            state.subtitle_border_color =
                riina_core::subtitle_color::SubtitleColor::from_mpv(&value);
        },
        |state| {
            state.subtitle_border_color = None;
        }
    ),
    property!(
        c"sub-back-color",
        Text,
        |state, value| {
            state.subtitle_shadow_color =
                riina_core::subtitle_color::SubtitleColor::from_mpv(&value);
        },
        |state| {
            state.subtitle_shadow_color = None;
        }
    ),
    property!(
        c"sub-bold",
        Flag,
        |state, flag| {
            state.subtitle_bold = Some(flag);
        },
        |state| {
            state.subtitle_bold = None;
        }
    ),
    property!(
        c"sub-italic",
        Flag,
        |state, flag| {
            state.subtitle_italic = Some(flag);
        },
        |state| {
            state.subtitle_italic = None;
        }
    ),
    property!(
        c"sub-font",
        Text,
        |state, value| {
            state.subtitle_font =
                riina_core::subtitle_controls::valid_font_name(&value).then_some(value);
        },
        |state| {
            state.subtitle_font = None;
        }
    ),
    property!(
        c"secondary-sub-visibility",
        Flag,
        |state, flag| {
            state.secondary_subtitle_visible = Some(flag);
        },
        |state| {
            state.secondary_subtitle_visible = None;
        }
    ),
    property!(
        c"secondary-sub-text",
        Text,
        |state, value| {
            state.secondary_subtitle_text = Some(value);
        },
        |state| {
            state.secondary_subtitle_text = None;
        }
    ),
    property!(
        c"secondary-sub-delay",
        Double,
        |state, value| {
            state.secondary_subtitle_delay = Some(value);
        },
        |state| {
            state.secondary_subtitle_delay = None;
        }
    ),
    property!(
        c"secondary-sub-pos",
        Double,
        |state, value| {
            state.secondary_subtitle_position = (0.0..=150.0).contains(&value).then_some(value);
        },
        |state| {
            state.secondary_subtitle_position = None;
        }
    ),
    property!(
        c"sub-text",
        Text,
        |state, value| {
            state.subtitle_text = Some(value);
        },
        |state| {
            state.subtitle_text = None;
        }
    ),
    property!(
        c"loop-file",
        Text,
        |state, value| {
            state.loop_file = Some(value);
        },
        |state| {
            state.loop_file = None;
        }
    ),
    property!(
        c"loop-playlist",
        Text,
        |state, value| {
            state.loop_playlist = Some(value);
        },
        |state| {
            state.loop_playlist = None;
        }
    ),
    property!(
        c"ab-loop-a",
        Node,
        |state, value| {
            state.ab_loop_a = loop_point(&value);
        },
        |state| {
            state.ab_loop_a = None;
        }
    ),
    property!(
        c"ab-loop-b",
        Node,
        |state, value| {
            state.ab_loop_b = loop_point(&value);
        },
        |state| {
            state.ab_loop_b = None;
        }
    ),
];

fn loop_point(value: &serde_json::Value) -> Option<f64> {
    // An unset point is usually "no". It must remain distinct from time zero.
    value
        .as_f64()
        .filter(|point| point.is_finite() && *point >= 0.0)
}

// Source, target and output geometry each have their own field. The target
// can describe presentation while the source still carries PQ/HLG.
fn video_parameters(
    value: serde_json::Value,
) -> Result<Option<riina_core::VideoParameters>, String> {
    if value.is_null() || value.as_object().is_some_and(|map| map.is_empty()) {
        Ok(None)
    } else {
        serde_json::from_value(value)
            .map(Some)
            .map_err(|error| error.to_string())
    }
}

fn observed(name: &[u8]) -> Option<&'static ObservedProperty> {
    use std::{collections::HashMap, sync::OnceLock};
    static LOOKUP: OnceLock<HashMap<&'static [u8], &'static ObservedProperty>> = OnceLock::new();
    LOOKUP
        .get_or_init(|| {
            OBSERVED
                .iter()
                .map(|property| (property.name.to_bytes(), property))
                .collect()
        })
        .get(name)
        .copied()
}

pub(super) unsafe fn apply_property(state: &mut Playback, property: &Property) {
    // SAFETY: the event holds a live, NUL-terminated property name until the worker waits again.
    let name = unsafe { CStr::from_ptr(property.name) }.to_bytes();
    let row = observed(name);
    if property.data.is_null() {
        if let Some(row) = row {
            (row.clear)(state);
        }
        return;
    }
    // SAFETY: the pointer and any borrowed node contents are live through this call.
    let result = unsafe { property_value(property) }
        .map_err(|error| error.to_string())
        .and_then(|value| {
            if let (Some(row), Some(value)) = (row, value) {
                if row.format == format::DOUBLE
                    && matches!(&value, PropertyValue::Double(number) if !number.is_finite())
                {
                    (row.clear)(state);
                } else {
                    (row.apply)(state, value)?;
                }
            }
            Ok(())
        });
    if let Err(error) = result {
        crate::record_notice(
            state,
            riina_core::NoticeSeverity::Error,
            format!("Cannot decode mpv property: {error}"),
        );
    }
}

unsafe fn property_value(
    property: &Property,
) -> Result<Option<PropertyValue>, ffi::NodeDecodeError> {
    let value = match property.format {
        format::NODE => {
            // SAFETY: this live non-null payload is tagged as an mpv_node.
            let node = unsafe { &*property.data.cast::<ffi::Node>() };
            // SAFETY: the worker copies the tree before waiting for the next event.
            PropertyValue::Node(unsafe { node.copy_json() }?)
        }
        format::FLAG => {
            // SAFETY: a live flag payload points to a non-null C integer.
            PropertyValue::Flag(unsafe { *property.data.cast::<i32>() } != 0)
        }
        format::DOUBLE => {
            // SAFETY: this non-null payload is tagged as a C double.
            PropertyValue::Double(unsafe { *property.data.cast::<f64>() })
        }
        format::INT64 => {
            // SAFETY: this non-null payload is tagged as a 64-bit integer.
            PropertyValue::Integer(unsafe { *property.data.cast::<i64>() })
        }
        format::STRING => {
            // SAFETY: string payloads carry a pointer to the C string pointer.
            let text = unsafe { *property.data.cast::<*const std::ffi::c_char>() };
            if text.is_null() {
                return Ok(None);
            }
            // SAFETY: the event owns this NUL-terminated string until the next wait.
            PropertyValue::Text(
                unsafe { CStr::from_ptr(text) }
                    .to_string_lossy()
                    .into_owned(),
            )
        }
        _ => return Ok(None),
    };
    Ok(Some(value))
}

#[cfg(test)]
mod observer_table_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn every_observed_property_is_unique_and_projects_a_sample_value() {
        let mut names = std::collections::HashSet::new();
        for row in OBSERVED {
            assert!(names.insert(row.name), "Duplicate property {:?}", row.name);
            assert!(std::ptr::eq(observed(row.name.to_bytes()).unwrap(), row));
            let mut state = Playback::default();
            (row.clear)(&mut state);
            let cleared = format!("{state:?}");
            if let Err(error) = (row.apply)(&mut state, sample(row)) {
                panic!("Could not project {:?}: {error}", row.name);
            }
            assert_ne!(
                format!("{state:?}"),
                cleared,
                "No change from {:?}",
                row.name
            );
        }
    }

    fn sample(row: &ObservedProperty) -> PropertyValue {
        match row.format {
            format::FLAG => PropertyValue::Flag(true),
            format::DOUBLE => PropertyValue::Double(1.25),
            format::INT64 => PropertyValue::Integer(35),
            format::STRING => PropertyValue::Text(
                match row.name.to_bytes() {
                    b"deinterlace" => "yes",
                    b"sub-color" | b"sub-outline-color" | b"sub-back-color" => "0.1/0.2/0.3/0.4",
                    _ => "Example",
                }
                .into(),
            ),
            format::NODE => PropertyValue::Node(match row.name.to_bytes() {
                b"ab-loop-a" | b"ab-loop-b" => json!(1.25),
                b"track-list" => json!([{"id": 1, "type": "audio"}]),
                b"chapter-list" => json!([{"time": 1.25, "title": "Opening"}]),
                b"playlist" => json!([{"id": 1, "filename": "clip.mkv"}]),
                b"audio-device-list" => json!([{"name": "default", "description": "Test device"}]),
                b"video-params" | b"video-target-params" | b"video-out-params" => {
                    json!({"w": 320, "h": 180})
                }
                b"video-frame-info" => json!({"interlaced": true}),
                b"metadata" => json!({"artist": "Test artist"}),
                b"vf" | b"af" => json!([]),
                _ => panic!("Provide a node sample for {:?}", row.name),
            }),
            _ => panic!("Provide a sample for format {}", row.format),
        }
    }
}

#[cfg(test)]
mod version_property_tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn loaded_engine_version_is_observed_and_can_become_unavailable() {
        let name = CString::new("mpv-version").unwrap();
        let version = CString::new("mpv 0.40.0-123-gabcdef").unwrap();
        let mut data = version.as_ptr();
        let property = Property {
            name: name.as_ptr(),
            format: format::STRING,
            data: (&mut data as *mut *const std::ffi::c_char).cast(),
        };
        let mut playback = Playback::default();
        // SAFETY: both pointers remain valid for the complete property copy.
        unsafe { apply_property(&mut playback, &property) };
        assert_eq!(
            playback.mpv_version.as_deref(),
            Some("mpv 0.40.0-123-gabcdef")
        );

        let unavailable = Property {
            name: name.as_ptr(),
            format: format::STRING,
            data: std::ptr::null_mut(),
        };
        // SAFETY: the null event is valid and does not dereference data.
        unsafe { apply_property(&mut playback, &unavailable) };
        assert_eq!(playback.mpv_version, None);
    }

    #[test]
    fn watch_later_path_policy_is_observed_and_can_become_unavailable() {
        let name = CString::new("ignore-path-in-watch-later-config").unwrap();
        let mut flag = 1_i32;
        let property = Property {
            name: name.as_ptr(),
            format: format::FLAG,
            data: (&mut flag as *mut i32).cast(),
        };
        let mut playback = Playback::default();
        // SAFETY: the flag remains valid for the complete property copy.
        unsafe { apply_property(&mut playback, &property) };
        assert_eq!(playback.ignore_path_in_watch_later_config, Some(true));

        let unavailable = Property {
            name: name.as_ptr(),
            format: format::FLAG,
            data: std::ptr::null_mut(),
        };
        // SAFETY: the null event is valid and does not dereference data.
        unsafe { apply_property(&mut playback, &unavailable) };
        assert_eq!(playback.ignore_path_in_watch_later_config, None);
    }

    #[test]
    fn save_position_policy_is_observed_and_can_become_unavailable() {
        let name = CString::new("save-position-on-quit").unwrap();
        let mut flag = 1_i32;
        let property = Property {
            name: name.as_ptr(),
            format: format::FLAG,
            data: (&mut flag as *mut i32).cast(),
        };
        let mut playback = Playback::default();
        // SAFETY: the flag remains valid for the complete property copy.
        unsafe { apply_property(&mut playback, &property) };
        assert_eq!(playback.save_position_on_quit, Some(true));

        let unavailable = Property {
            name: name.as_ptr(),
            format: format::FLAG,
            data: std::ptr::null_mut(),
        };
        // SAFETY: the null event is valid and does not dereference data.
        unsafe { apply_property(&mut playback, &unavailable) };
        assert_eq!(playback.save_position_on_quit, None);
    }
}

#[cfg(test)]
mod audio_device_property_tests {
    use super::*;

    #[test]
    fn observed_audio_devices_keep_engine_ids_and_descriptions() {
        let devices = decode_audio_devices(serde_json::json!([
            {"name": "auto", "description": "Autoselect device"},
            {"name": "wasapi/{endpoint}", "description": "Speakers"}
        ]))
        .unwrap();
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[1].name, "wasapi/{endpoint}");
        assert_eq!(devices[1].description, "Speakers");
    }

    #[test]
    fn malformed_native_audio_device_entries_are_rejected() {
        assert!(
            decode_audio_devices(serde_json::json!([
                {"name": "", "description": "No identifier"}
            ]))
            .is_err()
        );
        assert!(
            decode_audio_devices(serde_json::json!([
                {"name": "wasapi/device\u{0000}", "description": "Bad ID"}
            ]))
            .is_err()
        );
    }
}

#[cfg(test)]
mod metadata_property_tests {
    use super::metadata_value;

    #[test]
    fn metadata_fields_are_read_case_insensitively_and_reject_non_text_values() {
        let metadata = serde_json::json!({
            "ARTIST": "Björk",
            "album": "Debut",
            "track": 7
        });
        assert_eq!(
            metadata_value(&metadata, "artist").as_deref(),
            Some("Björk")
        );
        assert_eq!(metadata_value(&metadata, "ALBUM").as_deref(), Some("Debut"));
        assert_eq!(metadata_value(&metadata, "track"), None);
        assert_eq!(metadata_value(&metadata, "title"), None);
    }
}

#[cfg(test)]
mod dynamic_property_tests {
    use super::*;

    #[test]
    fn plugin_observed_node_properties_are_copied_as_json() {
        let name = CString::new("cache-buffering-state").unwrap();
        let mut node = ffi::Node {
            value: ffi::NodeValue { integer: 42 },
            format: format::INT64,
        };
        let property = Property {
            name: name.as_ptr(),
            format: format::NODE,
            data: (&mut node as *mut ffi::Node).cast(),
        };
        // SAFETY: the node and name remain valid while the helper copies them.
        let value = unsafe { observed_property_value(&property) }.unwrap();
        assert_eq!(value, serde_json::json!(42));
    }

    #[test]
    fn unavailable_plugin_observed_properties_become_json_null() {
        let name = CString::new("cache-buffering-state").unwrap();
        let property = Property {
            name: name.as_ptr(),
            format: format::NODE,
            data: std::ptr::null_mut(),
        };
        // SAFETY: null property data is explicitly represented as JSON null.
        let value = unsafe { observed_property_value(&property) }.unwrap();
        assert_eq!(value, serde_json::Value::Null);
    }
}

#[cfg(test)]
mod clock_property_tests {
    use super::*;

    #[test]
    fn invalid_clock_observations_clear_previous_values_and_valid_values_recover() {
        for property_name in ["time-pos", "duration"] {
            let name = CString::new(property_name).unwrap();
            let mut state = Playback::default();
            for value in [
                42.0,
                f64::NAN,
                17.0,
                f64::INFINITY,
                12.0,
                f64::NEG_INFINITY,
                -1.0,
                0.0,
            ] {
                let mut data = value;
                let property = Property {
                    name: name.as_ptr(),
                    format: format::DOUBLE,
                    data: (&mut data as *mut f64).cast(),
                };
                // SAFETY: the name and native double live through the observation copy.
                unsafe { apply_property(&mut state, &property) };
                let actual = if property_name == "time-pos" {
                    state.position
                } else {
                    state.duration
                };
                assert_eq!(
                    actual,
                    (value.is_finite() && value >= 0.0).then_some(value),
                    "{property_name}: {value}"
                );
                assert_eq!(state.error(), None);
            }
        }
    }
}
