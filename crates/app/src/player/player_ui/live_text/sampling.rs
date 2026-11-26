//! The player owns capture admission and one recognition task for its current frame.
use super::capture::{CaptureAction, CaptureState, FrameKey, RecognitionKey};
use crate::{
    LiveTextRegion, PlaybackStore, PlayerWindow,
    platform::text_recognition::{self, Cancellation, Region},
};
use riina_core::{CapturedFrame, Command, Phase, Playback, sync::lock};
use riina_mpv::CommandSender;
use slint::{ComponentHandle, ModelRc, Timer, TimerMode, VecModel};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[derive(Default)]
struct Sampling {
    frame: Option<FrameKey>,
    capture: CaptureState,
    capture_attempts: u8,
    attempted: Option<RecognitionKey>,
    active: Option<Arc<Analysis>>,
}

#[derive(Default)]
struct Analysis {
    cancellation: Cancellation,
    finished: AtomicBool,
}

pub(super) fn install(
    app: &PlayerWindow,
    sender: CommandSender,
    playback: PlaybackStore,
) -> Rc<Timer> {
    let timer = Rc::new(Timer::default());
    let sampling = Rc::new(RefCell::new(Sampling::default()));
    let sampler = sampling.clone();
    let weak = app.as_weak();
    let capture_playback = playback.clone();
    let timer_handle = Rc::downgrade(&timer);
    timer.start(
        TimerMode::SingleShot,
        Duration::from_millis(150),
        move || {
            if let (Some(app), Some(timer)) = (weak.upgrade(), timer_handle.upgrade()) {
                sampler
                    .borrow_mut()
                    .sample(&app, &timer, &sender, &capture_playback);
            }
        },
    );
    timer.stop();

    let weak = app.as_weak();
    let timer_handle = Rc::downgrade(&timer);
    app.on_live_text_frame_changed(move || {
        if let (Some(app), Some(timer)) = (weak.upgrade(), timer_handle.upgrade()) {
            let state = lock(&playback).clone();
            sampling.borrow_mut().refresh(&app, &timer, &state);
        }
    });
    let weak = app.as_weak();
    app.on_live_text_observation_changed(move |_| {
        if let Some(app) = weak.upgrade() {
            app.invoke_live_text_frame_changed();
        }
    });
    app.invoke_live_text_frame_changed();
    timer
}

impl Sampling {
    fn synchronize(&mut self, app: &PlayerWindow, state: &Playback) -> Option<FrameKey> {
        let frame = (app.get_observe_live_text() && state.phase == Phase::Paused && !state.seeking)
            .then(|| FrameKey::from_playback(state));
        if self.frame != frame {
            self.frame = frame;
            self.capture.invalidate();
            self.capture_attempts = 0;
            self.attempted = None;
            self.cancel_active();
            set_regions(app, Vec::new());
        }
        if self
            .active
            .as_ref()
            .is_some_and(|job| job.finished.load(Ordering::Acquire))
        {
            self.active = None;
        }
        frame
    }

    fn refresh(&mut self, app: &PlayerWindow, timer: &Timer, state: &Playback) {
        if self.synchronize(app, state).is_some()
            && self.active.is_none()
            && self.attempted.is_none_or(|key| !key.is_current(state))
        {
            super::super::features::wake_timer(timer);
        } else {
            timer.stop();
        }
    }

    fn sample(
        &mut self,
        app: &PlayerWindow,
        timer: &Timer,
        sender: &CommandSender,
        playback: &PlaybackStore,
    ) {
        let state = lock(playback).clone();
        let Some(frame) = self.synchronize(app, &state) else {
            return;
        };
        if self.active.is_some() {
            return;
        }
        match self.capture.observe(&state) {
            CaptureAction::Capture if self.capture_attempts < 2 => {
                self.capture_attempts += 1;
                if let Err(error) = sender.send_for_media(frame.generation(), Command::CaptureFrame)
                {
                    self.capture.request_failed();
                    if self.capture_attempts < 2 {
                        timer.restart();
                    } else {
                        crate::diagnostics::report(
                            app,
                            format!("Could not capture frame for text recognition: {error}"),
                        );
                    }
                }
            }
            CaptureAction::Capture => self.capture.request_failed(),
            CaptureAction::Recognize(key) if self.attempted != Some(key) => {
                if let Some(pixels) = state.captured_frame.frame.as_deref().cloned() {
                    self.attempted = Some(key);
                    self.active = launch(app, playback.clone(), key, pixels);
                }
            }
            CaptureAction::Recognize(_) | CaptureAction::Wait => {}
        }
    }

    fn cancel_active(&self) {
        if let Some(job) = &self.active {
            job.cancellation.cancel();
        }
    }
}

impl Drop for Sampling {
    fn drop(&mut self) {
        self.cancel_active();
    }
}

fn launch(
    app: &PlayerWindow,
    playback: PlaybackStore,
    key: RecognitionKey,
    frame: CapturedFrame,
) -> Option<Arc<Analysis>> {
    let job = Arc::new(Analysis::default());
    let completed = job.clone();
    let weak = app.as_weak();
    let (width, height) = (frame.width as f32, frame.height as f32);
    if let Err(error) =
        text_recognition::recognize(frame, job.cancellation.clone(), move |result| {
            let _ = crate::background::to_ui(weak, move |app| {
                completed.finished.store(true, Ordering::Release);
                if !completed.cancellation.is_cancelled()
                    && app.get_observe_live_text()
                    && key.is_current(&lock(&playback))
                {
                    match result {
                        Ok(regions) => set_regions(
                            &app,
                            regions
                                .into_iter()
                                .map(|region: Region| LiveTextRegion {
                                    text: region.text.into(),
                                    x: region.x / width,
                                    y: region.y / height,
                                    width: region.width / width,
                                    height: region.height / height,
                                })
                                .collect(),
                        ),
                        Err(error) => crate::diagnostics::report(
                            &app,
                            format!("Text recognition failed: {error}"),
                        ),
                    }
                }
                app.invoke_live_text_frame_changed();
            });
        })
    {
        crate::diagnostics::report(app, format!("Could not start text recognition: {error}"));
        return None;
    }
    Some(job)
}

fn set_regions(app: &PlayerWindow, regions: Vec<LiveTextRegion>) {
    app.global::<riina_ui::PlayerRenderedVideo>()
        .set_live_text_regions(ModelRc::new(VecModel::from(regions)));
}
