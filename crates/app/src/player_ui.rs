mod session_preferences;

#[cfg(any(target_os = "windows", target_os = "macos"))]
use crate::LiveTextRegion;
use crate::seek_osd::{self, Change, SeekOsd};
use crate::{
    CatalogRow, MenuCatalogRow, PlayerWindow, PluginMenuRow, RecentMediaRow, WelcomeWindow,
    media_open, platform,
    ui_dialogs::{self, Purpose},
};
use riina_core::{Command, LoopMode, Phase, Playback, Preferences, TrackKind, time_label};
use riina_mpv::{Player, video::VideoSurface};
use slint::{
    ComponentHandle, Model,
    winit_030::{EventResult, WinitWindowAccessor, winit},
};
#[cfg(not(target_os = "macos"))]
use std::path::Path;
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

// Slint 1.18.1 converts this private title sentinel in dynamic menu models to
// an actual MenuSeparator on every backend.
const SLINT_MENU_SEPARATOR_TITLE: &str = "\u{e001}⸺";
const PLUGIN_MENU_GROUP_SEPARATOR_TOKEN: &str = "\u{1f}group-separator";
static NEXT_PLAYER_WINDOW_REGISTRATION: AtomicU64 = AtomicU64::new(1);

const INCOMING_SURFACE_STABLE_FOR: Duration = Duration::from_millis(300);
const MANUAL_TRANSITION_CAPTURE_TIMEOUT: Duration = Duration::from_millis(500);
// A 4 fps clip changes frames every 250 ms; sampling at 10 Hz avoids skipping
// alternating frames while keeping screenshot-raw readback below display rate.
const NATIVE_MATERIAL_SAMPLE_INTERVAL: Duration = Duration::from_millis(100);
// Material-frame replies arrive asynchronously. Poll them at the UI cadence so
// a completed capture does not wait another full sampling interval to display.
const NATIVE_MATERIAL_UPDATE_INTERVAL: Duration = Duration::from_millis(16);
const NATIVE_MATERIAL_SAMPLE_TIMEOUT: Duration = Duration::from_millis(750);
const NATIVE_MATERIAL_CAPTURE_STARTUP_GRACE: Duration = Duration::from_millis(500);
type PendingNavigation = Rc<RefCell<Option<(Vec<Command>, Instant)>>>;

fn set_video_material_sample(app: &PlayerWindow, image: slint::Image, ready: bool) {
    app.set_video_blurred_frame(image);
    app.set_video_material_sample_ready(ready);
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MaterialSampleSource {
    Wgc,
    Mpv,
}

#[cfg(target_os = "windows")]
impl MaterialSampleSource {
    fn label(self) -> &'static str {
        match self {
            Self::Wgc => "wgc",
            Self::Mpv => "mpv",
        }
    }
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MaterialFrameKey {
    generation: u64,
    revision: u64,
    source: MaterialSampleSource,
}

#[cfg(target_os = "windows")]
struct MaterialTransformRequest {
    key: MaterialFrameKey,
    frame: Arc<riina_core::CapturedFrame>,
    captured_at: Option<Instant>,
}

#[cfg(target_os = "windows")]
struct MaterialTransformResult {
    key: MaterialFrameKey,
    captured_at: Option<Instant>,
    transform_time: Duration,
    pixels: Result<crate::video_blur::SampledFramePixels, String>,
}

#[cfg(target_os = "windows")]
fn start_material_transform_worker() -> (
    std::sync::mpsc::SyncSender<MaterialTransformRequest>,
    std::sync::mpsc::Receiver<MaterialTransformResult>,
) {
    let (request_sender, request_receiver) =
        std::sync::mpsc::sync_channel::<MaterialTransformRequest>(1);
    let (result_sender, result_receiver) =
        std::sync::mpsc::sync_channel::<MaterialTransformResult>(1);
    let worker = std::thread::Builder::new()
        .name("riina-material-transform".to_owned())
        .spawn(move || {
            while let Ok(mut request) = request_receiver.recv() {
                // A slow sample should not build a queue of obsolete video
                // frames. Prefer the newest waiting sample before processing.
                while let Ok(newer) = request_receiver.try_recv() {
                    request = newer;
                }
                let started = Instant::now();
                let input_width = request.frame.width;
                let input_height = request.frame.height;
                let input_offset = ((input_height / 2 * input_width + input_width / 2) * 4) as usize;
                let input_center = request
                    .frame
                    .rgba
                    .get(input_offset..input_offset + 3)
                    .map(|rgb| [rgb[0], rgb[1], rgb[2]]);
                let pixels = crate::video_blur::sampled_frame_pixels(request.frame.as_ref());
                if std::env::var_os("RIINA_TRACE_MATERIAL_PIXELS").is_some()
                    && let (Some(input_center), Ok(output)) = (input_center, pixels.as_ref())
                {
                    let output_offset =
                        ((output.height / 2 * output.width + output.width / 2) * 4) as usize;
                    if let Some(rgb) = output.rgba.get(output_offset..output_offset + 3) {
                        eprintln!(
                            "Windows material center source={} input={:02X}{:02X}{:02X} blurred={:02X}{:02X}{:02X}",
                            request.key.source.label(),
                            input_center[0], input_center[1], input_center[2],
                            rgb[0], rgb[1], rgb[2],
                        );
                    }
                }
                let result = MaterialTransformResult {
                    key: request.key,
                    captured_at: request.captured_at,
                    transform_time: started.elapsed(),
                    pixels,
                };
                let _ = result_sender.try_send(result);
            }
        });
    if let Err(error) = worker {
        eprintln!("Could not start the Windows material transform worker: {error}");
    }
    (request_sender, result_receiver)
}

#[cfg(target_os = "windows")]
fn install_windows_material_policy_monitor(app: &PlayerWindow) -> Rc<slint::Timer> {
    let timer = Rc::new(slint::Timer::default());
    let weak = app.as_weak();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(100),
        move || {
            let Some(app) = weak.upgrade() else { return };
            let enabled = crate::platform::native_materials_enabled();
            if let Some(accent) = crate::platform::windows_accent_color()
                && app.get_windows_accent_color() != accent
            {
                app.set_windows_accent_color(accent);
            }
            if let Some(foreground) = crate::platform::windows_accent_foreground()
                && app.get_windows_accent_foreground() != foreground
            {
                app.set_windows_accent_foreground(foreground);
            }
            if app.get_system_materials_enabled() != enabled {
                app.set_system_materials_enabled(enabled);
                if !enabled {
                    set_video_material_sample(&app, slint::Image::default(), false);
                }
            }
        },
    );
    timer
}

#[cfg(target_os = "windows")]
fn install_native_video_material_sampler(
    app: &PlayerWindow,
    sender: riina_mpv::CommandSender,
    latest: Arc<std::sync::Mutex<Playback>>,
    capture: Option<platform::NativeVideoMaterialCapture>,
) -> Rc<slint::Timer> {
    let timer = Rc::new(slint::Timer::default());
    let weak = app.as_weak();
    let last_request = Rc::new(Cell::new(None::<(u64, u64, Instant)>));
    let displayed_frame = Rc::new(Cell::new(None::<MaterialFrameKey>));
    let displayed_at = Rc::new(Cell::new(None::<Instant>));
    let displayed_position = Rc::new(Cell::new(None::<f64>));
    let submitted_frame = Rc::new(Cell::new(None::<MaterialFrameKey>));
    let capture_generation = Rc::new(Cell::new(None::<u64>));
    let capture_generation_started = Rc::new(Cell::new(None::<Instant>));
    let timer_request = last_request.clone();
    let timer_displayed = displayed_frame.clone();
    let timer_displayed_at = displayed_at.clone();
    let timer_displayed_position = displayed_position.clone();
    let timer_submitted = submitted_frame.clone();
    let timer_capture_generation = capture_generation.clone();
    let timer_capture_generation_started = capture_generation_started.clone();
    let timer_capture = capture;
    let (transform_sender, transform_results) = start_material_transform_worker();
    let trace_material_samples = std::env::var_os("RIINA_TRACE_MATERIAL").is_some();
    timer.start(
        slint::TimerMode::Repeated,
        NATIVE_MATERIAL_UPDATE_INTERVAL,
        move || {
            let Some(app) = weak.upgrade() else { return };
            let materials_enabled = app.get_system_materials_enabled();
            let Ok(state) = latest.lock().map(|state| state.clone()) else {
                return;
            };
            let completed_transform = transform_results.try_recv().ok();
            let material_visible = materials_enabled
                && app.get_native_video()
                && !app.get_native_transition_frame()
                && !app.get_crop_editing()
                // The Slint overlay is SDR. Keep the native HDR composition
                // on its existing tint instead of mixing an 8-bit screenshot.
                && !(app.get_hdr_source_detected() && app.get_hdr_enabled())
                && ((app.get_liquid_glass_osc()
                    && app.get_loaded()
                    && (app.get_controls_visible() || app.get_music_mode()))
                    || (app.get_liquid_glass_sidebar()
                        && (app.get_sidebar() != 0 || app.get_leading_sidebar() != 0))
                    || (app.get_liquid_glass_osd()
                        && app.get_loaded()
                        && (app.get_video_osd_visible()
                            || app.get_seek_osd_visible()
                            || !app.get_plugin_osd_message().is_empty()
                            || (app.get_fullscreen()
                                && app.get_controls_visible()
                                && app.get_display_time_and_battery_in_fullscreen()))));
            if timer_capture_generation.get() != Some(state.generation) {
                if let Some(capture) = &timer_capture {
                    capture.clear_latest();
                }
                timer_capture_generation.set(Some(state.generation));
                timer_capture_generation_started.set(Some(Instant::now()));
                timer_displayed.set(None);
                timer_displayed_at.set(None);
                timer_displayed_position.set(None);
                timer_submitted.set(None);
                set_video_material_sample(&app, slint::Image::default(), false);
            }
            let playback_active = state.video.is_some()
                && matches!(state.phase, Phase::Playing | Phase::Paused | Phase::Seeking);
            if !material_visible || !playback_active {
                if let Some(capture) = &timer_capture {
                    capture.set_enabled(false);
                }
                if timer_displayed.take().is_some() {
                    set_video_material_sample(&app, slint::Image::default(), false);
                }
                timer_displayed_at.set(None);
                timer_displayed_position.set(None);
                timer_submitted.set(None);
                timer_request.set(None);
                return;
            }

            if let Some(result) = completed_transform {
                let key = result.key;
                if timer_submitted.get() == Some(key) {
                    timer_submitted.set(None);
                }
                if key.generation == state.generation
                    && timer_displayed.get() != Some(key)
                    && result.captured_at.is_some_and(|captured_at| {
                        captured_at.elapsed() <= NATIVE_MATERIAL_SAMPLE_TIMEOUT
                    })
                {
                    match result.pixels {
                        Ok(pixels) => {
                            let image = crate::video_blur::slint_image_from_sampled_pixels(pixels);
                            set_video_material_sample(&app, image, true);
                            timer_displayed.set(Some(key));
                            timer_displayed_at.set(result.captured_at);
                            timer_displayed_position.set(state.position);
                            if trace_material_samples {
                                eprintln!(
                                    "Windows material sample source={} generation={} revision={} age_ms={} transform_ms={}",
                                    key.source.label(),
                                    key.generation,
                                    key.revision,
                                    result.captured_at.map_or(0, |captured_at| {
                                        captured_at.elapsed().as_millis()
                                    }),
                                    result.transform_time.as_millis(),
                                );
                            }
                        }
                        Err(error) => {
                            eprintln!("Could not prepare the Windows video material frame: {error}");
                        }
                    }
                }
            }

            if let Some(capture) = &timer_capture {
                capture.set_enabled(true);
                let sample = capture.latest().filter(|sample| {
                    sample.captured_at.elapsed() <= NATIVE_MATERIAL_SAMPLE_TIMEOUT
                });
                if let Some(sample) = sample.as_ref() {
                    let key = MaterialFrameKey {
                        generation: state.generation,
                        revision: sample.sequence,
                        source: MaterialSampleSource::Wgc,
                    };
                    if timer_displayed.get() != Some(key)
                        && timer_submitted.get() != Some(key)
                    {
                        let request = MaterialTransformRequest {
                            key,
                            frame: sample.frame.clone(),
                            captured_at: Some(sample.captured_at),
                        };
                        if transform_sender.try_send(request).is_ok() {
                            timer_submitted.set(Some(key));
                        }
                    }
                    timer_request.set(None);
                    return;
                }
                let capture_warmup = timer_capture_generation_started
                    .get()
                    .is_some_and(|started| started.elapsed() < NATIVE_MATERIAL_CAPTURE_STARTUP_GRACE);
                if capture_warmup {
                    return;
                }
            }

            let now = Instant::now();
            let frame_key = MaterialFrameKey {
                generation: state.generation,
                revision: state.material_frame_revision,
                source: MaterialSampleSource::Mpv,
            };
            if state.material_frame_generation == Some(state.generation)
                && state.material_frame_captured_at.is_some_and(|captured_at| {
                    now.saturating_duration_since(captured_at) <= NATIVE_MATERIAL_SAMPLE_TIMEOUT
                })
                && timer_displayed.get() != Some(frame_key)
                && timer_submitted.get() != Some(frame_key)
                && let Some(frame) = state.material_frame.as_ref()
            {
                let request = MaterialTransformRequest {
                    key: frame_key,
                    frame: frame.clone(),
                    captured_at: state.material_frame_captured_at,
                };
                if transform_sender.try_send(request).is_ok() {
                    timer_submitted.set(Some(frame_key));
                }
            }
            // A paused video can stop producing WGC frames; its last sample is
            // still valid while the playhead stays within one 4 fps frame.
            let paused_frame_still_matches = state.paused
                && state.position.is_some_and(|position| {
                    timer_displayed_position
                        .get()
                        .is_some_and(|displayed| (position - displayed).abs() <= 0.25)
                });
            if !paused_frame_still_matches && timer_displayed.get().is_some_and(|key| {
                key.generation == state.generation
                    && timer_displayed_at.get().is_none_or(|captured_at| {
                        now.saturating_duration_since(captured_at)
                            > NATIVE_MATERIAL_SAMPLE_TIMEOUT
                    })
            }) {
                timer_displayed.set(None);
                timer_displayed_at.set(None);
                timer_displayed_position.set(None);
                set_video_material_sample(&app, slint::Image::default(), false);
            }

            let paused_sample_current = state.paused
                && state.material_frame_generation == Some(state.generation)
                && state.material_frame_position == state.position
                && state.material_frame_captured_at.is_some_and(|captured_at| {
                    now.saturating_duration_since(captured_at) <= NATIVE_MATERIAL_SAMPLE_TIMEOUT
                });
            if paused_sample_current {
                timer_request.set(None);
                return;
            }
            let request_due =
                timer_request
                    .get()
                    .is_none_or(|(generation, revision, requested_at)| {
                        let elapsed = now.saturating_duration_since(requested_at);
                        generation != state.generation
                            || (elapsed >= NATIVE_MATERIAL_SAMPLE_INTERVAL
                                && (state.material_frame_revision > revision
                                    || elapsed >= NATIVE_MATERIAL_SAMPLE_TIMEOUT))
                    });
            if request_due {
                let result = sender.send(Command::CaptureMaterialFrame);
                // Also throttle a rejected enqueue so a busy command queue cannot
                // turn this visual-only sampler into a 10 Hz log/command loop.
                timer_request.set(Some((state.generation, state.material_frame_revision, now)));
                if let Err(error) = result {
                    eprintln!("Could not request a Windows video material sample: {error}");
                }
            }
        },
    );
    timer
}

pub(crate) fn help_menu_url(destination: &str) -> Option<&'static str> {
    match destination {
        "help" => Some("https://github.com/deminden/riina#status"),
        "release-highlights" => Some("https://github.com/deminden/riina/releases/latest"),
        "github" => Some("https://github.com/deminden/riina"),
        // The repository README is the project's user-facing homepage.
        "website" => Some("https://github.com/deminden/riina#readme"),
        _ => None,
    }
}

#[cfg(test)]
mod help_menu_tests {
    use super::help_menu_url;

    #[test]
    fn documented_help_destinations_map_to_project_pages() {
        assert_eq!(
            help_menu_url("help"),
            Some("https://github.com/deminden/riina#status")
        );
        assert_eq!(
            help_menu_url("release-highlights"),
            Some("https://github.com/deminden/riina/releases/latest")
        );
        assert_eq!(
            help_menu_url("github"),
            Some("https://github.com/deminden/riina")
        );
        assert_eq!(
            help_menu_url("website"),
            Some("https://github.com/deminden/riina#readme")
        );
        assert_eq!(help_menu_url("unknown"), None);
    }
}

fn queue_playlist_navigation(
    sender: &riina_mpv::CommandSender,
    pending: &PendingNavigation,
    command: Command,
    capture_transition_first: bool,
) {
    if !capture_transition_first {
        if let Err(error) = sender.send(command) {
            eprintln!("Could not navigate the playlist: {error}");
        }
        return;
    }

    let needs_capture = {
        let mut pending = pending.borrow_mut();
        if let Some((commands, _)) = pending.as_mut() {
            commands.push(command);
            false
        } else {
            *pending = Some((vec![command], Instant::now()));
            true
        }
    };
    if needs_capture && let Err(error) = sender.send(Command::CaptureTransitionFrame) {
        let commands = pending.borrow_mut().take().map(|(commands, _)| commands);
        eprintln!("Could not capture the outgoing video frame: {error}");
        if let Some(commands) = commands {
            for command in commands {
                if let Err(error) = sender.send(command) {
                    eprintln!("Could not navigate the playlist: {error}");
                }
            }
        }
    }
}

fn clear_screenshot_preview(app: &PlayerWindow) {
    app.set_screenshot_preview_visible(false);
    app.set_screenshot_preview_path("".into());
    app.set_screenshot_preview_image(slint::Image::default());
}

fn set_audio_equalizer_controls(
    app: &PlayerWindow,
    settings: &riina_core::audio_equalizer::Settings,
) {
    let profile_ids = settings.profile_ids();
    app.set_audio_equalizer_values(slint::ModelRc::new(slint::VecModel::from(
        settings.gains_db.map(|gain| gain as f32).to_vec(),
    )));
    app.set_audio_equalizer_profile_names(slint::ModelRc::new(slint::VecModel::from(
        profile_ids
            .iter()
            .map(|id| match id {
                riina_core::audio_equalizer::ProfileId::Manual => "Manual".to_owned(),
                riina_core::audio_equalizer::ProfileId::Preset(name) => name
                    .replace('_', " ")
                    .split_whitespace()
                    .map(|part| {
                        let mut chars = part.chars();
                        chars
                            .next()
                            .map(|first| first.to_uppercase().to_string() + chars.as_str())
                            .unwrap_or_default()
                    })
                    .collect::<Vec<_>>()
                    .join(" "),
                riina_core::audio_equalizer::ProfileId::User(name) => name.clone(),
            })
            .map(slint::SharedString::from)
            .collect::<Vec<_>>(),
    )));
    app.set_audio_equalizer_profile_index(settings.selected_profile_index().unwrap_or(0) as i32);
    app.set_audio_equalizer_user_profile_start_index(
        profile_ids
            .iter()
            .position(|id| matches!(id, riina_core::audio_equalizer::ProfileId::User(_)))
            .unwrap_or(profile_ids.len()) as i32,
    );
    app.set_audio_equalizer_can_delete_profile(settings.selected_profile_is_user());
    app.set_audio_equalizer_can_edit_profile(settings.selected_profile_is_user());
    app.set_audio_equalizer_can_save_profile(settings.is_manual_selected());
}

fn audio_equalizer_visible_profile_names(app: &PlayerWindow) -> Vec<String> {
    let names = app.get_audio_equalizer_profile_names();
    (1..names.row_count())
        .filter_map(|index| names.row_data(index))
        .map(|name| name.to_string())
        .collect()
}

fn audio_equalizer_name_is_display_duplicate(name: &str, visible_names: &[String]) -> bool {
    visible_names.iter().any(|visible| visible == name)
}

#[cfg(test)]
mod audio_equalizer_name_tests {
    use super::audio_equalizer_name_is_display_duplicate;

    #[test]
    fn duplicate_check_uses_the_visible_preset_and_user_profile_labels() {
        let visible_names = vec!["Flat".into(), "Rock".into(), "My mix".into()];
        assert!(audio_equalizer_name_is_display_duplicate(
            "Rock",
            &visible_names
        ));
        assert!(audio_equalizer_name_is_display_duplicate(
            "My mix",
            &visible_names
        ));
        assert!(!audio_equalizer_name_is_display_duplicate(
            "Manual",
            &visible_names
        ));
        assert!(!audio_equalizer_name_is_display_duplicate(
            "rock",
            &visible_names
        ));
    }
}

fn plugin_sidebar_tab_added(app: &PlayerWindow, identifier: &str, label: &str) {
    let mut identifiers = (0..app.get_plugin_sidebar_identifiers().row_count())
        .filter_map(|row| app.get_plugin_sidebar_identifiers().row_data(row))
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    let mut labels = (0..app.get_plugin_sidebar_labels().row_count())
        .filter_map(|row| app.get_plugin_sidebar_labels().row_data(row))
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    if let Some(index) = identifiers.iter().position(|current| current == identifier) {
        labels[index] = label.to_owned();
    } else {
        identifiers.push(identifier.to_owned());
        labels.push(label.to_owned());
    }
    app.set_plugin_sidebar_identifiers(slint::ModelRc::new(slint::VecModel::from(
        identifiers
            .iter()
            .map(|value| slint::SharedString::from(value.as_str()))
            .collect::<Vec<_>>(),
    )));
    app.set_plugin_sidebar_labels(slint::ModelRc::new(slint::VecModel::from(
        labels
            .iter()
            .map(|value| slint::SharedString::from(value.as_str()))
            .collect::<Vec<_>>(),
    )));
    let selected = app.get_plugin_sidebar_selected_identifier().to_string();
    app.set_plugin_sidebar_selected_index(
        identifiers
            .iter()
            .position(|current| current == &selected)
            .map_or(-1, |index| index as i32),
    );
}

fn plugin_menu_rows_for_plugin(
    identifier: &str,
    global: bool,
    items: &[riina_core::plugin_menu::PluginMenuItem],
) -> Vec<PluginMenuRow> {
    let scope = if global { "global" } else { "player" };
    fn append(
        identifier: &str,
        scope: &str,
        items: &[riina_core::plugin_menu::PluginMenuItem],
        parents: &mut Vec<String>,
        rows: &mut Vec<PluginMenuRow>,
    ) {
        for item in items {
            if item.separator {
                rows.push(PluginMenuRow {
                    token: format!("{identifier}\u{1f}{scope}\u{1f}").into(),
                    label: SLINT_MENU_SEPARATOR_TITLE.into(),
                    enabled: false,
                    selected: false,
                    shortcut: Default::default(),
                    section_header: false,
                });
                continue;
            }
            parents.push(item.title.clone());
            rows.push(PluginMenuRow {
                token: format!("{identifier}\u{1f}{scope}\u{1f}{}", item.id).into(),
                label: parents.join(" › ").into(),
                enabled: item.enabled && item.items.is_empty(),
                selected: item.selected,
                shortcut: item
                    .key_binding
                    .as_deref()
                    .and_then(crate::key_binding_preferences::shortcut_keys)
                    .unwrap_or_default(),
                // A submenu container is navigational in IINA; only its leaves
                // represent actions. The breadcrumb fallback must not invoke it.
                section_header: !item.items.is_empty(),
            });
            append(identifier, scope, &item.items, parents, rows);
            parents.pop();
        }
    }
    let mut rows = Vec::new();
    append(identifier, scope, items, &mut Vec::new(), &mut rows);
    rows
}

fn replace_plugin_menu_rows(
    app: &PlayerWindow,
    identifier: &str,
    plugin_name: &str,
    global: bool,
    items: &[riina_core::plugin_menu::PluginMenuItem],
) {
    let current_rows = (0..app.get_plugin_menu_rows().row_count())
        .filter_map(|index| app.get_plugin_menu_rows().row_data(index))
        .collect();
    app.set_plugin_menu_rows(slint::ModelRc::new(slint::VecModel::from(
        updated_plugin_menu_rows(current_rows, identifier, plugin_name, global, items),
    )));
    let order = (0..app.get_plugin_menu_order().row_count())
        .filter_map(|index| app.get_plugin_menu_order().row_data(index))
        .map(|identifier| identifier.to_string())
        .collect::<Vec<_>>();
    reorder_plugin_menu_rows(app, &order);
}

fn updated_plugin_menu_rows(
    current_rows: Vec<PluginMenuRow>,
    identifier: &str,
    plugin_name: &str,
    global: bool,
    items: &[riina_core::plugin_menu::PluginMenuItem],
) -> Vec<PluginMenuRow> {
    let mut groups = BTreeMap::<String, Vec<PluginMenuRow>>::new();
    for row in current_rows {
        if row.token.as_str() == PLUGIN_MENU_GROUP_SEPARATOR_TOKEN {
            continue;
        }
        if let Some((plugin, _)) = row.token.as_str().split_once('\u{1f}') {
            groups.entry(plugin.to_owned()).or_default().push(row);
        }
    }

    let mut global_rows = Vec::new();
    let mut player_rows = Vec::new();
    if let Some(old_rows) = groups.remove(identifier) {
        for row in old_rows {
            if row.token.as_str().split('\u{1f}').nth(1) == Some("header") {
                continue;
            }
            match row
                .token
                .as_str()
                .split_once('\u{1f}')
                .and_then(|(_, rest)| rest.split_once('\u{1f}'))
                .map(|(scope, _)| scope)
            {
                Some("global") if !global => global_rows.push(row),
                Some("player") if global => player_rows.push(row),
                _ => {}
            }
        }
    }
    let new_rows = plugin_menu_rows_for_plugin(identifier, global, items);
    if global {
        global_rows.extend(new_rows);
    } else {
        player_rows.extend(new_rows);
    }

    if !global_rows.is_empty() || !player_rows.is_empty() {
        let mut plugin_rows = vec![PluginMenuRow {
            token: format!("{identifier}\u{1f}header\u{1f}").into(),
            label: plugin_name.into(),
            enabled: false,
            selected: false,
            shortcut: Default::default(),
            section_header: true,
        }];
        plugin_rows.extend(global_rows);
        plugin_rows.extend(player_rows);
        groups.insert(identifier.to_owned(), plugin_rows);
    }
    flatten_plugin_menu_groups(groups)
}

pub(crate) fn reorder_plugin_menu_rows(app: &PlayerWindow, order: &[String]) {
    let mut groups = BTreeMap::<String, Vec<PluginMenuRow>>::new();
    for row in (0..app.get_plugin_menu_rows().row_count())
        .filter_map(|index| app.get_plugin_menu_rows().row_data(index))
    {
        if row.token.as_str() == PLUGIN_MENU_GROUP_SEPARATOR_TOKEN {
            continue;
        }
        if let Some((identifier, _)) = row.token.as_str().split_once('\u{1f}') {
            groups.entry(identifier.to_owned()).or_default().push(row);
        }
    }
    app.set_plugin_menu_rows(slint::ModelRc::new(slint::VecModel::from(
        flatten_plugin_menu_groups_in_order(groups, order),
    )));
}

pub(crate) fn reorder_all_open_plugin_menus(order: &[String]) {
    let windows = PLAYER_WINDOWS.with(|registered| {
        let mut registered = registered.borrow_mut();
        registered.retain(|(_, weak)| weak.upgrade().is_some());
        registered
            .iter()
            .filter_map(|(_, weak)| weak.upgrade())
            .collect::<Vec<_>>()
    });
    let model = slint::ModelRc::new(slint::VecModel::from(
        order
            .iter()
            .cloned()
            .map(slint::SharedString::from)
            .collect::<Vec<_>>(),
    ));
    for app in windows {
        app.set_plugin_menu_order(model.clone());
        reorder_plugin_menu_rows(&app, order);
    }
}

fn remove_plugin_menu_rows(app: &PlayerWindow, identifier: &str) {
    let mut groups = BTreeMap::<String, Vec<PluginMenuRow>>::new();
    for row in (0..app.get_plugin_menu_rows().row_count())
        .filter_map(|index| app.get_plugin_menu_rows().row_data(index))
    {
        if row.token.as_str() == PLUGIN_MENU_GROUP_SEPARATOR_TOKEN {
            continue;
        }
        if let Some((plugin, _)) = row.token.as_str().split_once('\u{1f}')
            && plugin != identifier
        {
            groups.entry(plugin.to_owned()).or_default().push(row);
        }
    }
    app.set_plugin_menu_rows(slint::ModelRc::new(slint::VecModel::from(
        flatten_plugin_menu_groups(groups),
    )));
    let order = (0..app.get_plugin_menu_order().row_count())
        .filter_map(|index| app.get_plugin_menu_order().row_data(index))
        .map(|identifier| identifier.to_string())
        .collect::<Vec<_>>();
    reorder_plugin_menu_rows(app, &order);
}

fn flatten_plugin_menu_groups(groups: BTreeMap<String, Vec<PluginMenuRow>>) -> Vec<PluginMenuRow> {
    flatten_plugin_menu_groups_in_order(groups, &[])
}

fn flatten_plugin_menu_groups_in_order(
    mut groups: BTreeMap<String, Vec<PluginMenuRow>>,
    order: &[String],
) -> Vec<PluginMenuRow> {
    let mut rows = Vec::new();
    let mut fallback = groups.keys().cloned().collect::<Vec<_>>();
    fallback.sort_by(|left, right| {
        let left_title = groups
            .get(left)
            .and_then(|rows| rows.first())
            .map(|row| row.label.as_str())
            .unwrap_or(left);
        let right_title = groups
            .get(right)
            .and_then(|rows| rows.first())
            .map(|row| row.label.as_str())
            .unwrap_or(right);
        left_title.cmp(right_title).then_with(|| left.cmp(right))
    });
    let identifiers = order
        .iter()
        .filter(|identifier| groups.contains_key(identifier.as_str()))
        .cloned()
        .chain(
            fallback
                .into_iter()
                .filter(|identifier| !order.contains(identifier)),
        )
        .collect::<Vec<_>>();
    for identifier in identifiers {
        let Some(group) = groups.remove(&identifier) else {
            continue;
        };
        if group.is_empty() {
            continue;
        }
        if !rows.is_empty() {
            rows.push(PluginMenuRow {
                token: PLUGIN_MENU_GROUP_SEPARATOR_TOKEN.into(),
                label: SLINT_MENU_SEPARATOR_TITLE.into(),
                enabled: false,
                selected: false,
                shortcut: Default::default(),
                section_header: false,
            });
        }
        rows.extend(group);
    }
    rows
}

#[cfg(test)]
mod plugin_menu_row_tests {
    use super::{PluginMenuRow, flatten_plugin_menu_groups_in_order, updated_plugin_menu_rows};
    use riina_core::plugin_menu::PluginMenuItem;
    use std::collections::BTreeMap;

    fn menu_item(id: &str, title: &str) -> PluginMenuItem {
        PluginMenuItem {
            id: id.into(),
            title: title.into(),
            enabled: true,
            selected: false,
            separator: false,
            key_binding: None,
            items: Vec::new(),
        }
    }

    #[test]
    fn global_and_player_items_share_one_display_name_heading() {
        let rows = updated_plugin_menu_rows(
            Vec::<PluginMenuRow>::new(),
            "org.example.test",
            "Example Plugin",
            true,
            &[menu_item("global-action", "Global action")],
        );
        let rows = updated_plugin_menu_rows(
            rows,
            "org.example.test",
            "Example Plugin",
            false,
            &[menu_item("player-action", "Player action")],
        );

        assert_eq!(
            rows.iter()
                .map(|row| row.label.as_str())
                .collect::<Vec<_>>(),
            ["Example Plugin", "Global action", "Player action"]
        );
        assert!(rows[0].section_header);
        assert!(rows[1].token.as_str().contains("\u{1f}global\u{1f}"));
        assert!(rows[2].token.as_str().contains("\u{1f}player\u{1f}"));
    }

    #[test]
    fn replacing_one_scope_preserves_the_other_scope_and_single_heading() {
        let mut global_parent = menu_item("global-tools", "Global tools");
        global_parent
            .items
            .push(menu_item("global-child", "Run global tool"));
        let rows = updated_plugin_menu_rows(
            Vec::<PluginMenuRow>::new(),
            "org.example.test",
            "Example Plugin",
            true,
            &[menu_item("global-action", "Global action"), global_parent],
        );
        let rows = updated_plugin_menu_rows(
            rows,
            "org.example.test",
            "Example Plugin",
            false,
            &[menu_item("old-player-action", "Old player action")],
        );
        let rows = updated_plugin_menu_rows(
            rows,
            "org.example.test",
            "Example Plugin",
            false,
            &[menu_item("new-player-action", "New player action")],
        );

        assert_eq!(
            rows.iter()
                .filter(|row| row.token.as_str().split('\u{1f}').nth(1) == Some("header"))
                .count(),
            1
        );
        assert_eq!(rows.len(), 5);
        assert!(rows.iter().any(|row| row.label == "Global action"));
        assert!(
            rows.iter()
                .any(|row| row.label == "Global tools" && row.section_header)
        );
        assert!(
            rows.iter()
                .any(|row| row.label == "Global tools › Run global tool")
        );
        assert!(rows.iter().any(|row| row.label == "New player action"));
        assert!(!rows.iter().any(|row| row.label == "Old player action"));
    }

    #[test]
    fn submenu_containers_are_not_actions_but_nested_leaves_remain_routable() {
        let mut parent = menu_item("parent", "Tools");
        parent.items.push(menu_item("child", "Run tool"));
        let rows = updated_plugin_menu_rows(
            Vec::<PluginMenuRow>::new(),
            "org.example.test",
            "Example Plugin",
            false,
            &[parent],
        );

        let container = rows
            .iter()
            .find(|row| row.label == "Tools")
            .expect("submenu breadcrumb is visible");
        assert!(container.section_header);
        assert!(!container.enabled);

        let leaf = rows
            .iter()
            .find(|row| row.label == "Tools › Run tool")
            .expect("nested action is visible");
        assert!(!leaf.section_header);
        assert!(leaf.token.as_str().ends_with("\u{1f}child"));
    }

    #[test]
    fn plugin_menu_order_follows_persisted_order_before_identifier_fallback() {
        let row = |identifier: &str| PluginMenuRow {
            token: format!("{identifier}\u{1f}header\u{1f}").into(),
            label: identifier.into(),
            enabled: false,
            selected: false,
            shortcut: Default::default(),
            section_header: true,
        };
        let groups = BTreeMap::from([
            ("a.plugin".into(), vec![row("a.plugin")]),
            ("b.plugin".into(), vec![row("b.plugin")]),
            ("c.plugin".into(), vec![row("c.plugin")]),
        ]);
        let rows =
            flatten_plugin_menu_groups_in_order(groups, &["c.plugin".into(), "a.plugin".into()]);
        let headers = rows
            .iter()
            .filter(|row| row.section_header)
            .map(|row| row.token.split('\u{1f}').next().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(headers, ["c.plugin", "a.plugin", "b.plugin"]);
    }

    #[test]
    fn plugin_menu_default_order_matches_the_name_sorted_preferences_list() {
        let row = |identifier: &str, name: &str| PluginMenuRow {
            token: format!("{identifier}\u{1f}header\u{1f}").into(),
            label: name.into(),
            enabled: false,
            selected: false,
            shortcut: Default::default(),
            section_header: true,
        };
        let groups = BTreeMap::from([
            ("a.plugin".into(), vec![row("a.plugin", "Zulu")]),
            ("z.plugin".into(), vec![row("z.plugin", "Alpha")]),
        ]);
        let rows = flatten_plugin_menu_groups_in_order(groups, &[]);
        let headers = rows
            .iter()
            .filter(|row| row.section_header)
            .map(|row| row.token.split('\u{1f}').next().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(headers, ["z.plugin", "a.plugin"]);
    }
}

fn plugin_sidebar_tab_removed(app: &PlayerWindow, identifier: &str) {
    let mut identifiers = (0..app.get_plugin_sidebar_identifiers().row_count())
        .filter_map(|row| app.get_plugin_sidebar_identifiers().row_data(row))
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    let mut labels = (0..app.get_plugin_sidebar_labels().row_count())
        .filter_map(|row| app.get_plugin_sidebar_labels().row_data(row))
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    if let Some(index) = identifiers.iter().position(|current| current == identifier) {
        identifiers.remove(index);
        labels.remove(index);
    }
    if app.get_plugin_sidebar_selected_identifier().as_str() == identifier {
        app.set_plugin_sidebar_selected_identifier("".into());
    }
    app.set_plugin_sidebar_identifiers(slint::ModelRc::new(slint::VecModel::from(
        identifiers
            .iter()
            .map(|value| slint::SharedString::from(value.as_str()))
            .collect::<Vec<_>>(),
    )));
    app.set_plugin_sidebar_labels(slint::ModelRc::new(slint::VecModel::from(
        labels
            .iter()
            .map(|value| slint::SharedString::from(value.as_str()))
            .collect::<Vec<_>>(),
    )));
    let selected = app.get_plugin_sidebar_selected_identifier().to_string();
    app.set_plugin_sidebar_selected_index(
        identifiers
            .iter()
            .position(|current| current == &selected)
            .map_or(-1, |index| index as i32),
    );
}

fn select_plugin_sidebar_tab(app: &PlayerWindow, identifier: &str) {
    app.set_plugin_sidebar_selected_identifier(identifier.into());
    if app.get_leading_sidebar() != 7 && app.get_sidebar() != 7 {
        // Use the configured IINA placement while leaving an already-open
        // Plugins panel on its current side when a different tab is selected.
        crate::sidebar_preferences::show_plugins_panel(app);
    }
    let index = (0..app.get_plugin_sidebar_identifiers().row_count())
        .find(|row| {
            app.get_plugin_sidebar_identifiers()
                .row_data(*row)
                .is_some_and(|value| value.as_str() == identifier)
        })
        .map_or(-1, |row| row as i32);
    app.set_plugin_sidebar_selected_index(index);
}

pub(crate) fn show_osd_message(
    app: &PlayerWindow,
    text: impl Into<slint::SharedString>,
    keep_stopped: bool,
) {
    if !app.get_osd_enabled() || app.get_crop_editing() {
        return;
    }
    app.set_video_osd_request_feedback(keep_stopped);
    app.set_video_osd_text(text.into());
    app.set_video_osd_progress(-1.0);
    app.invoke_show_video_osd();
}

fn plugin_sidebar_name(index: i32) -> Option<&'static str> {
    match index {
        1 => Some("audio"),
        2 => Some("subtitles"),
        3 => Some("chapter"),
        4 => Some("playlist"),
        5 => Some("layout"),
        6 => Some("video"),
        7 => Some("plugins"),
        _ => None,
    }
}

fn plugin_sidebar_index(name: &str) -> Option<i32> {
    match name {
        "audio" => Some(1),
        "subtitles" => Some(2),
        "chapter" => Some(3),
        "playlist" => Some(4),
        "layout" => Some(5),
        "video" => Some(6),
        "plugins" => Some(7),
        _ => None,
    }
}

fn plugin_window_properties(
    app: &PlayerWindow,
    native_host: Option<&slint::Weak<crate::NativeVideoHost>>,
) -> BTreeMap<String, serde_json::Value> {
    let mut properties = BTreeMap::from([
        ("loaded".into(), serde_json::Value::Bool(false)),
        ("music-mode".into(), serde_json::json!(app.get_music_mode())),
        ("fullscreen".into(), serde_json::json!(app.get_fullscreen())),
        (
            "pip".into(),
            serde_json::json!(app.get_picture_in_picture_active()),
        ),
        (
            "ontop".into(),
            serde_json::json!(app.get_always_float_on_top()),
        ),
        (
            "sidebar".into(),
            plugin_sidebar_name(app.get_sidebar())
                .or_else(|| plugin_sidebar_name(app.get_leading_sidebar()))
                .map_or(serde_json::Value::Null, |name| serde_json::json!(name)),
        ),
        ("visible".into(), serde_json::Value::Null),
        ("main".into(), serde_json::Value::Null),
        ("miniaturized".into(), serde_json::Value::Null),
        ("frame".into(), serde_json::Value::Null),
        (
            "window-size-adjusted-event".into(),
            serde_json::json!(app.get_window_size_adjusted_event()),
        ),
        ("screens".into(), serde_json::Value::Array(Vec::new())),
    ]);
    let read = |window: &slint::Window| {
        window.with_winit_window(|native| {
            let scale = native.scale_factor().max(0.1);
            let position = native.outer_position().ok().unwrap_or_default();
            let size = native.outer_size();
            let current = native.current_monitor();
            let primary = native.primary_monitor();
            let screens = native
                .available_monitors()
                .map(|monitor| {
                    let origin = monitor.position();
                    let size = monitor.size();
                    let monitor_scale = monitor.scale_factor().max(0.1);
                    serde_json::json!({
                        "frame": {
                            "x": origin.x as f64 / monitor_scale,
                            "y": origin.y as f64 / monitor_scale,
                            "width": size.width as f64 / monitor_scale,
                            "height": size.height as f64 / monitor_scale,
                        },
                        "main": primary.as_ref().is_some_and(|primary| primary == &monitor),
                        "current": current.as_ref().is_some_and(|current| current == &monitor),
                    })
                })
                .collect::<Vec<_>>();
            (
                serde_json::json!({
                    "x": position.x as f64 / scale,
                    "y": position.y as f64 / scale,
                    "width": size.width as f64 / scale,
                    "height": size.height as f64 / scale,
                }),
                native
                    .is_visible()
                    .map(serde_json::Value::Bool)
                    .unwrap_or(serde_json::Value::Null),
                native
                    .is_minimized()
                    .map(serde_json::Value::Bool)
                    .unwrap_or(serde_json::Value::Null),
                screens,
                native.has_focus(),
            )
        })
    };
    let state = native_host
        .and_then(slint::Weak::upgrade)
        .and_then(|host| read(host.window()))
        .or_else(|| read(app.window()))
        .unwrap_or_else(|| {
            (
                serde_json::Value::Null,
                serde_json::Value::Null,
                serde_json::Value::Null,
                Vec::new(),
                false,
            )
        });
    properties.insert("loaded".into(), serde_json::Value::Bool(true));
    properties.insert("frame".into(), state.0);
    properties.insert("visible".into(), state.1);
    properties.insert("miniaturized".into(), state.2);
    properties.insert("screens".into(), serde_json::Value::Array(state.3));
    properties.insert("main".into(), serde_json::json!(state.4));
    properties
}

fn set_plugin_window_frame(window: &slint::Window, x: f64, y: f64, width: f64, height: f64) {
    let scale = f64::from(window.scale_factor()).max(0.1);
    window.set_position(slint::PhysicalPosition::new(
        (x * scale).round() as i32,
        (y * scale).round() as i32,
    ));
    window.set_size(slint::PhysicalSize::new(
        (width * scale).round() as u32,
        (height * scale).round() as u32,
    ));
}

fn apply_plugin_window_action(
    app: &PlayerWindow,
    native_host: Option<&slint::Weak<crate::NativeVideoHost>>,
    action: riina_core::plugin::WindowAction,
) {
    use riina_core::plugin::WindowAction;

    match action {
        WindowAction::SetFrame {
            x,
            y,
            width,
            height,
        } => {
            set_plugin_window_frame(app.window(), x, y, width, height);
            if let Some(host) = native_host.and_then(slint::Weak::upgrade) {
                set_plugin_window_frame(host.window(), x, y, width, height);
            }
        }
        WindowAction::SetFullscreen(fullscreen) => {
            if app.get_fullscreen() != fullscreen {
                app.invoke_toggle_fullscreen();
            }
        }
        WindowAction::SetPictureInPicture(pip) => {
            if app.get_picture_in_picture_active() != pip {
                app.invoke_toggle_picture_in_picture();
            }
        }
        WindowAction::SetOnTop(ontop) => app.set_always_float_on_top(ontop),
        WindowAction::SetSidebar(name) => {
            let index = name.as_deref().map_or(Some(0), plugin_sidebar_index);
            if let Some(index) = index {
                app.set_sidebar(index);
            } else {
                app.set_status("Plugin requested an unsupported sidebar name".into());
            }
        }
        WindowAction::SetMiniaturized(miniaturized) => {
            app.window()
                .with_winit_window(|window| window.set_minimized(miniaturized));
            if let Some(host) = native_host.and_then(slint::Weak::upgrade) {
                host.window()
                    .with_winit_window(|window| window.set_minimized(miniaturized));
            }
        }
    }
}

fn dispatch_control_mouse_action(
    app: &PlayerWindow,
    player: &riina_mpv::CommandSender,
    index: i32,
) {
    use riina_core::control_preferences::MouseEffect;

    let Some(action) = riina_core::control_preferences::MouseAction::from_index(index) else {
        app.set_status("Invalid mouse action preference".into());
        return;
    };
    if app.get_crop_editing() || app.get_dialog_open() {
        return;
    }
    let effect = action.effect(app.get_paused());
    let result = match effect {
        MouseEffect::None => return,
        MouseEffect::Fullscreen => {
            app.invoke_toggle_fullscreen();
            return;
        }
        MouseEffect::SetPaused(paused) => {
            if !app.get_loaded() {
                return;
            }
            player.set_paused(paused)
        }
        MouseEffect::HideControls => {
            if app.get_control_auto_hide_enabled() {
                app.set_controls_visible(false);
            }
            return;
        }
        MouseEffect::PictureInPicture => {
            app.invoke_toggle_picture_in_picture();
            return;
        }
        MouseEffect::CycleAbLoop => {
            if !app.get_loaded() {
                return;
            }
            player.send(Command::CycleAbLoop)
        }
        MouseEffect::ResetSpeed => {
            if !app.get_loaded() {
                return;
            }
            player.send(Command::Speed(1.0))
        }
    };
    if let Err(error) = result {
        app.set_status(error.into());
    }
}

fn set_native_video_host_visible(
    host: Option<&slint::Weak<crate::NativeVideoHost>>,
    visible: bool,
) {
    let Some(host) = host.and_then(slint::Weak::upgrade) else {
        return;
    };
    // The native presentation retains this Winit handle for mpv's HWND
    // lifetime. Toggle the OS surface directly so Slint does not keep it shown.
    host.window()
        .with_winit_window(|window| window.set_visible(visible));
}

fn configured_osd_timeout(app: &PlayerWindow) -> Duration {
    Duration::try_from_secs_f64(f64::from(app.get_osd_timeout_secs())).unwrap_or_default()
}

fn control_hide_is_allowed(
    enabled: bool,
    loaded: bool,
    dialog_open: bool,
    crop_editing: bool,
    title_bar_hovered: bool,
    control_bar_hovered: bool,
    control_bar_dragging: bool,
) -> bool {
    enabled
        && loaded
        && !dialog_open
        && !crop_editing
        && !title_bar_hovered
        && !control_bar_hovered
        && !control_bar_dragging
}

fn schedule_control_hide(app: &PlayerWindow, timer: &slint::Timer) {
    timer.stop();
    if !app.get_control_auto_hide_enabled()
        || !app.get_controls_visible()
        || !app.get_loaded()
        || app.get_dialog_open()
        || app.get_crop_editing()
    {
        return;
    }
    let timeout = Duration::try_from_secs_f64(f64::from(app.get_control_auto_hide_timeout_secs()))
        .unwrap_or_default();
    let weak = app.as_weak();
    // IINA's controller deadline is interaction-driven; pausing does not pin its chrome.
    timer.start(slint::TimerMode::SingleShot, timeout, move || {
        if let Some(app) = weak.upgrade() {
            // The native controls HWND can retain hover while a separate
            // Quick Settings surface covers it. Do not let that stale hover
            // keep the transport tray over the panel indefinitely.
            let settings_panel_open = app.get_sidebar() != 0 || app.get_leading_sidebar() != 0;
            if settings_panel_open
                || control_hide_is_allowed(
                    app.get_control_auto_hide_enabled(),
                    app.get_loaded(),
                    app.get_dialog_open(),
                    app.get_crop_editing(),
                    app.get_title_bar_hovered(),
                    app.get_control_bar_hovered(),
                    app.get_control_bar_dragging(),
                )
            {
                app.set_controls_visible(false);
            }
        }
    });
}

fn refresh_video_menu(app: &PlayerWindow) {
    // Native check items toggle before dispatch. A no-op or rejected command
    // cannot dirty the observed selection, so renew the model identity to make
    // Slint rebuild the menu from confirmed state rather than that local toggle.
    app.set_video_crop_menu_labels(slint::ModelRc::new(slint::VecModel::from(
        riina_core::crop::MENU_LABELS
            .map(slint::SharedString::from)
            .to_vec(),
    )));
    app.set_video_aspect_menu_labels(slint::ModelRc::new(slint::VecModel::from(
        riina_core::video_controls::ASPECT_MENU_PRESETS
            .map(slint::SharedString::from)
            .to_vec(),
    )));
}

fn recent_media_rows(media: &[riina_core::RecentMedia]) -> Vec<RecentMediaRow> {
    media
        .iter()
        .map(|item| {
            let title = if item.title.trim().is_empty() {
                PathBuf::from(&item.path).file_name().map_or_else(
                    || item.path.clone(),
                    |name| name.to_string_lossy().into_owned(),
                )
            } else {
                item.title.clone()
            };
            RecentMediaRow {
                title: title.into(),
                position_label: time_label(Some(item.position)).into(),
            }
        })
        .collect()
}

fn recent_open_menu_rows(media: &[riina_core::RecentMedia]) -> Vec<MenuCatalogRow> {
    media
        .iter()
        .map(|item| {
            let label = if item.title.trim().is_empty() {
                PathBuf::from(&item.path).file_name().map_or_else(
                    || item.path.clone(),
                    |name| name.to_string_lossy().into_owned(),
                )
            } else {
                item.title.clone()
            };
            MenuCatalogRow {
                token: item.path.clone().into(),
                label: label.into(),
                selected: false,
            }
        })
        .collect()
}

fn remember_recent_open_document(
    app: &PlayerWindow,
    documents: &Rc<RefCell<Vec<riina_core::RecentMedia>>>,
    preferences_path: &std::path::Path,
    path: String,
    title: String,
) {
    if !app.get_record_recent_files() || path.is_empty() {
        return;
    }
    let native_path = path.clone();
    let mut documents = documents.borrow_mut();
    documents.retain(|document| document.path != path);
    documents.insert(
        0,
        riina_core::RecentMedia {
            path,
            title,
            position: 0.0,
        },
    );
    documents.truncate(10);
    let recent = documents.clone();
    drop(documents);
    app.set_recent_open_menu_rows(slint::ModelRc::new(slint::VecModel::from(
        recent_open_menu_rows(&recent),
    )));
    app.invoke_video_menu_changed();
    if let Err(error) = persist_recent_open_documents(preferences_path, &recent) {
        app.set_status(format!("Could not save Open Recent: {error}").into());
    }
    if let Err(error) = platform::remember_recent_document(&native_path) {
        app.set_status(format!("Could not update the system recent documents: {error}").into());
    }
}

fn persist_recent_open_documents(
    preferences_path: &std::path::Path,
    documents: &[riina_core::RecentMedia],
) -> std::io::Result<()> {
    crate::preference_store::update(preferences_path, &Preferences::default(), |preferences| {
        preferences.recent_open_documents = documents.to_vec();
        Ok(())
    })
    .map_err(std::io::Error::other)
}

fn resume_media_rows(
    media: &[riina_core::RecentMedia],
    resume_last_position: bool,
) -> Vec<RecentMediaRow> {
    if resume_last_position {
        recent_media_rows(media.get(..1).unwrap_or_default())
    } else {
        Vec::new()
    }
}

fn recent_document_rows(
    media: &[riina_core::RecentMedia],
    resume_last_position: bool,
) -> Vec<RecentMediaRow> {
    // IINA removes the last-played item from the document table only while its
    // separate Resume action is enabled.
    let offset = if resume_last_position { 1 } else { 0 };
    recent_media_rows(media.get(offset..).unwrap_or_default())
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn install_live_text(
    app: &PlayerWindow,
    sender: riina_mpv::CommandSender,
    latest: Arc<std::sync::Mutex<Playback>>,
) -> Rc<slint::Timer> {
    let requested_frame = Rc::new(Cell::new(None::<(u64, u64)>));
    let recognized_capture = Rc::new(Cell::new(None::<(u64, u64)>));
    let recognition_running = Arc::new(AtomicBool::new(false));
    let timer = Rc::new(slint::Timer::default());
    let weak = app.as_weak();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(300),
        move || {
            let Some(app) = weak.upgrade() else { return };
            let state = latest.lock().map(|state| state.clone()).unwrap_or_default();
            if !app.get_live_text_enabled()
                || !app.get_live_text_available()
                || !app.get_loaded()
                || state.phase != Phase::Paused
                || app.get_crop_editing()
                || app.get_music_mode()
            {
                requested_frame.set(None);
                recognized_capture.set(None);
                app.set_live_text_regions(slint::ModelRc::new(slint::VecModel::from(Vec::new())));
                return;
            }

            // One screenshot request per paused source generation prevents polling
            // from queueing duplicate frames while mpv completes the command.
            let frame_key = (
                state.generation,
                state.position.unwrap_or_default().to_bits(),
            );
            if requested_frame.get() != Some(frame_key) {
                match sender.send(Command::CaptureFrame) {
                    Ok(()) => requested_frame.set(Some(frame_key)),
                    Err(error) => app.set_status(
                        format!("Could not capture frame for text recognition: {error}").into(),
                    ),
                }
            }
            let Some(frame) = state.captured_frame.as_deref().cloned() else {
                return;
            };
            let key = (state.generation, state.completed_frame_captures);
            if recognized_capture.get() == Some(key) || recognition_running.load(Ordering::Acquire)
            {
                return;
            }
            recognized_capture.set(Some(key));
            recognition_running.store(true, Ordering::Release);
            let latest_result = latest.clone();
            let weak_result = app.as_weak();
            let running = recognition_running.clone();
            let width = frame.width as f32;
            let height = frame.height as f32;
            if let Err(error) = platform::text_recognition::recognize(frame, move |result| {
                let _ = slint::invoke_from_event_loop(move || {
                    running.store(false, Ordering::Release);
                    let Some(app) = weak_result.upgrade() else {
                        return;
                    };
                    let current = latest_result
                        .lock()
                        .map(|state| state.clone())
                        .unwrap_or_default();
                    if current.generation != key.0
                        || current.phase != Phase::Paused
                        || current.completed_frame_captures != key.1
                        || !app.get_live_text_enabled()
                        || app.get_crop_editing()
                        || app.get_music_mode()
                    {
                        return;
                    }
                    match result {
                        Ok(regions) => {
                            app.set_live_text_regions(slint::ModelRc::new(slint::VecModel::from(
                                regions
                                    .into_iter()
                                    .map(|region| LiveTextRegion {
                                        text: region.text.into(),
                                        x: region.x / width,
                                        y: region.y / height,
                                        width: region.width / width,
                                        height: region.height / height,
                                    })
                                    .collect::<Vec<_>>(),
                            )))
                        }
                        Err(error) => {
                            app.set_status(format!("Text recognition failed: {error}").into())
                        }
                    }
                });
            }) {
                recognition_running.store(false, Ordering::Release);
                app.set_status(format!("Could not start text recognition: {error}").into());
            }
        },
    );
    timer
}

fn refresh_welcome_history(
    welcome: &WelcomeWindow,
    media: &[riina_core::RecentMedia],
    resume_last_position: bool,
) {
    welcome.set_recent_media(slint::ModelRc::new(slint::VecModel::from(
        resume_media_rows(media, resume_last_position),
    )));
    welcome.set_recent_documents(slint::ModelRc::new(slint::VecModel::from(
        recent_document_rows(media, resume_last_position),
    )));
    welcome.set_recent_document_offset(if resume_last_position { 1 } else { 0 });
    welcome.set_selection_index(if resume_last_position || media.is_empty() {
        -1
    } else {
        0
    });
}

fn resume_engine_options(
    watch_later_directory: &std::path::Path,
    resume_last_position: bool,
    subtitle_auto_load: &riina_core::subtitle_controls::AutoLoadPreferences,
    network: &riina_core::network_preferences::Connection,
    network_cache: &riina_core::network_preferences::Cache,
    network_online_videos: &riina_core::network_preferences::OnlineVideos,
) -> Result<Vec<(String, String)>, String> {
    let enabled = if resume_last_position { "yes" } else { "no" }.to_owned();
    let mut options = vec![
        (
            "watch-later-directory".into(),
            watch_later_directory.to_string_lossy().into_owned(),
        ),
        ("save-position-on-quit".into(), enabled.clone()),
        ("resume-playback".into(), enabled),
    ];
    options.push((
        "sub-auto".into(),
        subtitle_auto_load.mpv_matching_mode().into(),
    ));
    options.push((
        "sub-file-paths".into(),
        subtitle_auto_load.mpv_search_paths().unwrap_or_default(),
    ));
    options.extend(network.engine_options()?);
    options.extend(network_cache.engine_options()?);
    options.extend(network_online_videos.engine_options()?);
    Ok(options)
}

fn install_playlist_selection(app: &PlayerWindow, selected: &[String]) {
    let current = app.get_playlist();
    let rows = (0..current.row_count())
        .filter_map(|index| current.row_data(index))
        .map(|mut row| {
            row.selected = selected
                .iter()
                .any(|token| token.as_str() == row.token.as_str());
            row
        })
        .collect::<Vec<_>>();
    app.set_playlist(slint::ModelRc::new(slint::VecModel::from(rows)));
}

fn selected_playlist_paths(app: &PlayerWindow, clicked_token: &str) -> Vec<String> {
    let selected = app
        .get_selected_playlist_tokens()
        .iter()
        .map(|token| token.to_string())
        .collect::<Vec<_>>();
    let selected = if selected.iter().any(|token| token == clicked_token) {
        selected
    } else {
        vec![clicked_token.to_owned()]
    };
    let rows = app.get_playlist();
    (0..rows.row_count())
        .filter_map(|index| rows.row_data(index))
        .filter(|row| selected.iter().any(|token| token == row.token.as_str()))
        .map(|row| row.path.to_string())
        .collect()
}

fn selected_playlist_ids(app: &PlayerWindow, clicked_token: &str) -> Result<Vec<i64>, String> {
    let selected = app
        .get_selected_playlist_tokens()
        .iter()
        .map(|token| token.to_string())
        .collect::<Vec<_>>();
    let selected = if selected.iter().any(|token| token == clicked_token) {
        selected
    } else {
        vec![clicked_token.to_owned()]
    };
    selected
        .iter()
        .map(|token| token.parse::<i64>().map_err(|error| error.to_string()))
        .collect()
}

fn playlist_paste_index(app: &PlayerWindow, clicked_token: &str) -> usize {
    let selected = app
        .get_selected_playlist_tokens()
        .iter()
        .map(|token| token.to_string())
        .collect::<Vec<_>>();
    let rows = app.get_playlist();
    (0..rows.row_count())
        .find(|index| {
            rows.row_data(*index)
                .is_some_and(|row| selected.iter().any(|token| token == row.token.as_str()))
        })
        .or_else(|| {
            (0..rows.row_count()).find(|index| {
                rows.row_data(*index)
                    .is_some_and(|row| row.token.as_str() == clicked_token)
            })
        })
        .unwrap_or(0)
}

fn audio_device_menu_rows(state: &Playback) -> Vec<MenuCatalogRow> {
    state
        .audio_devices
        .iter()
        .filter(|device| {
            device.name == "auto"
                || state.audio_output.as_deref().is_none_or(|driver| {
                    device
                        .name
                        .split_once('/')
                        .is_none_or(|(device_driver, _)| device_driver == driver)
                })
        })
        .map(|device| MenuCatalogRow {
            token: device.name.clone().into(),
            label: if device.description.trim().is_empty() {
                device.name.clone().into()
            } else {
                device.description.clone().into()
            },
            selected: state.audio_device.as_deref() == Some(device.name.as_str()),
        })
        .collect()
}

fn should_fallback_removed_audio_device(previous: Option<&Playback>, state: &Playback) -> bool {
    let active = matches!(
        state.phase,
        Phase::Playing | Phase::Paused | Phase::Buffering | Phase::Ended
    );
    let selection_changed = previous.is_none_or(|previous| {
        previous.audio_device != state.audio_device || previous.audio_devices != state.audio_devices
    });
    active
        && selection_changed
        && !state.audio_devices.is_empty()
        && state.audio_device.as_deref().is_some_and(|selected| {
            selected != "auto"
                && !state
                    .audio_devices
                    .iter()
                    .any(|device| device.name == selected)
        })
}

fn update_catalog(app: &PlayerWindow, state: &Playback) {
    app.set_downloaded_subtitle_save_available(
        crate::file_actions::downloaded_subtitle_path(state).is_some(),
    );
    app.set_has_active_audio_track(
        state
            .tracks
            .iter()
            .any(|track| track.kind == TrackKind::Audio && track.is_primary_selected()),
    );
    app.set_audio_device_menu_rows(slint::ModelRc::new(slint::VecModel::from(
        audio_device_menu_rows(state),
    )));
    let languages = crate::language_names::LanguageNames::current();
    for kind in [TrackKind::Video, TrackKind::Audio, TrackKind::Sub] {
        let tracks: Vec<_> = state
            .tracks
            .iter()
            .filter(|track| track.kind == kind)
            .collect();
        let mut rows = vec![CatalogRow {
            token: "no".into(),
            label: "<None>".into(),
            detail: "".into(),
            path: "".into(),
            language: "".into(),
            selected: !tracks.iter().any(|track| track.is_primary_selected()),
            playing: false,
            network: false,
        }];
        rows.extend(tracks.into_iter().map(|track| CatalogRow {
            token: track.id.to_string().into(),
            label: track.display_name().into(),
            detail: "".into(),
            path: "".into(),
            language: languages.display_name(track.lang.as_deref()).into(),
            selected: track.is_primary_selected(),
            playing: false,
            network: false,
        }));
        let selected_index = rows.iter().position(|row| row.selected).unwrap_or(0) as i32;
        let model = slint::ModelRc::new(slint::VecModel::from(rows));
        match kind {
            TrackKind::Video => {
                app.set_video_track_selected_index(selected_index);
                app.set_video_tracks(model);
            }
            TrackKind::Audio => {
                app.set_audio_track_selected_index(selected_index);
                app.set_audio_tracks(model);
            }
            TrackKind::Sub => {
                app.set_subtitle_track_selected_index(selected_index);
                app.set_subtitle_tracks(model);
            }
        }
    }
    let subtitle_tracks: Vec<_> = state
        .tracks
        .iter()
        .filter(|t| t.kind == TrackKind::Sub)
        .collect();
    let secondary_selected =
        |track: &&riina_core::Track| track.selected && track.main_selection == Some(1);
    let mut secondary_rows = vec![CatalogRow {
        token: "no".into(),
        label: "<None>".into(),
        detail: "".into(),
        path: "".into(),
        language: "".into(),
        selected: !subtitle_tracks.iter().any(secondary_selected),
        playing: false,
        network: false,
    }];
    secondary_rows.extend(subtitle_tracks.into_iter().map(|track| CatalogRow {
        token: track.id.to_string().into(),
        label: track.display_name().into(),
        detail: "".into(),
        path: "".into(),
        language: languages.display_name(track.lang.as_deref()).into(),
        selected: track.selected && track.main_selection == Some(1),
        playing: false,
        network: false,
    }));
    app.set_secondary_subtitle_track_selected_index(
        secondary_rows
            .iter()
            .position(|row| row.selected)
            .unwrap_or(0) as i32,
    );
    app.set_secondary_subtitle_tracks(slint::ModelRc::new(slint::VecModel::from(secondary_rows)));
    app.set_chapters(slint::ModelRc::new(slint::VecModel::from(
        state
            .chapters
            .iter()
            .enumerate()
            .map(|(index, chapter)| CatalogRow {
                token: index.to_string().into(),
                detail: "".into(),
                path: "".into(),
                language: "".into(),
                label: format!(
                    "{}  {}",
                    time_label(Some(chapter.time)),
                    chapter
                        .title
                        .clone()
                        .unwrap_or_else(|| format!("Chapter {}", index + 1))
                )
                .into(),
                selected: state.position.is_some_and(|position| {
                    position >= chapter.time
                        && state
                            .chapters
                            .get(index + 1)
                            .is_none_or(|next| position < next.time)
                }),
                playing: false,
                network: false,
            })
            .collect::<Vec<_>>(),
    )));
    // IINA omits the first chapter boundary because it coincides with the bar origin.
    app.set_chapter_times(slint::ModelRc::new(slint::VecModel::from(
        state
            .chapters
            .iter()
            .skip(1)
            .map(|chapter| chapter.time as f32)
            .filter(|time| time.is_finite() && *time >= 0.0)
            .collect::<Vec<_>>(),
    )));
    let selected_playlist_tokens = app.get_selected_playlist_tokens();
    app.set_playlist(slint::ModelRc::new(slint::VecModel::from(
        state
            .playlist
            .iter()
            .map(|entry| {
                let token = entry.id.to_string();
                let file_label = entry.title.clone().unwrap_or_else(|| {
                    if entry.filename.contains("://") {
                        entry.filename.clone()
                    } else {
                        PathBuf::from(&entry.filename).file_name().map_or_else(
                            || entry.filename.clone(),
                            |name| name.to_string_lossy().into_owned(),
                        )
                    }
                });
                let metadata = (app.get_playlist_show_metadata()
                    && (!app.get_playlist_show_metadata_in_music_mode() || app.get_music_mode()))
                .then(|| crate::playlist_metadata::lookup(&entry.filename))
                .flatten();
                CatalogRow {
                    token: token.clone().into(),
                    path: entry.filename.clone().into(),
                    language: "".into(),
                    label: metadata
                        .as_ref()
                        .map_or(file_label, |metadata| metadata.title.clone())
                        .into(),
                    detail: metadata
                        .map_or_else(String::new, |metadata| metadata.artist)
                        .into(),
                    selected: selected_playlist_tokens
                        .iter()
                        .any(|selected| selected.as_str() == token),
                    playing: entry.current,
                    network: crate::platform::is_http_url(entry.filename.as_str()),
                }
            })
            .collect::<Vec<_>>(),
    )));
    if !state
        .playlist
        .iter()
        .any(|entry| entry.id.to_string() == app.get_selected_playlist_token().as_str())
    {
        app.set_selected_playlist_token("".into());
    }
    let selection = app.get_selected_playlist_tokens();
    let retained = selection
        .iter()
        .filter(|token| {
            state
                .playlist
                .iter()
                .any(|entry| entry.id.to_string() == token.as_str())
        })
        .collect::<Vec<_>>();
    if retained != selection.iter().collect::<Vec<_>>() {
        app.set_selected_playlist_tokens(slint::ModelRc::new(slint::VecModel::from(retained)));
    }
    let retained_selection = app
        .get_selected_playlist_tokens()
        .iter()
        .map(|token| token.to_string())
        .collect::<Vec<_>>();
    install_playlist_selection(app, &retained_selection);
}

fn install_playlist_metadata_refresh(
    app: &PlayerWindow,
    playback: Arc<std::sync::Mutex<Playback>>,
) -> Rc<slint::Timer> {
    let timer = Rc::new(slint::Timer::default());
    let weak = app.as_weak();
    let last_state = Rc::new(Cell::new(None::<(u64, bool, bool, bool)>));
    let observed = last_state.clone();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(300),
        move || {
            let Some(app) = weak.upgrade() else { return };
            let state = (
                crate::playlist_metadata::revision(),
                app.get_playlist_show_metadata(),
                app.get_playlist_show_metadata_in_music_mode(),
                app.get_music_mode(),
            );
            if observed.get() == Some(state) {
                return;
            }
            observed.set(Some(state));
            if let Ok(playback) = playback.lock() {
                update_catalog(&app, &playback.clone());
            }
        },
    );
    timer
}

fn current_chapter(state: &Playback) -> Option<usize> {
    state.position.and_then(|position| {
        state
            .chapters
            .iter()
            .rposition(|chapter| chapter.time <= position)
    })
}

fn window_video_geometry(state: &Playback) -> Option<(f32, f32, i32)> {
    if matches!(state.phase, Phase::Loading | Phase::Stopped | Phase::Failed) {
        return None;
    }
    let video = state.window_size_video_parameters()?;
    let width = video.dw.filter(|size| *size > 0).unwrap_or(video.w);
    let height = video.dh.filter(|size| *size > 0).unwrap_or(video.h);
    if width == 0 || height == 0 {
        return None;
    }
    Some((
        width as f32,
        height as f32,
        state.video_rotation.or(video.rotate).unwrap_or(0) as i32,
    ))
}

fn media_geometry_requires_resize(
    previous_generation: Option<u64>,
    geometry_was_missing: bool,
    state: &Playback,
) -> bool {
    window_video_geometry(state).is_some()
        && (previous_generation != Some(state.generation) || geometry_was_missing)
}

#[cfg(target_os = "macos")]
fn resize_for_trackpad_pinch(window: &slint::Window, delta: f64) {
    use slint::winit_030::winit::dpi::{PhysicalPosition, PhysicalSize};

    if !delta.is_finite() || delta <= -1.0 {
        return;
    }
    let _ = window.with_winit_window(|native| {
        if native.fullscreen().is_some() || native.is_maximized() {
            return;
        }
        let current = native.inner_size();
        if current.width == 0 || current.height == 0 {
            return;
        }
        let scale = native.scale_factor();
        let monitor = native.current_monitor().map(|monitor| monitor.size());
        let min_width = (285.0 * scale).round().max(1.0) as u32;
        let min_height = (120.0 * scale).round().max(1.0) as u32;
        let (max_width, max_height) = monitor.map_or((u32::MAX, u32::MAX), |size| {
            (
                (f64::from(size.width) * 0.98).round() as u32,
                (f64::from(size.height) * 0.94).round() as u32,
            )
        });
        let lower = (f64::from(min_width) / f64::from(current.width))
            .max(f64::from(min_height) / f64::from(current.height));
        let upper = (f64::from(max_width) / f64::from(current.width))
            .min(f64::from(max_height) / f64::from(current.height));
        if !upper.is_finite() || upper < lower {
            return;
        }
        let factor = (1.0 + delta).clamp(lower, upper);
        let size = PhysicalSize::new(
            (f64::from(current.width) * factor).round() as u32,
            (f64::from(current.height) * factor).round() as u32,
        );
        let position = native.outer_position().ok();
        if let Some(position) = position {
            let width_change = i64::from(size.width) - i64::from(current.width);
            let height_change = i64::from(size.height) - i64::from(current.height);
            native.set_outer_position(PhysicalPosition::new(
                (i64::from(position.x) - width_change / 2)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
                (i64::from(position.y) - height_change / 2)
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
            ));
        }
        let _ = native.request_inner_size(size);
    });
}

#[cfg(target_os = "macos")]
fn install_macos_control_gestures(app: &PlayerWindow, player: riina_mpv::CommandSender) {
    let weak = app.as_weak();
    let force_touch_active = Cell::new(false);
    let mut pinch_fullscreen_direction = None::<bool>;
    platform::install_control_gestures(&app.window(), move |gesture| {
        let Some(app) = weak.upgrade() else { return };
        match gesture {
            platform::ControlGesture::Pressure { stage } => {
                if stage < 2 {
                    force_touch_active.set(false);
                } else if stage == 2 && !force_touch_active.replace(true) {
                    dispatch_control_mouse_action(
                        &app,
                        &player,
                        app.get_control_force_touch_index(),
                    );
                }
            }
            platform::ControlGesture::Pinch { delta, phase } => {
                use platform::GesturePhase;
                use riina_core::control_preferences::PinchAction;

                if app.get_crop_editing() || app.get_dialog_open() {
                    return;
                }
                let Ok(settings) = crate::control_preferences::current_player(&app) else {
                    return;
                };
                match settings.pinch_action {
                    PinchAction::None => {}
                    PinchAction::Fullscreen => {
                        if phase == GesturePhase::Started {
                            pinch_fullscreen_direction = None;
                        }
                        if matches!(phase, GesturePhase::Started | GesturePhase::Moved)
                            && delta.is_finite()
                            && delta != 0.0
                        {
                            let wants_fullscreen = delta > 0.0;
                            if pinch_fullscreen_direction.is_none()
                                && wants_fullscreen != app.get_fullscreen()
                            {
                                app.invoke_toggle_fullscreen();
                            }
                            pinch_fullscreen_direction = Some(wants_fullscreen);
                        }
                        if matches!(phase, GesturePhase::Ended | GesturePhase::Cancelled) {
                            pinch_fullscreen_direction = None;
                        }
                    }
                    PinchAction::AdjustWindowSize => {
                        if app.get_fullscreen() {
                            return;
                        }
                        if matches!(phase, GesturePhase::Started | GesturePhase::Moved) {
                            resize_for_trackpad_pinch(&app.window(), delta);
                        } else if matches!(phase, GesturePhase::Ended | GesturePhase::Cancelled) {
                            app.invoke_window_aspect_changed();
                        }
                    }
                }
            }
        }
    });
}

struct UiUpdateResources<'a> {
    audio_device_player: Option<&'a riina_mpv::CommandSender>,
    hdr_player: Option<&'a riina_mpv::CommandSender>,
    saved_video_filters: &'a Arc<std::sync::Mutex<riina_core::video_filters::Settings>>,
    saved_audio_filters: &'a Arc<std::sync::Mutex<riina_core::video_filters::Settings>>,
    transition_overlay: &'a AtomicU64,
    transition_release_generation: &'a AtomicU64,
}

fn update_ui(
    app: &PlayerWindow,
    state: Playback,
    resources: UiUpdateResources<'_>,
    displayed: &mut Option<Playback>,
    last_seek: &mut SeekOsd,
    last_video: &mut crate::video_osd::VideoOsd,
    last_screenshot: &mut crate::screenshot_osd::ScreenshotOsd,
) -> bool {
    use riina_core::video_reflection::Axis;
    if app.get_native_video() {
        let show_transition = {
            #[cfg(target_os = "windows")]
            {
                crate::transition_frame::should_display_until_incoming_stable(
                    &state,
                    app.get_playlist_auto_play_next(),
                    match resources
                        .transition_release_generation
                        .load(Ordering::Acquire)
                    {
                        u64::MAX => None,
                        generation => Some(generation),
                    },
                )
            }
            #[cfg(not(target_os = "windows"))]
            {
                crate::transition_frame::should_display(&state, app.get_playlist_auto_play_next())
            }
        };
        // The transition surface has an opaque black matte beneath its image.
        // Keep the surface disabled unless the retained frame is present, so
        // a missing capture can never become a black rectangle over playback.
        let show_transition =
            show_transition && crate::transition_frame::cover_frame(&state).is_some();
        let source_generation = state.transition_frame_generation.unwrap_or(u64::MAX);
        let incoming_generation = state
            .incoming_transition_frame_generation
            .unwrap_or(u64::MAX);
        let key = source_generation.rotate_left(13)
            ^ state.transition_frame_revision.rotate_left(29)
            ^ incoming_generation.rotate_left(37)
            ^ state.incoming_transition_frame_revision.rotate_left(7)
            ^ u64::from(show_transition);
        if resources.transition_overlay.swap(key, Ordering::AcqRel) != key {
            let image = if show_transition {
                crate::transition_frame::cover_frame(&state).map(|frame| {
                    let mut rgba = frame.rgba.as_ref().to_vec();
                    for pixel in rgba.as_chunks_mut::<4>().0 {
                        // Video pixels are an opaque surface. mpv's raw RGBA
                        // capture may retain a partial alpha channel, which
                        // lets the reconfiguring native child bleed through
                        // the transition cover as a brief double image.
                        pixel[3] = u8::MAX;
                    }
                    let pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                        &rgba,
                        frame.width,
                        frame.height,
                    );
                    slint::Image::from_rgba8(pixels)
                })
            } else {
                None
            };
            if let Some(image) = image {
                // Install the opaque image before exposing its cover rectangle.
                // Slint can schedule a render between property updates; enabling
                // the cover first would briefly show its black backing instead of
                // the retained frame during a native-output handoff.
                app.set_video_frame(image);
                app.set_native_transition_frame(true);
            } else {
                // Keep the last opaque frame until the cover is disabled. Clearing
                // it first can expose the cover's black backing for one render.
                app.set_native_transition_frame(false);
                app.set_video_frame(slint::Image::default());
            }
        }
    }
    let previous_generation = app.get_window_geometry_source().parse::<u64>().ok();
    app.set_window_geometry_source(state.generation.to_string().into());
    if let Ok(settings) = resources.saved_video_filters.lock() {
        let active = state.video_filters.as_deref().unwrap_or_default();
        crate::ui_models::set_rows_if_changed(
            &app.get_saved_video_filter_rows(),
            crate::video_filter_window::saved_filter_rows(&settings.saved, active),
            |rows| app.set_saved_video_filter_rows(rows),
        );
    }
    if let Ok(settings) = resources.saved_audio_filters.lock() {
        let active = state.audio_filters.as_deref().unwrap_or_default();
        crate::ui_models::set_rows_if_changed(
            &app.get_saved_audio_filter_rows(),
            crate::video_filter_window::saved_filter_rows(&settings.saved, active),
            |rows| app.set_saved_audio_filter_rows(rows),
        );
    }
    #[cfg(target_os = "macos")]
    let current_media_path = match crate::key_binding_preferences::current_media_location(&state) {
        crate::key_binding_preferences::CurrentMediaLocation::Local(path) => {
            std::path::absolute(path).ok()
        }
        crate::key_binding_preferences::CurrentMediaLocation::NoMedia
        | crate::key_binding_preferences::CurrentMediaLocation::NonLocal => None,
    };
    #[cfg(not(target_os = "macos"))]
    let current_media_path = state
        .playlist
        .iter()
        .find(|entry| entry.current || entry.playing)
        .map(|entry| PathBuf::from(&entry.filename));
    #[cfg(target_os = "macos")]
    platform::set_represented_file(&app.window(), current_media_path.as_deref());
    #[cfg(target_os = "macos")]
    let title_path = current_media_path.as_deref();
    #[cfg(target_os = "macos")]
    let filename = title_path
        .and_then(platform::display_name_for_path)
        .unwrap_or_else(|| state.title.clone());
    #[cfg(not(target_os = "macos"))]
    let title_path = current_media_path.as_deref();
    #[cfg(not(target_os = "macos"))]
    let filename = title_path
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| state.title.clone());
    let window_title = if filename.is_empty() {
        if cfg!(target_os = "macos") {
            "Riina".to_owned()
        } else {
            "riina".to_owned()
        }
    } else if cfg!(target_os = "macos") {
        filename
    } else {
        format!("{filename} — riina")
    };
    app.set_window_title(window_title.into());
    app.set_screenshot_available(
        state
            .tracks
            .iter()
            .any(|track| track.kind == TrackKind::Video && track.is_primary_selected())
            && !matches!(state.phase, Phase::Loading | Phase::Stopped | Phase::Failed),
    );
    app.set_reveal_current_media_available(
        match crate::key_binding_preferences::current_media_location(&state) {
            crate::key_binding_preferences::CurrentMediaLocation::Local(_) => true,
            crate::key_binding_preferences::CurrentMediaLocation::NoMedia
            | crate::key_binding_preferences::CurrentMediaLocation::NonLocal => false,
        },
    );
    app.set_delete_current_media_available(matches!(
        crate::key_binding_preferences::current_media_location(&state),
        crate::key_binding_preferences::CurrentMediaLocation::Local(_)
    ));
    app.set_save_current_playlist_available(!state.playlist.is_empty());
    app.set_reflection_known(
        state.video_filters.is_some()
            && state.video.is_some()
            && !matches!(state.phase, Phase::Loading | Phase::Stopped | Phase::Failed),
    );
    app.set_video_mirrored(
        Axis::Horizontal
            .observed(state.video_filters.as_deref())
            .unwrap_or(false),
    );
    app.set_video_flipped(
        Axis::Vertical
            .observed(state.video_filters.as_deref())
            .unwrap_or(false),
    );
    let filters = state.video_filters.as_deref().unwrap_or_default();
    app.set_delogo_enabled(
        state.video.is_some()
            && state.video_filters.is_some()
            && !matches!(state.phase, Phase::Loading | Phase::Stopped | Phase::Failed),
    );
    app.set_delogo_active(
        filters
            .iter()
            .any(|filter| filter.label.as_deref() == Some(riina_core::delogo::FILTER_LABEL)),
    );
    let keep_stopped_feedback = app.get_video_osd_request_feedback();
    if displayed
        .as_ref()
        .is_none_or(|old| old.generation != state.generation)
        || matches!(state.phase, Phase::Loading | Phase::Failed)
        || (state.phase == Phase::Stopped && !keep_stopped_feedback)
        || (state.error.is_some() && !keep_stopped_feedback)
    {
        app.set_video_osd_visible(false);
        app.set_video_osd_request_feedback(false);
    }
    match last_seek.observe_with_suppression(&state, app.get_disable_osd_seek()) {
        Change::Hide => app.set_seek_osd_visible(false),
        Change::Show => app.invoke_show_seek_osd(),
        Change::Keep => {}
    }
    last_video.set_volume_limit(if app.get_volume_boost() { 200.0 } else { 100.0 });
    last_video.set_disable_speed(app.get_disable_osd_speed());
    last_video.set_disable_pause_resume(app.get_disable_osd_pause_resume());
    let video_message = last_video.observe(&state);
    if !app.get_osd_enabled() {
        last_video.clear_pending();
    } else if let Some(message) = video_message {
        app.set_video_osd_request_feedback(message.kind == crate::video_osd::Kind::Aspect);
        app.set_video_osd_text(message.text.into());
        app.set_video_osd_progress(message.progress.unwrap_or(-1.0));
        app.invoke_show_video_osd();
    }
    match last_screenshot.observe(&state) {
        crate::screenshot_osd::Change::Hide => {
            clear_screenshot_preview(app);
        }
        crate::screenshot_osd::Change::Show(path)
            if app.get_osd_enabled() && !app.get_crop_editing() =>
        {
            if !app.get_screenshot_show_preview() {
                app.set_video_osd_request_feedback(false);
                app.set_video_osd_text("Screenshot Captured".into());
                app.set_video_osd_progress(-1.0);
                app.invoke_show_video_osd();
            } else {
                match slint::Image::load_from_path(std::path::Path::new(&path)) {
                    Ok(image) => {
                        let size = image.size();
                        let (width, height) =
                            crate::screenshot_osd::preview_size(size.width, size.height);
                        app.set_screenshot_preview_image(image);
                        app.set_screenshot_preview_width(width);
                        app.set_screenshot_preview_height(height);
                        app.set_screenshot_preview_path(if app.get_screenshot_save_to_file() {
                            path.into()
                        } else {
                            "".into()
                        });
                        app.invoke_show_screenshot_preview();
                    }
                    Err(_) => {
                        // Saving succeeded even if the preview decoder cannot open
                        // this format; preserve reference normal-message fallback.
                        app.set_video_osd_request_feedback(false);
                        app.set_video_osd_text("Screenshot Captured".into());
                        app.set_video_osd_progress(-1.0);
                        app.invoke_show_video_osd();
                    }
                }
            }
        }
        _ => {}
    }
    if app.get_crop_editing() {
        clear_screenshot_preview(app);
    }
    app.set_seek_position_label(seek_osd::clock(state.position).into());
    app.set_seek_duration_label(seek_osd::clock(state.duration).into());
    app.set_playlist_shuffled(state.playlist_shuffled);
    if should_fallback_removed_audio_device(displayed.as_ref(), &state)
        && let Some(audio_device_player) = resources.audio_device_player
        && let Err(error) = audio_device_player.send(Command::AudioDevice("auto".into()))
    {
        app.set_status(format!("Could not restore automatic audio output: {error}").into());
    }
    // Preserve row identity across clock ticks so focus and accessibility indexes
    // are not destroyed by unrelated time-pos notifications.
    if displayed.as_ref().is_none_or(|old| {
        old.tracks != state.tracks
            || old.chapters != state.chapters
            || old.playlist != state.playlist
            || old.audio_devices != state.audio_devices
            || old.audio_device != state.audio_device
            || old.audio_output != state.audio_output
            || current_chapter(old) != current_chapter(&state)
    }) {
        update_catalog(app, &state);
        *displayed = Some(state.clone());
    }
    let first_loaded = (!app.get_loaded() || previous_generation != Some(state.generation))
        && matches!(
            state.phase,
            Phase::Playing | Phase::Paused | Phase::Buffering | Phase::Ended
        );
    if previous_generation != Some(state.generation)
        || matches!(state.phase, Phase::Loading | Phase::Stopped | Phase::Failed)
    {
        // Do not leave the prior media's resize commands enabled while a
        // replacement is loading or after the current item has stopped.
        app.set_video_window_width(0.0);
        app.set_video_window_height(0.0);
        app.set_video_window_rotation(0);
    }
    app.set_loaded(!matches!(
        state.phase,
        Phase::Stopped | Phase::Failed | Phase::Loading
    ));
    app.set_paused(state.paused);
    app.set_playing(state.phase == Phase::Playing);
    if matches!(state.phase, Phase::Paused | Phase::Ended | Phase::Failed) {
        app.set_controls_visible(true);
    }
    let mut resize_for_media = false;
    if let Some((width, height, rotation)) = window_video_geometry(&state) {
        let geometry_was_missing =
            app.get_video_window_width() <= 0.0 || app.get_video_window_height() <= 0.0;
        app.set_video_window_width(width);
        app.set_video_window_height(height);
        app.set_video_window_rotation(rotation);
        if media_geometry_requires_resize(previous_generation, geometry_was_missing, &state) {
            // mpv can publish the loaded phase before its first usable video
            // dimensions. Retry initial sizing when those dimensions arrive.
            resize_for_media = true;
        }
    }
    if first_loaded {
        app.invoke_activity();
        if let Some(entry) = state.playlist.iter().find(|entry| entry.current) {
            let title = entry
                .title
                .as_deref()
                .filter(|title| !title.trim().is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    PathBuf::from(&entry.filename).file_name().map_or_else(
                        || entry.filename.clone(),
                        |name| name.to_string_lossy().into_owned(),
                    )
                });
            app.invoke_media_opened(entry.filename.clone().into(), title.into());
        }
        if let Some(message) = crate::video_osd::file_start_message(
            &state.title,
            app.get_osd_enabled(),
            app.get_disable_osd_file_start(),
        ) {
            app.set_video_osd_request_feedback(false);
            app.set_video_osd_text(message.text.into());
            app.set_video_osd_progress(message.progress.unwrap_or(-1.0));
            app.invoke_show_video_osd();
        }
    }
    app.set_seekable(state.seekable && state.duration.is_some());
    app.set_position(state.position.unwrap_or(0.0) as f32);
    app.set_duration(state.duration.unwrap_or(0.0) as f32);
    app.set_ab_loop_a(state.ab_loop_a.unwrap_or_default() as f32);
    app.set_ab_loop_a_known(state.ab_loop_a.is_some());
    app.set_ab_loop_b(state.ab_loop_b.unwrap_or_default() as f32);
    app.set_ab_loop_b_known(state.ab_loop_b.is_some());
    app.set_volume(state.volume as f32);
    app.set_volume_menu_title(
        format!(
            "Volume: {}{}",
            crate::localized_number::format_delay(state.volume),
            if state.muted { " (Muted)" } else { "" }
        )
        .into(),
    );
    app.set_muted(state.muted);
    app.set_speed_value(state.speed as f32);
    app.set_speed_slider_position(riina_core::speed_controls::slider_position(state.speed) as f32);
    app.set_speed_edit_value(riina_core::speed_controls::text(state.speed).into());
    app.set_video_aspect_known(state.video_aspect_override.is_some());
    let display_aspect = state.display_video_parameters().and_then(|video| {
        riina_core::window_aspect::display_aspect_ratio(
            video.w,
            video.h,
            video.dw,
            video.dh,
            state.video_rotation.or(video.rotate),
            state.video_aspect_override,
        )
    });
    app.set_video_display_aspect_ratio(display_aspect.unwrap_or(0.0) as f32);
    let is_audio = state.audio_status() == riina_core::music_mode::AudioStatus::Audio;
    let album_art = state
        .tracks
        .iter()
        .find(|track| track.kind == riina_core::TrackKind::Video && track.albumart);
    let has_visible_subtitles = state.tracks.iter().any(|track| {
        track.kind == riina_core::TrackKind::Sub
            && ((state.subtitle_visible && track.is_primary_selected())
                || (state.secondary_subtitle_visible.unwrap_or(false)
                    && track.selected
                    && track.main_selection == Some(1)))
    });
    app.set_music_mode_is_audio(is_audio);
    app.set_music_mode_has_album_art(album_art.is_some());
    app.set_music_mode_has_visible_subtitles(has_visible_subtitles);
    let artwork_aspect = if is_audio {
        album_art
            .and_then(|track| track.width.zip(track.height))
            .filter(|(width, height)| *width > 0 && *height > 0)
            .map(|(width, height)| width as f64 / height as f64)
    } else {
        display_aspect
    };
    app.set_music_mode_artwork_aspect_ratio(artwork_aspect.unwrap_or(0.0) as f32);
    app.set_video_aspect_index(
        state
            .video_aspect
            .preset_index()
            .map_or(-1, |index| index as i32),
    );
    app.set_video_aspect_menu_index(
        state
            .video_aspect
            .menu_index()
            .map_or(-1, |index| index as i32),
    );
    app.set_video_rotation_known(state.video_rotation.is_some());
    app.set_deinterlace_known(state.deinterlace.is_some() && state.video.is_some());
    app.set_deinterlace_enabled(state.deinterlace.unwrap_or(false));
    app.set_video_crop_known(
        state.video_filters.is_some()
            && state
                .video
                .as_ref()
                .is_some_and(|video| video.w > 0 && video.h > 0),
    );
    app.set_video_crop_index(
        riina_core::crop::observed_index(&state).map_or(-1, |index| index as i32),
    );
    app.set_video_crop_menu_index(
        riina_core::crop::observed_menu_index(&state).map_or(-1, |index| index as i32),
    );
    app.set_video_equalizer_values(slint::ModelRc::new(slint::VecModel::from(
        state
            .video_equalizer
            .map(|value| value.unwrap_or(0) as i32)
            .to_vec(),
    )));
    app.set_video_equalizer_known(slint::ModelRc::new(slint::VecModel::from(
        state.video_equalizer.map(|value| value.is_some()).to_vec(),
    )));
    if let Some(gains) = state.audio_equalizer {
        app.set_audio_equalizer_values(slint::ModelRc::new(slint::VecModel::from(
            gains.map(|gain| gain as f32).to_vec(),
        )));
    }
    app.set_hardware_decoding_known(state.hardware_decoding.is_some());
    app.set_hardware_decoding_enabled(
        state
            .hardware_decoding
            .as_deref()
            .is_some_and(|mode| mode != "no"),
    );
    app.set_video_rotation_index(
        state
            .video_rotation
            .filter(|value| matches!(value, 0 | 90 | 180 | 270))
            .map_or(-1, |value| (value / 90) as i32),
    );
    app.set_audio_delay(state.audio_delay as f32);
    app.set_audio_delay_text(crate::localized_number::format_delay(state.audio_delay).into());
    app.set_subtitle_delay(state.subtitle_delay as f32);
    app.set_subtitle_delay_text(crate::localized_number::format_delay(state.subtitle_delay).into());
    app.set_subtitle_scale_known(state.subtitle_scale.is_some());
    app.set_subtitle_font_size_known(state.subtitle_font_size.is_some());
    app.set_subtitle_font_known(state.subtitle_font.is_some());
    if let Some(font) = &state.subtitle_font {
        app.set_subtitle_font(font.clone().into());
    }
    app.set_subtitle_font_size(state.subtitle_font_size.unwrap_or(0.0) as f32);
    if let Some(size) = state.subtitle_font_size {
        app.set_subtitle_font_size_preference(size as f32);
    }
    app.set_subtitle_outline_size_known(state.subtitle_outline_size.is_some());
    app.set_subtitle_outline_size(state.subtitle_outline_size.unwrap_or(0.0) as f32);
    let ui_color = |color: Option<riina_core::subtitle_color::SubtitleColor>| {
        let [r, g, b, a] = color
            .unwrap_or(riina_core::subtitle_color::SubtitleColor::CLEAR)
            .rgba();
        slint::Color::from_argb_u8(
            (a * 255.0).round() as u8,
            (r * 255.0).round() as u8,
            (g * 255.0).round() as u8,
            (b * 255.0).round() as u8,
        )
    };
    app.set_subtitle_text_color_known(state.subtitle_text_color.is_some());
    app.set_subtitle_border_color_known(state.subtitle_border_color.is_some());
    app.set_subtitle_shadow_color_known(state.subtitle_shadow_color.is_some());
    app.set_subtitle_text_color(ui_color(state.subtitle_text_color));
    app.set_subtitle_border_color(ui_color(state.subtitle_border_color));
    app.set_subtitle_shadow_color(ui_color(state.subtitle_shadow_color));
    let subtitle_style = state.subtitle_style_availability();
    app.set_subtitle_text_style_available(subtitle_style.text_controls);
    app.set_subtitle_ass_style_warning(subtitle_style.ass_warning);
    let scale = state.subtitle_scale.unwrap_or(1.0).clamp(0.1, 10.0);
    app.set_subtitle_scale_slider(
        (if scale >= 1.0 {
            scale - 1.0
        } else {
            1.0 - scale.recip()
        }) as f32,
    );
    app.set_subtitle_position_known(state.subtitle_position.is_some());
    app.set_subtitle_position(state.subtitle_position.unwrap_or(0.0) as f32);
    app.set_secondary_subtitle_delay_known(state.secondary_subtitle_delay.is_some());
    app.set_secondary_subtitle_delay(state.secondary_subtitle_delay.unwrap_or(0.0) as f32);
    app.set_secondary_subtitle_delay_text(
        crate::localized_number::format_delay(state.secondary_subtitle_delay.unwrap_or(0.0)).into(),
    );
    app.set_secondary_subtitle_position_known(state.secondary_subtitle_position.is_some());
    app.set_secondary_subtitle_position(state.secondary_subtitle_position.unwrap_or(0.0) as f32);
    app.set_subtitle_visible(state.subtitle_visible);
    app.set_secondary_subtitle_visibility_known(state.secondary_subtitle_visible.is_some());
    app.set_secondary_subtitle_visible(state.secondary_subtitle_visible.unwrap_or(false));
    app.set_loop_label(
        match state.loop_mode() {
            LoopMode::Off => "Off",
            LoopMode::File => "File",
            LoopMode::Playlist => "Playlist",
        }
        .into(),
    );
    let hdr_source_detected = state
        .video
        .as_ref()
        .or(state.video_target.as_ref())
        .or(state.video_output.as_ref())
        .is_some_and(riina_core::VideoParameters::is_hdr);
    let hdr_source_changed = app.get_hdr_source_detected() != hdr_source_detected;
    app.set_hdr_source_detected(hdr_source_detected);
    if first_loaded || hdr_source_changed {
        let enable_hdr = app.get_default_hdr_mode_enabled()
            && app.get_hdr_source_detected()
            && app.get_hdr_display_active();
        app.set_hdr_enabled(enable_hdr);
        if let Some(player) = resources.hdr_player
            && let Err(error) = player.send(Command::SetHdrEnabledWithIcc {
                enabled: enable_hdr,
                load_icc_profile: app.get_load_icc_profile(),
            })
        {
            app.set_status(format!("Could not apply the HDR output preference: {error}").into());
        }
    }
    app.set_media_title(state.title.into());
    app.set_music_artist(state.artist.unwrap_or_default().into());
    app.set_music_album(state.album.unwrap_or_default().into());
    app.set_position_label(time_label(state.position).into());
    app.set_duration_label(time_label(state.duration).into());
    // Playback state belongs to controls and transient OSD, not permanent text
    // over the video. Keep failures visible while ordinary clock ticks stay quiet.
    app.set_status(state.error.unwrap_or_default().into());
    resize_for_media
}

pub(crate) struct UiSession {
    // Native callbacks retain senders only. Field order keeps the target alive
    // until this sole native player owner has joined its decoder worker.
    pub(crate) player: Option<Player>,
    #[cfg(target_os = "windows")]
    _plugin_overlay_presenter: Rc<RefCell<platform::PluginOverlayPresenter>>,
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    _plugin_sidebar_presenter: Rc<RefCell<platform::PluginSidebarPresenter>>,
    #[cfg(target_os = "macos")]
    _system_media_controls: Rc<RefCell<platform::SystemMediaControls>>,
    #[cfg(target_os = "macos")]
    _system_media_controls_timer: Rc<slint::Timer>,
    #[cfg(target_os = "macos")]
    _picture_in_picture: Rc<RefCell<Option<platform::MacPictureInPicture>>>,
    #[cfg(target_os = "macos")]
    _picture_in_picture_timer: Rc<slint::Timer>,
    #[cfg(target_os = "macos")]
    _mac_scroll_event_monitor: Option<platform::MacScrollEventMonitor>,
    #[cfg(target_os = "macos")]
    _mac_plugin_menu_observer: Option<platform::MacPluginMenuObserver>,
    _plugin_host: Option<crate::plugin_host::PluginHost>,
    _plugin_host_timer: Option<Rc<slint::Timer>>,
    _plugin_osd_timer: Option<Rc<slint::Timer>>,
    _presentation: Option<platform::NativePresentation>,
    _hdr_display_timer: Option<Rc<slint::Timer>>,
    _live_text_timer: Option<Rc<slint::Timer>>,
    // The repeated sampler is held for the same lifetime as the player window.
    _appearance_timer: Rc<slint::Timer>,
    _pause_resume_timer: Rc<slint::Timer>,
    _thumbnail_preview_timer: Rc<slint::Timer>,
    _transition_frame_timer: Option<Rc<slint::Timer>>,
    _native_material_timer: Option<Rc<slint::Timer>>,
    #[cfg(target_os = "windows")]
    _windows_materials_policy_timer: Rc<slint::Timer>,
    _subtitle_priority_timer: Rc<slint::Timer>,
    _online_subtitles_window: Rc<RefCell<Option<crate::OnlineSubtitlesWindow>>>,
    _online_subtitles_timer: Rc<slint::Timer>,
    _playback_history_window: Rc<RefCell<Option<crate::PlaybackHistoryWindow>>>,
    _playback_history_timer: Rc<slint::Timer>,
    _video_filters_window: Rc<RefCell<Option<crate::VideoFiltersWindow>>>,
    _video_filters_timer: Rc<slint::Timer>,
    _audio_filters_window: Rc<RefCell<Option<crate::VideoFiltersWindow>>>,
    _audio_filters_timer: Rc<slint::Timer>,
    _playlist_metadata_timer: Rc<slint::Timer>,
    _file_drop_timer: Rc<slint::Timer>,
    _player_window_focus: PlayerWindowFocusRegistration,
    prefs_path: PathBuf,
    prefs: Preferences,
    preference_writer: crate::preference_store::Writer,
    audio_equalizer: Arc<std::sync::Mutex<riina_core::audio_equalizer::Settings>>,
    saved_video_filters: Arc<std::sync::Mutex<riina_core::video_filters::Settings>>,
    saved_audio_filters: Arc<std::sync::Mutex<riina_core::video_filters::Settings>>,
    recent_media: Rc<RefCell<Vec<riina_core::RecentMedia>>>,
    recent_open_documents: Rc<RefCell<Vec<riina_core::RecentMedia>>>,
    recent_open_cleared: Rc<Cell<bool>>,
    playback_history: Arc<std::sync::Mutex<crate::plugin_host::PlaybackHistoryState>>,
    record_playback_history: Arc<AtomicBool>,
    history_cleared: Arc<AtomicBool>,
    latest_playback: Arc<std::sync::Mutex<Playback>>,
}

enum PendingWelcomeOpen {
    Url(String),
    Recent(riina_core::RecentMedia, bool),
}

#[cfg(target_os = "macos")]
thread_local! {
    static FOCUSED_PLAYER_WINDOW_COUNT: Cell<usize> = const { Cell::new(0) };
}

thread_local! {
    static PLAYER_WINDOWS: RefCell<Vec<(u64, slint::Weak<PlayerWindow>)>> = const { RefCell::new(Vec::new()) };
}

struct PlayerWindowFocusRegistration {
    focused: Rc<Cell<bool>>,
    registry_id: u64,
}

#[derive(Clone, Copy, Default)]
struct ScrollInputState {
    axis: Option<bool>,
    precise: Option<bool>,
    natural_direction: Option<bool>,
    phase_empty: bool,
    phase_began: bool,
    phase_ended: bool,
    momentum_allowed: bool,
    mouse_in_window: bool,
    suppress_current: bool,
    appkit: bool,
}

impl Drop for PlayerWindowFocusRegistration {
    fn drop(&mut self) {
        PLAYER_WINDOWS.with(|registered| {
            registered
                .borrow_mut()
                .retain(|(id, _)| *id != self.registry_id);
        });
        set_player_window_focus(&self.focused, false);
    }
}

fn set_player_window_focus(focused: &Cell<bool>, now_focused: bool) {
    #[cfg(target_os = "macos")]
    {
        let was_focused = focused.replace(now_focused);
        if was_focused != now_focused {
            FOCUSED_PLAYER_WINDOW_COUNT.with(|count| {
                count.set(if now_focused {
                    count.get().saturating_add(1)
                } else {
                    count.get().saturating_sub(1)
                });
            });
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = focused.replace(now_focused);
}

#[cfg(target_os = "macos")]
fn some_player_window_is_focused() -> bool {
    FOCUSED_PLAYER_WINDOW_COUNT.with(|count| count.get() > 0)
}

fn install_file_drop(
    app: &PlayerWindow,
    player: riina_mpv::CommandSender,
    playback: Arc<std::sync::Mutex<Playback>>,
    trackpad_pause_generation: Rc<Cell<Option<u64>>>,
    _scroll_input: Rc<Cell<ScrollInputState>>,
) -> (Rc<slint::Timer>, PlayerWindowFocusRegistration) {
    let registry_id = NEXT_PLAYER_WINDOW_REGISTRATION.fetch_add(1, Ordering::Relaxed);
    PLAYER_WINDOWS.with(|registered| {
        registered.borrow_mut().push((registry_id, app.as_weak()));
    });
    let pending_paths = Rc::new(RefCell::new(Vec::<std::ffi::OsString>::new()));
    let flush_paths = pending_paths.clone();
    let weak_app = app.as_weak();
    let timer_player = player.clone();
    let timer = Rc::new(slint::Timer::default());
    timer.start(
        slint::TimerMode::SingleShot,
        Duration::from_millis(80),
        move || {
            let paths = std::mem::take(&mut *flush_paths.borrow_mut());
            if !paths.is_empty()
                && let Some(app) = weak_app.upgrade()
            {
                media_open::dropped(&app, paths, timer_player.clone());
            }
        },
    );
    let event_paths = pending_paths;
    let event_timer = timer.clone();
    let focused = Rc::new(Cell::new(
        app.window()
            .with_winit_window(|window| window.has_focus())
            .unwrap_or(false),
    ));
    let event_scroll_input = _scroll_input;
    let event_player = player;
    let event_playback = playback;
    let event_app = app.as_weak();
    let event_trackpad_pause_generation = trackpad_pause_generation;
    #[cfg(target_os = "macos")]
    FOCUSED_PLAYER_WINDOW_COUNT.with(|count| {
        if focused.get() {
            count.set(count.get().saturating_add(1));
        }
    });
    let event_focused = Rc::downgrade(&focused);
    app.window().on_winit_window_event(move |_window, event| {
        if let winit::event::WindowEvent::DroppedFile(path) = event {
            event_paths.borrow_mut().push(path.clone().into_os_string());
            event_timer.restart();
        }
        if let winit::event::WindowEvent::Focused(now_focused) = event
            && let Some(focused) = event_focused.upgrade()
        {
            set_player_window_focus(&focused, *now_focused);
        }
        if matches!(event, winit::event::WindowEvent::CursorEntered { .. }) {
            let mut input = event_scroll_input.get();
            input.mouse_in_window = true;
            event_scroll_input.set(input);
        } else if matches!(event, winit::event::WindowEvent::CursorLeft { .. }) {
            let mut input = event_scroll_input.get();
            input.mouse_in_window = false;
            input.momentum_allowed = false;
            event_scroll_input.set(input);
        }
        #[cfg(not(target_os = "macos"))]
        if let winit::event::WindowEvent::MouseWheel { delta, phase, .. } = event {
            use winit::event::{MouseScrollDelta, TouchPhase};
            let (delta_x, delta_y, precise) = match delta {
                MouseScrollDelta::LineDelta(x, y) => (f64::from(*x), f64::from(*y), false),
                MouseScrollDelta::PixelDelta(position) => (position.x, position.y, true),
            };
            let mut input = event_scroll_input.get();
            input.appkit = false;
            input.precise = Some(precise);
            input.natural_direction = None;
            input.phase_empty = !precise;
            input.phase_began = *phase == TouchPhase::Started;
            input.phase_ended = matches!(phase, TouchPhase::Ended | TouchPhase::Cancelled);
            if !precise {
                // Mouse-wheel events are independent. IINA prioritizes horizontal
                // whenever an event contains a horizontal delta.
                input.axis = (delta_x != 0.0 || delta_y != 0.0).then_some(delta_x != 0.0);
            } else {
                match phase {
                    TouchPhase::Started => {
                        // IINA chooses the first non-zero axis when a gesture begins,
                        // then keeps it through diagonal trackpad motion.
                        let axis = if delta_x != 0.0 {
                            Some(true)
                        } else if delta_y != 0.0 {
                            Some(false)
                        } else {
                            None
                        };
                        input.axis = axis;
                    }
                    TouchPhase::Ended | TouchPhase::Cancelled => {
                        input.axis = None;
                    }
                    TouchPhase::Moved if input.axis.is_none() => {
                        if delta_x != 0.0 || delta_y != 0.0 {
                            input.axis = Some(delta_x != 0.0);
                        }
                    }
                    TouchPhase::Moved => {}
                }
            }
            event_scroll_input.set(input);
        }
        if matches!(event, winit::event::WindowEvent::MouseWheel { .. }) {
            let input = event_scroll_input.get();
            if input.suppress_current {
                return EventResult::PreventDefault;
            }
            if !input.phase_empty
                && input.phase_ended
                && let Some(generation) = event_trackpad_pause_generation.replace(None)
                && event_playback
                    .lock()
                    .is_ok_and(|state| state.generation == generation)
                && let Err(error) = event_player.set_paused(false)
                && let Some(app) = event_app.upgrade()
            {
                app.set_status(error.into());
            }
        }
        EventResult::Propagate
    });
    (
        timer,
        PlayerWindowFocusRegistration {
            focused,
            registry_id,
        },
    )
}

fn scroll_action_handler(
    app: slint::Weak<PlayerWindow>,
    player: riina_mpv::CommandSender,
    playback: Arc<std::sync::Mutex<Playback>>,
    scroll_input: Rc<Cell<ScrollInputState>>,
    trackpad_pause_generation: Rc<Cell<Option<u64>>>,
    override_action: Option<riina_core::control_preferences::ScrollAction>,
) -> impl FnMut(f32, f32) -> bool + 'static {
    move |delta_x, delta_y| {
        let Some(app) = app.upgrade() else {
            return false;
        };
        if app.get_scroll_input_suppressed() {
            return true;
        }
        let input = scroll_input.get();
        let horizontal = if input.appkit {
            input.axis.unwrap_or(false)
        } else {
            input.axis.unwrap_or_else(|| delta_x.abs() > delta_y.abs())
        };
        let mut delta_x = delta_x;
        let mut delta_y = delta_y;
        if input.appkit {
            if input.natural_direction == Some(true) {
                delta_y = -delta_y;
            } else {
                delta_x = -delta_x;
            }
        }
        let delta = if horizontal { delta_x } else { delta_y };
        if delta == 0.0 {
            return false;
        }
        let Ok(settings) = crate::control_preferences::current_player(&app) else {
            return false;
        };
        let precise = input.precise.unwrap_or_else(|| delta.abs() < 60.0);
        let action = override_action.unwrap_or(if horizontal {
            settings.horizontal_scroll_action
        } else {
            settings.vertical_scroll_action
        });
        let effect = riina_core::control_preferences::scroll_effect_for_input(
            &settings,
            action,
            horizontal,
            f64::from(delta),
            precise,
            input.phase_empty,
        );
        let command = match effect {
            riina_core::control_preferences::ScrollEffect::None => return false,
            riina_core::control_preferences::ScrollEffect::AdjustVolume(delta) => {
                Command::AdjustVolume {
                    delta,
                    boost: app.get_volume_boost(),
                }
            }
            riina_core::control_preferences::ScrollEffect::Seek(seconds) => {
                if !input.phase_empty
                    && input.phase_began
                    && trackpad_pause_generation.get().is_none()
                    && let Some(state) = playback.lock().ok().map(|state| state.clone())
                    && state.phase == Phase::Playing
                    && !state.paused
                {
                    match player.set_paused(true) {
                        Ok(()) => trackpad_pause_generation.set(Some(state.generation)),
                        Err(error) => app.set_status(error.into()),
                    }
                }
                Command::SeekWithType {
                    seconds,
                    seek_type: settings.seek_type,
                }
            }
            riina_core::control_preferences::ScrollEffect::MultiplySpeed(factor) => {
                let current = f64::from(app.get_speed_value()).clamp(0.05, 4.0);
                let target = (current * factor).clamp(0.05, 4.0);
                Command::MultiplySpeed(target / current)
            }
        };
        app.invoke_activity();
        if let Err(error) = player.send(command) {
            app.set_status(error.into());
        }
        true
    }
}

fn dispatch_welcome_open(
    app: &PlayerWindow,
    request: PendingWelcomeOpen,
    sender: &riina_mpv::CommandSender,
) {
    match request {
        PendingWelcomeOpen::Url(value) => media_open::url(app, &value, sender.clone()),
        PendingWelcomeOpen::Recent(media, resume) => {
            media_open::open_recent(app, media, resume, sender.clone());
        }
    }
}

fn install_pause_on_minimize(
    app: &PlayerWindow,
    playback: Arc<std::sync::Mutex<Playback>>,
) -> Rc<slint::Timer> {
    let timer = Rc::new(slint::Timer::default());
    let weak_app = app.as_weak();
    let mut minimized = false;
    let mut paused_generation = None;
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(50),
        move || {
            let Some(app) = weak_app.upgrade() else {
                return;
            };
            let now_minimized = app.window().is_minimized();
            if now_minimized && !minimized && app.get_pause_when_minimized() {
                let state = playback.lock().ok().map(|state| state.clone());
                if let Some(state) =
                    state.filter(|state| state.phase == Phase::Playing && !state.paused)
                {
                    paused_generation = Some(state.generation);
                    app.invoke_set_paused(true);
                }
            } else if !now_minimized
                && minimized
                && let Some(generation) = paused_generation.take()
                && playback.lock().is_ok_and(|state| {
                    state.generation == generation && state.phase == Phase::Playing && state.paused
                })
            {
                app.invoke_set_paused(false);
            }
            minimized = now_minimized;
        },
    );
    timer
}

#[cfg(target_os = "macos")]
fn dispatch_system_media_action(
    app: &PlayerWindow,
    action: platform::RemoteMediaAction,
    latest_playback: &Arc<std::sync::Mutex<Playback>>,
    player: &riina_mpv::CommandSender,
    paused_by_inactive: &mut Option<u64>,
    application_active: &mut bool,
    player_window_was_focused: bool,
) {
    use platform::RemoteMediaAction as Action;

    match action {
        Action::Play => {
            if !crate::key_binding_preferences::dispatch_system_media_key(
                app,
                "PLAYONLY",
                latest_playback,
            ) {
                app.invoke_set_paused(false);
            }
        }
        Action::Pause => {
            if !crate::key_binding_preferences::dispatch_system_media_key(
                app,
                "PAUSEONLY",
                latest_playback,
            ) {
                app.invoke_set_paused(true);
            }
        }
        Action::Toggle => {
            if !crate::key_binding_preferences::dispatch_system_media_key(
                app,
                "PLAYPAUSE",
                latest_playback,
            ) {
                app.invoke_toggle_pause();
            }
        }
        Action::Stop => {
            if !crate::key_binding_preferences::dispatch_system_media_key(
                app,
                "STOP",
                latest_playback,
            ) {
                app.invoke_stop();
            }
        }
        Action::Next => {
            if !crate::key_binding_preferences::dispatch_system_media_key(
                app,
                "NEXT",
                latest_playback,
            ) {
                app.invoke_playlist_next();
            }
        }
        Action::Previous => {
            if !crate::key_binding_preferences::dispatch_system_media_key(
                app,
                "PREV",
                latest_playback,
            ) {
                app.invoke_playlist_previous();
            }
        }
        Action::Forward(seconds) => {
            if !crate::key_binding_preferences::dispatch_system_media_key(
                app,
                "FORWARD",
                latest_playback,
            ) && seconds.is_finite()
                && seconds > 0.0
            {
                app.invoke_execute_mpv_input_action(format!("seek {seconds} exact").into());
            }
        }
        Action::Backward(seconds) => {
            if !crate::key_binding_preferences::dispatch_system_media_key(
                app,
                "REWIND",
                latest_playback,
            ) && seconds.is_finite()
                && seconds > 0.0
            {
                app.invoke_execute_mpv_input_action(format!("seek -{seconds} exact").into());
            }
        }
        Action::Position(seconds) if seconds.is_finite() && seconds >= 0.0 => {
            app.invoke_seek(seconds as f32);
        }
        Action::Position(_) => {}
        Action::Rate(rate) if rate.is_finite() && rate > 0.0 => {
            let _ = app.invoke_set_speed_text(rate.to_string().into());
        }
        Action::Rate(_) => {}
        Action::Repeat => {
            if let Err(error) = player.send(Command::CycleLoop) {
                app.set_status(error.into());
            }
        }
        Action::Inactive => {
            *application_active = false;
            if app.get_pause_when_inactive()
                && (player_window_was_focused || some_player_window_is_focused())
                && paused_by_inactive.is_none()
            {
                let state = latest_playback.lock().ok().map(|state| state.clone());
                if let Some(state) =
                    state.filter(|state| state.phase == Phase::Playing && !state.paused)
                {
                    *paused_by_inactive = Some(state.generation);
                    app.invoke_set_paused(true);
                }
            }
        }
        Action::Active => *application_active = true,
        Action::Sleep if app.get_pause_when_goes_to_sleep() => {
            if latest_playback
                .lock()
                .is_ok_and(|state| state.phase == Phase::Playing && !state.paused)
            {
                app.invoke_set_paused(true);
            }
        }
        Action::Sleep => {}
    }
}

impl Drop for UiSession {
    fn drop(&mut self) {
        #[cfg(target_os = "macos")]
        self._system_media_controls
            .borrow_mut()
            .synchronize(false, "", None, None);
        // Release session ownership before its presentation on every platform.
        // Native playback joins here; GL callbacks retain their player until the
        // renderer has released its surface during RenderingTeardown.
        self.player.take();
    }
}

pub(crate) fn action_after_launch()
-> Result<riina_core::ActionAfterLaunch, Box<dyn std::error::Error>> {
    Ok(load_preferences()?.action_after_launch)
}

#[cfg(target_os = "windows")]
pub(crate) fn windows_initial_theme()
-> Result<Option<winit::window::Theme>, Box<dyn std::error::Error>> {
    Ok(match load_preferences()?.effective_theme_material() {
        riina_core::ThemeMaterial::Dark => Some(winit::window::Theme::Dark),
        riina_core::ThemeMaterial::Light => Some(winit::window::Theme::Light),
        riina_core::ThemeMaterial::System => None,
    })
}

fn load_preferences() -> Result<Preferences, Box<dyn std::error::Error>> {
    let path = platform::preferences_path()?;
    match std::fs::read_to_string(path) {
        Ok(text) => Preferences::parse(&text)
            .map_err(std::io::Error::other)
            .map_err(Into::into),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Preferences::default()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn accepts_first_mouse_at_launch() -> Result<bool, Box<dyn std::error::Error>> {
    let path = platform::preferences_path()?;
    let preferences = match std::fs::read_to_string(path) {
        Ok(text) => Preferences::parse(&text).map_err(std::io::Error::other)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Preferences::default(),
        Err(error) => return Err(error.into()),
    };
    Ok(preferences.controls.accepts_first_mouse)
}

pub(crate) fn configure_player(
    app: &PlayerWindow,
    native: Option<platform::NativePresentation>,
    media: Vec<std::ffi::OsString>,
    welcome: Option<&WelcomeWindow>,
    startup_resume_position: Option<f64>,
) -> Result<UiSession, Box<dyn std::error::Error>> {
    #[cfg(target_os = "linux")]
    {
        let weak = app.as_weak();
        app.on_toggle_picture_in_picture(move || {
            if let Some(app) = weak.upgrade() {
                app.set_status(
                    "Picture-in-Picture is unavailable on this presentation path".into(),
                );
            }
        });
    }
    let prefs_path = platform::preferences_path()?;
    let plugin_menu_order = std::fs::read_to_string(prefs_path.with_file_name("plugin-order.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<Vec<String>>(&text).ok())
        .unwrap_or_default();
    app.set_plugin_menu_order(slint::ModelRc::new(slint::VecModel::from(
        plugin_menu_order
            .into_iter()
            .map(slint::SharedString::from)
            .collect::<Vec<_>>(),
    )));
    let saved_preferences = match std::fs::read_to_string(&prefs_path) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    let mut prefs = match saved_preferences.as_deref() {
        Some(text) => Preferences::parse(text).map_err(std::io::Error::other)?,
        None => Preferences::default(),
    };
    if let Err(error) = crate::online_subtitles_credentials::restore_preferences(
        &mut prefs,
        saved_preferences.as_deref(),
        &prefs_path,
    ) {
        app.set_status(format!("OpenSubtitles credential storage: {error}").into());
    }
    let audio_equalizer = Arc::new(std::sync::Mutex::new(prefs.audio_equalizer.clone()));
    let saved_video_filters = Arc::new(std::sync::Mutex::new(prefs.video_filters.clone()));
    let saved_audio_filters = Arc::new(std::sync::Mutex::new(prefs.audio_filters.clone()));
    if let Ok(settings) = audio_equalizer.lock() {
        set_audio_equalizer_controls(app, &settings);
    }
    let playback_history_path = crate::playback_history_store::path_for_preferences(&prefs_path);
    let mut playback_history_write_path = Some(playback_history_path.clone());
    let playback_history_entries = match crate::playback_history_store::load(
        &playback_history_path,
        &prefs.playback_history,
    ) {
        Ok(entries) => {
            prefs.playback_history.clone_from(&entries);
            if !playback_history_path.exists()
                && let Err(error) =
                    crate::playback_history_store::save(&playback_history_path, &entries)
            {
                app.set_status(format!("Could not migrate playback history: {error}").into());
                playback_history_write_path = None;
            }
            entries
        }
        Err(error) => {
            app.set_status(format!("Could not load playback history: {error}").into());
            playback_history_write_path = None;
            prefs.playback_history.clone()
        }
    };
    let plugin_host = match crate::plugin_host::PluginHost::start(
        &prefs_path,
        &prefs.recent_media,
        &playback_history_entries,
    ) {
        Ok(host) => Some(host),
        Err(error) => {
            app.set_status(error.into());
            None
        }
    };
    let plugin_host_handle = plugin_host
        .as_ref()
        .map(crate::plugin_host::PluginHost::handle);
    #[cfg(target_os = "macos")]
    let mac_plugin_menu_observer = plugin_host_handle.as_ref().and_then(|host| {
        match platform::MacPluginMenuObserver::new(app, host.clone()) {
            Ok(observer) => Some(observer),
            Err(error) => {
                app.set_status(format!("Could not observe the Plugin menu: {error}").into());
                None
            }
        }
    });
    let menu_action_host = plugin_host_handle.clone();
    let menu_action_app = app.as_weak();
    app.on_activate_plugin_menu_item(move |token| {
        let Some((identifier, scope_and_id)) = token.as_str().split_once('\u{1f}') else {
            return;
        };
        let Some((scope, item_id)) = scope_and_id.split_once('\u{1f}') else {
            return;
        };
        let global = match scope {
            "global" => true,
            "player" => false,
            _ => return,
        };
        let Some(host) = &menu_action_host else {
            if let Some(app) = menu_action_app.upgrade() {
                app.set_status("The plugin runtime is unavailable.".into());
            }
            return;
        };
        if let Err(error) =
            host.activate_plugin_menu_item(identifier.into(), global, item_id.into())
            && let Some(app) = menu_action_app.upgrade()
        {
            app.set_status(format!("Could not run plugin menu action: {error}").into());
        }
    });
    let menu_update_host = plugin_host_handle.clone();
    let menu_update_app = app.as_weak();
    app.on_plugin_menu_will_open(move || {
        if let Some(host) = &menu_update_host
            && let Err(error) = host.notify_menu_update()
            && let Some(app) = menu_update_app.upgrade()
        {
            app.set_status(
                format!("Could not notify plugins that their menu is opening: {error}").into(),
            );
        }
    });
    let reload_plugins_host = plugin_host_handle.clone();
    let reload_plugins_app = app.as_weak();
    app.on_reload_plugins(move || {
        let Some(host) = &reload_plugins_host else {
            if let Some(app) = reload_plugins_app.upgrade() {
                app.set_status("The plugin runtime is unavailable.".into());
            }
            return;
        };
        if let Err(error) = host.reload_all()
            && let Some(app) = reload_plugins_app.upgrade()
        {
            app.set_status(format!("Could not reload plugins: {error}").into());
        }
    });
    let close_event_sent = Rc::new(Cell::new(false));
    let close_event_sent_once = close_event_sent.clone();
    let close_event_host = plugin_host_handle.clone();
    let weak = app.as_weak();
    app.on_window_will_close(move || {
        if close_event_sent_once.replace(true) {
            return;
        }
        if let Some(host) = &close_event_host
            && let Err(error) = host.notify_window_will_close()
            && let Some(app) = weak.upgrade()
        {
            app.set_status(format!("Could not notify plugins before closing: {error}").into());
        }
    });
    let plugin_window_host = native.as_ref().map(platform::NativePresentation::host);
    let plugin_command_sender = Arc::new(std::sync::Mutex::new(None::<riina_mpv::CommandSender>));
    #[cfg(target_os = "windows")]
    let plugin_overlay_presenter =
        Rc::new(RefCell::new(platform::PluginOverlayPresenter::default()));
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    let plugin_sidebar_presenter =
        Rc::new(RefCell::new(platform::PluginSidebarPresenter::default()));
    #[cfg(target_os = "windows")]
    let plugin_overlay_parent = native
        .as_ref()
        .map(platform::NativePresentation::webview_parent_window);
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    {
        let weak = app.as_weak();
        let presenter = plugin_sidebar_presenter.clone();
        app.on_select_plugin_sidebar_tab(move |identifier| {
            let Some(app) = weak.upgrade() else { return };
            let identifier = identifier.to_string();
            select_plugin_sidebar_tab(&app, &identifier);
            presenter.borrow_mut().select(Some(identifier));
        });
    }
    let plugin_osd_timer = plugin_host_handle
        .as_ref()
        .map(|_| Rc::new(slint::Timer::default()));
    let plugin_osd_timer_weak = plugin_osd_timer.as_ref().map(Rc::downgrade);
    let plugin_osd_generation = Rc::new(Cell::new(0_u64));
    let plugin_host_timer = plugin_host_handle.as_ref().map(|handle| {
        let timer = Rc::new(slint::Timer::default());
        let messages = handle.clone();
        #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
        let sidebar_host = handle.clone();
        let window_loaded_host = handle.clone();
        let window_loaded_sent = Rc::new(Cell::new(false));
        let window_messages = handle.clone();
        let window_host = plugin_window_host.clone();
        let last_window_properties =
            Rc::new(RefCell::new(None::<BTreeMap<String, serde_json::Value>>));
        let last_window_update = Rc::new(Cell::new(Instant::now()));
        let player_commands = plugin_command_sender.clone();
        let weak = app.as_weak();
        let osd_timer = plugin_osd_timer_weak.clone();
        let osd_generation = plugin_osd_generation.clone();
        #[cfg(target_os = "windows")]
        let overlay = plugin_overlay_presenter.clone();
        #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
        let sidebar_presenter = plugin_sidebar_presenter.clone();
        #[cfg(target_os = "windows")]
        let overlay_host = handle.clone();
        #[cfg(target_os = "windows")]
        let overlay_parent = plugin_overlay_parent.clone();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(25),
            move || {
                let Some(app) = weak.upgrade() else { return };
                #[cfg(target_os = "linux")]
                platform::pump_linux_webview_events();
                if !window_loaded_sent.get() && window_loaded_host.notify_window_loaded().is_ok() {
                    window_loaded_sent.set(true);
                }
                if last_window_update.get().elapsed() >= Duration::from_millis(125) {
                    last_window_update.set(Instant::now());
                    let properties = plugin_window_properties(&app, window_host.as_ref());
                    if last_window_properties.borrow().as_ref() != Some(&properties)
                        && window_messages
                            .update_window_properties(properties.clone())
                            .is_ok()
                    {
                        *last_window_properties.borrow_mut() = Some(properties);
                    }
                }
                let Ok(messages) = messages.drain_messages() else {
                    return;
                };
                for message in messages {
                    match message {
                        crate::plugin_host::PluginHostMessage::Log(text)
                        | crate::plugin_host::PluginHostMessage::Status(text) => {
                            app.set_status(text.into());
                        }
                        crate::plugin_host::PluginHostMessage::PlayerCommand(command) => {
                            let result = player_commands.lock().ok().and_then(|sender| {
                                sender.as_ref().map(|sender| sender.send(command))
                            });
                            if let Some(Err(error)) = result {
                                app.set_status(
                                    format!("Plugin player command failed: {error}").into(),
                                );
                            }
                        }
                        crate::plugin_host::PluginHostMessage::ObserveMpvProperty(property) => {
                            let result = player_commands.lock().ok().and_then(|sender| {
                                sender.as_ref().map(|sender| {
                                    sender.send(Command::ObserveMpvProperty(property))
                                })
                            });
                            if let Some(Err(error)) = result {
                                app.set_status(
                                    format!("Could not observe mpv plugin property: {error}")
                                        .into(),
                                );
                            }
                        }
                        crate::plugin_host::PluginHostMessage::SetUiVisibility(disabled) => {
                            app.set_plugin_ui_disabled(disabled);
                        }
                        crate::plugin_host::PluginHostMessage::WindowAction(action) => {
                            apply_plugin_window_action(&app, window_host.as_ref(), action);
                        }
                        crate::plugin_host::PluginHostMessage::ShowOsd(text) => {
                            let generation = osd_generation.get().wrapping_add(1);
                            osd_generation.set(generation);
                            app.set_plugin_osd_message(text.into());
                            if let Some(timer) =
                                osd_timer.as_ref().and_then(|timer| timer.upgrade())
                            {
                                let weak = app.as_weak();
                                let current = osd_generation.clone();
                                timer.start(
                                    slint::TimerMode::SingleShot,
                                    Duration::from_secs(3),
                                    move || {
                                        if current.get() == generation
                                            && let Some(app) = weak.upgrade()
                                        {
                                            app.set_plugin_osd_message("".into());
                                        }
                                    },
                                );
                            }
                        }
                        crate::plugin_host::PluginHostMessage::ShowAlert(title, text) => {
                            rfd::MessageDialog::new()
                                .set_title(title)
                                .set_description(text)
                                .set_buttons(rfd::MessageButtons::Ok)
                                .show();
                        }
                        crate::plugin_host::PluginHostMessage::SidebarTabAdded {
                            plugin_identifier,
                            label,
                        } => {
                            plugin_sidebar_tab_added(&app, &plugin_identifier, &label);
                        }
                        crate::plugin_host::PluginHostMessage::PluginMenuUpdated {
                            plugin_identifier,
                            plugin_name,
                            global,
                            items,
                        } => replace_plugin_menu_rows(
                            &app,
                            &plugin_identifier,
                            &plugin_name,
                            global,
                            &items,
                        ),
                        crate::plugin_host::PluginHostMessage::RemovePluginMenu(identifier) => {
                            remove_plugin_menu_rows(&app, &identifier);
                        }
                        #[cfg(target_os = "windows")]
                        message @ crate::plugin_host::PluginHostMessage::Overlay { .. } => {
                            let result = if let Some(parent) = overlay_parent.as_ref() {
                                overlay.borrow_mut().apply_host_message(
                                    parent,
                                    &overlay_host,
                                    message,
                                )
                            } else {
                                Err("Plugin video overlays require native video presentation"
                                    .into())
                            };
                            if let Err(error) = result {
                                app.set_status(error.into());
                            }
                        }
                        #[cfg(target_os = "windows")]
                        crate::plugin_host::PluginHostMessage::RemoveOverlay(identifier) => {
                            overlay.borrow_mut().remove(&identifier);
                        }
                        #[cfg(any(
                            target_os = "windows",
                            target_os = "macos",
                            target_os = "linux"
                        ))]
                        message @ crate::plugin_host::PluginHostMessage::Sidebar { .. } => {
                            if let crate::plugin_host::PluginHostMessage::Sidebar {
                                plugin_identifier,
                                effect,
                                ..
                            } = &message
                            {
                                match effect {
                                    riina_core::plugin_sidebar::PluginSidebarEffect::Show => {
                                        select_plugin_sidebar_tab(&app, plugin_identifier);
                                    }
                                    riina_core::plugin_sidebar::PluginSidebarEffect::Hide => {
                                        if app.get_sidebar() == 7 {
                                            app.set_sidebar(0);
                                        }
                                        if app.get_leading_sidebar() == 7 {
                                            app.set_leading_sidebar(0);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            let result = app.window().with_winit_window(|parent| {
                                sidebar_presenter.borrow_mut().apply_host_message(
                                    parent,
                                    &sidebar_host,
                                    message,
                                )
                            });
                            if let Some(Err(error)) = result {
                                app.set_status(error.into());
                            }
                        }
                        #[cfg(not(target_os = "windows"))]
                        crate::plugin_host::PluginHostMessage::Overlay { .. } => {
                            app.set_status(
                                "Plugin video overlays are unavailable on this platform".into(),
                            );
                        }
                        #[cfg(not(target_os = "windows"))]
                        crate::plugin_host::PluginHostMessage::RemoveOverlay(_) => {}
                        #[cfg(any(
                            target_os = "windows",
                            target_os = "macos",
                            target_os = "linux"
                        ))]
                        crate::plugin_host::PluginHostMessage::RemoveSidebar(identifier) => {
                            sidebar_presenter.borrow_mut().remove(&identifier);
                            plugin_sidebar_tab_removed(&app, &identifier);
                        }
                    }
                }
                #[cfg(target_os = "windows")]
                overlay.borrow_mut().update(
                    app.get_window_width(),
                    app.get_window_height(),
                    f64::from(app.window().scale_factor()),
                );
                #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
                {
                    let selected = if app.get_sidebar() == 7 || app.get_leading_sidebar() == 7 {
                        let identifier = app.get_plugin_sidebar_selected_identifier();
                        (!identifier.is_empty()).then(|| identifier.to_string())
                    } else {
                        None
                    };
                    sidebar_presenter.borrow_mut().update(
                        app.get_plugin_sidebar_view_x(),
                        app.get_plugin_sidebar_view_y(),
                        app.get_plugin_sidebar_view_width(),
                        app.get_plugin_sidebar_view_height(),
                        selected,
                    );
                }
            },
        );
        timer
    });
    app.set_live_text_enabled(prefs.live_text_enabled);
    app.set_live_text_regions(slint::ModelRc::new(slint::VecModel::from(Vec::new())));
    app.set_live_text_available(false);
    #[cfg(target_os = "macos")]
    app.set_live_text_selectable(true);
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        let weak = app.as_weak();
        let _ = platform::text_recognition::check_availability(move |available| {
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(app) = weak.upgrade() {
                    app.set_live_text_available(available);
                    if !available {
                        app.set_live_text_enabled(false);
                    }
                }
            });
        });
    }
    app.set_hdr_enabled(prefs.hdr_enabled);
    app.set_default_hdr_mode_enabled(prefs.hdr_enabled);
    app.set_load_icc_profile(prefs.load_icc_profile);
    app.set_hdr_display_active(
        native
            .as_ref()
            .is_some_and(|presentation| presentation.hdr_display_active()),
    );
    let hdr_display_timer = native.as_ref().map(|presentation| {
        let timer = Rc::new(slint::Timer::default());
        let display = presentation.clone();
        let weak = app.as_weak();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_secs(2),
            move || {
                if let Some(app) = weak.upgrade() {
                    app.set_hdr_display_active(display.hdr_display_active());
                }
            },
        );
        timer
    });
    let recent_media = Rc::new(RefCell::new(prefs.recent_media.clone()));
    let recent_open_documents = Rc::new(RefCell::new(prefs.recent_open_documents.clone()));
    let recent_open_cleared = Rc::new(Cell::new(false));
    let plugin_recent_media = Arc::new(std::sync::Mutex::new(prefs.recent_media.clone()));
    let playback_history_session_id = crate::plugin_host::next_history_session_id();
    let playback_history = crate::plugin_host::shared_playback_history_state(
        playback_history_path,
        playback_history_entries.clone(),
        playback_history_write_path,
        Some(prefs_path.with_file_name("watch-later")),
    );
    let plugin_playback_history = playback_history.clone();
    let record_playback_history = Arc::new(AtomicBool::new(prefs.record_playback_history));
    let notify_record_playback_history = record_playback_history.clone();
    let history_cleared = crate::plugin_host::shared_history_merge_flag(&prefs_path);
    let watch_later_directory = prefs_path.with_file_name("watch-later");
    std::fs::create_dir_all(&watch_later_directory)?;
    let ytdl_plugin_active = crate::plugin_host::is_enabled_plugin(&prefs_path, "io.iina.ytdl");
    app.set_network_ytdl_plugin_active(ytdl_plugin_active);
    let mut network_online_videos = prefs.network_online_videos.clone();
    if ytdl_plugin_active {
        network_online_videos.enabled = false;
    }
    let mut engine_options = resume_engine_options(
        &watch_later_directory,
        prefs.resume_last_position,
        &prefs.subtitle_auto_load,
        &prefs.network,
        &prefs.network_cache,
        &network_online_videos,
    )
    .map_err(std::io::Error::other)?;
    engine_options.push((
        "keep-open".into(),
        riina_core::playlist_end_policy(prefs.keep_open_on_file_end, prefs.playlist_auto_play_next)
            .into(),
    ));
    engine_options.extend(
        prefs
            .audio
            .engine_options()
            .map_err(std::io::Error::other)?,
    );
    engine_options.push(("vd-lavc-threads".into(), prefs.video_threads.to_string()));
    engine_options.extend(
        prefs
            .hdr_tone_mapping
            .engine_options()
            .map_err(std::io::Error::other)?,
    );
    if !prefs.hdr_enabled {
        engine_options.extend(riina_core::hdr_preferences::disabled_output_options());
    }
    #[cfg(target_os = "macos")]
    {
        engine_options.retain(|(name, _)| name != "icc-profile-auto");
        engine_options.push(("icc-profile-auto".into(), "no".into()));
    }
    engine_options.extend(
        crate::advanced_preferences::engine_options(&prefs.advanced, &prefs_path)
            .map_err(std::io::Error::other)?,
    );
    app.set_volume_boost(prefs.volume_boost);
    app.set_auto_switch_to_music_mode(prefs.auto_switch_to_music_mode);
    app.set_disable_animations(prefs.disable_animations);
    app.set_system_reduce_motion(crate::platform::system_reduce_motion_enabled());
    app.set_platform_macos(cfg!(target_os = "macos"));
    app.set_platform_windows(cfg!(target_os = "windows"));
    #[cfg(target_os = "windows")]
    {
        app.set_system_materials_enabled(crate::platform::native_materials_enabled());
        if let Some(accent) = crate::platform::windows_accent_color() {
            app.set_windows_accent_color(accent);
        }
        if let Some(foreground) = crate::platform::windows_accent_foreground() {
            app.set_windows_accent_foreground(foreground);
        }
    }
    crate::appearance_preferences::install_system_typography(app);
    app.set_music_mode_show_playlist(prefs.music_mode_show_playlist);
    app.set_music_mode_show_album_art(prefs.music_mode_show_album_art);
    let latest_playback = Arc::new(std::sync::Mutex::new(Playback::default()));
    crate::about_window::connect(app, latest_playback.clone());
    let pause_resume_timer = install_pause_on_minimize(app, latest_playback.clone());
    crate::control_preferences::install_player(app, &prefs.controls);
    crate::key_binding_preferences::install_player(
        app,
        &prefs.key_bindings,
        latest_playback.clone(),
    );
    #[cfg(target_os = "macos")]
    let system_media_sender = Rc::new(RefCell::new(None::<riina_mpv::CommandSender>));
    #[cfg(target_os = "macos")]
    let (system_media_controls, system_media_controls_timer) = {
        let controls = Rc::new(RefCell::new(platform::SystemMediaControls::new(
            platform::playback_library(),
        )));
        let timer = Rc::new(slint::Timer::default());
        let controls_tick = controls.clone();
        let weak = app.as_weak();
        let playback = latest_playback.clone();
        let player = system_media_sender.clone();
        let mut skip_intervals = None::<(String, Option<f64>, Option<f64>)>;
        let mut paused_by_inactive = None;
        let mut application_active = true;
        let mut player_window_was_focused = app
            .window()
            .with_winit_window(|window| window.has_focus())
            .unwrap_or(false);
        let mut refresh_skip_intervals_at = Instant::now();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(50),
            move || {
                let Some(app) = weak.upgrade() else { return };
                let profile = app.get_key_binding_current_profile().to_string();
                let active = app.get_loaded() && app.get_key_binding_use_system_media_controls();
                if active
                    && (skip_intervals
                        .as_ref()
                        .is_none_or(|(cached_profile, _, _)| cached_profile != &profile)
                        || Instant::now() >= refresh_skip_intervals_at)
                {
                    let (forward, backward) =
                        crate::key_binding_preferences::system_skip_intervals_for_profile(&profile);
                    skip_intervals = Some((profile.clone(), forward, backward));
                    refresh_skip_intervals_at = Instant::now() + Duration::from_secs(1);
                }
                let (forward, backward) = if active {
                    let (_, forward, backward) = skip_intervals
                        .as_ref()
                        .expect("the active key profile's skip intervals are initialized");
                    (*forward, *backward)
                } else {
                    (None, None)
                };
                controls_tick
                    .borrow_mut()
                    .synchronize(active, &profile, forward, backward);
                let current = playback.lock().ok().map(|state| state.clone());
                if active && let Some(current) = current.as_ref() {
                    controls_tick
                        .borrow_mut()
                        .publish_playback(current, app.get_thumbnail_preview_enabled());
                }
                let actions = controls_tick.borrow().drain_actions().collect::<Vec<_>>();
                for action in actions {
                    if let Some(player) = player.borrow().as_ref() {
                        dispatch_system_media_action(
                            &app,
                            action,
                            &playback,
                            player,
                            &mut paused_by_inactive,
                            &mut application_active,
                            player_window_was_focused,
                        );
                    }
                }
                let focused = app
                    .window()
                    .with_winit_window(|window| window.has_focus())
                    .unwrap_or(false);
                if app.get_pause_when_inactive() {
                    if application_active
                        && !focused
                        && some_player_window_is_focused()
                        && paused_by_inactive.is_none()
                    {
                        let state = playback.lock().ok().map(|state| state.clone());
                        if let Some(state) =
                            state.filter(|state| state.phase == Phase::Playing && !state.paused)
                        {
                            paused_by_inactive = Some(state.generation);
                            app.invoke_set_paused(true);
                        }
                    } else if application_active
                        && focused
                        && let Some(generation) = paused_by_inactive
                        && playback.lock().is_ok_and(|state| {
                            state.generation == generation
                                && state.phase == Phase::Playing
                                && state.paused
                        })
                    {
                        app.invoke_set_paused(false);
                        paused_by_inactive = None;
                    }
                }
                player_window_was_focused = focused;
            },
        );
        (controls, timer)
    };
    app.set_hardware_decoder_index(crate::codec_preferences::index(prefs.hardware_decoder));
    app.set_subtitle_ass_override_index(crate::subtitle_preferences::index(
        prefs.subtitle_ass_override,
    ));
    app.set_secondary_subtitle_ass_override_index(crate::subtitle_preferences::index(
        prefs.secondary_subtitle_ass_override,
    ));
    crate::subtitle_position_preferences::install_player(app, prefs.subtitle_position);
    crate::subtitle_other_preferences::install_player(app, &prefs.subtitle_other);
    crate::subtitle_auto_load_preferences::install_player(app, &prefs.subtitle_auto_load);
    crate::online_subtitles_preferences::install_player(app, &prefs.online_subtitles);
    crate::network_cache_preferences::install_player(app, &prefs.network_cache);
    crate::network_ytdl_preferences::install_player(app, &prefs.network_online_videos);
    app.set_subtitle_font(prefs.subtitle_font.clone().into());
    app.set_subtitle_font_size_preference(prefs.subtitle_font_size as f32);
    crate::subtitle_preferences::install_style(app, prefs.subtitle_text_style);
    if let Some(welcome) = welcome {
        welcome.set_app_version(env!("CARGO_PKG_VERSION").into());
        refresh_welcome_history(welcome, &prefs.recent_media, prefs.resume_last_position);
    }
    app.set_video_aspect_presets(slint::ModelRc::new(slint::VecModel::from(
        riina_core::video_controls::ASPECT_PRESETS
            .map(slint::SharedString::from)
            .to_vec(),
    )));
    app.set_osd_enabled(prefs.enable_osd);
    app.set_disable_osd_file_start(prefs.disable_osd_file_start);
    app.set_disable_osd_pause_resume(prefs.disable_osd_pause_resume);
    app.set_disable_osd_speed(prefs.disable_osd_speed);
    app.set_disable_osd_seek(prefs.disable_osd_seek);
    app.set_osd_timeout_secs(prefs.osd_timeout_secs as f32);
    app.set_osd_text_size(prefs.osd_text_size as f32);
    app.set_display_time_and_battery_in_fullscreen(prefs.display_time_and_battery_in_fullscreen);
    crate::osc_preferences::install_player(app, prefs.control_bar);
    crate::screenshot_preferences::install(app, &prefs.screenshot);
    crate::thumbnail_preferences::install_player(app, &prefs.thumbnail_preview);
    app.set_pause_when_open(prefs.pause_when_open);
    app.set_play_when_entering_fullscreen(prefs.play_when_entering_fullscreen);
    app.set_pause_when_leaving_fullscreen(prefs.pause_when_leaving_fullscreen);
    app.set_pause_when_inactive(prefs.pause_when_inactive);
    app.set_pause_when_minimized(prefs.pause_when_minimized);
    app.set_pause_when_goes_to_sleep(prefs.pause_when_goes_to_sleep);
    app.set_quit_when_no_opened_window(prefs.quit_when_no_opened_window);
    app.set_keep_open_on_file_end(prefs.keep_open_on_file_end);
    app.set_playlist_auto_play_next(prefs.playlist_auto_play_next);
    app.set_playlist_auto_add(prefs.playlist_auto_add);
    app.set_playlist_show_metadata(prefs.playlist_show_metadata);
    app.set_playlist_show_metadata_in_music_mode(prefs.playlist_show_metadata_in_music_mode);
    app.set_auto_repeat(prefs.auto_repeat);
    app.set_default_repeat_mode_index(prefs.default_repeat_mode.index());
    app.set_fullscreen_when_open(prefs.fullscreen_when_open);
    app.set_resume_last_position(prefs.resume_last_position);
    app.set_record_playback_history(prefs.record_playback_history);
    app.set_record_recent_files(prefs.record_recent_files);
    app.set_track_all_files_in_recent_open_menu(prefs.track_all_files_in_recent_open_menu);
    app.set_recent_open_menu_rows(slint::ModelRc::new(slint::VecModel::from(
        recent_open_menu_rows(&recent_open_documents.borrow()),
    )));
    app.set_launch_action_index(prefs.action_after_launch.index());
    app.set_leading_sidebar_width(prefs.leading_sidebar_width as f32);
    app.set_trailing_sidebar_width(prefs.trailing_sidebar_width as f32);
    app.set_settings_at_leading(prefs.settings_at_leading);
    app.set_playlist_at_leading(prefs.playlist_at_leading);
    app.set_plugins_at_leading(prefs.plugins_at_leading);
    app.set_edge_to_edge_video(prefs.edge_to_edge_video);
    app.set_docked_controls(prefs.docked_controls);
    app.set_unlock_window_aspect_ratio(prefs.unlock_window_aspect_ratio);
    crate::window_resize_preferences::install_player(app, prefs.window_resize);
    app.set_compact_ui(prefs.compact_ui);
    app.set_always_float_on_top(prefs.always_float_on_top);
    app.set_always_show_on_top_icon(prefs.always_show_on_top_icon);
    crate::liquid_glass_preferences::install_player(app, prefs.liquid_glass);
    let appearance = prefs.effective_theme_material();
    app.set_appearance_mode(appearance.index());
    app.set_surface_material_title(crate::platform::surface_material_title().into());
    app.set_dark_theme(appearance.dark_for(prefs.dark));
    let appearance_timer = crate::appearance_preferences::connect(app);
    app.set_osc_position(i32::from(prefs.osc_position));
    app.set_control_bar_position_horizontal(prefs.control_bar_position_horizontal as f32);
    app.set_control_bar_position_vertical(prefs.control_bar_position_vertical as f32);
    app.set_show_chapter_pos(prefs.show_chapter_pos);
    crate::pip_preferences::install_player(app, prefs.picture_in_picture);
    crate::sidebar_preferences::connect(app, prefs_path.clone(), prefs.clone());
    crate::layout_preferences::connect(app, prefs_path.clone(), prefs.clone());
    refresh_video_menu(app);
    app.set_video_crop_presets(slint::ModelRc::new(slint::VecModel::from(
        riina_core::crop::LABELS
            .map(slint::SharedString::from)
            .to_vec(),
    )));
    app.set_primary_shortcut_is_meta(cfg!(target_os = "macos"));
    app.set_current_file_manager_name(
        if cfg!(target_os = "macos") {
            "Finder"
        } else if cfg!(target_os = "windows") {
            "Explorer"
        } else {
            "File Manager"
        }
        .into(),
    );
    let app_weak = app.as_weak();
    // Only one pending state snapshot is retained; a decoder can produce updates
    // faster than the UI consumes them without growing the application event queue.
    // State and capture completions share one handoff. A newer completion must
    // never be cleaned up while an older snapshot is still being rendered.
    let latest = Arc::new(std::sync::Mutex::new((
        None,
        crate::screenshot_output::Completions::default(),
    )));
    let transition_overlay = Arc::new(AtomicU64::new(u64::MAX));
    let transition_release_generation = Arc::new(AtomicU64::new(u64::MAX));
    let notify_transition_overlay = transition_overlay.clone();
    let notify_transition_release_generation = transition_release_generation.clone();
    let observed_playback = latest_playback.clone();
    let music_mode_state = Arc::new(std::sync::Mutex::new(riina_core::music_mode::State::new(
        prefs.auto_switch_to_music_mode,
    )));
    if native.is_none() {
        let original_size = Rc::new(Cell::new(None::<slint::PhysicalSize>));
        let saved_size = original_size.clone();
        let weak = app.as_weak();
        app.on_music_mode_changed(move || {
            if let Some(app) = weak.upgrade() {
                let window = app.window();
                if app.get_music_mode() {
                    if saved_size.get().is_none() {
                        saved_size.set(Some(window.size()));
                    }
                    let scale = f64::from(window.scale_factor());
                    let height = crate::music_mode_layout::window_height(
                        app.get_music_mode_show_album_art(),
                        app.get_music_mode_show_playlist(),
                        f64::from(app.get_music_mode_artwork_aspect_ratio()),
                    );
                    let target = slint::PhysicalSize::new(
                        (crate::music_mode_layout::WINDOW_WIDTH * scale).round() as u32,
                        (height * scale).round() as u32,
                    );
                    crate::window_aspect::resize_centered(
                        window,
                        riina_core::window_aspect::PixelSize {
                            width: window.size().width,
                            height: window.size().height,
                        },
                        riina_core::window_aspect::PixelSize {
                            width: target.width,
                            height: target.height,
                        },
                        None,
                    );
                } else if let Some(size) = saved_size.take() {
                    crate::window_aspect::resize_centered(
                        window,
                        riina_core::window_aspect::PixelSize {
                            width: window.size().width,
                            height: window.size().height,
                        },
                        riina_core::window_aspect::PixelSize {
                            width: size.width,
                            height: size.height,
                        },
                        None,
                    );
                }
            }
        });
    }
    let queued = Arc::new(AtomicBool::new(false));
    let displayed = Arc::new(std::sync::Mutex::new(None));
    let crop_editor = crate::crop_editor::Editor::default();
    let observed_crop_editor = crop_editor.clone();
    let last_seek = Arc::new(std::sync::Mutex::new(SeekOsd::default()));
    let mut video_osd =
        crate::video_osd::VideoOsd::new(prefs.disable_osd_speed, prefs.disable_osd_pause_resume);
    video_osd.set_volume_limit(prefs.volume_limit());
    let last_video = Arc::new(std::sync::Mutex::new(video_osd));
    let disable_video_osd = last_video.clone();
    let pending_video_osd = last_video.clone();
    let last_screenshot = Arc::new(std::sync::Mutex::new(
        crate::screenshot_osd::ScreenshotOsd::default(),
    ));
    let screenshot_scratch = Arc::new(crate::screenshot_output::Scratch::default());
    let opening_fullscreen = Arc::new(std::sync::Mutex::new(
        crate::opening_fullscreen::OpeningFullscreen::default(),
    ));
    #[cfg(target_os = "macos")]
    let picture_in_picture = Rc::new(RefCell::new(None::<platform::MacPictureInPicture>));
    #[cfg(target_os = "macos")]
    let picture_in_picture_state_dirty = Arc::new(AtomicBool::new(false));
    let notify_music_mode_state = music_mode_state.clone();
    let worker_scratch = screenshot_scratch.clone();
    let notify_plugin_host = plugin_host_handle.clone();
    let notify_plugin_recent_media = plugin_recent_media.clone();
    let notify_video_filter_settings = saved_video_filters.clone();
    let notify_audio_filter_settings = saved_audio_filters.clone();
    let notify_audio_device_sender = plugin_command_sender.clone();
    let notified_plugin_file_context = Arc::new(std::sync::Mutex::new(
        crate::plugin_files::PluginPlayerFileContext::default(),
    ));
    let last_notified_plugin_file_context = notified_plugin_file_context.clone();
    #[cfg(target_os = "macos")]
    let notify_picture_in_picture_state_dirty = picture_in_picture_state_dirty.clone();
    let notify = move |state: Playback| {
        // This closure remains owned by the engine until native destruction.
        let _retain_until_native_teardown = &worker_scratch;
        if let Ok(mut latest_playback) = observed_playback.lock() {
            *latest_playback = state.clone();
        }
        let plugin_context = crate::plugin_host::player_file_context_with_history(
            &state,
            &mut notify_plugin_recent_media
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            &mut plugin_playback_history
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            notify_record_playback_history.load(Ordering::Acquire),
            playback_history_session_id,
        );
        if let Some(error) = plugin_playback_history
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take_persistence_error()
        {
            let weak = app_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(app) = weak.upgrade() {
                    app.set_status(format!("Could not save playback history: {error}").into());
                }
            });
        }
        if let Ok(mut last_context) = last_notified_plugin_file_context.lock()
            && *last_context != plugin_context
            && let Some(host) = &notify_plugin_host
            && host
                .update_player_file_context(plugin_context.clone())
                .is_ok()
        {
            *last_context = plugin_context;
        }
        #[cfg(target_os = "macos")]
        notify_picture_in_picture_state_dirty.store(true, Ordering::Release);
        if let Ok(mut latest) = latest.lock() {
            latest.1.observe(&state);
            latest.0 = Some(state);
        }
        if queued.swap(true, Ordering::AcqRel) {
            return;
        }
        let latest = latest.clone();
        let queued = queued.clone();
        let weak = app_weak.clone();
        let displayed = displayed.clone();
        let last_seek = last_seek.clone();
        let last_video = last_video.clone();
        let last_screenshot = last_screenshot.clone();
        let crop_editor = observed_crop_editor.clone();
        let notify_saved_video_filters = notify_video_filter_settings.clone();
        let notify_saved_audio_filters = notify_audio_filter_settings.clone();
        let audio_device_sender = notify_audio_device_sender.clone();
        let opening_fullscreen = opening_fullscreen.clone();
        let music_mode_state = notify_music_mode_state.clone();
        let transition_overlay = notify_transition_overlay.clone();
        let transition_release_generation = notify_transition_release_generation.clone();
        let _ = slint::invoke_from_event_loop(move || {
            queued.store(false, Ordering::Release);
            if let (Some(app), Ok(mut latest)) = (weak.upgrade(), latest.lock())
                && let Some(state) = latest.0.take()
                && let Ok(mut displayed) = displayed.lock()
                && let Ok(mut last_seek) = last_seek.lock()
                && let Ok(mut last_video) = last_video.lock()
                && let Ok(mut last_screenshot) = last_screenshot.lock()
            {
                let first_loaded = (!app.get_loaded()
                    || app.get_window_geometry_source().parse::<u64>().ok()
                        != Some(state.generation))
                    && matches!(
                        state.phase,
                        Phase::Playing | Phase::Paused | Phase::Buffering | Phase::Ended
                    );
                let audio_status = state.audio_status();
                let captures = latest.1.take();
                let enter_fullscreen = opening_fullscreen.lock().is_ok_and(|mut opening| {
                    opening.observe(
                        &state,
                        app.get_fullscreen_when_open(),
                        app.get_fullscreen(),
                        false,
                    )
                });
                let clipboard_error = crate::screenshot_output::copy(&app, &captures);
                crop_editor.observe(&app, &state);
                let audio_device_sender = audio_device_sender
                    .lock()
                    .ok()
                    .and_then(|sender| sender.clone());
                let resize_for_media = update_ui(
                    &app,
                    state,
                    UiUpdateResources {
                        audio_device_player: audio_device_sender.as_ref(),
                        hdr_player: audio_device_sender.as_ref(),
                        saved_video_filters: &notify_saved_video_filters,
                        saved_audio_filters: &notify_saved_audio_filters,
                        transition_overlay: transition_overlay.as_ref(),
                        transition_release_generation: transition_release_generation.as_ref(),
                    },
                    &mut displayed,
                    &mut last_seek,
                    &mut last_video,
                    &mut last_screenshot,
                );
                if let Ok(mut mode) = music_mode_state.lock()
                    && let Some(active) = mode.observe(audio_status, app.get_fullscreen())
                {
                    app.set_music_mode(active);
                }
                if resize_for_media {
                    // Apply the complete geometry after automatic music mode has
                    // restored the video window. Coalesced snapshots may omit
                    // Loading, so media identity owns this transition.
                    app.invoke_resize_window_for_media();
                }
                if first_loaded {
                    app.invoke_initial_media_loaded();
                }
                crate::screenshot_output::cleanup(&app, &captures);
                if enter_fullscreen {
                    app.invoke_toggle_fullscreen();
                }
                if let Some(error) = clipboard_error {
                    app.set_status(format!("Could not copy screenshot: {error}").into());
                }
            }
        });
    };
    let player = if let Some(presentation) = &native {
        presentation.start_engine(engine_options, notify)
    } else {
        Player::start_with_options(&platform::playback_library(), false, engine_options, notify)
    };
    let player = match player {
        Ok(player) => player,
        Err(error) => {
            app.set_status(error.clone().into());
            return Err(std::io::Error::other(error).into());
        }
    };
    let history_plugin = plugin_host_handle.clone();
    let history_latest_playback = latest_playback.clone();
    let history_recent_media = plugin_recent_media.clone();
    let history_recording = record_playback_history.clone();
    let history_sync: crate::playback_history_window::HistorySync = Arc::new(move |history| {
        let Some(plugin) = &history_plugin else {
            return Ok(());
        };
        let playback = history_latest_playback
            .lock()
            .map_err(|_| "latest playback lock poisoned".to_owned())?
            .clone();
        let context = crate::plugin_host::player_file_context_with_history(
            &playback,
            &mut history_recent_media
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            history,
            history_recording.load(Ordering::Acquire),
            playback_history_session_id,
        );
        plugin.update_player_file_context(context)
    });
    let (playback_history_window, playback_history_timer) = crate::playback_history_window::install(
        app,
        playback_history.clone(),
        player.command_sender(),
        prefs_path.clone(),
        history_cleared.clone(),
        history_sync,
    );
    let ((video_filters_window, video_filters_timer), (audio_filters_window, audio_filters_timer)) =
        crate::video_filter_window::install(
            app,
            latest_playback.clone(),
            saved_video_filters.clone(),
            saved_audio_filters.clone(),
            player.command_sender(),
            prefs_path.clone(),
        );
    let playlist_metadata_timer = install_playlist_metadata_refresh(app, latest_playback.clone());
    if let Ok(mut sender) = plugin_command_sender.lock() {
        *sender = Some(player.command_sender());
    }
    let subtitle_priority_timer = Rc::new(slint::Timer::default());
    let subtitle_discovery_ready = Arc::new(AtomicU64::new(u64::MAX));
    let (online_subtitles_window, online_subtitles_timer) = crate::open_subtitles::connect_player(
        app,
        player.command_sender(),
        latest_playback.clone(),
        subtitle_discovery_ready.clone(),
    );
    let priority_player = player.command_sender();
    let priority_playback = latest_playback.clone();
    let priority_state = Rc::new(RefCell::new(
        crate::subtitle_priority::AutoPriority::default(),
    ));
    let subtitle_discovery = Rc::new(RefCell::new(
        crate::subtitle_discovery::AutoDiscovery::default(),
    ));
    let auto_folder_discovery = Rc::new(RefCell::new(
        crate::auto_playlist::AutoFolderAdder::default(),
    ));
    let auto_folder_state = auto_folder_discovery.clone();
    let auto_folder_player = player.command_sender();
    let auto_folder_playback = latest_playback.clone();
    let subtitle_discovery_state = subtitle_discovery.clone();
    let subtitle_discovery_player = player.command_sender();
    let subtitle_discovery_playback = latest_playback.clone();
    let subtitle_discovery_ready_controller = subtitle_discovery_ready.clone();
    let priority_controller = priority_state.clone();
    let priority_app = app.as_weak();
    subtitle_priority_timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(120),
        move || {
            let Some(app) = priority_app.upgrade() else {
                return;
            };
            let Ok(preferences) = crate::subtitle_auto_load_preferences::current_player(&app)
            else {
                return;
            };
            let Ok(state) = priority_playback.lock().map(|state| state.clone()) else {
                return;
            };
            if preferences.mode == riina_core::subtitle_controls::AutoLoadMode::Disabled {
                subtitle_discovery_ready_controller.store(state.generation, Ordering::Release);
            }
            if let Some(request) = auto_folder_state
                .borrow_mut()
                .observe(&state, app.get_playlist_auto_add())
            {
                let snapshot = state.clone();
                let request_check = request.clone();
                let playback = auto_folder_playback.clone();
                let sender = auto_folder_player.clone();
                let weak_app = app.as_weak();
                std::thread::spawn(move || {
                    let Some(command) = crate::auto_playlist::prepare(&request, &snapshot) else {
                        return;
                    };
                    let still_current = playback.lock().ok().is_some_and(|state| {
                        crate::auto_playlist::request_is_current(&request_check, &state)
                    });
                    if still_current && let Err(error) = sender.send(command) {
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(app) = weak_app.upgrade() {
                                app.set_status(error.into());
                            }
                        });
                    }
                });
            }
            if let Some((generation, media_path)) = subtitle_discovery_state
                .borrow_mut()
                .observe(&state, &preferences)
            {
                subtitle_discovery_ready_controller.store(u64::MAX, Ordering::Release);
                let preferences = preferences.clone();
                let playback = subtitle_discovery_playback.clone();
                let sender = subtitle_discovery_player.clone();
                let weak_app = app.as_weak();
                let ready = subtitle_discovery_ready_controller.clone();
                std::thread::spawn(move || {
                    let files =
                        crate::subtitle_discovery::find_matching_files(&media_path, &preferences);
                    let expected_path = media_path.to_string_lossy().into_owned();
                    let current = playback.lock().ok().is_some_and(|state| {
                        state.generation == generation
                            && matches!(
                                state.phase,
                                Phase::Playing | Phase::Paused | Phase::Buffering
                            )
                            && state
                                .playlist
                                .iter()
                                .find(|entry| entry.current)
                                .is_some_and(|entry| entry.filename == expected_path)
                    });
                    if current
                        && !files.is_empty()
                        && let Err(error) = sender.send(Command::AddSubtitlesWithoutSelection(
                            files
                                .into_iter()
                                .map(|path| path.to_string_lossy().into_owned())
                                .collect(),
                        ))
                    {
                        let weak_app = weak_app.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(app) = weak_app.upgrade() {
                                app.set_status(error.into());
                            }
                        });
                    }
                    ready.store(generation, Ordering::Release);
                });
            }
            let selected = priority_controller
                .borrow_mut()
                .observe(&state, &preferences);
            if let Some(id) = selected
                && let Err(error) = priority_player.send(Command::SelectTrack {
                    kind: TrackKind::Sub,
                    id: Some(id),
                })
            {
                app.set_status(error.into());
            }
        },
    );
    #[cfg(target_os = "macos")]
    let picture_in_picture = {
        let manager = objc2::MainThreadMarker::new()
            .ok_or_else(|| std::io::Error::other("PiP setup must run on the macOS main thread"))
            .and_then(|mtm| {
                platform::MacPictureInPicture::new(
                    player.command_sender(),
                    latest_playback.clone(),
                    mtm,
                )
                .map_err(std::io::Error::other)
            });
        let manager = match manager {
            Ok(manager) => Some(manager),
            Err(error) => {
                app.set_status(format!("macOS PiP unavailable: {error}").into());
                None
            }
        };
        *picture_in_picture.borrow_mut() = manager;
        let weak_app = app.as_weak();
        let toggle_manager = picture_in_picture.clone();
        app.on_toggle_picture_in_picture(move || {
            let result = toggle_manager
                .borrow_mut()
                .as_mut()
                .ok_or_else(|| "macOS PiP is unavailable on this system".to_owned())
                .and_then(platform::MacPictureInPicture::toggle);
            if let Err(error) = result
                && let Some(app) = weak_app.upgrade()
            {
                app.set_status(error.into());
            }
        });
        picture_in_picture.clone()
    };
    #[cfg(target_os = "macos")]
    let picture_in_picture_timer = {
        let manager = picture_in_picture.clone();
        let dirty = picture_in_picture_state_dirty.clone();
        let weak_app = app.as_weak();
        let timer = slint::Timer::default();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(100),
            move || {
                let mut active = false;
                if let Some(manager) = manager.borrow_mut().as_mut() {
                    active = manager.is_active();
                    if dirty.swap(false, Ordering::AcqRel) {
                        manager.synchronize_playback();
                    }
                }
                if let Some(app) = weak_app.upgrade()
                    && app.get_picture_in_picture_active() != active
                {
                    app.set_picture_in_picture_active(active);
                }
            },
        );
        Rc::new(timer)
    };
    let scroll_input = Rc::new(Cell::new(ScrollInputState::default()));
    #[cfg(target_os = "macos")]
    let mac_scroll_event_monitor = {
        let native_input = scroll_input.clone();
        match platform::MacScrollEventMonitor::new(move |event| {
            let mut input = native_input.get();
            input.appkit = true;
            input.precise = Some(event.precise);
            input.natural_direction = Some(event.natural_direction);
            input.phase_empty = event.phase_empty;
            input.phase_began = event.phase_began;
            input.phase_ended = event.phase_ended;
            if !event.momentum_phase_empty && !input.momentum_allowed {
                input.suppress_current = true;
                native_input.set(input);
                return;
            }
            input.suppress_current = false;
            input.momentum_allowed = event.phase_ended || input.mouse_in_window;
            if event.phase_empty || event.phase_began {
                input.axis = if event.delta_x != 0.0 {
                    Some(true)
                } else if event.delta_y != 0.0 {
                    Some(false)
                } else {
                    None
                };
            } else if event.phase_ended {
                input.axis = None;
            }
            native_input.set(input);
        }) {
            Ok(monitor) => Some(monitor),
            Err(error) => {
                app.set_status(format!("Native trackpad input unavailable: {error}").into());
                None
            }
        }
    };
    let trackpad_pause_generation = Rc::new(Cell::new(None));
    #[cfg(target_os = "macos")]
    install_macos_control_gestures(app, player.command_sender());
    let (file_drop_timer, player_window_focus) = install_file_drop(
        app,
        player.command_sender(),
        latest_playback.clone(),
        trackpad_pause_generation.clone(),
        scroll_input.clone(),
    );
    let thumbnail_preview_timer = Rc::new(slint::Timer::default());
    let thumbnail_controller =
        Rc::new(RefCell::new(crate::thumbnail_preview::Controller::default()));
    let thumbnail_hover = thumbnail_controller.clone();
    let thumbnail_weak = app.as_weak();
    app.on_timeline_hovered(move |seconds| {
        if let Some(app) = thumbnail_weak.upgrade() {
            thumbnail_hover.borrow_mut().hover(&app, f64::from(seconds));
        }
    });
    let thumbnail_weak = app.as_weak();
    let thumbnail_playback = latest_playback.clone();
    let thumbnail_tick = thumbnail_controller.clone();
    let thumbnail_library = platform::playback_library();
    let thumbnail_plugin_host = plugin_host_handle.clone();
    thumbnail_preview_timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(80),
        move || {
            let Some(app) = thumbnail_weak.upgrade() else {
                return;
            };
            let Ok(settings) = crate::thumbnail_preferences::current_player(&app) else {
                return;
            };
            let Ok(state) = thumbnail_playback.lock().map(|state| state.clone()) else {
                return;
            };
            let ready =
                thumbnail_tick
                    .borrow_mut()
                    .observe(&app, &state, &settings, &thumbnail_library);
            if ready && let Some(host) = &thumbnail_plugin_host {
                let _ = host.notify_thumbnails_ready();
            }
        },
    );
    let pending_navigation: PendingNavigation = Rc::new(RefCell::new(None));
    let transition_frame_timer = if cfg!(target_os = "windows") && native.is_some() {
        let timer = Rc::new(slint::Timer::default());
        let playback = latest_playback.clone();
        let sender = player.command_sender();
        let captured_generation = Rc::new(Cell::new(None::<u64>));
        let outgoing_capture_attempt = Rc::new(Cell::new(None::<(u64, Instant)>));
        let incoming_capture_attempt = Rc::new(Cell::new(None::<(u64, Instant)>));
        let incoming_capture_stability = Rc::new(Cell::new(None::<(u64, u64, u8, Instant)>));
        let surface_host = native.as_ref().map(|presentation| presentation.host());
        let surface_geometry = Rc::new(Cell::new(None::<(u64, slint::PhysicalSize, u64, Instant)>));
        let timer_generation = captured_generation.clone();
        let timer_outgoing_attempt = outgoing_capture_attempt.clone();
        let timer_incoming_attempt = incoming_capture_attempt.clone();
        let timer_incoming_stability = incoming_capture_stability.clone();
        let timer_transition_release_generation = transition_release_generation.clone();
        let timer_pending_navigation = pending_navigation.clone();
        let timer_surface_host = surface_host.clone();
        let timer_surface_geometry = surface_geometry.clone();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(80),
            move || {
                let Ok(state) = playback.lock().map(|state| state.clone()) else {
                    return;
                };
                let navigation = {
                    let mut pending = timer_pending_navigation.borrow_mut();
                    pending
                        .as_ref()
                        .is_some_and(|(_, requested_at)| {
                            state.transition_frame_generation == Some(state.generation)
                                && state
                                    .transition_frame_captured_at
                                    .is_some_and(|captured_at| captured_at >= *requested_at)
                                || requested_at.elapsed() >= MANUAL_TRANSITION_CAPTURE_TIMEOUT
                        })
                        .then(|| pending.take().map(|(commands, _)| commands))
                        .flatten()
                };
                if let Some(commands) = navigation {
                    for command in commands {
                        if let Err(error) = sender.send(command) {
                            eprintln!("Could not navigate the playlist: {error}");
                        }
                    }
                }
                if state.transition_frame_generation == Some(state.generation) {
                    // Only a successful asynchronous reply confirms that mpv
                    // retained a frame. Queue acceptance alone is not enough:
                    // the D3D11 output can reject a capture while reconfiguring.
                    timer_generation.set(Some(state.generation));
                    timer_outgoing_attempt.set(None);
                }
                let now = Instant::now();
                if crate::transition_frame::outgoing_capture_due(
                    &state,
                    timer_generation.get(),
                    timer_outgoing_attempt.get(),
                    now,
                ) {
                    // Capture before mpv tears down and reconfigures its
                    // D3D11 output. Retry rejected or failed captures while
                    // there is still time before the playlist advances.
                    match sender.send(Command::CaptureTransitionFrame) {
                        Ok(()) => timer_outgoing_attempt.set(Some((state.generation, now))),
                        Err(error) => {
                            timer_outgoing_attempt.set(None);
                            eprintln!("Could not retain the outgoing video frame: {error}");
                        }
                    }
                }
                if state
                    .transition_frame_generation
                    .is_some_and(|generation| generation < state.generation)
                    && crate::transition_frame::incoming_has_video(&state)
                    && !matches!(state.phase, Phase::Loading | Phase::Stopped | Phase::Failed)
                    && crate::transition_frame::incoming_capture_window_active(
                        &state,
                        Instant::now(),
                    )
                    && crate::transition_frame::incoming_capture_ready(&state)
                {
                    let now = Instant::now();
                    // Decoder captures can remain healthy while mpv's native
                    // child HWND is hidden or changing size. Check the actual
                    // presentation surface before considering cover release;
                    // doing this after the capture-stability branch let a
                    // stale but valid decoder frame uncover a blank swap chain.
                    let surface_stable = timer_surface_host
                        .as_ref()
                        .and_then(slint::Weak::upgrade)
                        .is_some_and(|host| {
                            let size = host.window().size();
                            let Some(surface_signature) =
                                crate::platform::native_video_surface_signature(host.window())
                            else {
                                timer_surface_geometry.set(None);
                                return false;
                            };
                            match timer_surface_geometry.get() {
                                Some((generation, previous_size, previous_surface, since))
                                    if generation == state.generation
                                        && previous_size == size
                                        && previous_surface == surface_signature =>
                                {
                                    now.duration_since(since) >= INCOMING_SURFACE_STABLE_FOR
                                }
                                _ => {
                                    timer_surface_geometry.set(Some((
                                        state.generation,
                                        size,
                                        surface_signature,
                                        now,
                                    )));
                                    false
                                }
                            }
                        });
                    let capture_is_usable = state.incoming_transition_frame_generation
                        == Some(state.generation)
                        && crate::transition_frame::incoming_frame_is_usable(&state);
                    if capture_is_usable {
                        let revision = state.incoming_transition_frame_revision;
                        let stable_capture = match timer_incoming_stability.get() {
                            Some((generation, last_revision, count, since))
                                if generation == state.generation && last_revision == revision =>
                            {
                                (generation, last_revision, count, since)
                            }
                            Some((generation, _, count, since))
                                if generation == state.generation =>
                            {
                                (generation, revision, count.saturating_add(1), since)
                            }
                            _ => (state.generation, revision, 1, now),
                        };
                        timer_incoming_stability.set(Some(stable_capture));
                        let (_, _, count, since) = stable_capture;
                        if crate::transition_frame::incoming_cover_release_ready(
                            count,
                            since,
                            now,
                            surface_stable,
                        ) {
                            timer_transition_release_generation
                                .store(state.generation, Ordering::Release);
                            return;
                        }
                    } else {
                        timer_incoming_stability.set(None);
                    }
                    let retry_wait_elapsed =
                        timer_incoming_attempt
                            .get()
                            .is_none_or(|(generation, attempted_at)| {
                                generation != state.generation
                                    || now.duration_since(attempted_at)
                                        >= Duration::from_millis(240)
                            });
                    if !retry_wait_elapsed {
                        return;
                    }
                    if !surface_stable {
                        return;
                    }
                    // Do not uncover the native child just because time-pos or
                    // track metadata appeared. Wait for host geometry to settle,
                    // then require a successful raw capture before uncovering.
                    if let Err(error) = sender.send(Command::CaptureIncomingTransitionFrame) {
                        eprintln!("Could not capture the incoming video frame: {error}");
                    } else {
                        timer_incoming_attempt.set(Some((state.generation, now)));
                    }
                }
            },
        );
        Some(timer)
    } else {
        None
    };
    #[cfg(target_os = "windows")]
    let native_material_timer = native.is_some().then(|| {
        install_native_video_material_sampler(
            app,
            player.command_sender(),
            latest_playback.clone(),
            native
                .as_ref()
                .and_then(platform::NativePresentation::material_capture),
        )
    });
    #[cfg(not(target_os = "windows"))]
    let native_material_timer = None;
    #[cfg(target_os = "windows")]
    let windows_materials_policy_timer = install_windows_material_policy_monitor(app);
    #[cfg(target_os = "macos")]
    {
        *system_media_sender.borrow_mut() = Some(player.command_sender());
    }
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    let live_text_timer = Some(install_live_text(
        app,
        player.command_sender(),
        latest_playback.clone(),
    ));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let live_text_timer = None;
    let manual_music_mode = music_mode_state.clone();
    let weak = app.as_weak();
    let toolbar_settings_window = weak.clone();
    app.on_toolbar_settings_changed(move || {
        if let Some(app) = toolbar_settings_window.upgrade() {
            crate::osc_preferences::refresh_visible_order(&app);
        }
    });
    crate::osc_preferences::refresh_visible_order(app);
    let clipboard_parent = weak.clone();
    app.on_copy_live_text(move |text| {
        if let Err(error) = crate::platform::clipboard::copy_text(text.as_str())
            && let Some(app) = clipboard_parent.upgrade()
        {
            app.set_status(format!("Could not copy recognized text: {error}").into());
        }
    });
    let live_text_parent = weak.clone();
    let live_text_path = prefs_path.clone();
    app.on_toggle_live_text(move || {
        let Some(app) = live_text_parent.upgrade() else {
            return;
        };
        if !app.get_live_text_available() {
            app.set_status("Live Text is unavailable on this system".into());
            return;
        }
        let enabled = !app.get_live_text_enabled();
        let result = crate::preference_store::update(
            &live_text_path,
            &Preferences::default(),
            |preferences| {
                preferences.live_text_enabled = enabled;
                Ok(())
            },
        );
        match result {
            Ok(()) => app.set_live_text_enabled(enabled),
            Err(error) => {
                app.set_status(format!("Could not save Live Text preference: {error}").into())
            }
        }
    });
    app.on_toggle_music_mode(move || {
        if let (Some(app), Ok(mut mode)) = (weak.upgrade(), manual_music_mode.lock()) {
            let active = mode.toggle_manually();
            if active && app.get_fullscreen() {
                app.invoke_toggle_fullscreen();
            }
            app.set_music_mode(active);
        }
    });
    let automatic_music_mode = music_mode_state.clone();
    app.on_auto_switch_music_mode_changed(move |enabled| {
        if let Ok(mut mode) = automatic_music_mode.lock() {
            mode.set_auto_switch(enabled);
        }
    });
    let clear_history_plugin_host = plugin_host_handle.clone();
    let clear_history_latest_playback = latest_playback.clone();
    let clear_history_recent_media = plugin_recent_media.clone();
    let clear_history_recording = record_playback_history.clone();
    let clear_history_session_id = playback_history_session_id;
    crate::screenshot_preferences::connect(
        app,
        player.command_sender(),
        prefs_path.clone(),
        prefs.clone(),
        latest_playback.clone(),
        plugin_host_handle,
        {
            let path = prefs_path.clone();
            let playback_history = playback_history.clone();
            let history_cleared = history_cleared.clone();
            let plugin_host = clear_history_plugin_host;
            let latest_playback = clear_history_latest_playback;
            let recent_media = clear_history_recent_media;
            let record_history = clear_history_recording;
            move || {
                let removed = crate::utilities_preferences::clear_playback_history(&path)?;
                let mut history = playback_history
                    .lock()
                    .map_err(|_| "playback history lock poisoned".to_owned())?;
                history.clear();
                history_cleared.store(true, Ordering::Release);
                if let Some(host) = &plugin_host {
                    let playback = latest_playback
                        .lock()
                        .map_err(|_| "latest playback lock poisoned".to_owned())?
                        .clone();
                    let context = crate::plugin_host::player_file_context_with_history(
                        &playback,
                        &mut recent_media
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner),
                        &mut history,
                        record_history.load(Ordering::Acquire),
                        clear_history_session_id,
                    );
                    host.update_player_file_context(context)
                        .map_err(|error| error.to_string())?;
                }
                Ok(removed)
            }
        },
    );
    let idle_timer = Rc::new(slint::Timer::default());
    let controls_fade_timer = Rc::new(slint::Timer::default());
    let controls_fade_parent = app.as_weak();
    let controls_fade_timeout = controls_fade_timer.clone();
    app.on_controls_visibility_changed(move |visible| {
        controls_fade_timeout.stop();
        let Some(app) = controls_fade_parent.upgrade() else {
            return;
        };
        if visible {
            app.set_controls_rendered(true);
        } else if app.get_disable_animations() || app.get_system_reduce_motion() {
            app.set_controls_rendered(false);
        } else {
            let weak = app.as_weak();
            controls_fade_timeout.start(
                slint::TimerMode::SingleShot,
                Duration::from_millis(250),
                move || {
                    if let Some(app) = weak.upgrade()
                        && !app.get_controls_visible()
                    {
                        app.set_controls_rendered(false);
                    }
                },
            );
        }
    });
    let opening_player = player.command_sender();
    let opening_app = app.as_weak();
    let resume_player = player.command_sender();
    let resume_app = app.as_weak();
    let resume_welcome = welcome.map(WelcomeWindow::as_weak);
    let recent_media_for_resume = recent_media.clone();
    app.on_resume_last_position_changed(move |enabled| {
        if let Err(error) = resume_player.send(Command::SetResumeLastPosition(enabled))
            && let Some(app) = resume_app.upgrade()
        {
            app.set_status(error.into());
        }
        if let Some(welcome) = resume_welcome.as_ref().and_then(slint::Weak::upgrade) {
            refresh_welcome_history(&welcome, &recent_media_for_resume.borrow(), enabled);
        }
    });
    let record_history = record_playback_history.clone();
    app.on_record_playback_history_changed(move |enabled| {
        record_history.store(enabled, Ordering::Release);
    });
    app.on_pause_when_open_changed(move |pause| {
        if let Err(error) = opening_player.send(Command::SetPauseWhenOpen(pause))
            && let Some(app) = opening_app.upgrade()
        {
            app.set_status(error.into());
        }
    });
    crate::window_controls::install(app, native.as_ref().map(platform::NativePresentation::host));
    crop_editor.install(app, player.command_sender());
    let reflection_player = player.command_sender();
    let weak = app.as_weak();
    app.on_toggle_video_reflection(move |horizontal| {
        if let Some(app) = weak.upgrade() {
            refresh_video_menu(&app);
            if !app.get_loaded()
                || !app.get_reflection_known()
                || app.get_crop_editing()
                || app.get_dialog_open()
            {
                return;
            }
            let axis = if horizontal {
                riina_core::video_reflection::Axis::Horizontal
            } else {
                riina_core::video_reflection::Axis::Vertical
            };
            if let Err(error) = reflection_player.send(Command::ToggleVideoReflection(axis)) {
                app.set_status(error.into());
            }
        }
    });
    let seek_timer = Rc::new(slint::Timer::default());
    let video_timer = Rc::new(slint::Timer::default());
    let screenshot_timer = Rc::new(slint::Timer::default());
    let disable_seek_timer = seek_timer.clone();
    let disable_video_timer = video_timer.clone();
    let disable_screenshot_timer = screenshot_timer.clone();
    let weak = app.as_weak();
    app.on_osd_enabled_changed(move |enabled| {
        if enabled {
            return;
        }
        let Some(app) = weak.upgrade() else { return };
        // Disabling OSD is an immediate visibility policy, so dismiss an
        // already-visible message instead of leaving it until its old deadline.
        disable_seek_timer.stop();
        disable_video_timer.stop();
        disable_screenshot_timer.stop();
        if let Ok(mut video_osd) = disable_video_osd.lock() {
            video_osd.clear_pending();
        }
        app.set_seek_osd_visible(false);
        app.set_video_osd_visible(false);
        app.set_video_osd_request_feedback(false);
        clear_screenshot_preview(&app);
    });
    let screenshot_player = player.command_sender();
    let weak = app.as_weak();
    app.on_take_screenshot(move || {
        let Some(app) = weak.upgrade() else { return };
        if !app.get_screenshot_available() || app.get_crop_editing() || app.get_dialog_open() {
            return;
        }
        app.invoke_activity();
        let result = crate::screenshot_preferences::current(&app).and_then(|screenshot_settings| {
            if !screenshot_settings.destinations.enabled() {
                return Ok(());
            }
            (if screenshot_settings.destinations.save_to_file {
                platform::screenshot_directory(screenshot_settings.directory.as_deref())
            } else {
                screenshot_scratch.directory()
            })
            .map_err(|error| error.to_string())
            .and_then(|directory| {
                screenshot_player.send(Command::Screenshot {
                    directory: directory.to_string_lossy().into_owned(),
                    content: screenshot_settings.content(),
                    encoding: screenshot_settings.encoding.clone(),
                })
            })
        });
        if let Err(error) = result {
            app.set_status(error.into());
        }
    });
    let weak = app.as_weak();
    app.on_open_screenshot_folder(move || {
        let Some(app) = weak.upgrade() else { return };
        if app.get_crop_editing() || app.get_dialog_open() {
            return;
        }
        if let Err(error) = crate::screenshot_preferences::current(&app).and_then(|settings| {
            platform::screenshot_directory(settings.directory.as_deref())
                .and_then(|directory| platform::open_directory(&directory))
                .map_err(|error| error.to_string())
        }) {
            app.set_status(error.to_string().into());
        }
    });
    let deinterlace_player = player.command_sender();
    let weak = app.as_weak();
    app.on_toggle_deinterlace(move || {
        let Some(app) = weak.upgrade() else { return };
        refresh_video_menu(&app);
        if !app.get_loaded()
            || !app.get_deinterlace_known()
            || app.get_crop_editing()
            || app.get_dialog_open()
        {
            return;
        }
        app.invoke_activity();
        if let Err(error) = deinterlace_player.send(Command::ToggleDeinterlace) {
            app.set_status(error.into());
        }
    });
    let weak = app.as_weak();
    app.on_show_seek_osd(move || {
        let Some(app) = weak.upgrade() else { return };
        if !app.get_osd_enabled() || app.get_disable_osd_seek() {
            return;
        }
        app.set_seek_osd_visible(true);
        clear_screenshot_preview(&app);
        app.set_video_osd_visible(false);
        let weak = weak.clone();
        seek_timer.start(
            slint::TimerMode::SingleShot,
            configured_osd_timeout(&app),
            move || {
                if let Some(app) = weak.upgrade() {
                    app.set_seek_osd_visible(false);
                }
            },
        );
    });
    let weak = app.as_weak();
    app.on_show_screenshot_preview(move || {
        let Some(app) = weak.upgrade() else { return };
        if !app.get_osd_enabled() || app.get_crop_editing() {
            return;
        }
        app.set_seek_osd_visible(false);
        app.set_video_osd_visible(false);
        app.set_screenshot_preview_visible(true);
        let weak = weak.clone();
        screenshot_timer.start(
            slint::TimerMode::SingleShot,
            Duration::from_secs(5),
            move || {
                if let Some(app) = weak.upgrade() {
                    clear_screenshot_preview(&app);
                }
            },
        );
    });
    let weak = app.as_weak();
    app.on_screenshot_preview_action(move |action| {
        let Some(app) = weak.upgrade() else { return };
        if !app.get_screenshot_preview_visible() || app.get_crop_editing() || app.get_dialog_open()
        {
            return;
        }
        let path = app.get_screenshot_preview_path();
        if path.is_empty() {
            return;
        }
        let path = std::path::Path::new(path.as_str());
        let result = match action {
            0 => {
                let _ = std::fs::remove_file(path);
                Ok(())
            }
            1 => platform::open_directory(path),
            2 => platform::reveal_file(path),
            _ => return,
        };
        clear_screenshot_preview(&app);
        if let Err(error) = result {
            app.set_status(error.to_string().into());
        }
    });
    let weak = app.as_weak();
    app.on_show_video_osd(move || {
        let Some(app) = weak.upgrade() else { return };
        if !app.get_osd_enabled() {
            return;
        }
        app.set_seek_osd_visible(false);
        clear_screenshot_preview(&app);
        app.set_video_osd_visible(true);
        let weak = weak.clone();
        let pending_video_osd = pending_video_osd.clone();
        video_timer.start(
            slint::TimerMode::SingleShot,
            configured_osd_timeout(&app),
            move || {
                let Some(app) = weak.upgrade() else { return };
                if !app.get_osd_enabled() {
                    if let Ok(mut video_osd) = pending_video_osd.lock() {
                        video_osd.clear_pending();
                    }
                    app.set_video_osd_visible(false);
                    app.set_video_osd_request_feedback(false);
                    return;
                }
                let next = pending_video_osd
                    .lock()
                    .ok()
                    .and_then(|mut video_osd| video_osd.take_pending());
                if let Some(message) = next {
                    app.set_video_osd_request_feedback(
                        message.kind == crate::video_osd::Kind::Aspect,
                    );
                    app.set_video_osd_text(message.text.into());
                    app.set_video_osd_progress(message.progress.unwrap_or(-1.0));
                    app.invoke_show_video_osd();
                } else {
                    app.set_video_osd_visible(false);
                    app.set_video_osd_request_feedback(false);
                }
            },
        );
    });
    let weak = app.as_weak();
    let activity_timer = idle_timer.clone();
    app.on_activity(move || {
        if let Some(app) = weak.upgrade()
            && !app.get_plugin_ui_disabled()
        {
            app.set_controls_visible(true);
            schedule_control_hide(&app, &activity_timer);
        }
    });
    let auto_hide_parent = app.as_weak();
    let auto_hide_timer = idle_timer.clone();
    app.on_control_bar_settings_changed(move || {
        let Some(app) = auto_hide_parent.upgrade() else {
            return;
        };
        if app.get_title_bar_hovered()
            || app.get_control_bar_hovered()
            || app.get_control_bar_dragging()
        {
            auto_hide_timer.stop();
            app.set_controls_visible(true);
        } else if app.get_control_auto_hide_enabled() {
            schedule_control_hide(&app, &auto_hide_timer);
        } else {
            auto_hide_timer.stop();
            if app.get_loaded() && !app.get_dialog_open() && !app.get_crop_editing() {
                app.set_controls_visible(true);
            }
        }
    });
    let hover_parent = app.as_weak();
    let hover_timer = idle_timer.clone();
    app.on_control_bar_hover_changed(move || {
        let Some(app) = hover_parent.upgrade() else {
            return;
        };
        if app.get_title_bar_hovered()
            || app.get_control_bar_hovered()
            || app.get_control_bar_dragging()
        {
            hover_timer.stop();
            app.set_controls_visible(true);
        } else {
            schedule_control_hide(&app, &hover_timer);
        }
    });
    let pending_single_click = Rc::new(RefCell::new(None::<i32>));
    let single_click_timer = Rc::new(slint::Timer::default());
    let pending = pending_single_click.clone();
    let timer = single_click_timer.clone();
    let single_player = player.command_sender();
    let single_app = app.as_weak();
    app.on_single_click_action(move |action| {
        *pending.borrow_mut() = Some(action);
        let pending = pending.clone();
        let player = single_player.clone();
        let weak = single_app.clone();
        // Slint reports clicked before double-clicked. Delay the single action
        // so a double click invokes only its configured action.
        timer.start(
            slint::TimerMode::SingleShot,
            Duration::from_millis(500),
            move || {
                let action = pending.borrow_mut().take();
                if let (Some(action), Some(app)) = (action, weak.upgrade()) {
                    dispatch_control_mouse_action(&app, &player, action);
                }
            },
        );
    });
    let pending = pending_single_click;
    let timer = single_click_timer;
    let double_player = player.command_sender();
    let double_app = app.as_weak();
    app.on_double_click_action(move |action| {
        timer.stop();
        pending.borrow_mut().take();
        let app = double_app.clone();
        let player = double_player.clone();
        // Fullscreen can move/hide the surface delivering this pointer release.
        // Let capture and double-click dispatch unwind before changing HWNDs.
        slint::Timer::single_shot(Duration::ZERO, move || {
            if let Some(app) = app.upgrade() {
                dispatch_control_mouse_action(&app, &player, action);
            }
        });
    });
    let button_player = player.command_sender();
    let button_app = app.as_weak();
    app.on_mouse_button_action(move |action| {
        if let Some(app) = button_app.upgrade() {
            dispatch_control_mouse_action(&app, &button_player, action);
        }
    });
    let scroll_player = player.command_sender();
    let scroll_playback = latest_playback.clone();
    let scroll_pause_generation = trackpad_pause_generation.clone();
    app.on_scroll_action_request(scroll_action_handler(
        app.as_weak(),
        scroll_player.clone(),
        scroll_playback.clone(),
        scroll_input.clone(),
        scroll_pause_generation.clone(),
        None,
    ));
    app.on_seek_scroll_action_request(scroll_action_handler(
        app.as_weak(),
        scroll_player.clone(),
        scroll_playback.clone(),
        scroll_input.clone(),
        scroll_pause_generation.clone(),
        Some(riina_core::control_preferences::ScrollAction::Seek),
    ));
    app.on_volume_scroll_action_request(scroll_action_handler(
        app.as_weak(),
        scroll_player,
        scroll_playback,
        scroll_input,
        scroll_pause_generation,
        Some(riina_core::control_preferences::ScrollAction::Volume),
    ));
    macro_rules! connect {
        ($method:ident, $command:expr) => {{
            let player = player.command_sender();
            let weak = app.as_weak();
            app.$method(move || {
                let command = $command;
                let show_stop = matches!(&command, Command::Stop);
                if let Some(app) = weak.upgrade() {
                    app.invoke_activity();
                    if matches!(command, Command::Stop) {
                        media_open::invalidate(&app);
                    }
                }
                match player.send(command) {
                    Err(error) => {
                        if let Some(app) = weak.upgrade() {
                            app.set_status(error.into());
                        }
                    }
                    Ok(()) if show_stop => {
                        if let Some(app) = weak.upgrade() {
                            let message = crate::video_osd::action_message(
                                crate::video_osd::ActionFeedback::Stop,
                            );
                            show_osd_message(&app, message.text, true);
                        }
                    }
                    Ok(()) => {}
                }
            });
        }};
    }
    // Let mpv toggle its current state: UI pause snapshots can lag an in-flight
    // transition, while the playback worker serializes this action with engine events.
    let pause_player = player.command_sender();
    let pause_app = app.as_weak();
    app.on_toggle_pause(move || {
        let Some(app) = pause_app.upgrade() else {
            return;
        };
        app.invoke_activity();
        if let Err(error) = pause_player.toggle_pause() {
            app.set_status(error.into());
        }
    });
    let set_pause_player = player.command_sender();
    let set_pause_app = app.as_weak();
    app.on_set_paused(move |paused| {
        let Some(app) = set_pause_app.upgrade() else {
            return;
        };
        app.invoke_activity();
        if let Err(error) = set_pause_player.send(Command::SetPaused(paused)) {
            app.set_status(error.into());
        }
    });
    let input_action_player = player.command_sender();
    let input_action_app = app.as_weak();
    app.on_execute_mpv_input_action(move |action| {
        let Some(app) = input_action_app.upgrade() else {
            return;
        };
        app.invoke_activity();
        if let Err(error) = input_action_player.send(Command::MpvInputAction(action.to_string())) {
            app.set_status(error.into());
        }
    });
    connect!(on_toggle_mute, Command::ToggleMute);
    let hdr_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_hdr_enabled(move |enabled| {
        let Some(app) = weak.upgrade() else { return };
        if !app.get_hdr_source_detected() || (enabled && !app.get_hdr_display_active()) {
            app.set_status("HDR needs an HDR source and an HDR-enabled display".into());
            return;
        }
        match hdr_player.send(Command::SetHdrEnabledWithIcc {
            enabled,
            load_icc_profile: app.get_load_icc_profile(),
        }) {
            Ok(()) => {
                app.set_hdr_enabled(enabled);
                app.invoke_activity();
            }
            Err(error) => app.set_status(error.into()),
        }
    });
    connect!(on_stop, Command::Stop);
    connect!(on_frame_step, Command::FrameStep);
    connect!(on_frame_back_step, Command::FrameBackStep);
    let chapter_player = player.command_sender();
    let weak = app.as_weak();
    app.on_adjust_chapter(move |direction| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
        }
        if let Err(error) = chapter_player.send(Command::AdjustChapter(direction))
            && let Some(app) = weak.upgrade()
        {
            app.set_status(error.into());
        }
    });
    let subtitle_player = player.command_sender();
    let weak = app.as_weak();
    app.on_subtitle_seek(move |direction| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
        }
        if let Err(error) = subtitle_player.send(Command::SubtitleSeek(direction))
            && let Some(app) = weak.upgrade()
        {
            app.set_status(error.into());
        }
    });
    let capture_transition_first = cfg!(target_os = "windows") && native.is_some();
    let playlist_navigation_sender = player.command_sender();
    let sender = playlist_navigation_sender.clone();
    let pending = pending_navigation.clone();
    let weak = app.as_weak();
    app.on_playlist_next(move || {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
        }
        queue_playlist_navigation(
            &sender,
            &pending,
            Command::PlaylistNext,
            capture_transition_first,
        );
    });
    let sender = playlist_navigation_sender.clone();
    let pending = pending_navigation.clone();
    let weak = app.as_weak();
    app.on_playlist_previous(move || {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
        }
        queue_playlist_navigation(
            &sender,
            &pending,
            Command::PlaylistPrevious,
            capture_transition_first,
        );
    });
    connect!(on_reset_speed, Command::Speed(1.0));
    let speed_slider_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_speed_slider(move |position| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            let result = riina_core::speed_controls::from_slider(position.into())
                .map_err(str::to_owned)
                .and_then(|command| speed_slider_player.send(command));
            if let Err(error) = result {
                app.set_status(error.into());
            }
        }
    });
    let speed_text_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_speed_text(move |text| {
        let result = riina_core::speed_controls::from_text(text.as_str())
            .map_err(str::to_owned)
            .and_then(|command| speed_text_player.send(command));
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            if let Err(error) = result {
                app.set_status(error.into());
                return false;
            }
        }
        true
    });
    let aspect_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_video_aspect(move |text| {
        if text.is_empty() {
            return;
        }
        let aspect = riina_core::video_controls::AspectRatio::from_text(text.as_str());
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            if let Err(error) = aspect_player.send(Command::VideoAspect(aspect)) {
                app.set_status(error.into());
            }
        }
    });
    let rotation_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_menu_video_aspect(move |index| {
        let Some(app) = weak.upgrade() else { return };
        refresh_video_menu(&app);
        if !app.get_loaded()
            || !app.get_video_aspect_known()
            || app.get_crop_editing()
            || app.get_dialog_open()
        {
            return;
        }
        if let Some(label) = usize::try_from(index)
            .ok()
            .and_then(|index| riina_core::video_controls::ASPECT_MENU_PRESETS.get(index))
        {
            app.invoke_set_video_aspect((*label).into());
        }
    });
    let crop_player = player.command_sender();
    let crop_ui = app.as_weak();
    app.on_set_video_crop(move |index| {
        if index == 6 {
            if let Some(app) = crop_ui.upgrade() {
                app.invoke_begin_custom_crop();
            }
            return;
        }
        let preset = if index == 0 {
            None
        } else if let Some(preset) = usize::try_from(index - 1)
            .ok()
            .and_then(|index| riina_core::crop::Preset::ALL.get(index))
            .copied()
        {
            Some(preset)
        } else {
            return;
        };
        if let Some(app) = crop_ui.upgrade() {
            app.invoke_activity();
            if let Err(error) = crop_player.send(Command::VideoCrop(preset)) {
                app.set_status(error.into());
            }
        }
    });
    let delogo_player = player.command_sender();
    let weak = app.as_weak();
    app.on_toggle_delogo(move || {
        let Some(app) = weak.upgrade() else { return };
        if !app.get_delogo_enabled() || app.get_crop_editing() || app.get_dialog_open() {
            return;
        }
        app.invoke_activity();
        if app.get_delogo_active() {
            if let Err(error) = delogo_player.send(Command::RemoveDelogo) {
                app.set_status(error.into());
            }
        } else {
            app.invoke_begin_delogo();
        }
    });
    let menu_crop_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_menu_video_crop(move |index| {
        let Some(app) = weak.upgrade() else { return };
        refresh_video_menu(&app);
        if !app.get_loaded()
            || !app.get_video_crop_known()
            || app.get_crop_editing()
            || app.get_dialog_open()
        {
            return;
        }
        if index == riina_core::crop::MENU_CUSTOM_INDEX as i32 {
            app.invoke_begin_custom_crop();
            return;
        }
        let preset = if index == 0 {
            None
        } else if let Some(preset) = usize::try_from(index - 1)
            .ok()
            .and_then(|index| riina_core::crop::Preset::MENU_ALL.get(index))
            .copied()
        {
            Some(preset)
        } else {
            return;
        };
        app.invoke_activity();
        if let Err(error) = menu_crop_player.send(Command::VideoCrop(preset)) {
            app.set_status(error.into());
        }
    });
    let weak = app.as_weak();
    app.on_set_video_rotation(move |degrees| {
        if let Some(app) = weak.upgrade()
            && let Some(rotation) =
                riina_core::video_controls::Rotation::from_degrees(degrees.into())
        {
            app.invoke_activity();
            if let Err(error) = rotation_player.send(Command::VideoRotation(rotation)) {
                app.set_status(error.into());
            }
        }
    });
    let menu_rotation_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_menu_video_rotation(move |degrees| {
        let Some(app) = weak.upgrade() else { return };
        refresh_video_menu(&app);
        if !app.get_loaded()
            || !app.get_video_rotation_known()
            || app.get_crop_editing()
            || app.get_dialog_open()
        {
            return;
        }
        let Some(rotation) = riina_core::video_controls::Rotation::from_degrees(degrees.into())
        else {
            return;
        };
        app.invoke_activity();
        if let Err(error) = menu_rotation_player.send(Command::VideoRotation(rotation)) {
            app.set_status(error.into());
        }
    });
    let hardware_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_hardware_decoding(move |enabled| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            let mode = if enabled {
                crate::codec_preferences::hardware_decoder(app.get_hardware_decoder_index())
                    .unwrap_or(riina_core::video_controls::HardwareDecoder::Auto)
            } else {
                riina_core::video_controls::HardwareDecoder::Disabled
            };
            if let Err(error) = hardware_player.send(Command::HardwareDecoding(mode)) {
                app.set_status(error.into());
            }
        }
    });
    connect!(on_reset_subtitle_delay, Command::SubtitleDelay(0.0));
    let equalizer_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_video_equalizer(move |index, value| {
        let option = usize::try_from(index).ok().and_then(|index| {
            riina_core::video_controls::Equalizer::ALL
                .get(index)
                .copied()
        });
        if let Some(option) = option
            && value.is_finite()
            && (-100.0..=100.0).contains(&value)
            && let Some(app) = weak.upgrade()
        {
            app.invoke_activity();
            // NSSlider's integer action uses whole units. The backend validates
            // the range again so every caller obeys the same portable contract.
            if let Err(error) = equalizer_player.send(Command::VideoEqualizer {
                option,
                value: value as i64,
            }) {
                app.set_status(error.into());
            }
        }
    });
    let audio_eq_settings = audio_equalizer.clone();
    let audio_eq_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_audio_equalizer(move |index, value| {
        let Some(index) = usize::try_from(index).ok().filter(|index| *index < 10) else {
            return;
        };
        if !value.is_finite() || !(-12.0..=12.0).contains(&value) {
            return;
        }
        let Some(app) = weak.upgrade() else { return };
        let settings = {
            let Ok(mut settings) = audio_eq_settings.lock() else {
                app.set_status("Audio equalizer settings are unavailable".into());
                return;
            };
            let values = app.get_audio_equalizer_values();
            if values.row_count() == settings.gains_db.len() {
                for (index, gain) in settings.gains_db.iter_mut().enumerate() {
                    if let Some(value) = values.row_data(index) {
                        *gain = f64::from(value);
                    }
                }
            }
            settings.gains_db[index] = f64::from(value);
            settings.selected_profile = Some("custom".into());
            settings.selected_profile_is_user = false;
            settings.clone()
        };
        if let Err(error) = audio_eq_player.send(Command::AudioEqualizer(settings.gains_db)) {
            app.set_status(error.into());
            return;
        }
        set_audio_equalizer_controls(&app, &settings);
        app.invoke_activity();
    });
    let audio_eq_settings_for_profile = audio_equalizer.clone();
    let audio_eq_player_for_profile = player.command_sender();
    let weak = app.as_weak();
    app.on_select_audio_equalizer_profile(move |index| {
        let Some(app) = weak.upgrade() else { return };
        let Ok(mut settings) = audio_eq_settings_for_profile.lock() else {
            app.set_status("Audio equalizer settings are unavailable".into());
            return;
        };
        let Some(index) = usize::try_from(index).ok() else {
            app.set_status("Unknown audio equalizer profile".into());
            return;
        };
        if let Err(error) = settings.select_profile_at(index) {
            app.set_status(error.into());
            return;
        }
        let snapshot = settings.clone();
        drop(settings);
        if let Err(error) =
            audio_eq_player_for_profile.send(Command::AudioEqualizer(snapshot.gains_db))
        {
            app.set_status(error.into());
            return;
        }
        set_audio_equalizer_controls(&app, &snapshot);
    });
    let audio_eq_settings_for_validation = audio_equalizer.clone();
    let weak = app.as_weak();
    app.on_validate_audio_equalizer_profile_name(move |name, mode| {
        let Some(app) = weak.upgrade() else {
            return "Audio equalizer settings are unavailable".into();
        };
        let visible_names = audio_equalizer_visible_profile_names(&app);
        if audio_equalizer_name_is_display_duplicate(name.as_str(), &visible_names) {
            return "An audio equalizer profile already has that name".into();
        }
        let Ok(settings) = audio_eq_settings_for_validation.lock() else {
            return "Audio equalizer settings are unavailable".into();
        };
        let validation = match mode {
            1 => settings.validate_new_profile_name(name.as_str()),
            2 => {
                let current = settings.selected_profile.as_deref().unwrap_or_default();
                settings.validate_renamed_profile_name(current, name.as_str())
            }
            _ => return "".into(),
        };
        validation.err().unwrap_or("").into()
    });
    let audio_eq_settings_for_save = audio_equalizer.clone();
    let weak = app.as_weak();
    app.on_save_audio_equalizer_profile(move |name| {
        let Some(app) = weak.upgrade() else {
            return false;
        };
        let visible_names = audio_equalizer_visible_profile_names(&app);
        if audio_equalizer_name_is_display_duplicate(name.as_str(), &visible_names) {
            app.set_status("An audio equalizer profile already has that name".into());
            return false;
        }
        let Ok(mut settings) = audio_eq_settings_for_save.lock() else {
            app.set_status("Audio equalizer settings are unavailable".into());
            return false;
        };
        if let Err(error) = settings.save_profile(name.as_str()) {
            app.set_status(error.into());
            return false;
        }
        set_audio_equalizer_controls(&app, &settings);
        true
    });
    let audio_eq_settings_for_rename = audio_equalizer.clone();
    let weak = app.as_weak();
    app.on_rename_audio_equalizer_profile(move |replacement| {
        let Some(app) = weak.upgrade() else {
            return false;
        };
        let visible_names = audio_equalizer_visible_profile_names(&app);
        if audio_equalizer_name_is_display_duplicate(replacement.as_str(), &visible_names) {
            app.set_status("An audio equalizer profile already has that name".into());
            return false;
        }
        let Ok(mut settings) = audio_eq_settings_for_rename.lock() else {
            app.set_status("Audio equalizer settings are unavailable".into());
            return false;
        };
        let current = settings.selected_profile.clone().unwrap_or_default();
        if let Err(error) = settings.rename_profile(&current, replacement.as_str()) {
            app.set_status(error.into());
            return false;
        }
        set_audio_equalizer_controls(&app, &settings);
        true
    });
    let audio_eq_settings_for_delete = audio_equalizer.clone();
    let weak = app.as_weak();
    app.on_delete_audio_equalizer_profile(move || {
        let Some(app) = weak.upgrade() else { return };
        let Ok(mut settings) = audio_eq_settings_for_delete.lock() else {
            app.set_status("Audio equalizer settings are unavailable".into());
            return;
        };
        let selected = settings.selected_profile.clone().unwrap_or_default();
        if let Err(error) = settings.remove_profile(&selected) {
            app.set_status(error.into());
            return;
        }
        set_audio_equalizer_controls(&app, &settings);
    });
    let scale_player = player.command_sender();
    let absolute_scale_player = player.command_sender();
    let font_size_player = player.command_sender();
    crate::font_picker::connect(app);
    let outline_size_player = player.command_sender();
    let color_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_subtitle_color(move |target, value| {
        if let Some(app) = weak.upgrade() {
            let rgba = [value.red(), value.green(), value.blue(), value.alpha()]
                .map(|channel| f64::from(channel) / 255.0);
            let Ok(color) = riina_core::subtitle_color::SubtitleColor::from_rgba(rgba) else {
                return;
            };
            let command = match target {
                1 => Command::SubtitleTextColor(color),
                2 => Command::SubtitleShadowColor(color),
                3 => Command::SubtitleBorderColor(color),
                _ => return,
            };
            app.invoke_activity();
            if let Err(error) = color_player.send(command) {
                app.set_status(error.into());
            } else {
                match target {
                    1 => app.set_subtitle_text_preference_color(value),
                    2 => {
                        app.set_subtitle_shadow_preference_color(value);
                        app.set_subtitle_background_preference_color(value);
                    }
                    3 => app.set_subtitle_outline_preference_color(value),
                    _ => unreachable!("target was validated above"),
                }
            }
        }
    });
    let weak = app.as_weak();
    app.on_set_subtitle_outline_size(move |value| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            if let Err(error) = outline_size_player.send(Command::SubtitleOutlineSize(value as f64))
            {
                app.set_status(error.into());
            } else {
                app.set_subtitle_outline_size_preference(value);
            }
        }
    });
    let weak = app.as_weak();
    app.on_set_subtitle_font_size(move |value| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            if let Err(error) = font_size_player.send(Command::SubtitleFontSize(value as f64)) {
                app.set_status(error.into());
            } else {
                app.set_subtitle_font_size_preference(value);
            }
        }
    });
    let weak = app.as_weak();
    app.on_set_subtitle_scale_slider(move |value| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            // Each side advances in 0.05 scale units; the negative side is reciprocal.
            let magnitude = ((1.0 + f64::from(value).abs()) * 20.0).round() / 20.0;
            let scale = if value < 0.0 {
                magnitude.recip()
            } else {
                magnitude
            };
            if let Err(error) = absolute_scale_player.send(Command::SubtitleScale(scale)) {
                app.set_status(error.into());
            }
        }
    });
    let weak = app.as_weak();
    app.on_adjust_subtitle_scale(move |delta| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
        }
        if let Err(error) = scale_player.send(Command::AdjustSubtitleScale(delta as f64))
            && let Some(app) = weak.upgrade()
        {
            app.set_status(error.into());
        }
    });
    let multiply_scale_player = player.command_sender();
    let weak = app.as_weak();
    app.on_multiply_subtitle_scale(move |factor| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
        }
        if let Err(error) =
            multiply_scale_player.send(Command::MultiplySubtitleScale(factor as f64))
            && let Some(app) = weak.upgrade()
        {
            app.set_status(error.into());
        }
    });
    let position_player = player.command_sender();
    let weak = app.as_weak();
    app.on_adjust_subtitle_position(move |delta| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
        }
        if let Err(error) = position_player.send(Command::AdjustSubtitlePosition(delta as f64))
            && let Some(app) = weak.upgrade()
        {
            app.set_status(error.into());
        }
    });
    connect!(on_reset_audio_delay, Command::AudioDelay(0.0));
    let audio_delay_player = player.command_sender();
    let weak = app.as_weak();
    app.on_adjust_audio_delay(move |delta| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
        }
        if let Err(error) = audio_delay_player.send(Command::AdjustAudioDelay(delta as f64))
            && let Some(app) = weak.upgrade()
        {
            app.set_status(error.into());
        }
    });
    let delay_player = player.command_sender();
    let text_delay_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_delay_text(move |category, text| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            use riina_core::subtitle_controls::SubtitleLane;
            let normalized = match crate::localized_number::normalize_delay_entry(&text) {
                Ok(normalized) => normalized,
                Err(error) => {
                    app.set_status(error.into());
                    return false;
                }
            };
            let command = match category {
                1 => riina_core::timing_controls::parse_delay_seconds(&normalized)
                    .map(Command::AudioDelay)
                    .map_err(str::to_owned),
                2 => SubtitleLane::Primary
                    .delay_from_text(&normalized)
                    .map_err(str::to_owned),
                5 => SubtitleLane::Secondary
                    .delay_from_text(&normalized)
                    .map_err(str::to_owned),
                _ => Err("Unknown delay control".to_owned()),
            };
            if let Err(error) = command.and_then(|command| text_delay_player.send(command)) {
                app.set_status(error.into());
                return false;
            }
            return true;
        }
        false
    });
    let secondary_position_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_subtitle_position(move |secondary, value| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            if let Err(error) = secondary_position_player.send(if secondary {
                Command::SecondarySubtitlePosition(value as f64)
            } else {
                Command::SubtitlePosition(value as f64)
            }) {
                app.set_status(error.into());
            }
        }
    });
    let weak = app.as_weak();
    app.on_adjust_subtitle_delay(move |delta| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
        }
        if let Err(error) = delay_player.send(Command::AdjustSubtitleDelay(delta as f64))
            && let Some(app) = weak.upgrade()
        {
            app.set_status(error.into());
        }
    });
    connect!(on_cycle_ab_loop, Command::CycleAbLoop);
    let ab_loop_player = player.command_sender();
    let ab_loop_app = app.as_weak();
    app.on_set_ab_loop_point(move |point, value| {
        let Some(app) = ab_loop_app.upgrade() else {
            return;
        };
        if !app.get_loaded() || !value.is_finite() {
            return;
        }
        let point = match point {
            0 => riina_core::AbLoopPoint::A,
            1 => riina_core::AbLoopPoint::B,
            _ => return,
        };
        app.invoke_activity();
        if let Err(error) = ab_loop_player.send(Command::SetAbLoopPoint {
            point,
            value: Some(f64::from(value)),
        }) {
            app.set_status(error.into());
        }
    });
    connect!(on_toggle_file_loop, Command::ToggleFileLoop);
    connect!(
        on_toggle_secondary_subtitle_visibility,
        Command::ToggleSecondarySubtitleVisibility
    );
    connect!(on_cycle_audio_track, Command::CycleTrack(TrackKind::Audio));
    connect!(on_cycle_subtitle_track, Command::CycleTrack(TrackKind::Sub));
    connect!(on_cycle_video_track, Command::CycleTrack(TrackKind::Video));
    let audio_device_player = player.command_sender();
    let audio_device_state = latest_playback.clone();
    let audio_device_app = app.as_weak();
    app.on_set_audio_device(move |device| {
        let Some(app) = audio_device_app.upgrade() else {
            return;
        };
        let device = device.to_string();
        let available = audio_device_state.lock().ok().is_some_and(|state| {
            state
                .audio_devices
                .iter()
                .any(|available| available.name == device)
        });
        if !available {
            app.set_status("That audio device is no longer available".into());
            return;
        }
        app.invoke_activity();
        if let Err(error) = audio_device_player.send(Command::AudioDevice(device)) {
            app.set_status(error.into());
        }
    });
    connect!(
        on_toggle_subtitle_visibility,
        Command::ToggleSubtitleVisibility
    );
    let subtitle_encoding_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_subtitle_encoding(move |encoding| {
        let Some(app) = weak.upgrade() else { return };
        let encoding = encoding.to_string();
        if !riina_core::subtitle_controls::SUBTITLE_ENCODING_CHOICES
            .iter()
            .any(|(_, code)| *code == encoding.as_str())
        {
            app.set_status("Unsupported subtitle encoding".into());
            return;
        }
        app.invoke_activity();
        match subtitle_encoding_player.send(Command::SubtitleEncoding(encoding.clone())) {
            Ok(()) => app.set_subtitle_default_encoding(encoding.into()),
            Err(error) => app.set_status(error.into()),
        }
    });
    let selection_player = player.command_sender();
    let selection_state = latest_playback.clone();
    let weak = app.as_weak();
    app.on_select_item(move |category, token| {
        // IDs stay strings in the UI: Slint's float numbers cannot preserve every i64.
        let command = match category {
            5 => token
                .as_str()
                .parse::<i64>()
                .map(Some)
                .or_else(|error| if token == "no" { Ok(None) } else { Err(error) })
                .map(Command::SelectSecondarySubtitle)
                .map_err(|error| error.to_string()),
            1 | 2 | 6 => token
                .as_str()
                .parse::<i64>()
                .map(Some)
                .or_else(|error| if token == "no" { Ok(None) } else { Err(error) })
                .map(|id| Command::SelectTrack {
                    kind: match category {
                        1 => TrackKind::Audio,
                        6 => TrackKind::Video,
                        _ => TrackKind::Sub,
                    },
                    id,
                })
                .map_err(|error| error.to_string()),
            3 => token
                .parse::<usize>()
                .map(Command::SelectChapter)
                .map_err(|error| error.to_string()),
            4 => token
                .parse::<i64>()
                .map(Command::SelectPlaylistId)
                .map_err(|error| error.to_string()),
            _ => Err("Unknown selection category".into()),
        };
        let chapter_title = if category == 3 {
            token
                .parse::<usize>()
                .ok()
                .and_then(|index| selection_state.lock().ok()?.chapters.get(index).cloned())
                .and_then(|chapter| chapter.title)
        } else {
            None
        };
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            match command.and_then(|command| selection_player.send(command)) {
                Err(error) => app.set_status(error.into()),
                Ok(()) => {
                    if let Some(title) = chapter_title {
                        let message = crate::video_osd::action_message(
                            crate::video_osd::ActionFeedback::Chapter(title),
                        );
                        show_osd_message(&app, message.text, false);
                    }
                }
            }
        }
    });
    let delay_player = player.command_sender();
    let preview_weak = app.as_weak();
    app.on_preview_audio_delay(move |value| {
        if let Some(app) = preview_weak.upgrade() {
            app.set_audio_delay_preview_text(
                crate::localized_number::format_delay(f64::from(value)).into(),
            );
        }
    });
    let weak = app.as_weak();
    app.on_set_delay(move |category, value| {
        let command = match category {
            1 => Some(Command::AudioDelay(value as f64)),
            2 => Some(Command::SubtitleDelay(value as f64)),
            5 => Some(Command::SecondarySubtitleDelay(value as f64)),
            _ => None,
        };
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            if let Some(command) = command
                && let Err(error) = delay_player.send(command)
            {
                app.set_status(error.into());
            }
        }
    });
    let playlist_player = player.command_sender();
    let playlist_pending_navigation = pending_navigation.clone();
    let playlist_capture_transition_first = capture_transition_first;
    let weak = app.as_weak();
    app.on_playlist_action(move |action, token| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        if action == 12 {
            window.invoke_activity();
            let rows = window.get_playlist();
            let exists = (0..rows.row_count()).any(|index| {
                rows.row_data(index)
                    .is_some_and(|row| row.token.as_str() == token.as_str())
            });
            if !exists {
                window.set_status("The playlist item is no longer available".into());
                return;
            }
            match token.parse::<i64>() {
                Ok(id) => queue_playlist_navigation(
                    &playlist_player,
                    &playlist_pending_navigation,
                    Command::SelectPlaylistId(id),
                    playlist_capture_transition_first,
                ),
                Err(error) => {
                    window.set_status(format!("Invalid playlist item: {error}").into());
                }
            }
            return;
        }
        if matches!(action, 6..=11) {
            window.invoke_activity();
            let rows = window.get_playlist();
            let selected_index = (0..rows.row_count()).find(|index| {
                rows.row_data(*index)
                    .is_some_and(|row| row.token.as_str() == token.as_str())
            });
            let result = match action {
                6 | 7 => {
                    let paths = selected_playlist_paths(&window, token.as_str());
                    if paths.is_empty() {
                        Err("Select a playlist item first".into())
                    } else {
                        crate::platform::clipboard::copy_playlist_references(&paths).and_then(
                            |()| {
                                if action == 7 {
                                    selected_playlist_ids(&window, token.as_str()).and_then(|ids| {
                                        playlist_player.send(Command::RemovePlaylistIds(ids))
                                    })
                                } else {
                                    Ok(())
                                }
                            },
                        )
                    }
                }
                8 => crate::platform::clipboard::read_playlist_references().and_then(|paths| {
                    if paths.is_empty() {
                        return Err("Clipboard contains no playlist references".into());
                    }
                    let index = playlist_paste_index(&window, token.as_str());
                    let count = paths.len();
                    playlist_player
                        .send(Command::InsertPlaylistFiles { paths, index })
                        .map(|()| {
                            let message = crate::video_osd::action_message(
                                crate::video_osd::ActionFeedback::PlaylistAdded(count),
                            );
                            show_osd_message(&window, message.text, false);
                        })
                }),
                9 => selected_index
                    .and_then(|index| rows.row_data(index))
                    .ok_or_else(|| "Select a playlist item first".to_owned())
                    .and_then(|row| {
                        crate::media_open::open_in_new_player(&window, row.path.as_str())
                    }),
                10 => {
                    let urls = selected_playlist_paths(&window, token.as_str())
                        .into_iter()
                        .filter(|path| crate::platform::is_http_url(path))
                        .collect::<Vec<_>>();
                    if urls.is_empty() {
                        Err("Select an HTTP or HTTPS playlist entry first".into())
                    } else {
                        urls.iter().try_for_each(|url| {
                            crate::platform::open_http_url_in_browser(url)
                                .map_err(|error| error.to_string())
                        })
                    }
                }
                11 => {
                    let urls = selected_playlist_paths(&window, token.as_str())
                        .into_iter()
                        .filter(|path| crate::platform::is_http_url(path))
                        .collect::<Vec<_>>();
                    if urls.is_empty() {
                        Err("Select an HTTP or HTTPS playlist entry first".into())
                    } else {
                        crate::platform::clipboard::copy_text(&urls.join("\n"))
                    }
                }
                _ => unreachable!(),
            };
            if let Err(error) = result {
                window.set_status(error.into());
            } else if action == 7 {
                window.set_selected_playlist_token("".into());
                window.set_selected_playlist_tokens(slint::ModelRc::default());
                install_playlist_selection(&window, &[]);
            }
            return;
        }
        let command = match action {
            1 => selected_playlist_ids(&window, token.as_str()).map(Command::RemovePlaylistIds),
            2 | 3 => token
                .parse()
                .map(|id| Command::MovePlaylistId {
                    id,
                    direction: if action == 2 { -1 } else { 1 },
                })
                .map_err(|error: std::num::ParseIntError| error.to_string()),
            4 => Ok(Command::ClearPlaylist),
            5 => Ok(Command::CycleLoop),
            _ => Err("Unknown playlist action".into()),
        };
        window.invoke_activity();
        if let Err(error) = command.and_then(|command| playlist_player.send(command)) {
            window.set_status(error.into());
        } else if action == 4 {
            let message =
                crate::video_osd::action_message(crate::video_osd::ActionFeedback::PlaylistCleared);
            show_osd_message(&window, message.text, false);
        } else if action == 1 {
            window.set_selected_playlist_token("".into());
            window.set_selected_playlist_tokens(slint::ModelRc::default());
            install_playlist_selection(&window, &[]);
        }
    });
    let playlist_selection_anchor = Rc::new(RefCell::new(None::<String>));
    let selection_anchor = playlist_selection_anchor.clone();
    let weak = app.as_weak();
    app.on_select_playlist_row(move |index, extend, range| {
        let Some(app) = weak.upgrade() else {
            return slint::ModelRc::default();
        };
        let Ok(index) = usize::try_from(index) else {
            return slint::ModelRc::default();
        };
        let rows = app.get_playlist();
        let Some(clicked) = rows.row_data(index) else {
            return app.get_selected_playlist_tokens();
        };
        let clicked_token = clicked.token.to_string();
        let previous = app
            .get_selected_playlist_tokens()
            .iter()
            .map(|token| token.to_string())
            .collect::<Vec<_>>();
        let selected = if range {
            let anchor = selection_anchor
                .borrow()
                .as_deref()
                .and_then(|anchor| {
                    (0..rows.row_count()).find(|row| {
                        rows.row_data(*row)
                            .is_some_and(|entry| entry.token.as_str() == anchor)
                    })
                })
                .unwrap_or(index);
            let mut selected = if extend { previous } else { Vec::new() };
            for row in anchor.min(index)..=anchor.max(index) {
                if let Some(entry) = rows.row_data(row)
                    && !selected.iter().any(|token| token == entry.token.as_str())
                {
                    selected.push(entry.token.to_string());
                }
            }
            selected
        } else if extend {
            let mut selected = previous;
            if let Some(position) = selected.iter().position(|token| token == &clicked_token) {
                selected.remove(position);
                if selection_anchor.borrow().as_deref() == Some(clicked_token.as_str()) {
                    *selection_anchor.borrow_mut() = None;
                }
            } else {
                selected.push(clicked_token.clone());
                *selection_anchor.borrow_mut() = Some(clicked_token.clone());
            }
            selected
        } else {
            *selection_anchor.borrow_mut() = Some(clicked_token.clone());
            vec![clicked_token]
        };
        install_playlist_selection(&app, &selected);
        slint::ModelRc::new(slint::VecModel::from(
            selected.into_iter().map(Into::into).collect::<Vec<_>>(),
        ))
    });

    let reorder_player = player.command_sender();
    let weak = app.as_weak();
    app.on_reorder_playlist(move |tokens, insertion_slot| {
        let Some(app) = weak.upgrade() else { return };
        app.invoke_activity();
        let rows = app.get_playlist();
        let entries = (0..rows.row_count())
            .filter_map(|index| rows.row_data(index))
            .map(|row| row.token.parse::<i64>().ok())
            .collect::<Option<Vec<_>>>();
        let selected = tokens
            .iter()
            .map(|token| token.parse::<i64>().ok())
            .collect::<Option<Vec<_>>>();
        let plan = entries
            .as_deref()
            .zip(selected.as_deref())
            .and_then(|(entries, selected)| {
                usize::try_from(insertion_slot).ok().and_then(|slot| {
                    riina_core::playlist::reorder_selection(entries, selected, slot)
                })
            });
        let Some(plan) = plan else {
            app.set_status("The playlist drop position is no longer available".into());
            return;
        };
        for (from, to) in plan {
            if let Err(error) = reorder_player.send(Command::MovePlaylistEntry { from, to }) {
                app.set_status(error.into());
                break;
            }
        }
    });
    let shuffle_player = player.command_sender();
    let weak = app.as_weak();
    app.on_toggle_playlist_shuffle(move || {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            if let Err(error) = shuffle_player.send(Command::TogglePlaylistShuffle) {
                app.set_status(error.into());
            }
        }
    });
    let sort_player = player.command_sender();
    let weak = app.as_weak();
    app.on_sort_playlist(move |sort| {
        let sort = match sort {
            0 => Some(riina_core::playlist::PlaylistSort::FilenameAscending),
            1 => Some(riina_core::playlist::PlaylistSort::FilenameDescending),
            2 => Some(riina_core::playlist::PlaylistSort::PathAscending),
            3 => Some(riina_core::playlist::PlaylistSort::PathDescending),
            _ => None,
        };
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            let result = sort.map_or_else(
                || Err("Unknown playlist sort order".to_owned()),
                |sort| sort_player.send(Command::SortPlaylist(sort)),
            );
            if let Err(error) = result {
                app.set_status(error.into());
            }
        }
    });
    let browse_player = player.command_sender();
    let weak = app.as_weak();
    let browse_host = native.as_ref().map(platform::NativePresentation::host);
    app.on_browse_files(move |category| {
        let purpose = match category {
            1 => Purpose::Audio,
            2 => Purpose::Subtitle,
            4 => Purpose::Playlist,
            _ => return,
        };
        if let Some(app) = weak.upgrade() {
            ui_dialogs::open(
                &app,
                browse_host.clone(),
                purpose,
                browse_player.clone(),
                None,
            );
        }
    });
    for absolute in [true, false] {
        let player = player.command_sender();
        let weak = app.as_weak();
        let seek = move |seconds: f32| {
            if let Some(app) = weak.upgrade() {
                app.invoke_activity();
            }
            if let Err(error) = player.send(Command::Seek {
                seconds: seconds as f64,
                absolute,
            }) && let Some(app) = weak.upgrade()
            {
                app.set_status(error.into());
            }
        };
        if absolute {
            app.on_seek(seek);
        } else {
            app.on_seek_relative(seek);
        }
    }
    let weak = app.as_weak();
    app.on_open_jump_time_dialog(move || {
        if let Some(app) = weak.upgrade() {
            app.set_jump_time_draft(app.get_position_label());
            app.set_jump_time_error("".into());
            app.set_jump_time_dialog_visible(true);
        }
    });
    let weak = app.as_weak();
    app.on_open_url_dialog(move || {
        if let Some(app) = weak.upgrade() {
            app.set_url_dialog_draft("".into());
            app.set_url_dialog_visible(true);
        }
    });
    let jump_player = player.command_sender();
    let weak = app.as_weak();
    app.on_jump_to_time(move |input| {
        let Some(app) = weak.upgrade() else { return };
        let Some(seconds) = riina_core::time_input::parse_video_time(input.as_str()) else {
            app.set_jump_time_error("Enter a valid time such as 20:35 or 1:02:03.500.".into());
            return;
        };
        app.invoke_activity();
        match jump_player.send(Command::Seek {
            seconds,
            absolute: true,
        }) {
            Ok(()) => {
                app.set_jump_time_error("".into());
                app.set_jump_time_dialog_visible(false);
            }
            Err(error) => app.set_jump_time_error(error.into()),
        }
    });
    let boost_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_volume_boost(move |enabled| {
        if let Some(app) = weak.upgrade() {
            let volume = app.get_volume().min(if enabled { 200.0 } else { 100.0 });
            match boost_player.send(Command::SetVolumeBoost(enabled)) {
                Ok(()) => {
                    app.set_volume_boost(enabled);
                    app.set_volume(volume);
                }
                Err(error) => app.set_status(error.into()),
            }
            app.invoke_activity();
        }
    });
    let volume_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_volume(move |volume| {
        let Some(app) = weak.upgrade() else { return };
        app.invoke_activity();
        let limit = if app.get_volume_boost() { 200.0 } else { 100.0 };
        if let Err(error) = volume_player.send(Command::Volume((volume as f64).min(limit)))
            && let Some(app) = weak.upgrade()
        {
            app.set_status(error.into());
        }
    });
    let speed_player = player.command_sender();
    let weak = app.as_weak();
    app.on_speed(move |factor| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
        }
        if let Err(error) = speed_player.send(Command::MultiplySpeed(factor as f64))
            && let Some(app) = weak.upgrade()
        {
            app.set_status(error.into());
        }
    });
    let adjustment_player = player.command_sender();
    let weak = app.as_weak();
    app.on_adjust_volume(move |delta| {
        let Some(app) = weak.upgrade() else { return };
        app.invoke_activity();
        if let Err(error) = adjustment_player.send(Command::AdjustVolume {
            delta: delta as f64,
            boost: app.get_volume_boost(),
        }) {
            app.set_status(error.into());
        }
    });
    let weak = app.as_weak();
    let fullscreen_host = native.as_ref().map(platform::NativePresentation::host);
    app.on_toggle_fullscreen(move || {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            let fullscreen = !app.get_fullscreen();
            if fullscreen
                && app.get_loaded()
                && app.get_paused()
                && app.get_play_when_entering_fullscreen()
            {
                app.invoke_set_paused(false);
            } else if !fullscreen
                && app.get_loaded()
                && !app.get_paused()
                && app.get_pause_when_leaving_fullscreen()
            {
                app.invoke_set_paused(true);
            }
            if let Some(host) = fullscreen_host.as_ref().and_then(slint::Weak::upgrade) {
                if !host.invoke_set_fullscreen_window(fullscreen) {
                    return;
                }
            } else {
                app.window().set_fullscreen(fullscreen);
            }
            app.set_fullscreen(fullscreen);
        }
    });
    let open_player = player.command_sender();
    let weak = app.as_weak();
    let open_host = native.as_ref().map(platform::NativePresentation::host);
    let welcome_window = welcome.map(|welcome| welcome.as_weak());
    let pending_welcome_open = Rc::new(RefCell::new(None));
    let ready_pending = pending_welcome_open.clone();
    let ready_app = app.as_weak();
    let ready_player = player.command_sender();
    app.on_renderer_ready(move || {
        if let Some(request) = ready_pending.borrow_mut().take()
            && let Some(app) = ready_app.upgrade()
        {
            dispatch_welcome_open(&app, request, &ready_player);
        }
    });
    let file_welcome = welcome_window.clone();
    let file_open_host = open_host.clone();
    let file_dialog_host = open_host.clone();
    let new_window_app = app.as_weak();
    let new_window_host = open_host.clone();
    let new_window_sender = open_player.clone();
    let reveal_media_app = app.as_weak();
    let reveal_media_playback = latest_playback.clone();
    app.on_reveal_current_media(move || {
        if let Some(app) = reveal_media_app.upgrade() {
            crate::key_binding_preferences::reveal_current_media(&app, &reveal_media_playback);
        }
    });
    let save_playlist_app = app.as_weak();
    let save_playlist_host = open_host.clone();
    let save_playlist_playback = latest_playback.clone();
    app.on_save_current_playlist(move || {
        if let Some(app) = save_playlist_app.upgrade() {
            crate::playlist_export::save_current_playlist(
                &app,
                save_playlist_host.clone(),
                &save_playlist_playback,
            );
        }
    });
    let save_subtitle_app = app.as_weak();
    let save_subtitle_host = open_host.clone();
    let save_subtitle_playback = latest_playback.clone();
    app.on_save_downloaded_subtitle(move || {
        if let Some(app) = save_subtitle_app.upgrade() {
            crate::file_actions::save_downloaded_subtitle(
                &app,
                save_subtitle_host.clone(),
                &save_subtitle_playback,
            );
        }
    });
    let delete_media_app = app.as_weak();
    let delete_media_playback = latest_playback.clone();
    let delete_media_player = open_player.clone();
    app.on_delete_current_media(move || {
        if let Some(app) = delete_media_app.upgrade() {
            crate::file_actions::delete_current_media(
                &app,
                &delete_media_playback,
                delete_media_player.clone(),
            );
        }
    });
    app.on_open_in_new_window(move || {
        if let Some(app) = new_window_app.upgrade() {
            ui_dialogs::open(
                &app,
                new_window_host.clone(),
                Purpose::MediaInNewPlayer,
                new_window_sender.clone(),
                None,
            );
        }
    });
    app.on_open_file(move || {
        if let Some(app) = weak.upgrade() {
            let welcome_is_visible = file_welcome
                .as_ref()
                .and_then(slint::Weak::upgrade)
                .is_some_and(|welcome| welcome.window().is_visible());
            if welcome_is_visible {
                set_native_video_host_visible(file_open_host.as_ref(), true);
                if let Err(error) = app.show() {
                    app.set_status(error.to_string().into());
                    return;
                }
                if let Some(welcome) = file_welcome.as_ref().and_then(slint::Weak::upgrade) {
                    let _ = welcome.hide();
                }
            }
            ui_dialogs::open(
                &app,
                file_dialog_host.clone(),
                Purpose::Media,
                open_player.clone(),
                welcome_is_visible.then(|| file_welcome.clone()).flatten(),
            );
        }
    });
    let remember_documents = recent_open_documents.clone();
    let remember_preferences_path = prefs_path.clone();
    let remember_app = app.as_weak();
    app.on_remember_recent_path(move |path| {
        if let Some(app) = remember_app.upgrade() {
            let title = PathBuf::from(path.as_str()).file_name().map_or_else(
                || path.to_string(),
                |name| name.to_string_lossy().into_owned(),
            );
            remember_recent_open_document(
                &app,
                &remember_documents,
                &remember_preferences_path,
                path.to_string(),
                title,
            );
        }
    });
    let open_recent_documents = recent_open_documents.clone();
    let open_recent_preferences_path = prefs_path.clone();
    let recent_player = player.command_sender();
    let open_recent_app = app.as_weak();
    app.on_open_recent_path(move |path| {
        let Some(app) = open_recent_app.upgrade() else {
            return;
        };
        let Some(media) = open_recent_documents
            .borrow()
            .iter()
            .find(|item| item.path == path.as_str())
            .cloned()
        else {
            return;
        };
        remember_recent_open_document(
            &app,
            &open_recent_documents,
            &open_recent_preferences_path,
            media.path.clone(),
            media.title.clone(),
        );
        media_open::open_recent(
            &app,
            media,
            app.get_resume_last_position(),
            recent_player.clone(),
        );
    });
    let clear_recent_documents = recent_open_documents.clone();
    let clear_recent_flag = recent_open_cleared.clone();
    let clear_preferences_path = prefs_path.clone();
    let clear_recent_app = app.as_weak();
    app.on_clear_open_recent(move || {
        if let Some(app) = clear_recent_app.upgrade() {
            clear_recent_flag.set(true);
            clear_recent_documents.borrow_mut().clear();
            app.set_recent_open_menu_rows(slint::ModelRc::new(slint::VecModel::from(Vec::<
                MenuCatalogRow,
            >::new(
            ))));
            app.invoke_video_menu_changed();
            if let Err(error) = persist_recent_open_documents(
                &clear_preferences_path,
                &clear_recent_documents.borrow(),
            ) {
                app.set_status(format!("Could not clear Open Recent: {error}").into());
            }
            if let Err(error) = platform::clear_recent_documents() {
                app.set_status(format!("Could not clear system recent documents: {error}").into());
            }
        }
    });
    let open_url = player.command_sender();
    let weak = app.as_weak();
    let open_url_host = open_host.clone();
    let url_welcome = welcome_window.clone();
    let url_pending = pending_welcome_open.clone();
    app.on_open_url(move |value| {
        if let Some(app) = weak.upgrade() {
            let from_welcome = url_welcome
                .as_ref()
                .and_then(slint::Weak::upgrade)
                .is_some_and(|welcome| welcome.window().is_visible());
            if from_welcome {
                if let Err(error) = media_open::validate_url(value.as_str()) {
                    if let Some(welcome) = url_welcome.as_ref().and_then(slint::Weak::upgrade) {
                        welcome.set_url_error(error.into());
                    }
                    return;
                }
                set_native_video_host_visible(open_url_host.as_ref(), true);
                if let Err(error) = app.show() {
                    app.set_status(error.to_string().into());
                    return;
                }
                if let Some(welcome) = url_welcome.as_ref().and_then(slint::Weak::upgrade) {
                    welcome.set_url_error("".into());
                    let _ = welcome.hide();
                }
                let request = PendingWelcomeOpen::Url(value.to_string());
                if app.get_engine_ready() {
                    dispatch_welcome_open(&app, request, &open_url);
                } else {
                    *url_pending.borrow_mut() = Some(request);
                }
            } else {
                media_open::url(&app, value.as_str(), open_url.clone());
            }
        }
    });
    let weak = app.as_weak();
    app.on_open_help_link(move |destination| {
        let Some(app) = weak.upgrade() else {
            return;
        };
        let Some(url) = help_menu_url(destination.as_str()) else {
            return;
        };
        if let Err(error) = crate::platform::open_http_url_in_browser(url) {
            app.set_status(format!("Could not open the Help link: {error}").into());
        }
    });
    let weak = app.as_weak();
    let debug_playback = latest_playback.clone();
    app.on_dump_debug_info(move || {
        let Some(app) = weak.upgrade() else {
            return;
        };
        let snapshot = match debug_playback.lock() {
            Ok(playback) => playback.clone(),
            Err(_) => {
                app.set_status("Could not read player state for the debug dump".into());
                return;
            }
        };
        match crate::debug_dump::write_download(&snapshot) {
            Ok(path) => {
                let _ = rfd::MessageDialog::new()
                    .set_title("Debug Info Saved")
                    .set_description(format!(
                        "Debug information was saved to {}. The file may contain media filenames and URLs. Review it before sharing.",
                        path.display()
                    ))
                    .set_buttons(rfd::MessageButtons::Ok)
                    .show();
            }
            Err(error) => app.set_status(format!("Could not save debug information: {error}").into()),
        }
    });
    let recent_player = player.command_sender();
    let recent_host = open_host.clone();
    let recent_media_for_open = recent_media.clone();
    let weak = app.as_weak();
    let recent_welcome = welcome_window.clone();
    let recent_pending = pending_welcome_open.clone();
    app.on_open_recent(move |index| {
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        let media = recent_media_for_open.borrow().get(index).cloned();
        if let (Some(app), Some(media)) = (weak.upgrade(), media) {
            let from_welcome = recent_welcome
                .as_ref()
                .and_then(slint::Weak::upgrade)
                .is_some_and(|welcome| welcome.window().is_visible());
            if from_welcome {
                set_native_video_host_visible(recent_host.as_ref(), true);
                if let Err(error) = app.show() {
                    app.set_status(error.to_string().into());
                    return;
                }
                if let Some(welcome) = recent_welcome.as_ref().and_then(slint::Weak::upgrade) {
                    let _ = welcome.hide();
                }
                let request = PendingWelcomeOpen::Recent(media, app.get_resume_last_position());
                if app.get_engine_ready() {
                    dispatch_welcome_open(&app, request, &recent_player);
                } else {
                    *recent_pending.borrow_mut() = Some(request);
                }
            } else {
                media_open::open_recent(
                    &app,
                    media,
                    app.get_resume_last_position(),
                    recent_player.clone(),
                );
            }
        }
    });
    let opened_documents = recent_open_documents.clone();
    let opened_preferences_path = prefs_path.clone();
    let opened_app = app.as_weak();
    let opened_welcome = welcome_window.clone();
    app.on_media_opened(move |path, title| {
        if let Some(app) = opened_app.upgrade()
            && app.get_track_all_files_in_recent_open_menu()
        {
            remember_recent_open_document(
                &app,
                &opened_documents,
                &opened_preferences_path,
                path.to_string(),
                title.to_string(),
            );
        }
        if let Some(welcome) = opened_welcome.as_ref().and_then(slint::Weak::upgrade) {
            let _ = welcome.hide();
        }
    });
    let weak = app.as_weak();
    app.window().on_close_requested(move || {
        if weak.upgrade().is_some_and(|app| app.get_dialog_open()) {
            slint::CloseRequestResponse::KeepWindowShown
        } else {
            if let Some(app) = weak.upgrade() {
                app.invoke_dismiss_font_picker();
                // Remove the borrowed texture before hide suspends its context.
                // Teardown runs inside the backend's window borrow and cannot
                // safely dirty a property that schedules another redraw.
                app.set_video_frame(slint::Image::default());
                app.set_engine_ready(false);
                app.invoke_close_player();
            }
            slint::CloseRequestResponse::HideWindow
        }
    });
    let minimize_window = app.as_weak();
    app.on_minimize_window(move || {
        if let Some(app) = minimize_window.upgrade() {
            platform::minimize_window(app.window());
        }
    });
    let zoom_window = app.as_weak();
    app.on_zoom_window(move || {
        if let Some(app) = zoom_window.upgrade() {
            platform::toggle_zoom_window(app.window());
        }
    });
    let close_player = app.as_weak();
    app.on_close_player(move || {
        if let Some(app) = close_player.upgrade() {
            app.invoke_window_will_close();
            app.invoke_stop();
        }
        let _ = slint::quit_event_loop();
    });

    if let Some(welcome) = welcome {
        let weak = app.as_weak();
        welcome.on_open_file(move || {
            if let Some(app) = weak.upgrade() {
                app.invoke_open_file();
            }
        });
        let weak = app.as_weak();
        welcome.on_open_url(move |value| {
            if let Some(app) = weak.upgrade() {
                app.invoke_open_url(value);
            }
        });
        let weak = app.as_weak();
        welcome.on_open_recent(move |index| {
            if let Some(app) = weak.upgrade() {
                app.invoke_open_recent(index);
            }
        });
    }

    if prefs
        .audio_equalizer
        .gains_db
        .iter()
        .any(|gain| *gain != 0.0)
    {
        player
            .send(Command::AudioEqualizer(prefs.audio_equalizer.gains_db))
            .map_err(std::io::Error::other)?;
    }
    player
        .send(Command::SetVolumeBoost(prefs.volume_boost))
        .map_err(std::io::Error::other)?;
    player
        .send(Command::SetPauseWhenOpen(prefs.pause_when_open))
        .map_err(std::io::Error::other)?;
    player
        .send(Command::HardwareDecoding(prefs.hardware_decoder))
        .map_err(std::io::Error::other)?;
    player
        .send(Command::SubtitleAssOverride {
            lane: riina_core::subtitle_controls::SubtitleLane::Primary,
            level: prefs.subtitle_ass_override,
        })
        .map_err(std::io::Error::other)?;
    player
        .send(Command::SubtitleAssOverride {
            lane: riina_core::subtitle_controls::SubtitleLane::Secondary,
            level: prefs.secondary_subtitle_ass_override,
        })
        .map_err(std::io::Error::other)?;
    player
        .send(Command::SubtitleFont(prefs.subtitle_font.clone()))
        .map_err(std::io::Error::other)?;
    player
        .send(Command::SubtitleFontSize(prefs.subtitle_font_size))
        .map_err(std::io::Error::other)?;
    for command in crate::subtitle_preferences::commands(prefs.subtitle_text_style) {
        player.send(command).map_err(std::io::Error::other)?;
    }
    for command in prefs.subtitle_position.commands() {
        player.send(command).map_err(std::io::Error::other)?;
    }
    for command in prefs.subtitle_other.commands() {
        player.send(command).map_err(std::io::Error::other)?;
    }
    crate::fullscreen_info::connect(app);
    if native.is_some() {
        app.set_engine_ready(true);
        player
            .send(Command::Volume(prefs.volume))
            .map_err(std::io::Error::other)?;
        if !media.is_empty() {
            media_open::startup_at(app, media, startup_resume_position, player.command_sender());
        }
        return Ok(UiSession {
            player: Some(player),
            #[cfg(target_os = "windows")]
            _plugin_overlay_presenter: plugin_overlay_presenter,
            #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
            _plugin_sidebar_presenter: plugin_sidebar_presenter,
            #[cfg(target_os = "macos")]
            _system_media_controls: system_media_controls,
            #[cfg(target_os = "macos")]
            _system_media_controls_timer: system_media_controls_timer,
            #[cfg(target_os = "macos")]
            _picture_in_picture: picture_in_picture,
            #[cfg(target_os = "macos")]
            _picture_in_picture_timer: picture_in_picture_timer,
            #[cfg(target_os = "macos")]
            _mac_scroll_event_monitor: mac_scroll_event_monitor,
            #[cfg(target_os = "macos")]
            _mac_plugin_menu_observer: mac_plugin_menu_observer,
            _presentation: native,
            _hdr_display_timer: hdr_display_timer,
            _live_text_timer: live_text_timer,
            _appearance_timer: appearance_timer,
            _pause_resume_timer: pause_resume_timer,
            _thumbnail_preview_timer: thumbnail_preview_timer,
            _transition_frame_timer: transition_frame_timer,
            _native_material_timer: native_material_timer,
            #[cfg(target_os = "windows")]
            _windows_materials_policy_timer: windows_materials_policy_timer,
            _subtitle_priority_timer: subtitle_priority_timer.clone(),
            _online_subtitles_window: online_subtitles_window,
            _online_subtitles_timer: online_subtitles_timer,
            _playback_history_window: playback_history_window,
            _playback_history_timer: playback_history_timer,
            _video_filters_window: video_filters_window,
            _video_filters_timer: video_filters_timer,
            _audio_filters_window: audio_filters_window,
            _audio_filters_timer: audio_filters_timer,
            _playlist_metadata_timer: playlist_metadata_timer,
            _file_drop_timer: file_drop_timer,
            _player_window_focus: player_window_focus,
            _plugin_host: plugin_host,
            _plugin_host_timer: plugin_host_timer,
            _plugin_osd_timer: plugin_osd_timer,
            preference_writer: crate::preference_store::Writer::new(
                prefs_path.clone(),
                prefs.clone(),
            ),
            prefs_path,
            prefs,
            audio_equalizer,
            saved_video_filters,
            saved_audio_filters,
            recent_media,
            recent_open_documents,
            recent_open_cleared,
            playback_history,
            record_playback_history,
            history_cleared,
            latest_playback,
        });
    }
    let pending_media = Rc::new(RefCell::new(media));
    let pending_resume_position = Rc::new(Cell::new(startup_resume_position));
    let render_weak = app.as_weak();
    let render_player = player.clone();
    let render_resume_position = pending_resume_position.clone();
    let mut surface: Option<VideoSurface> = None;
    #[cfg(target_os = "macos")]
    let mut icc_profile_display: Option<(usize, Option<Vec<u8>>, bool)> = None;
    #[cfg(target_os = "macos")]
    let mut icc_profile_auto_active: Option<bool> = None;
    let mut blur_surface: Option<crate::video_blur::VideoBlurSurface> = None;
    let render_generation = Rc::new(Cell::new(0_u64));
    let mut started = false;
    let repaint_pending = Arc::new(AtomicBool::new(false));
    let volume = prefs.volume;
    #[cfg(target_os = "macos")]
    let render_picture_in_picture = picture_in_picture.clone();
    app.window()
        .set_rendering_notifier(move |state, graphics_api| {
            match state {
                slint::RenderingState::RenderingSetup => {
                    render_generation.set(render_generation.get().wrapping_add(1));
                    if let Some(app) = render_weak.upgrade()
                        && app.get_native_video()
                    {
                        // Windows native playback is already presented by mpv's
                        // D3D11 child window. Creating libmpv's OpenGL render API
                        // context for that same player produces a second video
                        // feed above it and leaves doubled frames during handoff.
                        app.set_engine_ready(true);
                        if !started {
                            started = true;
                            let _ = render_player.send(Command::Volume(volume));
                            let paths = std::mem::take(&mut *pending_media.borrow_mut());
                            if !paths.is_empty() {
                                media_open::startup_at(
                                    &app,
                                    paths,
                                    render_resume_position.take(),
                                    render_player.command_sender(),
                                );
                            }
                        }
                    } else if let slint::GraphicsAPI::NativeOpenGL { get_proc_address } =
                        graphics_api
                    {
                        let weak = render_weak.clone();
                        let pending = repaint_pending.clone();
                        // SAFETY: notifier guarantees that this GL context is current.
                        match unsafe {
                            VideoSurface::new(
                                render_player.clone(),
                                &get_proc_address,
                                Box::new(move || {
                                    if pending.swap(true, Ordering::AcqRel) {
                                        return;
                                    }
                                    let weak = weak.clone();
                                    let pending = pending.clone();
                                    let _ = slint::invoke_from_event_loop(move || {
                                        pending.store(false, Ordering::Release);
                                        if let Some(app) = weak.upgrade() {
                                            app.window().request_redraw();
                                        }
                                    });
                                }),
                            )
                        } {
                            Ok(video) => {
                                // The video-backed material is an optional visual enhancement:
                                // failure to create it must not prevent playback.
                                match unsafe {
                                    crate::video_blur::VideoBlurSurface::new(&get_proc_address)
                                } {
                                    Ok(blur) => blur_surface = Some(blur),
                                    Err(error) => eprintln!("Could not initialize video blur: {error}"),
                                }
                                #[cfg(target_os = "macos")]
                                if let Some(manager) =
                                    render_picture_in_picture.borrow_mut().as_mut()
                                    && let Err(error) =
                                        unsafe { manager.configure_rendering(&get_proc_address) }
                                    && let Some(app) = render_weak.upgrade()
                                {
                                    app.set_status(
                                        format!("PiP renderer initialization failed: {error}")
                                            .into(),
                                    );
                                }
                                surface = Some(video);
                                if let Some(app) = render_weak.upgrade() {
                                    app.set_engine_ready(true);
                                }
                            }
                            Err(error) => {
                                if let Some(app) = render_weak.upgrade() {
                                    app.set_status(
                                        format!("Video initialization failed: {error}").into(),
                                    );
                                }
                            }
                        }
                    }
                }
                slint::RenderingState::BeforeRendering => {
                    if let (Some(video), Some(app)) = (surface.as_mut(), render_weak.upgrade()) {
                        #[cfg(target_os = "macos")]
                        if let Some(display) = platform::display_identity(&app.window()) {
                            if !app.get_load_icc_profile()
                                && let Some((cached_display, profile, _)) =
                                    icc_profile_display.take()
                            {
                                icc_profile_display =
                                    Some((cached_display, profile, false));
                            }
                            let needs_profile = icc_profile_display
                                .as_ref()
                                .is_none_or(|(cached_display, _, attempted)| {
                                    *cached_display != display
                                        || (app.get_load_icc_profile() && !attempted)
                                });
                            if needs_profile {
                                let attempted = app.get_load_icc_profile();
                                let mut profile = attempted
                                    .then(|| platform::icc_profile_for_window(&app.window()))
                                    .flatten();
                                if let Some(bytes) = profile.as_deref() {
                                    // SAFETY: this callback runs on Slint's current GL render context.
                                    if let Err(error) = unsafe { video.set_icc_profile(bytes) } {
                                        eprintln!("Could not install the macOS display ICC profile: {error}");
                                        profile = None;
                                    }
                                }
                                icc_profile_display = Some((display, profile, attempted));
                            }
                            let profile_available = icc_profile_display
                                .as_ref()
                                .is_some_and(|(_, profile, _)| profile.is_some());
                            let hdr_output_active = app.get_hdr_source_detected()
                                && app.get_hdr_enabled()
                                && app.get_hdr_display_active();
                            let should_use_icc = app.get_load_icc_profile()
                                && profile_available
                                && !hdr_output_active;
                            if icc_profile_auto_active != Some(should_use_icc) {
                                if let Err(error) = render_player
                                    .send(Command::SetIccProfileAuto(should_use_icc))
                                {
                                    eprintln!("Could not update macOS display ICC processing: {error}");
                                } else {
                                    icc_profile_auto_active = Some(should_use_icc);
                                }
                            }
                        }
                        let scale = app.window().scale_factor();
                        let width = (app.get_render_width() * scale).clamp(1.0, 8192.0) as u32;
                        let height = (app.get_render_height() * scale).clamp(1.0, 8192.0) as u32;
                        // SAFETY: the notifier uses the same context throughout its lifetime.
                        match unsafe { video.render(width, height) } {
                            Ok(()) => {
                                let sidebar_blur = app.get_liquid_glass_sidebar()
                                    && (app.get_sidebar() != 0 || app.get_leading_sidebar() != 0);
                                let controller_blur = app.get_liquid_glass_osc()
                                    && app.get_loaded()
                                    && (app.get_controls_visible() || app.get_music_mode());
                                let osd_blur = app.get_liquid_glass_osd()
                                    && app.get_loaded()
                                    && (app.get_video_osd_visible()
                                        || app.get_seek_osd_visible()
                                        || !app.get_plugin_osd_message().is_empty()
                                        || (app.get_fullscreen()
                                            && app.get_controls_visible()
                                            && app.get_display_time_and_battery_in_fullscreen()));
                                let blur_enabled = app.get_system_materials_enabled()
                                    && (sidebar_blur || controller_blur || osd_blur);
                                if blur_enabled {
                                    if let Some(blur) = blur_surface.as_mut() {
                                        // SAFETY: both textures belong to this window's current GL context.
                                        match unsafe {
                                            blur.render(
                                                glow::NativeTexture(video.texture_id()),
                                                width,
                                                height,
                                            )
                                        } {
                                            Ok(()) => {
                                                let image = unsafe {
                                                    slint::BorrowedOpenGLTextureBuilder::new_gl_2d_rgba_texture(
                                                        blur.texture_id(),
                                                        (width.div_ceil(4), height.div_ceil(4)).into(),
                                                    )
                                                    .build()
                                                };
                                                set_video_material_sample(&app, image, true);
                                            }
                                            Err(error) => eprintln!("Could not render video blur: {error}"),
                                        }
                                    }
                                } else {
                                    set_video_material_sample(&app, slint::Image::default(), false);
                                }
                                let image = unsafe {
                                    slint::BorrowedOpenGLTextureBuilder::new_gl_2d_rgba_texture(
                                        video.texture_id(),
                                        (width, height).into(),
                                    )
                                    .build()
                                };
                                app.set_video_frame(image);
                                #[cfg(target_os = "macos")]
                                if let Some(manager) =
                                    render_picture_in_picture.borrow_mut().as_mut()
                                    && let Err(error) = unsafe { manager.render_frame(video) }
                                {
                                    app.set_status(
                                        format!("PiP frame rendering failed: {error}").into(),
                                    );
                                }
                                if !started {
                                    started = true;
                                    let _ = render_player.send(Command::Volume(volume));
                                    let paths = std::mem::take(&mut *pending_media.borrow_mut());
                                    if !paths.is_empty() {
                                        media_open::startup_at(
                                            &app,
                                            paths,
                                            render_resume_position.take(),
                                            render_player.command_sender(),
                                        );
                                    }
                                }
                            }
                            Err(error) => {
                                app.set_status(format!("Video rendering failed: {error}").into())
                            }
                        }
                    }
                }
                slint::RenderingState::AfterRendering => {
                    if let Some(video) = surface.as_ref() {
                        // Slint presents immediately after this notifier; report at its
                        // submission boundary so libmpv can pace the next video frame.
                        // SAFETY: same live renderer and context as the composed frame.
                        unsafe { video.report_swap() };
                    }
                }
                slint::RenderingState::RenderingTeardown => {
                    #[cfg(target_os = "macos")]
                    if let Some(manager) = render_picture_in_picture.borrow_mut().as_mut() {
                        // SAFETY: RenderingTeardown runs before Slint releases its GL context.
                        unsafe { manager.teardown_rendering() };
                    }
                    drop(blur_surface.take());
                    drop(surface.take());
                    #[cfg(target_os = "macos")]
                    {
                        icc_profile_display = None;
                        icc_profile_auto_active = None;
                    }
                    // Context loss also needs UI invalidation, but only after
                    // the backend releases its mutable window borrow. A later
                    // setup must not have its new frame cleared by stale work.
                    let retired_generation = render_generation.get();
                    let generation = render_generation.clone();
                    let weak = render_weak.clone();
                    slint::Timer::single_shot(Duration::ZERO, move || {
                        if generation.get() == retired_generation
                            && let Some(app) = weak.upgrade()
                            && !app.get_native_video()
                        {
                            app.set_video_frame(slint::Image::default());
                            set_video_material_sample(&app, slint::Image::default(), false);
                            app.set_engine_ready(false);
                        }
                    });
                }
                _ => {}
            }
        })?;
    Ok(UiSession {
        player: Some(player),
        #[cfg(target_os = "windows")]
        _plugin_overlay_presenter: plugin_overlay_presenter,
        #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
        _plugin_sidebar_presenter: plugin_sidebar_presenter,
        #[cfg(target_os = "macos")]
        _system_media_controls: system_media_controls,
        #[cfg(target_os = "macos")]
        _system_media_controls_timer: system_media_controls_timer,
        #[cfg(target_os = "macos")]
        _picture_in_picture: picture_in_picture,
        #[cfg(target_os = "macos")]
        _picture_in_picture_timer: picture_in_picture_timer,
        #[cfg(target_os = "macos")]
        _mac_scroll_event_monitor: mac_scroll_event_monitor,
        #[cfg(target_os = "macos")]
        _mac_plugin_menu_observer: mac_plugin_menu_observer,
        _presentation: None,
        _hdr_display_timer: None,
        _live_text_timer: live_text_timer,
        _appearance_timer: appearance_timer,
        _pause_resume_timer: pause_resume_timer,
        _thumbnail_preview_timer: thumbnail_preview_timer,
        _transition_frame_timer: transition_frame_timer,
        _native_material_timer: native_material_timer,
        #[cfg(target_os = "windows")]
        _windows_materials_policy_timer: windows_materials_policy_timer,
        _subtitle_priority_timer: subtitle_priority_timer,
        _online_subtitles_window: online_subtitles_window,
        _online_subtitles_timer: online_subtitles_timer,
        _playback_history_window: playback_history_window,
        _playback_history_timer: playback_history_timer,
        _video_filters_window: video_filters_window,
        _video_filters_timer: video_filters_timer,
        _audio_filters_window: audio_filters_window,
        _audio_filters_timer: audio_filters_timer,
        _playlist_metadata_timer: playlist_metadata_timer,
        _file_drop_timer: file_drop_timer,
        _player_window_focus: player_window_focus,
        _plugin_host: plugin_host,
        _plugin_host_timer: plugin_host_timer,
        _plugin_osd_timer: plugin_osd_timer,
        preference_writer: crate::preference_store::Writer::new(prefs_path.clone(), prefs.clone()),
        prefs_path,
        prefs,
        audio_equalizer,
        saved_video_filters,
        saved_audio_filters,
        recent_media,
        recent_open_documents,
        recent_open_cleared,
        playback_history,
        record_playback_history,
        history_cleared,
        latest_playback,
    })
}

#[cfg(test)]
mod control_auto_hide_tests {
    use super::control_hide_is_allowed;

    #[test]
    fn deadline_hides_only_an_idle_loaded_controller() {
        assert!(control_hide_is_allowed(
            true, true, false, false, false, false, false
        ));
        let blocked_states = [
            (false, true, false, false, false, false, false),
            (true, false, false, false, false, false, false),
            (true, true, true, false, false, false, false),
            (true, true, false, true, false, false, false),
            (true, true, false, false, true, false, false),
            (true, true, false, false, false, true, false),
            (true, true, false, false, false, false, true),
        ];
        for (enabled, loaded, dialog, crop, title, controller, dragging) in blocked_states {
            assert!(!control_hide_is_allowed(
                enabled, loaded, dialog, crop, title, controller, dragging
            ));
        }
    }
}

#[cfg(test)]
mod audio_device_menu_tests {
    use super::{audio_device_menu_rows, should_fallback_removed_audio_device};
    use riina_core::{Phase, Playback, audio_preferences::OutputDevice};

    #[test]
    fn device_menu_uses_native_ids_and_marks_the_observed_selection() {
        let state = Playback {
            audio_output: Some("wasapi".into()),
            audio_device: Some("wasapi/{speaker-id}".into()),
            audio_devices: vec![
                OutputDevice {
                    name: "auto".into(),
                    description: "Autoselect device".into(),
                },
                OutputDevice {
                    name: "wasapi/{speaker-id}".into(),
                    description: "Speakers (USB Audio)".into(),
                },
                OutputDevice {
                    name: "dsound/{speaker-id}".into(),
                    description: "Incompatible DirectSound device".into(),
                },
            ],
            ..Playback::default()
        };
        let rows = audio_device_menu_rows(&state);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].token.as_str(), "auto");
        assert!(!rows[0].selected);
        assert_eq!(rows[1].token.as_str(), "wasapi/{speaker-id}");
        assert_eq!(rows[1].label.as_str(), "Speakers (USB Audio)");
        assert!(rows[1].selected);
    }

    #[test]
    fn device_menu_uses_native_id_when_description_is_missing() {
        let state = Playback {
            audio_devices: vec![OutputDevice {
                name: "wasapi/{speaker-id}".into(),
                description: "  ".into(),
            }],
            ..Playback::default()
        };
        let rows = audio_device_menu_rows(&state);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label.as_str(), "wasapi/{speaker-id}");
    }

    #[test]
    fn removed_device_falls_back_once_only_for_active_playback() {
        let previous = Playback {
            phase: Phase::Playing,
            audio_device: Some("wasapi/{speaker-id}".into()),
            audio_devices: vec![OutputDevice {
                name: "wasapi/{speaker-id}".into(),
                description: "Speakers".into(),
            }],
            ..Playback::default()
        };
        let removed = Playback {
            phase: Phase::Playing,
            audio_device: previous.audio_device.clone(),
            audio_devices: vec![OutputDevice {
                name: "wasapi/{headphones-id}".into(),
                description: "Headphones".into(),
            }],
            ..Playback::default()
        };
        assert!(should_fallback_removed_audio_device(
            Some(&previous),
            &removed
        ));
        assert!(!should_fallback_removed_audio_device(
            Some(&removed),
            &removed
        ));

        let stopped = Playback {
            phase: Phase::Stopped,
            ..removed.clone()
        };
        assert!(!should_fallback_removed_audio_device(
            Some(&previous),
            &stopped
        ));
        let no_devices = Playback {
            audio_devices: Vec::new(),
            ..removed
        };
        assert!(!should_fallback_removed_audio_device(
            Some(&previous),
            &no_devices
        ));
    }
}

#[cfg(test)]
mod welcome_tests {
    use super::{
        recent_document_rows, recent_media_rows, resume_engine_options, resume_media_rows,
    };
    use riina_core::RecentMedia;

    #[test]
    fn resume_item_is_not_repeated_in_recent_documents() {
        let history = [
            RecentMedia {
                path: "resume.mkv".into(),
                title: "Resume item".into(),
                position: 12.0,
            },
            RecentMedia {
                path: "older.mkv".into(),
                title: "Older item".into(),
                position: 3.0,
            },
        ];

        let resume = recent_media_rows(&history);
        let resume_action = resume_media_rows(&history, true);
        let documents = recent_document_rows(&history, true);

        assert_eq!(resume[0].title.as_str(), "Resume item");
        assert_eq!(resume_action[0].title.as_str(), "Resume item");
        assert_eq!(documents.len(), 1);
        assert_eq!(documents[0].title.as_str(), "Older item");

        assert!(resume_media_rows(&history, false).is_empty());
        let all_documents = recent_document_rows(&history, false);
        assert_eq!(all_documents.len(), 2);
        assert_eq!(all_documents[0].title.as_str(), "Resume item");
    }

    #[test]
    fn startup_options_configure_resume_and_subtitle_autoload() {
        let directory = std::path::Path::new("profile/watch-later");
        let enabled = resume_engine_options(
            directory,
            true,
            &riina_core::subtitle_controls::AutoLoadPreferences::default(),
            &riina_core::network_preferences::Connection::default(),
            &riina_core::network_preferences::Cache::default(),
            &riina_core::network_preferences::OnlineVideos::default(),
        )
        .unwrap();
        assert_eq!(enabled[0].0, "watch-later-directory");
        assert_eq!(enabled[0].1, directory.to_string_lossy());
        assert_eq!(enabled[1], ("save-position-on-quit".into(), "yes".into()));
        assert_eq!(enabled[2], ("resume-playback".into(), "yes".into()));
        assert_eq!(enabled[3], ("sub-auto".into(), "exact".into()));
        assert_eq!(enabled[4], ("sub-file-paths".into(), "".into()));
        assert_eq!(enabled[5], ("rtsp-transport".into(), "tcp".into()));
        assert_eq!(enabled[6], ("cache".into(), "yes".into()));
        assert_eq!(enabled[9], ("cache-secs".into(), "36000".into()));
        assert_eq!(enabled[11], ("ytdl".into(), "yes".into()));
        assert_eq!(enabled[12], ("ytdl-raw-options".into(), "".into()));

        let disabled = resume_engine_options(
            directory,
            false,
            &riina_core::subtitle_controls::AutoLoadPreferences::default(),
            &riina_core::network_preferences::Connection::default(),
            &riina_core::network_preferences::Cache::default(),
            &riina_core::network_preferences::OnlineVideos::default(),
        )
        .unwrap();
        assert_eq!(disabled[1].1, "no");
        assert_eq!(disabled[2].1, "no");
    }
}

#[cfg(test)]
mod window_video_geometry_tests {
    use super::{media_geometry_requires_resize, window_video_geometry};
    use riina_core::{Phase, Playback, VideoParameters};

    fn video(width: u32, height: u32, display: Option<(u32, u32)>) -> VideoParameters {
        VideoParameters {
            pixelformat: None,
            w: width,
            h: height,
            dw: display.map(|(width, _)| width),
            dh: display.map(|(_, height)| height),
            rotate: Some(90),
            colormatrix: None,
            colorlevels: None,
            primaries: None,
            gamma: None,
            max_luma: None,
            min_luma: None,
            signal_peak: None,
        }
    }

    #[test]
    fn natural_size_uses_display_dimensions_and_observed_rotation() {
        let state = Playback {
            phase: Phase::Playing,
            video: Some(video(640, 360, Some((1280, 720)))),
            video_rotation: Some(180),
            ..Playback::default()
        };
        assert_eq!(window_video_geometry(&state), Some((1280.0, 720.0, 180)));
    }

    #[test]
    fn natural_size_falls_back_to_source_dimensions_when_display_size_is_zero() {
        let state = Playback {
            phase: Phase::Playing,
            video: Some(video(640, 360, Some((0, 0)))),
            ..Playback::default()
        };
        assert_eq!(window_video_geometry(&state), Some((640.0, 360.0, 90)));
    }

    #[test]
    fn coalesced_file_change_resizes_even_without_a_loading_snapshot() {
        let state = Playback {
            generation: 2,
            phase: Phase::Playing,
            video: Some(video(640, 480, None)),
            ..Playback::default()
        };
        assert!(media_geometry_requires_resize(Some(1), false, &state));
        assert!(!media_geometry_requires_resize(Some(2), false, &state));
    }

    #[test]
    fn late_geometry_resizes_once_in_the_same_media_generation() {
        let mut state = Playback {
            generation: 2,
            phase: Phase::Playing,
            ..Playback::default()
        };
        assert!(!media_geometry_requires_resize(Some(1), true, &state));
        state.video = Some(video(640, 480, None));
        assert!(media_geometry_requires_resize(Some(2), true, &state));
        assert!(!media_geometry_requires_resize(Some(2), false, &state));
        state.phase = Phase::Loading;
        assert!(!media_geometry_requires_resize(Some(1), true, &state));
    }

    #[test]
    fn missing_or_zero_geometry_does_not_enable_video_size_commands() {
        assert_eq!(window_video_geometry(&Playback::default()), None);
        let state = Playback {
            phase: Phase::Playing,
            video: Some(video(0, 360, None)),
            ..Playback::default()
        };
        assert_eq!(window_video_geometry(&state), None);

        for phase in [Phase::Loading, Phase::Stopped, Phase::Failed] {
            let state = Playback {
                phase,
                video: Some(video(640, 360, None)),
                ..Playback::default()
            };
            assert_eq!(window_video_geometry(&state), None);
        }
    }
}
