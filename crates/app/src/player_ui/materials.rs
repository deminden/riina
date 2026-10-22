//! Native video samples feed only the SDR control materials.
use crate::PlayerWindow;
#[cfg(target_os = "windows")]
use crate::platform;
#[cfg(target_os = "windows")]
use riina_core::{Command, Phase, Playback};
#[cfg(target_os = "windows")]
use slint::ComponentHandle;
#[cfg(target_os = "windows")]
use std::{
    cell::Cell,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

// A 4 fps clip changes frames every 250 ms; sampling at 10 Hz avoids skipping
// alternating frames while keeping screenshot-raw readback below display rate.
#[cfg(target_os = "windows")]
const NATIVE_MATERIAL_SAMPLE_INTERVAL: Duration = Duration::from_millis(100);
// Material-frame replies arrive asynchronously. Poll them at the UI cadence so
// a completed capture does not wait another full sampling interval to display.
#[cfg(target_os = "windows")]
const NATIVE_MATERIAL_UPDATE_INTERVAL: Duration = Duration::from_millis(16);
#[cfg(target_os = "windows")]
const NATIVE_MATERIAL_SAMPLE_TIMEOUT: Duration = Duration::from_millis(750);
#[cfg(target_os = "windows")]
const NATIVE_MATERIAL_CAPTURE_STARTUP_GRACE: Duration = Duration::from_millis(500);

pub(super) fn set_video_material_sample(app: &PlayerWindow, image: slint::Image, ready: bool) {
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
pub(super) fn install_windows_material_policy_monitor(app: &PlayerWindow) -> Rc<slint::Timer> {
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
pub(super) fn install_native_video_material_sampler(
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
