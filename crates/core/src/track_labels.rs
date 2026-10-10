//! Stable demux metadata labels, independent of decoded/display transformations.

pub(crate) fn number(value: f64) -> String {
    // Labels round to three decimal places with ties away from zero. Converting
    // the rounded number rather than fixed-width text removes trailing zeroes.
    let rounded = (value * 1000.0).round() / 1000.0;
    if rounded == 0.0 {
        "0".into()
    } else {
        rounded.to_string()
    }
}

#[cfg(test)]
mod tests {
    use crate::Track;
    use serde_json::json;

    #[test]
    fn video_labels_use_demux_dimensions_and_rounded_frame_rate() {
        for (fps, expected) in [
            (24.0, "24"),
            (24000.0 / 1001.0, "23.976"),
            (29.9701, "29.97"),
            (0.0625, "0.063"),
            (0.0, "0"),
        ] {
            let track: Track = serde_json::from_value(json!({
                "id": 1, "type": "video", "title": "Main", "codec": "h264",
                "demux-w": 1920, "demux-h": 1080, "demux-fps": fps, "default": true,
            }))
            .unwrap();
            assert_eq!(
                track.display_name(),
                format!("Main (h264, 1920×1080, {expected}fps, Default)")
            );
        }
    }

    #[test]
    fn track_labels_omit_missing_metadata_without_adding_blank_title_spacing() {
        for (metadata, expected) in [
            (json!({"id": 1, "type": "video"}), "No Title"),
            (
                json!({"id": 1, "type": "video", "demux-w": 1920}),
                "No Title",
            ),
            (
                json!({"id": 1, "type": "video", "demux-fps": 25.0}),
                "No Title (25fps)",
            ),
            (
                json!({"id": 1, "type": "video", "title": "", "codec": "vp9"}),
                "(vp9)",
            ),
            (
                json!({"id": 1, "type": "audio", "title": "Sound", "codec": "aac", "demux-channel-count": 6, "demux-samplerate": 44100, "demux-w": 1920, "demux-h": 1080}),
                "Sound (aac, 6ch, 44.1kHz)",
            ),
            (
                json!({"id": 1, "type": "sub", "title": "Captions", "codec": "subrip", "lang": "eng", "demux-fps": 24.0}),
                "Captions (subrip)",
            ),
        ] {
            let track: Track = serde_json::from_value(metadata).unwrap();
            assert_eq!(track.display_name(), expected);
        }
    }
}
