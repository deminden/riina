//! Folder expansion, subtitle discovery and selection run outside input callbacks.
use super::playlist_subtitles::Assignments;
use crate::{
    PlaybackStore, PlayerWindow,
    playlist_metadata::series,
    subtitle_discovery::{AutoDiscovery, Readiness},
};
use riina_core::{
    Command, Phase, Playback, PlaylistEntry, TrackKind,
    subtitle_controls::{AutoLoadMode, AutoLoadPreferences},
    sync::lock,
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
    folder_series: series::Cache,
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
        crate::auto_playlist::FolderDiscovery::default(),
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
            let options = folder_options(&app);
            if (!options.series || state.explicit_playlist || state.playlist.is_empty())
                && lock(&folder_series).take().is_some()
            {
                app.invoke_refresh_playlist_metadata();
            }
            if let Some(request) = auto_folder_state.borrow_mut().observe(&state, options) {
                discover_playlist_files(
                    &app,
                    request,
                    state.clone(),
                    auto_folder_playback.clone(),
                    auto_folder_player.clone(),
                    folder_series.clone(),
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

fn folder_options(app: &PlayerWindow) -> crate::auto_playlist::Options {
    let preferences = app.global::<riina_ui::PlayerPreferences>();
    crate::auto_playlist::Options {
        append: preferences.get_playlist_auto_add(),
        series: AutoLoadMode::from_index(preferences.get_subtitle_auto_load_mode_index())
            .is_some_and(|mode| mode != AutoLoadMode::Disabled),
    }
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
        self.assignments.observe_preferences(preferences);
        if state.phase == Phase::Stopped && state.playlist.is_empty() {
            self.assignments.reset();
            self.last_loaded = None;
        }
        let Some(media) = current_media(state) else {
            return;
        };
        let filename = media.filename.as_str();
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
        let explicit = state.explicit_playlist;
        let Some(epoch) = self.assignments.scan_epoch(filename, explicit).filter(|_| {
            preferences.mode != riina_core::subtitle_controls::AutoLoadMode::Disabled
                && crate::file_actions::local_media_path(filename).is_some()
        }) else {
            self.ready.mark_ready(generation);
            return;
        };
        let Some((_, media_path)) = scan else { return };
        if let Some(paths) = self.assignments.automatic_for(state, media) {
            load_automatic(app, &self.player, generation, paths);
            self.ready.mark_ready(generation);
            return;
        }
        let request = self.ready.begin(generation);
        let ready = self.ready.clone();
        let playback = self.playback.clone();
        let assignments = self.assignments.clone();
        let sender = self.player.clone();
        let preferences = preferences.clone();
        let media = media.clone();
        let weak = app.as_weak();
        let started = crate::background::spawn_named("riina-subtitle-discovery", move || {
            let matches = crate::subtitle_discovery::find_matches(
                &media_path,
                &preferences,
                !explicit,
                || ready.is_current(request) && lock(&playback).generation == generation,
            );
            let _ = crate::background::to_ui(weak, move |app| {
                let state = lock(&playback).clone();
                if state.generation == generation
                    && state.explicit_playlist == explicit
                    && current_media(&state).is_some_and(|current| {
                        current.id == media.id && current.filename == media.filename
                    })
                    && ready.is_current(request)
                    && crate::subtitle_auto_load_preferences::current_player(&app)
                        .is_ok_and(|current| current == preferences)
                {
                    let paths = assignments.record_automatic(epoch, &state, &media, matches);
                    load_automatic(&app, &sender, generation, paths);
                }
                ready.complete(request);
            });
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

fn load_automatic(app: &PlayerWindow, player: &CommandSender, generation: u64, paths: Vec<String>) {
    if !paths.is_empty()
        && let Err(error) = player.send(Command::AddMatchedSubtitles {
            generation,
            paths,
            select_first: false,
        })
    {
        crate::diagnostics::report(app, error);
    }
}

fn current_media(state: &Playback) -> Option<&PlaylistEntry> {
    if !matches!(
        state.phase,
        Phase::Playing | Phase::Paused | Phase::Buffering | Phase::Seeking
    ) {
        return None;
    }
    state.playlist.iter().find(|entry| entry.current)
}

fn discover_playlist_files(
    app: &PlayerWindow,
    request: crate::auto_playlist::Request,
    snapshot: Arc<Playback>,
    playback: crate::PlaybackStore,
    sender: riina_mpv::CommandSender,
    series: series::Cache,
) {
    let weak_app = app.as_weak();
    let started = crate::background::spawn_named("riina-playlist-auto-add", move || {
        let Some(prepared) = crate::auto_playlist::prepare(&request, &snapshot) else {
            return;
        };
        let _ = crate::background::to_ui(weak_app, move |app| {
            let state = lock(&playback).clone();
            if folder_options(&app).for_playback(&state) != request.options
                || !crate::auto_playlist::request_is_current(&request, &state)
            {
                return;
            }
            if let Some(prefixes) = prepared.series {
                *lock(&series) = Some(Arc::new(prefixes));
                app.invoke_refresh_playlist_metadata();
            }
            if let Some(command) = prepared.insert
                && let Err(error) = sender.send_for_media(request.generation, command)
            {
                crate::diagnostics::report(&app, error);
            }
        });
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
            series::Cache::default(),
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
