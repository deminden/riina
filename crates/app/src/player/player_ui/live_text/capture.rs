//! Match recognition requests to one paused playback frame.
use riina_core::{Phase, Playback};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FrameKey {
    generation: u64,
    position: Option<u64>,
    completed_seeks: u64,
}

impl FrameKey {
    pub(super) fn from_playback(state: &Playback) -> Self {
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
pub(super) struct RecognitionKey {
    pub(super) frame: FrameKey,
    pub(super) capture: u64,
}

impl RecognitionKey {
    pub(super) fn is_current(self, state: &Playback) -> bool {
        state.phase == Phase::Paused
            && self.frame == FrameKey::from_playback(state)
            && self.capture == state.captured_frame.revision
    }
}

#[derive(Clone, Copy)]
struct CaptureRequest {
    frame: FrameKey,
    before_capture: u64,
    before_finished: u64,
}

#[derive(Default)]
pub(super) struct CaptureState {
    pending: Option<CaptureRequest>,
    ready: Option<RecognitionKey>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum CaptureAction {
    Capture,
    Wait,
    Recognize(RecognitionKey),
}

impl CaptureState {
    pub(super) fn observe(&mut self, state: &Playback) -> CaptureAction {
        let frame = FrameKey::from_playback(state);
        if let Some(pending) = self.pending.take()
            && pending.frame.generation == state.generation
        {
            if pending.before_finished == state.finished_frame_capture_requests {
                self.pending = Some(pending);
                return CaptureAction::Wait;
            }
            if pending.frame == frame && pending.before_capture != state.captured_frame.revision {
                self.ready = Some(RecognitionKey {
                    frame,
                    capture: state.captured_frame.revision,
                });
            }
        }
        if let Some(ready) = self.ready.filter(|key| key.is_current(state)) {
            return CaptureAction::Recognize(ready);
        }
        self.ready = None;
        self.pending = Some(CaptureRequest {
            frame,
            before_capture: state.captured_frame.revision,
            before_finished: state.finished_frame_capture_requests,
        });
        CaptureAction::Capture
    }

    pub(super) fn request_failed(&mut self) {
        self.pending = None;
    }

    pub(super) fn invalidate(&mut self) {
        // Keep an outstanding request until its reply arrives. Otherwise an old
        // reply after a seek or re-enable could be mistaken for the new frame.
        self.ready = None;
    }
}
