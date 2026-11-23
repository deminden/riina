//! Folder expansion, subtitle discovery and selection run outside input callbacks.
use crate::{PlayerWindow, subtitle_discovery::Readiness};
use riina_core::{Command, Phase, Playback, TrackKind};
use slint::ComponentHandle;
use std::{cell::RefCell, rc::Rc, sync::Arc, time::Duration};

pub(super) fn install(
    app: &PlayerWindow,
    player: &riina_mpv::CommandSender,
    latest_playback: crate::PlaybackStore,
    subtitle_discovery_ready: Arc<Readiness>,
) -> Rc<slint::Timer> {
    let subtitle_priority_timer = Rc::new(slint::Timer::default());
    let priority_player = player.clone();
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
    let auto_folder_player = player.clone();
    let auto_folder_playback = latest_playback.clone();
    let subtitle_discovery_state = subtitle_discovery.clone();
    let subtitle_discovery_player = player.clone();
    let subtitle_discovery_playback = latest_playback.clone();
    let subtitle_discovery_ready_controller = subtitle_discovery_ready.clone();
    let priority_controller = priority_state.clone();
    let priority_app = app.as_weak();
    let settings_timer = Rc::downgrade(&subtitle_priority_timer);
    app.global::<riina_ui::PlayerPreferences>()
        .on_discovery_settings_changed(move || {
            if let Some(timer) = settings_timer.upgrade() {
                super::features::wake_timer(&timer);
            }
        });
    subtitle_priority_timer.start(
        slint::TimerMode::SingleShot,
        Duration::from_millis(120),
        move || {
            let Some(app) = priority_app.upgrade() else {
                return;
            };
            let Ok(preferences) = crate::subtitle_auto_load_preferences::current_player(&app)
            else {
                return;
            };
            let state = riina_core::sync::lock(&priority_playback).clone();
            if preferences.mode == riina_core::subtitle_controls::AutoLoadMode::Disabled {
                subtitle_discovery_ready_controller.mark_ready(state.generation);
            }
            if let Some(request) = auto_folder_state.borrow_mut().observe(
                &state,
                app.global::<riina_ui::PlayerPreferences>()
                    .get_playlist_auto_add(),
            ) {
                discover_playlist_files(
                    &app,
                    request,
                    state.clone(),
                    auto_folder_playback.clone(),
                    auto_folder_player.clone(),
                );
            }
            if let Some((generation, media_path)) = subtitle_discovery_state
                .borrow_mut()
                .observe(&state, &preferences)
            {
                let request = subtitle_discovery_ready_controller.begin(generation);
                let preferences = preferences.clone();
                let playback = subtitle_discovery_playback.clone();
                let sender = subtitle_discovery_player.clone();
                let weak_app = app.as_weak();
                let ready = subtitle_discovery_ready_controller.clone();
                let started =
                    crate::background::spawn_named("riina-subtitle-discovery", move || {
                        let files = crate::subtitle_discovery::find_matching_files(
                            &media_path,
                            &preferences,
                        );
                        let expected_path = media_path.to_string_lossy().into_owned();
                        let current = ready.is_current(request) && {
                            let state = riina_core::sync::lock(&playback);
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
                        };
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
                            // The result has no recipient once its window or event loop closes.
                            let _ = crate::background::to_ui(weak_app, move |app| {
                                crate::diagnostics::report(&app, error);
                            });
                        }
                        ready.complete(request);
                    });
                if let Err(error) = started {
                    subtitle_discovery_ready_controller.complete(request);
                    crate::diagnostics::report(
                        &app,
                        format!("Could not start subtitle discovery: {error}"),
                    );
                }
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
                crate::diagnostics::report(&app, error);
            }
        },
    );
    subtitle_priority_timer
}

fn discover_playlist_files(
    app: &PlayerWindow,
    request: crate::auto_playlist::Request,
    snapshot: Arc<Playback>,
    playback: crate::PlaybackStore,
    sender: riina_mpv::CommandSender,
) {
    let weak_app = app.as_weak();
    let started = crate::background::spawn_named("riina-playlist-auto-add", move || {
        let Some(command) = crate::auto_playlist::prepare(&request, &snapshot) else {
            return;
        };
        let still_current =
            crate::auto_playlist::request_is_current(&request, &riina_core::sync::lock(&playback));
        if still_current && let Err(error) = sender.send(command) {
            // The result has no recipient once its window or event loop closes.
            let _ = crate::background::to_ui(weak_app, move |app| {
                crate::diagnostics::report(&app, error);
            });
        }
    });
    if let Err(error) = started {
        crate::diagnostics::report(app, format!("Could not start playlist discovery: {error}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use riina_ui::PlayerPreferences;

    #[test]
    fn discovery_observes_settings_once_and_returns_to_idle() -> Result<(), slint::PlatformError> {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new()?;
        let (commands, recording) = riina_mpv::CommandSender::recording(4);
        let timer = install(
            &app,
            &commands,
            crate::PlaybackStore::default(),
            Arc::default(),
        );
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(120));
        assert!(!timer.running());

        app.global::<PlayerPreferences>()
            .set_subtitle_auto_load_priority("test".into());
        i_slint_backend_testing::mock_elapsed_time(Duration::ZERO);
        assert!(timer.running());
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(120));
        assert!(!timer.running());
        assert_eq!(
            recording.commands.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        );
        Ok(())
    }
}
