//! Folder expansion, subtitle discovery and selection run outside input callbacks.
use crate::{PlayerWindow, subtitle_discovery::Readiness};
use riina_core::{Command, Phase, Playback, TrackKind};
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

pub(super) fn install(
    app: &PlayerWindow,
    player: &riina_mpv::CommandSender,
    latest_playback: Arc<Mutex<Playback>>,
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
                subtitle_discovery_ready_controller.mark_ready(state.generation);
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
                let request = subtitle_discovery_ready_controller.begin(generation);
                let preferences = preferences.clone();
                let playback = subtitle_discovery_playback.clone();
                let sender = subtitle_discovery_player.clone();
                let weak_app = app.as_weak();
                let ready = subtitle_discovery_ready_controller.clone();
                std::thread::spawn(move || {
                    let files =
                        crate::subtitle_discovery::find_matching_files(&media_path, &preferences);
                    let expected_path = media_path.to_string_lossy().into_owned();
                    let current = ready.is_current(request)
                        && playback.lock().ok().is_some_and(|state| {
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
                    ready.complete(request);
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
    subtitle_priority_timer
}
