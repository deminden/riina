//! Native video samples feed only the SDR control materials.
use crate::PlayerWindow;
use crate::platform::{
    self,
    video_materials::{native_blur, set_video_material_sample},
};
use riina_core::{Command, Phase, Playback};
use slint::ComponentHandle;
use std::{
    cell::Cell,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

// A 4 fps clip changes frames every 250 ms; sampling at 10 Hz avoids skipping
// alternating frames while keeping screenshot-raw readback below display rate.
const NATIVE_MATERIAL_SAMPLE_INTERVAL: Duration = Duration::from_millis(100);
// Material-frame replies arrive asynchronously. Poll them at the UI cadence so
// a completed capture does not wait another full sampling interval to display.
const NATIVE_MATERIAL_UPDATE_INTERVAL: Duration = Duration::from_millis(16);
const NATIVE_MATERIAL_SAMPLE_TIMEOUT: Duration = Duration::from_millis(750);
const NATIVE_MATERIAL_CAPTURE_STARTUP_GRACE: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MaterialSampleSource {
    Wgc,
    Mpv,
}

impl MaterialSampleSource {
    fn label(self) -> &'static str {
        match self {
            Self::Wgc => "wgc",
            Self::Mpv => "mpv",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MaterialFrameKey {
    generation: u64,
    revision: u64,
    source: MaterialSampleSource,
}

struct MaterialTransformRequest {
    epoch: u64,
    key: MaterialFrameKey,
    frame: Arc<riina_core::CapturedFrame>,
    captured_at: Option<Instant>,
}

struct MaterialTransformResult {
    epoch: u64,
    key: MaterialFrameKey,
    captured_at: Option<Instant>,
    transform_time: Duration,
    pixels: Result<native_blur::SampledFramePixels, String>,
}

fn start_material_transform_worker() -> (
    std::sync::mpsc::SyncSender<MaterialTransformRequest>,
    std::sync::mpsc::Receiver<MaterialTransformResult>,
) {
    let (request_sender, request_receiver) =
        std::sync::mpsc::sync_channel::<MaterialTransformRequest>(1);
    let (result_sender, result_receiver) =
        std::sync::mpsc::sync_channel::<MaterialTransformResult>(1);
    let worker = crate::background::spawn_named("riina-material-transform", move || {
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
            let pixels = native_blur::sampled_frame_pixels(request.frame.as_ref());
            if std::env::var_os("RIINA_TRACE_MATERIAL_PIXELS").is_some()
                && let (Some(input_center), Ok(output)) = (input_center, pixels.as_ref())
            {
                let output_offset =
                    ((output.height / 2 * output.width + output.width / 2) * 4) as usize;
                if let Some(rgb) = output.rgba.get(output_offset..output_offset + 3) {
                    crate::diagnostics::warn!(
                        "Windows material center source={} input={:02X}{:02X}{:02X} blurred={:02X}{:02X}{:02X}",
                        request.key.source.label(),
                        input_center[0],
                        input_center[1],
                        input_center[2],
                        rgb[0],
                        rgb[1],
                        rgb[2],
                    );
                }
            }
            let result = MaterialTransformResult {
                epoch: request.epoch,
                key: request.key,
                captured_at: request.captured_at,
                transform_time: started.elapsed(),
                pixels,
            };
            // The UI consumes at display cadence and may already own a pending result.
            let _ = result_sender.try_send(result);
        }
    });
    if let Err(error) = worker {
        crate::diagnostics::warn!("Could not start the Windows material transform worker: {error}");
    }
    (request_sender, result_receiver)
}

pub(crate) fn install_windows_material_policy_monitor(app: &PlayerWindow) -> Rc<slint::Timer> {
    let timer = Rc::new(slint::Timer::default());
    let weak = app.as_weak();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(100),
        move || {
            let Some(app) = weak.upgrade() else { return };
            let enabled = crate::platform::native_materials_enabled();
            if let Some(accent) = crate::platform::windows_accent_color()
                && app
                    .global::<riina_ui::PlayerAppearance>()
                    .get_windows_accent_color()
                    != accent
            {
                app.global::<riina_ui::PlayerAppearance>()
                    .set_windows_accent_color(accent);
            }
            if let Some(foreground) = crate::platform::windows_accent_foreground()
                && app
                    .global::<riina_ui::PlayerAppearance>()
                    .get_windows_accent_foreground()
                    != foreground
            {
                app.global::<riina_ui::PlayerAppearance>()
                    .set_windows_accent_foreground(foreground);
            }
            if app
                .global::<riina_ui::PlayerAppearance>()
                .get_system_materials_enabled()
                != enabled
            {
                app.global::<riina_ui::PlayerAppearance>()
                    .set_system_materials_enabled(enabled);
                if !enabled {
                    set_video_material_sample(&app, slint::Image::default(), false);
                }
            }
        },
    );
    timer
}

pub(crate) struct MaterialTimer {
    timer: Rc<slint::Timer>,
    sampler: Rc<MaterialSampler>,
}

impl MaterialTimer {
    pub(crate) fn observe(&self, app: &PlayerWindow, state: &Playback) {
        self.sampler.observe_generation(app, state);
        if app.get_native_video_material_visible()
            && playback_active(state)
            && !self.timer.running()
        {
            self.timer.restart();
        }
    }
}

pub(crate) fn install_native_video_material_sampler(
    app: &PlayerWindow,
    sender: riina_mpv::CommandSender,
    latest: crate::PlaybackStore,
    capture: Option<platform::NativeVideoMaterialCapture>,
) -> MaterialTimer {
    let timer = Rc::new(slint::Timer::default());
    let weak = app.as_weak();
    let sampler = Rc::new(MaterialSampler::new(sender, latest, capture));
    let tick_sampler = sampler.clone();
    let tick_timer = Rc::downgrade(&timer);
    timer.start(
        slint::TimerMode::Repeated,
        NATIVE_MATERIAL_UPDATE_INTERVAL,
        move || {
            if weak.upgrade().is_some_and(|app| tick_sampler.tick(&app)) {
                return;
            }
            if let Some(timer) = tick_timer.upgrade() {
                timer.stop();
            }
        },
    );
    timer.stop();
    let visibility_timer = Rc::downgrade(&timer);
    let visibility_app = app.as_weak();
    let visibility_sampler = Rc::downgrade(&sampler);
    app.on_native_video_material_visibility_changed(move |visible| {
        if visible
            && let (Some(app), Some(timer), Some(sampler)) = (
                visibility_app.upgrade(),
                visibility_timer.upgrade(),
                visibility_sampler.upgrade(),
            )
        {
            let state = riina_core::sync::lock(&sampler.latest).clone();
            MaterialTimer { timer, sampler }.observe(&app, &state);
        }
    });
    MaterialTimer { timer, sampler }
}

struct MaterialSampler {
    sender: riina_mpv::CommandSender,
    latest: crate::PlaybackStore,
    capture: Option<platform::NativeVideoMaterialCapture>,
    transform_sender: std::sync::mpsc::SyncSender<MaterialTransformRequest>,
    transform_results: std::sync::mpsc::Receiver<MaterialTransformResult>,
    activity_epoch: Cell<u64>,
    last_request: Cell<Option<(u64, u64, Instant)>>,
    displayed: Cell<Option<MaterialFrameKey>>,
    displayed_at: Cell<Option<Instant>>,
    displayed_position: Cell<Option<f64>>,
    submitted: Cell<Option<MaterialFrameKey>>,
    capture_generation: Cell<Option<u64>>,
    capture_generation_started: Cell<Option<Instant>>,
    trace_material_samples: bool,
}

impl MaterialSampler {
    fn new(
        sender: riina_mpv::CommandSender,
        latest: crate::PlaybackStore,
        capture: Option<platform::NativeVideoMaterialCapture>,
    ) -> Self {
        let (transform_sender, transform_results) = start_material_transform_worker();
        Self {
            sender,
            latest,
            capture,
            transform_sender,
            transform_results,
            activity_epoch: Cell::new(0),
            last_request: Cell::new(None),
            displayed: Cell::new(None),
            displayed_at: Cell::new(None),
            displayed_position: Cell::new(None),
            submitted: Cell::new(None),
            capture_generation: Cell::new(None),
            capture_generation_started: Cell::new(None),
            trace_material_samples: std::env::var_os("RIINA_TRACE_MATERIAL").is_some(),
        }
    }

    fn tick(&self, app: &PlayerWindow) -> bool {
        let state = riina_core::sync::lock(&self.latest).clone();
        let completed_transform = self.transform_results.try_recv().ok();
        self.observe_generation(app, &state);
        if !app.get_native_video_material_visible() || !playback_active(&state) {
            self.clear_inactive_sample(app);
            return false;
        }
        if let Some(result) = completed_transform {
            self.publish_transform(app, &state, result);
        }
        if !self.poll_native_capture(&state) {
            self.poll_mpv_capture(app, &state);
        }
        true
    }

    fn observe_generation(&self, app: &PlayerWindow, state: &riina_core::Playback) {
        if self.capture_generation.get() != Some(state.generation) {
            if let Some(capture) = &self.capture {
                capture.clear_latest();
            }
            self.capture_generation.set(Some(state.generation));
            self.capture_generation_started.set(Some(Instant::now()));
            self.displayed.set(None);
            self.displayed_at.set(None);
            self.displayed_position.set(None);
            self.submitted.set(None);
            set_video_material_sample(app, slint::Image::default(), false);
        }
    }

    fn clear_inactive_sample(&self, app: &PlayerWindow) {
        self.activity_epoch
            .set(self.activity_epoch.get().wrapping_add(1));
        if let Some(capture) = &self.capture {
            capture.set_enabled(false);
        }
        if self.displayed.take().is_some() {
            set_video_material_sample(app, slint::Image::default(), false);
        }
        self.displayed_at.set(None);
        self.displayed_position.set(None);
        self.submitted.set(None);
        self.last_request.set(None);
    }

    fn publish_transform(
        &self,
        app: &PlayerWindow,
        state: &riina_core::Playback,
        result: MaterialTransformResult,
    ) {
        if result.epoch != self.activity_epoch.get() {
            return;
        }
        let key = result.key;
        if self.submitted.get() == Some(key) {
            self.submitted.set(None);
        }
        if key.generation == state.generation
            && self.displayed.get() != Some(key)
            && result
                .captured_at
                .is_some_and(|captured_at| captured_at.elapsed() <= NATIVE_MATERIAL_SAMPLE_TIMEOUT)
        {
            match result.pixels {
                Ok(pixels) => {
                    let image = native_blur::slint_image_from_sampled_pixels(pixels);
                    set_video_material_sample(app, image, true);
                    self.displayed.set(Some(key));
                    self.displayed_at.set(result.captured_at);
                    self.displayed_position.set(state.position);
                    if self.trace_material_samples {
                        crate::diagnostics::warn!(
                            "Windows material sample source={} generation={} revision={} age_ms={} transform_ms={}",
                            key.source.label(),
                            key.generation,
                            key.revision,
                            result
                                .captured_at
                                .map_or(0, |captured_at| { captured_at.elapsed().as_millis() }),
                            result.transform_time.as_millis(),
                        );
                    }
                }
                Err(error) => {
                    crate::diagnostics::warn!(
                        "Could not prepare the Windows video material frame: {error}"
                    );
                }
            }
        }
    }

    fn poll_native_capture(&self, state: &riina_core::Playback) -> bool {
        if let Some(capture) = &self.capture {
            capture.set_enabled(true);
            let sample = capture
                .latest()
                .filter(|sample| sample.captured_at.elapsed() <= NATIVE_MATERIAL_SAMPLE_TIMEOUT);
            if let Some(sample) = sample.as_ref() {
                let key = MaterialFrameKey {
                    generation: state.generation,
                    revision: sample.sequence,
                    source: MaterialSampleSource::Wgc,
                };
                if self.displayed.get() != Some(key) && self.submitted.get() != Some(key) {
                    let request = MaterialTransformRequest {
                        epoch: self.activity_epoch.get(),
                        key,
                        frame: sample.frame.clone(),
                        captured_at: Some(sample.captured_at),
                    };
                    if self.transform_sender.try_send(request).is_ok() {
                        self.submitted.set(Some(key));
                    }
                }
                self.last_request.set(None);
                return true;
            }
            let capture_warmup = self
                .capture_generation_started
                .get()
                .is_some_and(|started| started.elapsed() < NATIVE_MATERIAL_CAPTURE_STARTUP_GRACE);
            if capture_warmup {
                return true;
            }
        }
        false
    }

    fn poll_mpv_capture(&self, app: &PlayerWindow, state: &riina_core::Playback) {
        let now = Instant::now();
        let frame_key = MaterialFrameKey {
            generation: state.generation,
            revision: state.material_frame.revision,
            source: MaterialSampleSource::Mpv,
        };
        if state.material_frame.generation == Some(state.generation)
            && state.material_frame.captured_at.is_some_and(|captured_at| {
                now.saturating_duration_since(captured_at) <= NATIVE_MATERIAL_SAMPLE_TIMEOUT
            })
            && self.displayed.get() != Some(frame_key)
            && self.submitted.get() != Some(frame_key)
            && let Some(frame) = state.material_frame.frame.as_ref()
        {
            let request = MaterialTransformRequest {
                epoch: self.activity_epoch.get(),
                key: frame_key,
                frame: frame.clone(),
                captured_at: state.material_frame.captured_at,
            };
            if self.transform_sender.try_send(request).is_ok() {
                self.submitted.set(Some(frame_key));
            }
        }
        // A paused video can stop producing WGC frames; its last sample is
        // still valid while the playhead stays within one 4 fps frame.
        let paused_frame_still_matches = state.paused
            && state.position.is_some_and(|position| {
                self.displayed_position
                    .get()
                    .is_some_and(|displayed| (position - displayed).abs() <= 0.25)
            });
        if !paused_frame_still_matches
            && self.displayed.get().is_some_and(|key| {
                key.generation == state.generation
                    && self.displayed_at.get().is_none_or(|captured_at| {
                        now.saturating_duration_since(captured_at) > NATIVE_MATERIAL_SAMPLE_TIMEOUT
                    })
            })
        {
            self.displayed.set(None);
            self.displayed_at.set(None);
            self.displayed_position.set(None);
            set_video_material_sample(app, slint::Image::default(), false);
        }

        let paused_sample_current = state.paused
            && state.material_frame.generation == Some(state.generation)
            && state.material_frame.position == state.position
            && state.material_frame.captured_at.is_some_and(|captured_at| {
                now.saturating_duration_since(captured_at) <= NATIVE_MATERIAL_SAMPLE_TIMEOUT
            });
        if paused_sample_current {
            self.last_request.set(None);
            return;
        }
        let request_due =
            self.last_request
                .get()
                .is_none_or(|(generation, revision, requested_at)| {
                    let elapsed = now.saturating_duration_since(requested_at);
                    generation != state.generation
                        || (elapsed >= NATIVE_MATERIAL_SAMPLE_INTERVAL
                            && (state.material_frame.revision > revision
                                || elapsed >= NATIVE_MATERIAL_SAMPLE_TIMEOUT))
                });
        if request_due {
            let result = self.sender.send(Command::CaptureMaterialFrame);
            // Also throttle a rejected enqueue so a busy command queue cannot
            // turn this visual-only sampler into a 10 Hz log/command loop.
            self.last_request
                .set(Some((state.generation, state.material_frame.revision, now)));
            if let Err(error) = result {
                crate::diagnostics::warn!(
                    "Could not request a Windows video material sample: {error}"
                );
            }
        }
    }
}

fn playback_active(state: &Playback) -> bool {
    state.video.is_some() && matches!(state.phase, Phase::Playing | Phase::Paused | Phase::Seeking)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_sampler_wakes_for_visible_sdr_video_and_stops_under_hdr()
    -> Result<(), slint::PlatformError> {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new()?;
        let latest = crate::PlaybackStore::default();
        let (sender, recording) = riina_mpv::CommandSender::recording(4);
        let sampler = install_native_video_material_sampler(&app, sender, latest.clone(), None);
        assert!(!sampler.timer.running());

        app.set_native_video(true);
        app.set_loaded(true);
        let state = Arc::new(Playback {
            phase: Phase::Playing,
            video: Some(serde_json::from_value(serde_json::json!({ "w": 320, "h": 180 })).unwrap()),
            ..Playback::default()
        });
        *riina_core::sync::lock(&latest) = state.clone();
        sampler.observe(&app, &state);
        assert!(sampler.timer.running());
        i_slint_backend_testing::mock_elapsed_time(NATIVE_MATERIAL_UPDATE_INTERVAL);
        assert_eq!(
            recording.commands.try_recv(),
            Ok(Command::CaptureMaterialFrame)
        );

        app.global::<riina_ui::PlaybackVideoState>()
            .set_hdr_source_detected(true);
        i_slint_backend_testing::mock_elapsed_time(NATIVE_MATERIAL_UPDATE_INTERVAL);
        assert!(!sampler.timer.running());
        assert_eq!(
            recording.commands.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        );
        Ok(())
    }
}
