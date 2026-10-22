//! Copy native observations into owned playback state before the next event.
use crate::ffi::{self, Property};
use riina_core::{Phase, Playback};
use std::ffi::CStr;
#[cfg(test)]
use std::ffi::CString;

fn decode_audio_devices(
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

pub(super) unsafe fn observed_property_value(
    property: &Property,
) -> Result<serde_json::Value, String> {
    if property.data.is_null() {
        return Ok(serde_json::Value::Null);
    }
    if property.format != 6 {
        return Err(format!(
            "mpv returned format {} instead of a generic node",
            property.format
        ));
    }
    // SAFETY: MPV_FORMAT_NODE selects the live node arm and the callback copies
    // it before the current mpv_wait_event result is replaced.
    let node = unsafe { &*property.data.cast::<ffi::Node>() };
    let value = unsafe { node.copy_json()? };
    let encoded = serde_json::to_vec(&value).map_err(|error| error.to_string())?;
    if encoded.len() > 1024 * 1024 {
        return Err("mpv property value exceeds the 1 MiB plugin event limit".into());
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

pub(super) unsafe fn apply_property(state: &mut Playback, property: &Property) {
    // SAFETY: caller processes the event before asking mpv for another one. Format
    // tags are checked before dereferencing; unavailable properties remain unknown.
    let name = unsafe { CStr::from_ptr(property.name) }.to_bytes();
    if property.data.is_null() {
        if let Some(option) = riina_core::video_controls::Equalizer::ALL
            .into_iter()
            .find(|option| option.property().as_bytes() == name)
        {
            state.video_equalizer[option.index()] = None;
        }
        match name {
            b"time-pos" => state.position = None,
            b"duration" => state.duration = None,
            b"mpv-version" => state.mpv_version = None,
            b"ffmpeg-version" => state.ffmpeg_version = None,
            b"ignore-path-in-watch-later-config" => state.ignore_path_in_watch_later_config = None,
            b"save-position-on-quit" => state.save_position_on_quit = None,
            b"track-list" => state.tracks.clear(),
            b"chapter-list" => state.chapters.clear(),
            b"playlist" => state.playlist.clear(),
            b"metadata" => {
                state.artist = None;
                state.album = None;
            }
            b"audio-device-list" => state.audio_devices.clear(),
            b"sub-text" => state.subtitle_text = None,
            b"secondary-sub-text" => state.secondary_subtitle_text = None,
            b"secondary-sub-visibility" => state.secondary_subtitle_visible = None,
            b"secondary-sub-delay" => state.secondary_subtitle_delay = None,
            b"secondary-sub-pos" => state.secondary_subtitle_position = None,
            b"sub-scale" => state.subtitle_scale = None,
            b"sub-font-size" => state.subtitle_font_size = None,
            b"sub-font" => state.subtitle_font = None,
            b"sub-outline-size" => state.subtitle_outline_size = None,
            b"sub-color" => state.subtitle_text_color = None,
            b"sub-outline-color" => state.subtitle_border_color = None,
            b"sub-back-color" => state.subtitle_shadow_color = None,
            b"sub-bold" => state.subtitle_bold = None,
            b"sub-italic" => state.subtitle_italic = None,
            b"sub-blur" => state.subtitle_blur = None,
            b"sub-spacing" => state.subtitle_spacing = None,
            b"sub-shadow-offset" => state.subtitle_shadow_offset = None,
            b"sub-pos" => state.subtitle_position = None,
            b"video-params" => state.video = None,
            b"video-target-params" => state.video_target = None,
            b"video-out-params" => state.video_output = None,
            b"vf" => state.video_filters = None,
            b"af" => {
                state.audio_equalizer = None;
                state.audio_filters = None;
            }
            b"video-aspect-override" => state.video_aspect_override = None,
            b"video-rotate" => state.video_rotation = None,
            b"deinterlace" => state.deinterlace = None,
            b"deinterlace-active" => state.deinterlace_active = None,
            b"video-frame-info" => state.video_frame_info = None,
            b"hwdec-current" => state.hardware_decoder = None,
            b"hwdec" => state.hardware_decoding = None,
            b"current-ao" => state.audio_output = None,
            b"audio-device" => state.audio_device = None,
            b"loop-file" => state.loop_file = None,
            b"loop-playlist" => state.loop_playlist = None,
            b"ab-loop-a" => state.ab_loop_a = None,
            b"ab-loop-b" => state.ab_loop_b = None,
            _ => {}
        }
        return;
    }
    if property.format == 6 {
        let node = unsafe { &*property.data.cast::<ffi::Node>() };
        let result = unsafe { node.copy_json() }.and_then(|value| {
            match name {
                b"ab-loop-a" | b"ab-loop-b" => {
                    // Native values are either a numeric timestamp or "no".
                    // Unknown/unavailable points must not become a zero marker.
                    let point = value
                        .as_f64()
                        .filter(|point| point.is_finite() && *point >= 0.0);
                    if name == b"ab-loop-a" {
                        state.ab_loop_a = point;
                    } else {
                        state.ab_loop_b = point;
                    }
                }
                b"track-list" => {
                    state.tracks = serde_json::from_value(value).map_err(|e| e.to_string())?;
                    state.track_list_observed = true;
                }
                b"vf" => {
                    state.video_filters =
                        Some(serde_json::from_value(value).map_err(|e| e.to_string())?);
                }
                b"af" => {
                    let raw_filters: Vec<serde_json::Value> =
                        serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
                    let filters: Vec<riina_core::crop::VideoFilter> =
                        serde_json::from_value(value).map_err(|e| e.to_string())?;
                    state.audio_equalizer =
                        riina_core::audio_equalizer::observed_gains(&raw_filters);
                    state.audio_filters = Some(filters);
                }
                b"chapter-list" => {
                    state.chapters = serde_json::from_value(value).map_err(|e| e.to_string())?
                }
                b"playlist" => {
                    state.playlist = serde_json::from_value(value).map_err(|e| e.to_string())?
                }
                b"audio-device-list" => state.audio_devices = decode_audio_devices(value)?,
                b"metadata" => {
                    state.artist = metadata_value(&value, "artist");
                    state.album = metadata_value(&value, "album");
                }
                b"video-frame-info" => {
                    state.video_frame_info =
                        if value.is_null() || value.as_object().is_some_and(|map| map.is_empty()) {
                            None
                        } else {
                            Some(serde_json::from_value(value).map_err(|e| e.to_string())?)
                        };
                }
                b"video-params" | b"video-target-params" | b"video-out-params" => {
                    // The native VO target describes presentation, while source
                    // parameters can still carry a PQ/HLG transfer function.
                    let destination = if name == b"video-params" {
                        &mut state.video
                    } else if name == b"video-target-params" {
                        &mut state.video_target
                    } else {
                        &mut state.video_output
                    };
                    *destination =
                        if value.is_null() || value.as_object().is_some_and(|map| map.is_empty()) {
                            None
                        } else {
                            Some(serde_json::from_value(value).map_err(|e| e.to_string())?)
                        };
                }
                _ => {}
            }
            Ok(())
        });
        if let Err(error) = result {
            state.error = Some(format!("Cannot decode mpv property: {error}"));
        }
    } else if property.format == 4 {
        let value = unsafe { *property.data.cast::<i64>() };
        if name == b"video-rotate" {
            state.video_rotation = Some(value);
        }
        if let Some(option) = riina_core::video_controls::Equalizer::ALL
            .into_iter()
            .find(|option| option.property().as_bytes() == name)
        {
            // Unavailable or invalid native values must not masquerade as a
            // neutral setting in controls that would overwrite an unknown state.
            state.video_equalizer[option.index()] = (-100..=100).contains(&value).then_some(value);
        }
    } else if property.format == 5 {
        let value = unsafe { *property.data.cast::<f64>() };
        if !value.is_finite() {
            match name {
                b"time-pos" => state.position = None,
                b"duration" => state.duration = None,
                b"video-aspect-override" => state.video_aspect_override = None,
                b"sub-scale" => state.subtitle_scale = None,
                b"sub-font-size" => state.subtitle_font_size = None,
                b"sub-outline-size" => state.subtitle_outline_size = None,
                b"sub-blur" => state.subtitle_blur = None,
                b"sub-spacing" => state.subtitle_spacing = None,
                b"sub-shadow-offset" => state.subtitle_shadow_offset = None,
                b"sub-pos" => state.subtitle_position = None,
                b"secondary-sub-delay" => state.secondary_subtitle_delay = None,
                b"secondary-sub-pos" => state.secondary_subtitle_position = None,
                _ => {}
            }
            return;
        }
        match name {
            b"time-pos" => state.position = (value >= 0.0).then_some(value),
            b"duration" => state.duration = (value >= 0.0).then_some(value),
            b"volume" => state.volume = value,
            b"speed" => state.speed = value,
            b"video-aspect-override" => state.video_aspect_override = Some(value),
            b"audio-delay" => state.audio_delay = value,
            b"sub-delay" => state.subtitle_delay = value,
            b"secondary-sub-delay" => state.secondary_subtitle_delay = Some(value),
            b"secondary-sub-pos" => {
                state.secondary_subtitle_position = (0.0..=150.0).contains(&value).then_some(value)
            }
            b"sub-scale" => state.subtitle_scale = (0.0..=100.0).contains(&value).then_some(value),
            b"sub-font-size" => state.subtitle_font_size = (value > 0.0).then_some(value),
            b"sub-outline-size" => state.subtitle_outline_size = (value >= 0.0).then_some(value),
            b"sub-blur" => state.subtitle_blur = (0.0..=20.0).contains(&value).then_some(value),
            b"sub-spacing" => state.subtitle_spacing = Some(value),
            b"sub-shadow-offset" => state.subtitle_shadow_offset = (value >= 0.0).then_some(value),
            b"sub-pos" => state.subtitle_position = (0.0..=150.0).contains(&value).then_some(value),
            _ => {}
        }
    } else if property.format == 3 {
        let flag = unsafe { *property.data.cast::<i32>() } != 0;
        match name {
            b"pause" => state.paused = flag,
            b"ignore-path-in-watch-later-config" => {
                state.ignore_path_in_watch_later_config = Some(flag)
            }
            b"save-position-on-quit" => state.save_position_on_quit = Some(flag),
            b"paused-for-cache" => state.buffering = flag,
            b"seeking" => state.seeking = flag,
            b"seekable" => state.seekable = flag,
            b"mute" => state.muted = flag,
            b"sub-visibility" => state.subtitle_visible = flag,
            b"secondary-sub-visibility" => state.secondary_subtitle_visible = Some(flag),
            b"sub-bold" => state.subtitle_bold = Some(flag),
            b"sub-italic" => state.subtitle_italic = Some(flag),
            b"deinterlace-active" => state.deinterlace_active = Some(flag),
            b"eof-reached" => {
                if flag {
                    state.phase = Phase::Ended;
                } else if state.phase == Phase::Ended {
                    state.phase = Phase::Playing;
                }
            }
            _ => {}
        }
    } else if property.format == 1 {
        let text = unsafe { *property.data.cast::<*const std::ffi::c_char>() };
        if !text.is_null() {
            let value = unsafe { CStr::from_ptr(text) }
                .to_string_lossy()
                .into_owned();
            match name {
                b"media-title" => state.title = value,
                b"mpv-version" => state.mpv_version = Some(value),
                b"ffmpeg-version" => state.ffmpeg_version = Some(value),
                b"current-ao" => state.audio_output = Some(value),
                b"audio-device" => state.audio_device = Some(value),
                b"hwdec-current" => state.hardware_decoder = Some(value),
                b"hwdec" => state.hardware_decoding = Some(value),
                b"deinterlace" => {
                    state.deinterlace = match value.as_str() {
                        "yes" => Some(true),
                        "no" => Some(false),
                        _ => None,
                    }
                }
                b"sub-text" => state.subtitle_text = Some(value),
                b"sub-font" => {
                    state.subtitle_font =
                        riina_core::subtitle_controls::valid_font_name(&value).then_some(value);
                }
                b"sub-color" => {
                    state.subtitle_text_color =
                        riina_core::subtitle_color::SubtitleColor::from_mpv(&value)
                }
                b"sub-outline-color" => {
                    state.subtitle_border_color =
                        riina_core::subtitle_color::SubtitleColor::from_mpv(&value)
                }
                b"sub-back-color" => {
                    state.subtitle_shadow_color =
                        riina_core::subtitle_color::SubtitleColor::from_mpv(&value)
                }
                b"secondary-sub-text" => state.secondary_subtitle_text = Some(value),
                b"loop-file" => state.loop_file = Some(value),
                b"loop-playlist" => state.loop_playlist = Some(value),
                _ => {}
            }
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
            format: 1,
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
            format: 1,
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
            format: 3,
            data: (&mut flag as *mut i32).cast(),
        };
        let mut playback = Playback::default();
        // SAFETY: the flag remains valid for the complete property copy.
        unsafe { apply_property(&mut playback, &property) };
        assert_eq!(playback.ignore_path_in_watch_later_config, Some(true));

        let unavailable = Property {
            name: name.as_ptr(),
            format: 3,
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
            format: 3,
            data: (&mut flag as *mut i32).cast(),
        };
        let mut playback = Playback::default();
        // SAFETY: the flag remains valid for the complete property copy.
        unsafe { apply_property(&mut playback, &property) };
        assert_eq!(playback.save_position_on_quit, Some(true));

        let unavailable = Property {
            name: name.as_ptr(),
            format: 3,
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
            format: 4,
        };
        let property = Property {
            name: name.as_ptr(),
            format: 6,
            data: (&mut node as *mut ffi::Node).cast(),
        };
        // SAFETY: the node and name remain valid while the helper copies them.
        assert_eq!(
            unsafe { observed_property_value(&property) }.unwrap(),
            serde_json::json!(42)
        );
    }

    #[test]
    fn unavailable_plugin_observed_properties_become_json_null() {
        let name = CString::new("cache-buffering-state").unwrap();
        let property = Property {
            name: name.as_ptr(),
            format: 6,
            data: std::ptr::null_mut(),
        };
        // SAFETY: null property data is explicitly represented as JSON null.
        assert_eq!(
            unsafe { observed_property_value(&property) }.unwrap(),
            serde_json::Value::Null
        );
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
                    format: 5,
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
                assert_eq!(state.error, None);
            }
        }
    }
}
