//! Paused-frame capture and asynchronous recognition are scoped to one frame identity.
use crate::{PlayerWindow, platform::text_recognition};
use riina_core::{Phase, Playback, Preferences};
use riina_mpv::CommandSender;
use riina_ui::PlayerMenuActions;
use slint::ComponentHandle;
use std::{path::Path, rc::Rc};

mod capture;
mod sampling;

pub(super) fn observe(app: &PlayerWindow, previous: Option<&Playback>, state: &Playback) {
    let eligible = can_sample(state);
    app.set_live_text_frame_paused(eligible);
    if eligible
        && previous.is_none_or(|old| {
            !can_sample(old)
                || capture::FrameKey::from_playback(old) != capture::FrameKey::from_playback(state)
                || old.finished_frame_capture_requests != state.finished_frame_capture_requests
        })
    {
        app.invoke_live_text_frame_changed();
    }
}

fn can_sample(state: &Playback) -> bool {
    state.phase == Phase::Paused && !state.seeking && state.display_video_parameters().is_some()
}

pub(super) fn initialize(app: &PlayerWindow, enabled: bool, use_system_services: bool) {
    app.set_live_text_enabled(enabled);
    app.global::<riina_ui::PlayerRenderedVideo>()
        .set_live_text_regions(slint::ModelRc::new(slint::VecModel::from(Vec::new())));
    app.set_live_text_available(false);
    if text_recognition::SELECTABLE {
        app.set_live_text_selectable(true);
    }
    if use_system_services && text_recognition::SUPPORTED {
        let weak = app.as_weak();
        let probe = text_recognition::check_availability(move |available| {
            // The result has no recipient once its window or event loop closes.
            let _ = crate::background::to_ui(weak, move |app| {
                app.set_live_text_available(available);
                if !available {
                    app.set_live_text_enabled(false);
                }
            });
        });
        if let Err(error) = probe {
            crate::diagnostics::warn!("Could not check Live Text availability: {error}");
        }
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
    text_recognition::SUPPORTED.then(|| sampling::install(app, sender, latest))
}

#[cfg(test)]
mod tests {
    use super::capture::{CaptureAction, CaptureState, FrameKey, RecognitionKey};
    use super::*;
    use riina_core::Command;
    use std::{sync::Arc, time::Duration};

    #[test]
    fn sampling_runs_only_for_an_eligible_paused_frame() -> Result<(), slint::PlatformError> {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new()?;
        let playback = crate::PlaybackStore::default();
        *riina_core::sync::lock(&playback) = Arc::new(paused());
        let (sender, recording) = CommandSender::recording(4);
        let timer = sampling::install(&app, sender, playback);
        assert!(!timer.running());

        app.set_loaded(true);
        app.set_live_text_enabled(true);
        app.set_live_text_available(true);
        app.set_live_text_frame_paused(true);
        i_slint_backend_testing::mock_elapsed_time(Duration::ZERO);
        assert!(timer.running());
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(160));
        assert_eq!(recording.commands.try_recv(), Ok(Command::CaptureFrame));
        assert!(!timer.running());

        app.set_picture_in_picture_active(true);
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(160));
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
            seeking: true,
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
