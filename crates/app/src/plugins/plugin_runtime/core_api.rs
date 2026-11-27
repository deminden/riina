//! Typed playback and window API bindings.
use super::{AvailablePermissions, HostEvent, UiEffect, output::record};
use crate::plugin_files::{PluginFiles, WindowModes};
use riina_core::{Command, control_preferences::SeekType, plugin::WindowAction};
use rquickjs::{Ctx, Object, function::Func};
use std::{
    cell::Cell,
    sync::{Arc, Mutex},
};

thread_local! {
    // Each realm has its own worker. The callback and its microtasks read the
    // modes at the confirmed transition even if the host has since moved on.
    pub(super) static EVENT_WINDOW_MODES: Cell<Option<WindowModes>> = const { Cell::new(None) };
}

pub(super) fn install_plugin_core_api<'js>(
    ctx: Ctx<'js>,
    iina_api: Object<'js>,
    events: Arc<Mutex<Vec<HostEvent>>>,
    files: PluginFiles,
    permissions: AvailablePermissions,
    allowed_domains: Vec<String>,
) -> rquickjs::Result<()> {
    let core_api = Object::new(ctx.clone())?;
    install_transport(&core_api, &events, &files, permissions, &allowed_domains)?;
    install_status_readers(&core_api, &files)?;
    install_track_actions(&core_api, &events, &files, permissions, &allowed_domains)?;
    install_catalog_readers(&core_api, &files)?;
    install_window_actions(&core_api, &events, &files)?;
    iina_api.set("core", core_api)?;
    ctx.eval::<(), _>(include_str!("core_api.js"))?;
    Ok(())
}

fn install_transport(
    core_api: &Object<'_>,
    events: &Arc<Mutex<Vec<HostEvent>>>,
    files: &PluginFiles,
    permissions: AvailablePermissions,
    allowed_domains: &[String],
) -> rquickjs::Result<()> {
    let open_events = events.clone();
    let open_files = files.clone();
    let open_domains = allowed_domains.to_vec();
    core_api.set(
        "open",
        Func::from(move |value: String| -> rquickjs::Result<()> {
            let path = plugin_media_reference(
                &value,
                &open_files,
                permissions.file_system,
                permissions.network_request,
                &open_domains,
            )
            .map_err(core_js_error)?;
            record(&open_events, UiEffect::CoreCommand(Command::Load(path)));
            Ok(())
        }),
    )?;
    let osd_events = events.clone();
    let osd_permitted = permissions.show_osd;
    core_api.set(
        "osd",
        Func::from(move |message: String| {
            if osd_permitted {
                record(&osd_events, UiEffect::ShowOsd(message));
            }
        }),
    )?;
    for (name, command) in [
        ("pause", Command::SetPaused(true)),
        ("resume", Command::SetPaused(false)),
        ("stop", Command::Stop(None)),
    ] {
        let command_events = events.clone();
        core_api.set(
            name,
            Func::from(move || record(&command_events, UiEffect::CoreCommand(command.clone()))),
        )?;
    }
    let seek_events = events.clone();
    core_api.set(
        "seek",
        Func::from(move |seconds: f64, exact: bool| -> rquickjs::Result<()> {
            if !seconds.is_finite() {
                return Err(core_js_error("Seek must be a finite number".into()));
            }
            let seek_type = if exact {
                SeekType::Exact
            } else {
                SeekType::Keyframe
            };
            record(
                &seek_events,
                UiEffect::CoreCommand(Command::SeekWithType { seconds, seek_type }),
            );
            Ok(())
        }),
    )?;
    let seek_events = events.clone();
    core_api.set(
        "seekTo",
        Func::from(move |seconds: f64| -> rquickjs::Result<()> {
            if !seconds.is_finite() || seconds < 0.0 {
                return Err(core_js_error(
                    "Absolute seek must be a finite non-negative number".into(),
                ));
            }
            record(
                &seek_events,
                UiEffect::CoreCommand(Command::Seek {
                    seconds,
                    absolute: true,
                }),
            );
            Ok(())
        }),
    )?;
    let speed_events = events.clone();
    core_api.set(
        "setSpeed",
        Func::from(move |speed: f64| -> rquickjs::Result<()> {
            if !speed.is_finite() || speed < 0.01 {
                return Err(core_js_error(
                    "Speed must be finite and at least 0.01".into(),
                ));
            }
            record(&speed_events, UiEffect::CoreCommand(Command::Speed(speed)));
            Ok(())
        }),
    )?;
    let ui_events = events.clone();
    core_api.set(
        "setUIVisibility",
        Func::from(move |visible: bool| {
            record(&ui_events, UiEffect::SetUiVisibility(visible));
        }),
    )?;
    let chapter_events = events.clone();
    core_api.set(
        "playChapter",
        Func::from(move |index: i32| -> rquickjs::Result<()> {
            let index = usize::try_from(index)
                .map_err(|_| core_js_error("Chapter index must be non-negative".into()))?;
            record(
                &chapter_events,
                UiEffect::CoreCommand(Command::SelectChapter(index)),
            );
            Ok(())
        }),
    )?;
    Ok(())
}

fn install_status_readers(core_api: &Object<'_>, files: &PluginFiles) -> rquickjs::Result<()> {
    for name in ["__readStatus", "__readCoreProperty"] {
        let files = files.clone();
        core_api.set(
            name,
            Func::from(move |name: String| -> rquickjs::Result<Option<String>> {
                files
                    .core_status_property(&name)
                    .map(|value| value.map(|value| value.to_string()))
                    .map_err(core_js_error)
            }),
        )?;
    }
    Ok(())
}

fn install_track_actions(
    core_api: &Object<'_>,
    events: &Arc<Mutex<Vec<HostEvent>>>,
    files: &PluginFiles,
    permissions: AvailablePermissions,
    allowed_domains: &[String],
) -> rquickjs::Result<()> {
    let track_write_events = events.clone();
    core_api.set(
        "__writeCoreProperty",
        Func::from(
            move |name: String, serialized: String| -> rquickjs::Result<()> {
                let value: serde_json::Value = serde_json::from_str(&serialized)
                    .map_err(|_| core_js_error("Track property value is not valid JSON".into()))?;
                let command = match (name.as_str(), value) {
                    ("audio.id", serde_json::Value::Null) => None,
                    ("audio.id", value) => Some(Command::SelectTrack {
                        kind: riina_core::TrackKind::Audio,
                        id: json_track_id(value)?,
                    }),
                    ("audio.delay", value) => Some(Command::AudioDelay(json_finite_number(
                        value,
                        "Audio delay",
                    )?)),
                    ("audio.volume", value) => {
                        let volume = json_finite_number(value, "Volume")?;
                        if !(0.0..=200.0).contains(&volume) {
                            return Err(core_js_error("Volume must be between 0 and 200".into()));
                        }
                        Some(Command::Volume(volume))
                    }
                    ("audio.muted", serde_json::Value::Bool(muted)) => {
                        Some(Command::SetMuted(muted))
                    }
                    ("audio.muted", _) => {
                        return Err(core_js_error("Muted must be a boolean".into()));
                    }
                    ("subtitle.id", serde_json::Value::Null) => None,
                    ("subtitle.id", value) => Some(Command::SelectTrack {
                        kind: riina_core::TrackKind::Sub,
                        id: json_track_id(value)?,
                    }),
                    ("subtitle.secondID", serde_json::Value::Null) => None,
                    ("subtitle.secondID", value) => Some(Command::SelectSecondarySubtitle(Some(
                        json_track_id(value)?.ok_or_else(|| {
                            core_js_error("Subtitle track id must be an integer".into())
                        })?,
                    ))),
                    ("subtitle.delay", value) => Some(Command::SubtitleDelay(json_finite_number(
                        value,
                        "Subtitle delay",
                    )?)),
                    ("video.id", serde_json::Value::Null) => None,
                    ("video.id", value) => Some(Command::SelectTrack {
                        kind: riina_core::TrackKind::Video,
                        id: json_track_id(value)?,
                    }),
                    _ => None,
                };
                if let Some(command) = command {
                    record(&track_write_events, UiEffect::CoreCommand(command));
                }
                Ok(())
            },
        ),
    )?;
    let track_load_events = events.clone();
    let track_load_files = files.clone();
    let track_load_domains = allowed_domains.to_vec();
    core_api.set(
        "__loadTrack",
        Func::from(move |kind: String, value: String| -> rquickjs::Result<()> {
            if value.len() > 16 * 1024 || value.contains('\0') {
                return Err(core_js_error("Track media reference is invalid".into()));
            }
            let path = plugin_media_reference(
                &value,
                &track_load_files,
                permissions.file_system,
                permissions.network_request,
                &track_load_domains,
            )
            .map_err(core_js_error)?;
            let command = match kind.as_str() {
                "audio" => Command::AddAudio(path),
                "subtitle" => Command::AddSubtitle(path),
                "video" => Command::AddVideo(path),
                _ => return Err(core_js_error("Unknown track type".into())),
            };
            record(&track_load_events, UiEffect::CoreCommand(command));
            Ok(())
        }),
    )?;
    Ok(())
}

fn install_catalog_readers(core_api: &Object<'_>, files: &PluginFiles) -> rquickjs::Result<()> {
    install_catalog_reader(
        core_api,
        "__readChapters",
        files,
        PluginFiles::core_chapters,
    )?;
    install_catalog_reader(
        core_api,
        "__readRecentDocuments",
        files,
        PluginFiles::core_recent_documents,
    )?;
    install_catalog_reader(
        core_api,
        "__readHistory",
        files,
        PluginFiles::core_playback_history,
    )?;
    install_catalog_reader(core_api, "__readVersion", files, PluginFiles::core_version)?;
    Ok(())
}

fn install_catalog_reader<T: serde::Serialize + 'static>(
    core_api: &Object<'_>,
    name: &'static str,
    files: &PluginFiles,
    read: fn(&PluginFiles) -> Result<T, String>,
) -> rquickjs::Result<()> {
    let files = files.clone();
    core_api.set(
        name,
        Func::from(move || -> rquickjs::Result<String> {
            read(&files)
                .and_then(|value| serde_json::to_string(&value).map_err(|error| error.to_string()))
                .map_err(core_js_error)
        }),
    )
}

fn install_window_actions(
    core_api: &Object<'_>,
    events: &Arc<Mutex<Vec<HostEvent>>>,
    files: &PluginFiles,
) -> rquickjs::Result<()> {
    let window_files = files.clone();
    core_api.set(
        "__readWindowProperty",
        Func::from(move |name: String| -> rquickjs::Result<Option<String>> {
            let event_value = EVENT_WINDOW_MODES
                .get()
                .and_then(|modes| modes.property(&name));
            event_value
                .map_or_else(
                    || window_files.core_window_property(&name),
                    |value| Ok(Some(value)),
                )
                .and_then(|value| {
                    value
                        .map(|value| {
                            serde_json::to_string(&value).map_err(|error| error.to_string())
                        })
                        .transpose()
                })
                .map_err(core_js_error)
        }),
    )?;
    let window_events = events.clone();
    let window_files = files.clone();
    core_api.set(
        "__writeWindowProperty",
        Func::from(
            move |name: String, serialized: String| -> rquickjs::Result<bool> {
                if !window_files.player_window_loaded().map_err(core_js_error)? {
                    return Ok(true);
                }
                let value: serde_json::Value = serde_json::from_str(&serialized)
                    .map_err(|_| core_js_error("Window property value is not valid JSON".into()))?;
                let Some(action) = plugin_window_action(&name, value)? else {
                    return Ok(false);
                };
                record(&window_events, UiEffect::WindowAction(action));
                Ok(true)
            },
        ),
    )?;
    Ok(())
}

fn json_track_id(value: serde_json::Value) -> rquickjs::Result<Option<i64>> {
    value
        .as_i64()
        .filter(|id| *id > 0)
        .map(Some)
        .ok_or_else(|| core_js_error("Track id must be a positive integer".into()))
}

fn json_finite_number(value: serde_json::Value, label: &str) -> rquickjs::Result<f64> {
    value
        .as_f64()
        .filter(|number| number.is_finite())
        .ok_or_else(|| core_js_error(format!("{label} must be a finite number")))
}

fn plugin_window_action(
    name: &str,
    value: serde_json::Value,
) -> rquickjs::Result<Option<WindowAction>> {
    let invalid = |label: &str| core_js_error(format!("Window {label} has an invalid value"));
    let action = match name {
        "frame" => {
            let object = value.as_object().ok_or_else(|| invalid("frame"))?;
            let number = |name: &str| {
                object
                    .get(name)
                    .cloned()
                    .ok_or_else(|| invalid("frame"))
                    .and_then(|value| json_finite_number(value, "frame"))
            };
            let (x, y, width, height) = (
                number("x")?,
                number("y")?,
                number("width")?,
                number("height")?,
            );
            if width <= 0.0 || height <= 0.0 {
                return Err(invalid("frame"));
            }
            WindowAction::SetFrame {
                x,
                y,
                width,
                height,
            }
        }
        "fullscreen" => {
            WindowAction::SetFullscreen(value.as_bool().ok_or_else(|| invalid("fullscreen"))?)
        }
        "pip" => WindowAction::SetPictureInPicture(value.as_bool().ok_or_else(|| invalid("pip"))?),
        "ontop" => WindowAction::SetOnTop(value.as_bool().ok_or_else(|| invalid("ontop"))?),
        "sidebar" => {
            let name = match value {
                serde_json::Value::String(name) if name.len() <= 128 && !name.contains('\0') => {
                    Some(name)
                }
                serde_json::Value::String(_) => return Err(invalid("sidebar")),
                _ => None,
            };
            WindowAction::SetSidebar(name)
        }
        "miniaturized" => {
            WindowAction::SetMiniaturized(value.as_bool().ok_or_else(|| invalid("miniaturized"))?)
        }
        _ => return Ok(None),
    };
    Ok(Some(action))
}

pub(super) fn plugin_media_reference(
    value: &str,
    files: &PluginFiles,
    file_system_granted: bool,
    network_granted: bool,
    allowed_domains: &[String],
) -> Result<String, String> {
    if value.len() > 16 * 1024 || value.contains('\0') {
        return Err("Media reference is invalid".into());
    }
    if riina_core::media_reference::is_network_url_candidate(value) {
        riina_core::media_reference::validate_network_url(value)
            .map_err(|error| error.to_string())?;
        if !network_granted || !plugin_media_url_allowed(value, allowed_domains) {
            return Err("Network permission and an allowed domain are required".into());
        }
        return Ok(value.to_owned());
    }
    if let Ok(url) = url::Url::parse(value) {
        match url.scheme() {
            "file" => {
                let path = url
                    .to_file_path()
                    .map_err(|_| "File URL is invalid".to_owned())?;
                return files
                    .existing_path(&path.to_string_lossy(), file_system_granted)
                    .map(|path| path.to_string_lossy().into_owned());
            }
            "data" | "memory" => return Ok(value.to_owned()),
            _ if !file_system_granted => {
                return Err("The file-system permission is required for this URL scheme".into());
            }
            _ => return Ok(value.to_owned()),
        }
    }
    files
        .existing_path(value, file_system_granted)
        .map(|path| path.to_string_lossy().into_owned())
}

fn plugin_media_url_allowed(value: &str, allowed_domains: &[String]) -> bool {
    if riina_core::media_reference::validate_network_url(value).is_err() {
        return false;
    }
    let Ok(outer_url) = url::Url::parse(value) else {
        return false;
    };
    let url = if outer_url.scheme() == "ytdl" {
        let Some(source) = value.strip_prefix("ytdl://") else {
            return false;
        };
        let Ok(source_url) = url::Url::parse(source) else {
            return false;
        };
        if riina_core::media_reference::validate_network_url(source).is_err() {
            return false;
        }
        source_url
    } else {
        outer_url
    };
    if !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    let Some(host) = url.host_str().map(str::to_ascii_lowercase) else {
        return false;
    };
    allowed_domains.iter().any(|pattern| {
        if pattern == "*" {
            return true;
        }
        if let Some(domain) = pattern.strip_prefix("*.") {
            let domain = domain.to_ascii_lowercase();
            host.len() > domain.len()
                && host
                    .strip_suffix(&domain)
                    .is_some_and(|prefix| prefix.ends_with('.'))
        } else {
            host == pattern.to_ascii_lowercase()
        }
    })
}

fn core_js_error(message: String) -> rquickjs::Error {
    rquickjs::Error::new_from_js_message("iina.core", "a valid permitted player command", message)
}
