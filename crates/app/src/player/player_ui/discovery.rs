//! Folder expansion, subtitle discovery and selection run outside input callbacks.
use super::playlist_subtitles::Assignments;
use crate::{
    PlaybackStore, PlayerWindow,
    subtitle_discovery::{AutoDiscovery, Readiness},
};
use riina_core::{
    Command, Phase, Playback, TrackKind, subtitle_controls::AutoLoadPreferences, sync::lock,
};
use riina_mpv::CommandSender;
use slint::ComponentHandle;
use std::{cell::RefCell, rc::Rc, sync::Arc, time::Duration};

pub(super) fn install(
    app: &PlayerWindow,
    player: &CommandSender,
    latest_playback: PlaybackStore,
    subtitle_discovery_ready: Arc<Readiness>,
    subtitle_assignments: Assignments,
) -> Rc<slint::Timer> {
    let subtitle_priority_timer = Rc::new(slint::Timer::default());
    let priority_player = player.clone();
    let priority_playback = latest_playback.clone();
    let priority_state = Rc::new(RefCell::new(
        crate::subtitle_priority::AutoPriority::default(),
    ));
    let subtitles = RefCell::new(SubtitleDiscovery {
        last_loaded: None,
        automatic: AutoDiscovery::default(),
        player: player.clone(),
        playback: latest_playback.clone(),
        assignments: subtitle_assignments,
        ready: subtitle_discovery_ready,
    });
    let auto_folder_discovery = Rc::new(RefCell::new(
        crate::auto_playlist::AutoFolderAdder::default(),
    ));
    let auto_folder_state = auto_folder_discovery.clone();
    let auto_folder_player = player.clone();
    let auto_folder_playback = latest_playback.clone();
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
            let state = lock(&priority_playback).clone();
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
            subtitles.borrow_mut().observe(&app, &state, &preferences);
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

struct SubtitleDiscovery {
    last_loaded: Option<(u64, String)>,
    automatic: AutoDiscovery,
    player: CommandSender,
    playback: PlaybackStore,
    assignments: Assignments,
    ready: Arc<Readiness>,
}

impl SubtitleDiscovery {
    fn observe(&mut self, app: &PlayerWindow, state: &Playback, preferences: &AutoLoadPreferences) {
        if state.phase == Phase::Stopped && state.playlist.is_empty() {
            self.assignments.reset();
            self.last_loaded = None;
        }
        let Some(filename) = current_media(state) else {
            return;
        };
        let generation = state.generation;
        if !self
            .last_loaded
            .as_ref()
            .is_some_and(|(loaded_generation, loaded_file)| {
                *loaded_generation == generation && loaded_file == filename
            })
        {
            self.last_loaded = Some((generation, filename.into()));
            self.load_manual(app, generation, filename);
        }
        let scan = self.automatic.observe(state, preferences);
        let Some(epoch) = self.assignments.scan_epoch(filename).filter(|_| {
            preferences.mode != riina_core::subtitle_controls::AutoLoadMode::Disabled
                && crate::file_actions::local_media_path(filename).is_some()
        }) else {
            self.ready.mark_ready(generation);
            return;
        };
        let Some((_, media_path)) = scan else { return };
        let request = self.ready.begin(generation);
        let ready = self.ready.clone();
        let playback = self.playback.clone();
        let assignments = self.assignments.clone();
        let sender = self.player.clone();
        let preferences = preferences.clone();
        let filename = filename.to_owned();
        let weak = app.as_weak();
        let started = crate::background::spawn_named("riina-subtitle-discovery", move || {
            let paths = crate::subtitle_discovery::find_matching_files(&media_path, &preferences)
                .into_iter()
                .filter_map(|path| path.into_os_string().into_string().ok())
                .collect();
            let still_current = {
                let state = lock(&playback);
                state.generation == generation && current_media(&state) == Some(filename.as_str())
            };
            if still_current && ready.is_current(request) {
                let paths = assignments.record_automatic(&filename, epoch, generation, paths);
                if !paths.is_empty()
                    && let Err(error) = sender.send(Command::AddMatchedSubtitles {
                        generation,
                        paths,
                        select_first: false,
                    })
                {
                    let _ = crate::background::to_ui(weak, move |app| {
                        crate::diagnostics::report(&app, error);
                    });
                }
            }
            ready.complete(request);
        });
        if let Err(error) = started {
            self.ready.complete(request);
            crate::diagnostics::report(app, format!("Could not start subtitle discovery: {error}"));
        }
    }

    fn load_manual(&self, app: &PlayerWindow, generation: u64, filename: &str) {
        let paths = self.assignments.manual(filename, generation);
        for (batch, paths) in paths.chunks(64).enumerate() {
            if let Err(error) = self.player.send(Command::AddMatchedSubtitles {
                generation,
                paths: paths.to_vec(),
                select_first: batch == 0,
            }) {
                crate::diagnostics::report(app, error);
                break;
            }
        }
    }
}

fn current_media(state: &Playback) -> Option<&str> {
    if !matches!(
        state.phase,
        Phase::Playing | Phase::Paused | Phase::Buffering | Phase::Seeking
    ) {
        return None;
    }
    state
        .playlist
        .iter()
        .find(|entry| entry.current)
        .map(|entry| entry.filename.as_str())
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
            Assignments::default(),
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
