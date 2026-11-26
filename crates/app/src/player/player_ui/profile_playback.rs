//! Share live playback preferences among players using the same profile.

use super::{PlayerContext, volume_preference::VolumePreference};
use crate::{PlayerWindow, PreferencesWindow, key_binding_preferences, osc_preferences};
use riina_core::{
    Command, Playback, Preferences,
    audio_preferences::ReplayGainOptions,
    key_bindings::Settings as KeyBindings,
    osc_preferences::{ControllerOptions, TimePrecision},
};
use riina_ui::{PlaybackTimelineState, PlayerControlActions};
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    collections::BTreeMap,
    path::{Path, PathBuf},
    rc::{Rc, Weak},
    time::Duration,
};

#[derive(Default)]
pub(super) struct ProfilePlayback {
    players: RefCell<Vec<Weak<PlayerPreferences>>>,
    volumes: RefCell<BTreeMap<PathBuf, Weak<VolumePreference>>>,
    pub(super) form: crate::platform::preferences::ControllerOptionsForm,
}

struct PlayerPreferences {
    context: PlayerContext,
    form: RefCell<slint::Weak<PreferencesWindow>>,
    retry_gain: slint::Timer,
    retry_font: slint::Timer,
}

impl PlayerPreferences {
    fn apply_font(self: &Rc<Self>, font: String) {
        self.retry_font.stop();
        if self.context.app().is_none() {
            return;
        }
        match self
            .context
            .commands()
            .send(Command::SubtitleFont(font.clone()))
        {
            Ok(()) => {}
            Err(riina_mpv::EngineError::QueueFull) => {
                let player = Rc::downgrade(self);
                self.retry_font.start(
                    slint::TimerMode::SingleShot,
                    Duration::from_millis(50),
                    move || {
                        if let Some(player) = player.upgrade() {
                            player.apply_font(font.clone());
                        }
                    },
                );
            }
            Err(error) => self.context.report(error),
        }
    }

    fn apply_gain(self: &Rc<Self>, options: ReplayGainOptions) {
        self.retry_gain.stop();
        if self.context.app().is_none() {
            return;
        }
        match self
            .context
            .commands()
            .send(Command::SetReplayGain(options))
        {
            Ok(()) => {}
            Err(riina_mpv::EngineError::QueueFull) => {
                let player = Rc::downgrade(self);
                self.retry_gain.start(
                    slint::TimerMode::SingleShot,
                    Duration::from_millis(50),
                    move || {
                        if let Some(player) = player.upgrade() {
                            player.apply_gain(options);
                        }
                    },
                );
            }
            Err(error) => self.context.report(error),
        }
    }
}

#[derive(Clone)]
pub(crate) struct ProfilePlaybackBinding {
    profile: Rc<ProfilePlayback>,
    player: Rc<PlayerPreferences>,
}

impl ProfilePlaybackBinding {
    pub(crate) fn attach(&self, form: &PreferencesWindow) {
        self.player.form.replace(form.as_weak());
        self.profile
            .connect_key_binding_files(self.player.context.preferences_path(), form);
    }

    pub(crate) fn publish_key_bindings(&self, settings: &KeyBindings) {
        self.profile
            .publish_key_bindings(self.player.context.preferences_path(), settings);
    }

    pub(crate) fn publish_replay_gain(&self, options: ReplayGainOptions) {
        self.player.retry_gain.stop();
        self.profile.publish_replay_gain(
            self.player.context.preferences_path(),
            Some(&self.player),
            options,
        );
    }

    pub(crate) fn publish_subtitle_font(&self, font: &str) {
        self.player.retry_font.stop();
        self.profile.publish_subtitle_font(
            self.player.context.preferences_path(),
            Some(&self.player),
            font,
        );
    }
}

impl ProfilePlayback {
    pub(super) fn connect_key_binding_files(
        self: &Rc<Self>,
        path: &Path,
        form: &PreferencesWindow,
    ) {
        let profile = Rc::downgrade(self);
        let path = path.to_owned();
        form.global::<riina_ui::KeyBindingSettingsState>()
            .on_profile_file_changed(move |name| {
                if let Some(profile) = profile.upgrade() {
                    profile.reload_key_bindings(&path, name.as_str());
                }
            });
    }

    pub(super) fn publish_key_bindings(&self, path: &Path, settings: &KeyBindings) {
        let bindings = key_binding_preferences::load_bindings(&settings.current_profile);
        self.for_each_player(path, |player, app| {
            player.context.preferences().adopt_committed(|saved| {
                saved.key_bindings.clone_from(settings);
            });
            match &bindings {
                Ok(bindings) => key_binding_preferences::install_bindings(
                    app,
                    settings,
                    bindings.clone(),
                    player.context.playback().clone(),
                ),
                Err(_) => key_binding_preferences::install_player(
                    app,
                    settings,
                    player.context.playback().clone(),
                ),
            }
            refresh_key_binding_form(player, settings);
        });
        crate::platform::browser_activation::refresh_idle_menu();
    }

    fn reload_key_bindings(&self, path: &Path, name: &str) {
        let Ok(bindings) = key_binding_preferences::load_bindings(name) else {
            return;
        };
        self.for_each_player(path, |player, app| {
            let settings = player.context.preferences().snapshot().key_bindings;
            if settings.current_profile == name {
                key_binding_preferences::install_bindings(
                    app,
                    &settings,
                    bindings.clone(),
                    player.context.playback().clone(),
                );
                refresh_key_binding_form(player, &settings);
            }
        });
        crate::platform::browser_activation::refresh_idle_menu();
    }

    pub(super) fn volume(&self, path: &Path, prefs: &Preferences) -> Rc<VolumePreference> {
        let mut volumes = self.volumes.borrow_mut();
        volumes.retain(|_, service| service.strong_count() > 0);
        if let Some(volume) = volumes.get(path).and_then(Weak::upgrade) {
            return volume;
        }
        let volume = Rc::new(VolumePreference::new(path, prefs));
        volumes.insert(path.to_owned(), Rc::downgrade(&volume));
        volume
    }

    pub(super) fn connect(
        self: &Rc<Self>,
        app: &PlayerWindow,
        context: &PlayerContext,
    ) -> ProfilePlaybackBinding {
        let player = Rc::new(PlayerPreferences {
            context: context.clone(),
            form: RefCell::default(),
            retry_gain: slint::Timer::default(),
            retry_font: slint::Timer::default(),
        });
        let mut players = self.players.borrow_mut();
        players.retain(|target| target.strong_count() > 0);
        players.push(Rc::downgrade(&player));
        drop(players);

        let actions = app.global::<PlayerControlActions>();
        crate::platform::bind_controller_alignment_feedback(app);
        let clocks = Rc::clone(self);
        let binding = ProfilePlaybackBinding {
            profile: self.clone(),
            player: player.clone(),
        };
        actions.on_publish_controller_options(move || {
            if let Some(app) = player.context.app() {
                clocks.publish(
                    player.context.preferences_path(),
                    osc_preferences::player_controller_options(&app),
                );
            }
        });
        let toggle = context.clone();
        actions.on_toggle_time_display(move || {
            save(&toggle, |options| {
                options.show_remaining_time = !options.show_remaining_time;
            });
        });
        let select = context.clone();
        actions.on_select_time_precision(move |index| {
            if let Some(precision) = TimePrecision::from_index(index) {
                save(&select, |options| options.time_precision = precision);
            }
        });
        binding
    }

    #[cfg(target_os = "macos")]
    pub(super) fn adopt_application_gain(&self, path: &Path, options: ReplayGainOptions) {
        self.publish_replay_gain(path, None, options);
    }

    fn publish_replay_gain(
        &self,
        path: &Path,
        source: Option<&Rc<PlayerPreferences>>,
        options: ReplayGainOptions,
    ) {
        self.for_each_player(path, |player, _| {
            if source.is_some_and(|source| Rc::ptr_eq(source, player)) {
                return;
            }
            player
                .context
                .preferences()
                .adopt_committed(|saved| saved.audio.set_replay_gain_options(options));
            if let Some(form) = player.form.borrow().upgrade() {
                crate::audio_preferences::install_replay_gain(&form, options);
            }
            player.apply_gain(options);
        });
    }

    #[cfg(target_os = "macos")]
    pub(super) fn adopt_application_font(&self, path: &Path, font: &str) {
        self.publish_subtitle_font(path, None, font);
    }

    fn publish_subtitle_font(
        &self,
        path: &Path,
        source: Option<&Rc<PlayerPreferences>>,
        font: &str,
    ) {
        self.for_each_player(path, |player, _| {
            if source.is_some_and(|source| Rc::ptr_eq(source, player)) {
                return;
            }
            player.context.preferences().adopt_committed(|saved| {
                saved.subtitle_font = font.to_owned();
            });
            if let Some(form) = player.form.borrow().upgrade() {
                form.global::<crate::SubtitleSettingsState>()
                    .set_subtitle_font(font.into());
            }
            player.apply_font(font.to_owned());
        });
    }

    pub(super) fn publish(&self, path: &Path, options: ControllerOptions) {
        self.publish_players(path, options);
        self.form.publish(path, options);
    }

    pub(super) fn publish_players(&self, path: &Path, options: ControllerOptions) {
        self.for_each_player(path, |player, app| {
            let target = &player.context;
            target
                .preferences()
                .adopt_committed(|saved| saved.control_bar.controller = options);
            osc_preferences::install_player_controller_options(app, options);
            let state = riina_core::sync::lock(target.playback()).clone();
            update_labels(app, &state);
            app.global::<PlayerControlActions>()
                .invoke_refresh_controller_options();
        });
    }

    fn for_each_player(
        &self,
        path: &Path,
        mut update: impl FnMut(&Rc<PlayerPreferences>, &PlayerWindow),
    ) {
        let players = self.players.borrow().clone();
        for player in players.into_iter().filter_map(|target| target.upgrade()) {
            if player.context.preferences_path() == path
                && let Some(app) = player.context.app()
            {
                update(&player, &app);
            }
        }
    }
}

fn refresh_key_binding_form(player: &PlayerPreferences, settings: &KeyBindings) {
    if let Some(form) = player.form.borrow().upgrade()
        && let Err(error) = key_binding_preferences::refresh(&form, settings)
    {
        player.context.report(error);
    }
}

pub(super) fn update_labels(app: &PlayerWindow, state: &Playback) {
    let options = osc_preferences::player_controller_options(app);
    let timeline = app.global::<PlaybackTimelineState>();
    timeline.set_position_label(options.elapsed_label(state.position).into());
    timeline.set_duration_label(options.end_label(state).into());
}

fn save(context: &PlayerContext, edit: impl FnOnce(&mut ControllerOptions)) {
    let Some(app) = context.app() else {
        return;
    };
    let model = context.preferences();
    let mut candidate = model.snapshot();
    edit(&mut candidate.control_bar.controller);
    let options = candidate.control_bar.controller;
    if let Err(error) = model.save(candidate) {
        context.report(error);
        return;
    }
    osc_preferences::install_player_controller_options(&app, options);
    app.global::<PlayerControlActions>()
        .invoke_publish_controller_options();
    context.activity();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preferences::model::PreferenceModel;
    use std::sync::{Arc, Mutex};

    #[test]
    fn audio_and_subtitle_defaults_reach_peer_players_and_forms_on_the_same_profile()
    -> Result<(), Box<dyn std::error::Error>> {
        use riina_core::audio_preferences::ReplayGain;
        use std::sync::mpsc::TryRecvError;

        i_slint_backend_testing::init_no_event_loop();
        let directory = riina_test_support::TempDir::new("profile-replay-gain")?;
        let shared = directory.path().join("shared.json");
        let other = directory.path().join("other.json");
        let profiles = Rc::new(ProfilePlayback::default());
        let attach = |path: &Path| -> Result<_, slint::PlatformError> {
            let app = PlayerWindow::new()?;
            let (commands, engine) = riina_mpv::CommandSender::recording(8);
            let model = PreferenceModel::new(path.to_owned(), Preferences::default());
            let context = PlayerContext::new(
                &app,
                commands,
                Default::default(),
                model.clone(),
                None,
                path.to_owned(),
            );
            let binding = profiles.connect(&app, &context);
            let form = PreferencesWindow::new()?;
            binding.attach(&form);
            Ok((app, engine, model, binding, form))
        };

        let (source_app, source_engine, source_model, source, _source_form) = attach(&shared)?;
        let (peer, peer_engine, peer_model, _peer_binding, peer_form) = attach(&shared)?;
        let (other_app, other_engine, other_model, _other_binding, _other_form) = attach(&other)?;
        let mut selected = source_model.snapshot();
        let previous = selected.audio.replay_gain_options();
        let options = ReplayGainOptions {
            mode: ReplayGain::Album,
            preamp: -2.5,
            ..previous
        };
        selected.audio.set_replay_gain_options(options);
        source_model.save(selected)?;
        source.publish_replay_gain(options);

        assert_eq!(source_engine.commands.try_recv(), Err(TryRecvError::Empty));
        assert_eq!(
            peer_engine.commands.try_recv(),
            Ok(Command::SetReplayGain(options))
        );
        assert_eq!(peer_model.snapshot().audio.replay_gain_options(), options);
        let audio = peer_form.global::<riina_ui::AudioSettingsState>();
        assert_eq!(audio.get_audio_replay_gain_index(), 2);
        assert_eq!(audio.get_audio_replay_gain_preamp_text(), "-2.5");
        assert_eq!(other_engine.commands.try_recv(), Err(TryRecvError::Empty));
        assert_eq!(other_model.snapshot().audio.replay_gain_options(), previous);

        let mut selected = source_model.snapshot();
        selected.subtitle_font = "Test family".into();
        source_model.save(selected)?;
        source.publish_subtitle_font("Test family");
        assert_eq!(source_engine.commands.try_recv(), Err(TryRecvError::Empty));
        assert_eq!(
            peer_engine.commands.try_recv(),
            Ok(Command::SubtitleFont("Test family".into()))
        );
        assert_eq!(peer_model.snapshot().subtitle_font, "Test family");
        assert_eq!(
            peer_form
                .global::<crate::SubtitleSettingsState>()
                .get_subtitle_font(),
            "Test family"
        );
        assert_eq!(other_engine.commands.try_recv(), Err(TryRecvError::Empty));

        let settings = KeyBindings {
            current_profile: "Movist Default".into(),
            use_system_media_controls: false,
            ..KeyBindings::default()
        };
        let mut selected = source_model.snapshot();
        selected.key_bindings = settings.clone();
        source_model.save(selected)?;
        source.publish_key_bindings(&settings);
        assert_eq!(peer_model.snapshot().key_bindings, settings);
        assert_eq!(other_model.snapshot().key_bindings, KeyBindings::default());
        assert_eq!(
            peer_form
                .global::<riina_ui::KeyBindingSettingsState>()
                .get_key_binding_current_profile(),
            "Movist Default"
        );
        for player in [&source_app, &peer] {
            let seek = Rc::new(std::cell::Cell::new(0.0));
            let captured = seek.clone();
            player
                .global::<riina_ui::PlayerMenuActions>()
                .on_seek_relative(move |seconds| captured.set(seconds));
            let controls = player.global::<PlayerControlActions>();
            assert!(controls.invoke_dispatch_key_binding(
                slint::platform::Key::LeftArrow.into(),
                false,
                false,
                false,
                false,
                false,
            ));
            assert_eq!(seek.get(), -10.0);
            assert!(!controls.invoke_dispatch_key_binding(
                "PLAY".into(),
                false,
                false,
                false,
                false,
                false
            ));
        }
        assert_ne!(
            other_app
                .global::<riina_ui::PlayerPreferences>()
                .get_key_binding_current_profile(),
            "Movist Default"
        );
        Ok(())
    }

    #[test]
    fn controller_gestures_update_paused_players_and_merge_with_other_edits()
    -> Result<(), Box<dyn std::error::Error>> {
        i_slint_backend_testing::init_no_event_loop();
        let directory = riina_test_support::TempDir::new("controller-clock")?;
        let path = directory.path().join("preferences.json");
        let clocks = Rc::new(ProfilePlayback::default());
        let first = PlayerWindow::new()?;
        let second = PlayerWindow::new()?;
        let (commands, _engine) = riina_mpv::CommandSender::recording(8);
        let playback = Arc::new(Mutex::new(Arc::new(Playback {
            duration: Some(60.0),
            position: Some(30.0),
            remaining_time: Some(30.0),
            scaled_remaining_time: Some(15.0),
            paused: true,
            ..Default::default()
        })));
        let attach = |app| {
            let model = PreferenceModel::new(path.clone(), Preferences::default());
            let context = PlayerContext::new(
                app,
                commands.clone(),
                playback.clone(),
                model.clone(),
                None,
                path.clone(),
            );
            clocks.connect(app, &context);
            model
        };
        let first_model = attach(&first);
        let second_model = attach(&second);
        first
            .global::<PlayerControlActions>()
            .invoke_toggle_time_display();
        second
            .global::<PlayerControlActions>()
            .invoke_select_time_precision(3);
        for app in [&first, &second] {
            let timeline = app.global::<PlaybackTimelineState>();
            assert_eq!(timeline.get_position_label(), "00:30.000");
            assert_eq!(timeline.get_duration_label(), "-00:30.000");
        }

        let mut edited = first_model.snapshot();
        edited.volume = 37.0;
        first_model.save(edited)?;
        let saved = Preferences::parse(&std::fs::read_to_string(&path)?)?;
        assert_eq!(
            saved.control_bar.controller,
            second_model.snapshot().control_bar.controller
        );
        assert!(saved.control_bar.controller.show_remaining_time);
        assert_eq!(
            saved.control_bar.controller.time_precision,
            TimePrecision::Milliseconds
        );
        assert!((saved.volume - 37.0).abs() < f64::EPSILON);
        Ok(())
    }
}
