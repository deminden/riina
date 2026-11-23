//! Paused-frame capture and asynchronous recognition are scoped to one frame identity.
use crate::{LiveTextRegion, PlayerWindow, platform::text_recognition};
#[cfg(test)]
use riina_core::Playback;
use riina_core::{Command, Phase, Preferences};
use riina_mpv::CommandSender;
use riina_ui::PlayerMenuActions;
use slint::ComponentHandle;
use std::{
    path::Path,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

mod capture;
#[cfg(test)]
use capture::RecognitionKey;
use capture::{CaptureAction, CaptureState, FrameKey};

pub(super) fn initialize(app: &PlayerWindow, enabled: bool, use_system_services: bool) {
    app.set_live_text_enabled(enabled);
    app.global::<riina_ui::PlayerRenderedVideo>()
        .set_live_text_regions(slint::ModelRc::new(slint::VecModel::from(Vec::new())));
    app.set_live_text_available(false);
    if text_recognition::NATIVE_SELECTION {
        app.set_live_text_selectable(true);
    }
    if use_system_services && text_recognition::SUPPORTED {
        let weak = app.as_weak();
        let _ = text_recognition::check_availability(move |available| {
            // The result has no recipient once its window or event loop closes.
            let _ = crate::background::to_ui(weak, move |app| {
                app.set_live_text_available(available);
                if !available {
                    app.set_live_text_enabled(false);
                }
            });
        });
    }
}

pub(super) fn install_actions(app: &PlayerWindow, prefs_path: &Path) {
    let clipboard_parent = app.as_weak();
    app.on_copy_live_text(move |text| {
        if let Err(error) = crate::platform::clipboard::copy_text(text.as_str())
            && let Some(app) = clipboard_parent.upgrade()
        {
            crate::diagnostics::report(&app, format!("Could not copy recognized text: {error}"));
        }
    });
    let live_text_parent = app.as_weak();
    let live_text_path = prefs_path.to_owned();
    app.global::<PlayerMenuActions>()
        .on_toggle_live_text(move || {
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
                Err(error) => crate::diagnostics::report(
                    &app,
                    format!("Could not save Live Text preference: {error}"),
                ),
            }
        });
}

pub(super) fn install(
    app: &PlayerWindow,
    sender: CommandSender,
    latest: crate::PlaybackStore,
) -> Option<Rc<slint::Timer>> {
    text_recognition::SUPPORTED.then(|| install_sampling(app, sender, latest))
}

fn install_sampling(
    app: &PlayerWindow,
    sender: CommandSender,
    latest: crate::PlaybackStore,
) -> Rc<slint::Timer> {
    let mut capture_state = CaptureState::default();
    let mut displayed_frame = None;
    let mut recognized_capture = None;
    let recognition_running = Arc::new(AtomicBool::new(false));
    let timer = Rc::new(slint::Timer::default());
    let weak = app.as_weak();
    let weak_timer = Rc::downgrade(&timer);
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(300),
        move || {
            let Some(app) = weak.upgrade() else { return };
            let state = latest.lock().map(|state| state.clone()).unwrap_or_default();
            let enabled = app.get_observe_live_text();
            if !enabled || state.phase != Phase::Paused {
                capture_state.invalidate();
                displayed_frame = None;
                recognized_capture = None;
                app.global::<riina_ui::PlayerRenderedVideo>()
                    .set_live_text_regions(slint::ModelRc::new(slint::VecModel::from(Vec::new())));
                if !enabled && let Some(timer) = weak_timer.upgrade() {
                    timer.stop();
                }
                return;
            }

            let frame_key = FrameKey::from_playback(&state);
            if displayed_frame != Some(frame_key) {
                displayed_frame = Some(frame_key);
                app.global::<riina_ui::PlayerRenderedVideo>()
                    .set_live_text_regions(slint::ModelRc::new(slint::VecModel::from(Vec::new())));
            }
            let key = match capture_state.observe(&state) {
                CaptureAction::Capture => {
                    if let Err(error) = sender.send(Command::CaptureFrame) {
                        capture_state.request_failed();
                        crate::diagnostics::report(
                            &app,
                            format!("Could not capture frame for text recognition: {error}"),
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
            let Some(frame) = state.captured_frame.frame.as_deref().cloned() else {
                return;
            };
            recognized_capture = Some(key);
            recognition_running.store(true, Ordering::Release);
            let latest_result = latest.clone();
            let weak_result = app.as_weak();
            let running = recognition_running.clone();
            let width = frame.width as f32;
            let height = frame.height as f32;
            if let Err(error) = text_recognition::recognize(frame, move |result| {
                let rejected = running.clone();
                if slint::invoke_from_event_loop(move || {
                    running.store(false, Ordering::Release);
                    let Some(app) = weak_result.upgrade() else {
                        return;
                    };
                    let current = latest_result
                        .lock()
                        .map(|state| state.clone())
                        .unwrap_or_default();
                    if !key.is_current(&current) || !app.get_observe_live_text() {
                        return;
                    }
                    match result {
                        Ok(regions) => app
                            .global::<riina_ui::PlayerRenderedVideo>()
                            .set_live_text_regions(slint::ModelRc::new(slint::VecModel::from(
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
                            ))),
                        Err(error) => crate::diagnostics::report(
                            &app,
                            format!("Text recognition failed: {error}"),
                        ),
                    }
                })
                .is_err()
                {
                    // The stopped event loop cannot clear the flag for us.
                    rejected.store(false, Ordering::Release);
                }
            }) {
                recognition_running.store(false, Ordering::Release);
                recognized_capture = None;
                crate::diagnostics::report(
                    &app,
                    format!("Could not start text recognition: {error}"),
                );
            }
        },
    );
    if !app.get_observe_live_text() {
        timer.stop();
    }
    let weak_timer = Rc::downgrade(&timer);
    app.on_live_text_observation_changed(move |enabled| {
        // An ineligible timer gets one final tick to clear its regions.
        if enabled && let Some(timer) = weak_timer.upgrade() {
            super::features::wake_timer(&timer);
        }
    });
    timer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sampling_runs_only_for_an_eligible_paused_frame() -> Result<(), slint::PlatformError> {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new()?;
        let playback = crate::PlaybackStore::default();
        *riina_core::sync::lock(&playback) = Arc::new(paused());
        let (sender, recording) = CommandSender::recording(4);
        let timer = install_sampling(&app, sender, playback);
        assert!(!timer.running());

        app.set_loaded(true);
        app.set_live_text_enabled(true);
        app.set_live_text_available(true);
        app.set_live_text_frame_paused(true);
        i_slint_backend_testing::mock_elapsed_time(Duration::ZERO);
        assert!(timer.running());
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(300));
        assert_eq!(recording.commands.try_recv(), Ok(Command::CaptureFrame));

        app.set_live_text_frame_paused(false);
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(300));
        assert!(!timer.running());
        assert_eq!(
            recording.commands.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        );
        Ok(())
    }

    fn paused() -> Playback {
        Playback {
            generation: 3,
            phase: Phase::Paused,
            paused: true,
            position: Some(1.0),
            captured_frame: riina_core::FrameSlot {
                revision: 2,
                ..Default::default()
            },
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
        state.captured_frame.revision += 1;
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
            capture: state.captured_frame.revision,
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
        sought.captured_frame.revision += 1;
        sought.finished_frame_capture_requests += 1;
        assert_eq!(capture.observe(&sought), CaptureAction::Capture);
        assert_eq!(capture.observe(&sought), CaptureAction::Wait);
        sought.captured_frame.revision += 1;
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
