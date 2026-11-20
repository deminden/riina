//! Paused-frame capture and asynchronous recognition are scoped to one frame identity.
use riina_core::{Phase, Playback};
#[cfg(any(target_os = "windows", target_os = "macos"))]
use {
    crate::{LiveTextRegion, PlayerWindow, platform},
    riina_core::Command,
    slint::ComponentHandle,
    std::{
        rc::Rc,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FrameKey {
    generation: u64,
    position: Option<u64>,
    completed_seeks: u64,
}

impl FrameKey {
    fn from_playback(state: &Playback) -> Self {
        Self {
            generation: state.generation,
            position: state
                .position
                .filter(|value| value.is_finite())
                .map(f64::to_bits),
            completed_seeks: state.completed_seeks,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RecognitionKey {
    frame: FrameKey,
    capture: u64,
}

impl RecognitionKey {
    fn is_current(self, state: &Playback) -> bool {
        state.phase == Phase::Paused
            && self.frame == FrameKey::from_playback(state)
            && self.capture == state.completed_frame_captures
    }
}

#[derive(Clone, Copy)]
struct CaptureRequest {
    frame: FrameKey,
    before_capture: u64,
    before_finished: u64,
}

#[derive(Default)]
struct CaptureState {
    pending: Option<CaptureRequest>,
    ready: Option<RecognitionKey>,
}

#[derive(Debug, PartialEq, Eq)]
enum CaptureAction {
    Capture,
    Wait,
    Recognize(RecognitionKey),
}

impl CaptureState {
    fn observe(&mut self, state: &Playback) -> CaptureAction {
        let frame = FrameKey::from_playback(state);
        if let Some(pending) = self.pending.take()
            && pending.frame.generation == state.generation
        {
            if pending.before_finished == state.finished_frame_capture_requests {
                self.pending = Some(pending);
                return CaptureAction::Wait;
            }
            if pending.frame == frame && pending.before_capture != state.completed_frame_captures {
                self.ready = Some(RecognitionKey {
                    frame,
                    capture: state.completed_frame_captures,
                });
            }
        }
        if let Some(ready) = self.ready.filter(|key| key.is_current(state)) {
            return CaptureAction::Recognize(ready);
        }
        self.ready = None;
        self.pending = Some(CaptureRequest {
            frame,
            before_capture: state.completed_frame_captures,
            before_finished: state.finished_frame_capture_requests,
        });
        CaptureAction::Capture
    }

    fn request_failed(&mut self) {
        self.pending = None;
    }

    fn invalidate(&mut self) {
        // Keep an outstanding request until its reply arrives. Otherwise an old
        // reply after a seek or re-enable could be mistaken for the new frame.
        self.ready = None;
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
pub(super) fn install(
    app: &PlayerWindow,
    sender: riina_mpv::CommandSender,
    latest: Arc<std::sync::Mutex<Playback>>,
) -> Rc<slint::Timer> {
    let mut capture_state = CaptureState::default();
    let mut displayed_frame = None;
    let mut recognized_capture = None;
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
                capture_state.invalidate();
                displayed_frame = None;
                recognized_capture = None;
                app.set_live_text_regions(slint::ModelRc::new(slint::VecModel::from(Vec::new())));
                return;
            }

            let frame_key = FrameKey::from_playback(&state);
            if displayed_frame != Some(frame_key) {
                displayed_frame = Some(frame_key);
                app.set_live_text_regions(slint::ModelRc::new(slint::VecModel::from(Vec::new())));
            }
            let key = match capture_state.observe(&state) {
                CaptureAction::Capture => {
                    if let Err(error) = sender.send(Command::CaptureFrame) {
                        capture_state.request_failed();
                        app.set_status(
                            format!("Could not capture frame for text recognition: {error}").into(),
                        );
                    }
                    return;
                }
                CaptureAction::Wait => return,
                CaptureAction::Recognize(key) => key,
            };
            if recognized_capture == Some(key) || recognition_running.load(Ordering::Acquire) {
                return;
            }
            // Copy pixels only for a new recognition job, never on idle polling.
            let Some(frame) = state.captured_frame.as_deref().cloned() else {
                return;
            };
            recognized_capture = Some(key);
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
                    if !key.is_current(&current)
                        || !app.get_live_text_enabled()
                        || !app.get_live_text_available()
                        || !app.get_loaded()
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
                recognized_capture = None;
                app.set_status(format!("Could not start text recognition: {error}").into());
            }
        },
    );
    timer
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paused() -> Playback {
        Playback {
            generation: 3,
            phase: Phase::Paused,
            paused: true,
            position: Some(1.0),
            completed_frame_captures: 2,
            finished_frame_capture_requests: 2,
            ..Playback::default()
        }
    }

    #[test]
    fn recognition_waits_for_the_requested_capture_instead_of_reusing_old_pixels() {
        let mut capture = CaptureState::default();
        let mut state = paused();
        assert_eq!(capture.observe(&state), CaptureAction::Capture);
        assert_eq!(capture.observe(&state), CaptureAction::Wait);
        state.completed_frame_captures += 1;
        state.finished_frame_capture_requests += 1;
        let CaptureAction::Recognize(key) = capture.observe(&state) else {
            panic!("Fresh capture not recognized")
        };
        assert!(key.is_current(&state));
        assert_eq!(capture.observe(&state), CaptureAction::Recognize(key));
    }

    #[test]
    fn a_seek_discards_old_recognition_before_another_capture_completes() {
        let state = paused();
        let key = RecognitionKey {
            frame: FrameKey::from_playback(&state),
            capture: state.completed_frame_captures,
        };
        assert!(!key.is_current(&Playback {
            position: Some(2.0),
            ..paused()
        }));
        assert!(!key.is_current(&Playback {
            completed_seeks: 1,
            ..paused()
        }));
        assert!(!key.is_current(&Playback {
            generation: 4,
            ..paused()
        }));
        assert!(!key.is_current(&Playback {
            phase: Phase::Playing,
            paused: false,
            ..paused()
        }));
    }

    #[test]
    fn a_superseded_capture_finishes_before_requesting_the_new_paused_frame() {
        let mut capture = CaptureState::default();
        assert_eq!(capture.observe(&paused()), CaptureAction::Capture);
        capture.invalidate();
        let mut sought = Playback {
            position: Some(2.0),
            ..paused()
        };
        assert_eq!(capture.observe(&sought), CaptureAction::Wait);
        sought.completed_frame_captures += 1;
        sought.finished_frame_capture_requests += 1;
        assert_eq!(capture.observe(&sought), CaptureAction::Capture);
        assert_eq!(capture.observe(&sought), CaptureAction::Wait);
        sought.completed_frame_captures += 1;
        sought.finished_frame_capture_requests += 1;
        let CaptureAction::Recognize(key) = capture.observe(&sought) else {
            panic!("Replacement capture not recognized")
        };
        assert!(key.is_current(&sought));
    }

    #[test]
    fn failed_admission_and_replaced_media_do_not_block_fresh_capture_requests() {
        let mut capture = CaptureState::default();
        assert_eq!(capture.observe(&paused()), CaptureAction::Capture);
        capture.request_failed();
        assert_eq!(capture.observe(&paused()), CaptureAction::Capture);
        assert_eq!(
            capture.observe(&Playback {
                generation: 4,
                ..paused()
            }),
            CaptureAction::Capture
        );
    }

    #[test]
    fn a_failed_native_capture_retries_without_analyzing_old_pixels() {
        let mut capture = CaptureState::default();
        let mut state = paused();
        assert_eq!(capture.observe(&state), CaptureAction::Capture);
        state.finished_frame_capture_requests += 1;
        assert_eq!(capture.observe(&state), CaptureAction::Capture);
        assert_eq!(capture.observe(&state), CaptureAction::Wait);
    }
}
