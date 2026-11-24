//! Project one inspected snapshot into the current tab.
use super::{properties, track};
use crate::{platform, ui_models::set_rows_if_changed};
use riina_core::{Playback, Track, time_label};
use riina_ui::{InspectorField, InspectorTab, MediaInspectorWindow};

fn field(label: &str, value: Option<String>, missing: &str) -> InspectorField {
    InspectorField {
        label: label.into(),
        web_link: value.as_deref().is_some_and(platform::is_http_url),
        available: value.is_some(),
        value: value.unwrap_or_else(|| missing.into()).into(),
        heading: false,
    }
}

fn heading(label: &str) -> InspectorField {
    InspectorField {
        label: label.into(),
        heading: true,
        ..InspectorField::default()
    }
}

pub(super) fn render(
    window: &MediaInspectorWindow,
    playback: &Playback,
    selected: Option<(usize, &Track)>,
    watches: &[String],
    values: &properties::Values,
) {
    let get = |property: &str| values.get(property).and_then(|value| value.clone());
    let fields = |group: &[properties::Field]| {
        group
            .iter()
            .map(|item| {
                let mut value = get(item.property);
                match item.property {
                    "media-title" if value == get("filename") => value = None,
                    "file-size" => value = value.map(|value| format_bytes(&value)),
                    "audio-bitrate" | "video-bitrate" => {
                        value = value.map(|value| format_bitrate(&value))
                    }
                    "audio-params/samplerate" => {
                        value = value.map(|value| numeric_suffix(&value, "Hz"))
                    }
                    "container-fps"
                    | "display-fps"
                    | "estimated-vf-fps"
                    | "estimated-display-fps" => {
                        value = value.map(|value| numeric_suffix(&value, "fps"));
                    }
                    _ => {}
                }
                field(item.label, value, "N/A")
            })
            .collect::<Vec<_>>()
    };

    match window.get_selected_tab() {
        InspectorTab::Tracks => {
            let rows = selected.map_or_else(
                || vec![heading("No tracks available")],
                |(index, track)| {
                    let mut rows = track::details(track)
                        .into_iter()
                        .map(|(label, value)| field(label, value, "N/A"))
                        .collect::<Vec<_>>();
                    for (label, key) in [("Source ID", "src-id"), ("Decoder", "decoder-desc")] {
                        rows.push(field(
                            label,
                            get(&format!("track-list/{index}/{key}")),
                            "N/A",
                        ));
                    }
                    rows
                },
            );
            set_rows_if_changed(&window.get_track_fields(), rows, |model| {
                window.set_track_fields(model)
            });
        }
        InspectorTab::File => {
            set_rows_if_changed(
                &window.get_file_fields(),
                fields(properties::FILE),
                |model| window.set_file_fields(model),
            );
        }
        InspectorTab::Status => {
            set_rows_if_changed(
                &window.get_status_fields(),
                fields(properties::STATUS),
                |model| window.set_status_fields(model),
            );
            let rows = watches
                .iter()
                .map(|key| field(key, get(key), "Unavailable"))
                .collect();
            set_rows_if_changed(&window.get_watches(), rows, |model| {
                window.set_watches(model)
            });
        }
        InspectorTab::General => {
            let mut rows = vec![field(
                "Duration",
                playback.duration.map(|seconds| time_label(Some(seconds))),
                "N/A",
            )];
            rows.push(heading("Video"));
            rows.push(field(
                "Dimensions",
                playback
                    .video
                    .as_ref()
                    .map(|video| format!("{} × {}", video.w, video.h)),
                "N/A",
            ));
            if let Some(output) = &playback.video_output {
                let color = output
                    .colormatrix
                    .iter()
                    .chain(&output.primaries)
                    .chain(&output.gamma)
                    .map(String::as_str)
                    .collect::<Vec<_>>();
                if !color.is_empty() {
                    rows.push(field("Output colorspace", Some(color.join(" / ")), "N/A"));
                }
            }
            rows.extend(fields(properties::VIDEO));
            rows.push(heading("Audio"));
            rows.extend(fields(properties::AUDIO));
            set_rows_if_changed(&window.get_general_fields(), rows, |model| {
                window.set_general_fields(model)
            });
        }
    }
}

fn format_bytes(value: &str) -> String {
    let Ok(bytes) = value.parse::<u64>() else {
        return value.into();
    };
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn format_bitrate(value: &str) -> String {
    let Ok(bits) = value.parse::<f64>() else {
        return value.into();
    };
    if bits < 1000.0 {
        format!("{bits:.0} bps")
    } else if bits < 1_000_000.0 {
        format!("{:.1} kbps", bits / 1000.0)
    } else {
        format!("{:.1} Mbps", bits / 1_000_000.0)
    }
}

fn numeric_suffix(value: &str, unit: &str) -> String {
    if value.parse::<f64>().is_ok() {
        format!("{value} {unit}")
    } else {
        value.into()
    }
}
