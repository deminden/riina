//! Keep screenshot file ownership and decoding separate from playback projection.
use super::{Capture, Scratch, decode};
use crate::{PlaybackStore, PlayerWindow, background::CoalescedUiCallback};
use riina_core::{CapturedFrame, Phase, Playback, sync::lock};
use riina_ui::{PlayerCropState, PlayerOsdState, PlayerPreferences};
use slint::ComponentHandle;
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

pub(crate) struct Output {
    app: slint::Weak<PlayerWindow>,
    playback: PlaybackStore,
    scratch: Arc<Scratch>,
    queue: Mutex<Queue>,
    delivery: CoalescedUiCallback,
}

#[derive(Default)]
struct Queue {
    processing: bool,
    next: Option<Request>,
    ready: Option<Ready>,
}

struct Request {
    capture: Capture,
    obsolete: Vec<String>,
    preview: bool,
    copy: bool,
}

struct Ready {
    capture: Capture,
    has_file: bool,
    preview: Option<Preview>,
    clipboard: Option<CapturedFrame>,
}

struct Preview {
    frame: CapturedFrame,
    width: f32,
    height: f32,
}

impl Output {
    pub(crate) fn new(app: &PlayerWindow, playback: PlaybackStore, scratch: Arc<Scratch>) -> Self {
        Self {
            app: app.as_weak(),
            playback,
            scratch,
            queue: Mutex::default(),
            delivery: CoalescedUiCallback::default(),
        }
    }

    pub(crate) fn submit(self: &Arc<Self>, app: &PlayerWindow, mut completed: Vec<Capture>) {
        let Some(capture) = completed.pop() else {
            return;
        };
        let prefs = app.global::<PlayerPreferences>();
        let request = Request {
            capture,
            obsolete: completed.into_iter().map(|capture| capture.path).collect(),
            preview: prefs.get_screenshot_show_preview() && shows_feedback(app),
            copy: prefs.get_screenshot_copy_to_clipboard(),
        };
        if !lock(&self.queue).push(request) {
            return;
        }
        let worker = self.clone();
        if let Err(error) = crate::background::spawn_named("riina-screenshot", move || worker.run())
        {
            let mut queue = lock(&self.queue);
            queue.processing = false;
            if let Some(mut request) = queue.next.take() {
                request.obsolete.push(request.capture.path);
                self.scratch.remove_completed(&request.obsolete);
            }
            drop(queue);
            crate::diagnostics::warn!("Could not process the screenshot: {error}");
            crate::player_ui::show_osd_message(app, "Screenshot Captured", false);
        }
    }

    fn run(self: Arc<Self>) {
        loop {
            let Some(request) = lock(&self.queue).next_request() else {
                return;
            };
            let ready = request.process(&self.scratch);
            lock(&self.queue).ready = Some(ready);
            self.delivery.post(|| {
                let worker = self.clone();
                move || worker.deliver()
            });
        }
    }

    fn deliver(&self) {
        let ready = lock(&self.queue).ready.take();
        let (Some(ready), Some(app)) = (ready, self.app.upgrade()) else {
            return;
        };
        if let Some(pixels) = &ready.clipboard
            && let Err(error) = crate::platform::clipboard::copy_pixels(pixels)
        {
            crate::diagnostics::report(&app, format!("Could not copy screenshot: {error}"));
        }
        if matches_current(&ready.capture, &lock(&self.playback)) && shows_feedback(&app) {
            ready.present(&app);
        }
    }
}

impl Queue {
    fn push(&mut self, mut request: Request) -> bool {
        if let Some(old) = self.next.take() {
            // A replaced request loses its visual feedback, and retains file cleanup.
            request.obsolete.extend(old.obsolete);
            request.obsolete.push(old.capture.path);
        }
        self.next = Some(request);
        !std::mem::replace(&mut self.processing, true)
    }

    fn next_request(&mut self) -> Option<Request> {
        let request = self.next.take();
        if request.is_none() {
            self.processing = false;
        }
        request
    }
}

impl Request {
    fn process(mut self, scratch: &Scratch) -> Ready {
        let path = Path::new(&self.capture.path);
        let has_file = !scratch.owns(path);
        let pixels = (self.preview || self.copy)
            .then(|| decode::load(path))
            .flatten();
        let preview = self
            .preview
            .then(|| pixels.as_ref().and_then(Preview::new))
            .flatten();
        if !has_file {
            self.obsolete.push(self.capture.path.clone());
        }
        scratch.remove_completed(&self.obsolete);
        Ready {
            capture: self.capture,
            has_file,
            preview,
            clipboard: pixels.filter(|_| self.copy),
        }
    }
}

impl Preview {
    fn new(pixels: &CapturedFrame) -> Option<Self> {
        let (width, height) = crate::screenshot_osd::preview_size(pixels.width, pixels.height);
        Some(Self {
            frame: pixels.downsample_to_fit(900, 600).ok()?,
            width,
            height,
        })
    }
}

impl Ready {
    fn present(self, app: &PlayerWindow) {
        let Some(preview) = self.preview.filter(|_| {
            app.global::<PlayerPreferences>()
                .get_screenshot_show_preview()
        }) else {
            // Capture succeeded even if its pixels could not be displayed.
            crate::player_ui::show_osd_message(app, "Screenshot Captured", false);
            return;
        };
        let pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
            &preview.frame.rgba,
            preview.frame.width,
            preview.frame.height,
        );
        let osd = app.global::<PlayerOsdState>();
        osd.set_screenshot_preview_image(slint::Image::from_rgba8(pixels));
        osd.set_screenshot_preview_width(preview.width);
        osd.set_screenshot_preview_height(preview.height);
        osd.set_screenshot_preview_path(if self.has_file {
            self.capture.path.into()
        } else {
            Default::default()
        });
        app.invoke_show_screenshot_preview();
    }
}

fn shows_feedback(app: &PlayerWindow) -> bool {
    crate::player::presentation::playback_osd_allowed(app)
        && !app.global::<PlayerCropState>().get_crop_editing()
}

fn matches_current(capture: &Capture, state: &Playback) -> bool {
    capture.generation == state.generation
        && capture.sequence == state.completed_screenshots
        && !matches!(state.phase, Phase::Loading | Phase::Stopped | Phase::Failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(sequence: u64) -> Request {
        Request {
            capture: Capture {
                generation: 3,
                sequence,
                path: format!("{sequence}.png"),
            },
            obsolete: Vec::new(),
            preview: true,
            copy: true,
        }
    }

    #[test]
    fn busy_worker_coalesces_new_feedback_and_keeps_retired_cleanup_paths() {
        let mut queue = Queue::default();
        assert!(queue.push(request(1)));
        assert_eq!(queue.next_request().unwrap().capture.sequence, 1);
        assert!(!queue.push(request(2)));
        assert!(!queue.push(request(3)));
        let next = queue.next_request().unwrap();
        assert_eq!(next.capture.sequence, 3);
        assert_eq!(next.obsolete, ["2.png"]);
        assert!(queue.next_request().is_none());
        assert!(queue.push(request(4)));
    }

    #[test]
    fn only_the_current_captures_feedback_can_reach_the_player() {
        let capture = request(2).capture;
        let mut state = Playback {
            generation: 3,
            completed_screenshots: 2,
            phase: Phase::Paused,
            ..Playback::default()
        };
        assert!(matches_current(&capture, &state));
        state.completed_screenshots += 1;
        assert!(!matches_current(&capture, &state));
        state.completed_screenshots -= 1;
        state.begin_load();
        assert!(!matches_current(&capture, &state));
    }
}
