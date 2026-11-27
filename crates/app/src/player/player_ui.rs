#[cfg(target_os = "macos")]
mod application_tools;
mod audio_controls;
mod catalog;
mod chapters;
mod context;
mod control_actions;
mod controller_arrows;
mod profile_playback;
mod volume_preference;
pub(crate) use profile_playback::ProfilePlaybackBinding;
mod delays;
mod dialogs;
mod discovery;
pub(crate) mod embedded_input;
mod engine_start;
mod features;
mod feedback;
mod fullscreen;
#[cfg(test)]
pub(crate) mod glue_tests;
mod hdr_output;
mod history_session;
mod idle_media;
pub(crate) mod lifecycle;
mod live_text;
mod media_actions;
mod music_mode;
mod playback_mailbox;
mod playlist_actions;
mod playlist_drop;
mod playlist_hover;
mod playlist_menu;
mod playlist_subtitles;
mod recent_activity;
#[cfg(target_os = "macos")]
pub(crate) use recent_activity::distinct_documents;
mod rendering;
mod startup;
mod thumbnail_actions;
mod transitions;
mod transport;
mod window_actions;
use feedback::clear_screenshot_preview;
mod initial_projection;
pub(crate) mod input;
mod observation_dispatch;
mod observations;
mod observer_state;
pub(in crate::player) use observer_state::ObserverId;
mod plugin_menus;
mod plugins;
mod subtitle_controls;
mod subtitle_menu;
mod video_controls;
pub(crate) use context::PlayerContext;
use context::bind;
use engine_start::EngineStart;
use transitions::PendingNavigation;
mod session_preferences;

pub(crate) use plugin_menus::refresh_plugin_menu_shortcuts;
use plugin_menus::reorder_plugin_menu_rows;

use crate::{
    PlayerWindow, WelcomeWindow, media_open, platform, preference_load::LoadedPreferences,
    preferences::model::PreferenceModel,
};
use riina_core::{Command, Phase, Playback};
use riina_mpv::Player;
use slint::ComponentHandle;
use std::rc::Rc;

pub(crate) fn help_menu_url(destination: &str) -> Option<&'static str> {
    match destination {
        "help" => Some("https://github.com/deminden/riina#status"),
        "github" => Some("https://github.com/deminden/riina"),
        // The repository README is the project's user-facing homepage.
        "website" => Some("https://github.com/deminden/riina#readme"),
        _ => None,
    }
}

#[cfg(test)]
mod help_menu_tests {
    use super::help_menu_url;

    #[test]
    fn documented_help_destinations_map_to_project_pages() {
        assert_eq!(
            help_menu_url("help"),
            Some("https://github.com/deminden/riina#status")
        );
        assert_eq!(
            help_menu_url("github"),
            Some("https://github.com/deminden/riina")
        );
        assert_eq!(
            help_menu_url("website"),
            Some("https://github.com/deminden/riina#readme")
        );
        assert_eq!(help_menu_url("unknown"), None);
    }
}

pub(crate) fn reorder_all_open_plugin_menus(order: &[String]) {
    let windows = input::open_player_windows();
    let model = slint::ModelRc::new(slint::VecModel::from(
        order
            .iter()
            .cloned()
            .map(slint::SharedString::from)
            .collect::<Vec<_>>(),
    ));
    for app in windows {
        app.set_plugin_menu_order(model.clone());
        reorder_plugin_menu_rows(&app, order);
    }
}

pub(crate) fn project_open_window_policy(owner: &PlayerWindow, always_open_new: bool) {
    owner
        .global::<riina_ui::PlayerPreferences>()
        .set_open_in_new_player(always_open_new);
    for app in input::open_player_windows() {
        app.global::<riina_ui::PlayerPreferences>()
            .set_open_in_new_player(always_open_new);
    }
}

pub(crate) fn show_osd_message(
    app: &PlayerWindow,
    text: impl Into<slint::SharedString>,
    keep_stopped: bool,
) {
    if !super::presentation::playback_osd_allowed(app)
        || app.global::<riina_ui::PlayerCropState>().get_crop_editing()
    {
        return;
    }
    let visibility = if keep_stopped {
        crate::video_osd::Visibility::KeepStopped
    } else {
        crate::video_osd::Visibility::Normal
    };
    crate::video_osd::present(app, text, "", None, visibility);
}

pub(crate) fn show_subtitle_saved(app: &PlayerWindow) {
    if app.get_loaded()
        && !app
            .global::<riina_ui::PlayerMusicModeState>()
            .get_music_mode()
        && !app.global::<riina_ui::PlayerCropState>().get_crop_editing()
    {
        crate::video_osd::present(
            app,
            "Subtitle saved",
            "",
            None,
            crate::video_osd::Visibility::Required,
        );
    }
}

fn window_video_geometry(state: &Playback) -> Option<(f32, f32, i32)> {
    if matches!(state.phase, Phase::Loading | Phase::Stopped | Phase::Failed) {
        return None;
    }
    // libmpv can report its subtitle canvas as a video target for a song
    // without artwork. Such a target cannot set the next main window size.
    if state.video.is_none()
        && !state
            .tracks
            .iter()
            .any(|track| track.kind == riina_core::TrackKind::Video)
    {
        return None;
    }
    let video = state.window_size_video_parameters()?;
    let width = video.dw.filter(|size| *size > 0).unwrap_or(video.w);
    let height = video.dh.filter(|size| *size > 0).unwrap_or(video.h);
    if width == 0 || height == 0 {
        return None;
    }
    Some((
        width as f32,
        height as f32,
        state.video_rotation.or(video.rotate).unwrap_or(0) as i32,
    ))
}

fn media_geometry_requires_resize(
    previous_generation: Option<u64>,
    geometry_was_missing: bool,
    state: &Playback,
) -> bool {
    window_video_geometry(state).is_some()
        && (previous_generation != Some(state.generation) || geometry_was_missing)
}

enum SessionResource<T> {
    Live(T),
    #[cfg(test)]
    Recorded(glue_tests::DropProbe),
}

pub(crate) struct UiSession {
    // Native callbacks retain senders only. Field order keeps the target alive
    // until this sole native player owner has joined its decoder worker.
    player: Option<SessionResource<Player>>,
    idle_media_available: Option<Rc<std::cell::Cell<bool>>>,
    platform_extras: platform::SessionExtras,
    _plugins: plugins::RuntimeSession,
    application: ApplicationServices,
    debug_information: crate::debug_dump::Registration,
    window_menu: platform::window_menu::Registration,
    _presentation: Option<SessionResource<platform::NativePresentation>>,
    _live_text_timer: Option<Rc<slint::Timer>>,
    _preference_recovery_notice: Option<Rc<slint::Timer>>,
    observer_state: observer_state::ObserverRegistration,
    // The repeated sampler is held for the same lifetime as the player window.
    _appearance_timer: Rc<slint::Timer>,
    features: features::PlaybackFeatures,
    pub(crate) preference_model: PreferenceModel,
    preference_state: session_preferences::SessionState,
}

impl UiSession {
    pub(super) fn prepare_to_close(&self) {
        self.features.display_blackout.retire();
        self.features.fullscreen.retire();
        self.debug_information.retire();
        self.window_menu.retire();
        self.observer_state.release_power();
        self.observer_state.release_touch_bar();
        self.platform_extras.prepare_to_close();
    }

    pub(crate) fn player(&self) -> Option<&Player> {
        match self.player.as_ref()? {
            SessionResource::Live(player) => Some(player),
            #[cfg(test)]
            SessionResource::Recorded(_) => None,
        }
    }

    pub(crate) fn open_browser_link(
        &self,
        app: &PlayerWindow,
        link: crate::browser_links::Link,
    ) -> Result<(), String> {
        let player = self.player().ok_or("The playback engine is unavailable")?;
        let append = link.enqueue && media_open::has_playlist(app);
        let presentation = link.launch.presentation;
        let playback = link.playback(append);
        let is_stream = !append && media_open::validate_url(&playback.path).is_ok();
        if is_stream {
            self.application.open_url.open_browser_stream(
                app,
                &player.command_sender(),
                playback,
            )?;
        } else if append {
            super::playlist_edits::submit(
                app,
                &player.command_sender(),
                Command::OpenMediaLink(playback),
            )?;
        } else {
            player.send(Command::OpenMediaLink(playback))?;
        }
        if append {
            if let Some(presentation) = presentation {
                observation_dispatch::apply_startup_presentation(app, presentation);
            }
        } else {
            let request = if is_stream {
                app.get_media_request_generation()
            } else {
                app.set_status("Opening…".into());
                media_open::invalidate(app)
            };
            if let Some(presentation) = presentation {
                self.observer_state
                    .queue_startup_presentation(request, presentation);
            }
        }
        Ok(())
    }

    pub(crate) fn release_player(&mut self) {
        if let Some(player) = self.player.take() {
            match player {
                SessionResource::Live(player) => drop(player),
                #[cfg(test)]
                SessionResource::Recorded(probe) => drop(probe),
            }
        }
    }
}

impl Drop for UiSession {
    fn drop(&mut self) {
        self.prepare_to_close();
        // Release session ownership before its presentation on every platform.
        // Native playback joins here; GL callbacks retain their player until the
        // renderer has released its surface during RenderingTeardown.
        self.release_player();
    }
}

struct SessionStartup {
    engine: EngineStart,
    media_geometry: crate::window_aspect::MediaGeometry,
    application: ApplicationServices,
}

pub(crate) use plugins::ApplicationPlugins;
pub(crate) use startup::InitialMedia;

#[derive(Clone, Default)]
pub(crate) struct ApplicationServices {
    pub(crate) plugins: Rc<ApplicationPlugins>,
    pub(crate) filters: Rc<crate::video_filter_window::ApplicationFilters>,
    pub(crate) history: Rc<crate::playback_history_window::ApplicationHistory>,
    inspector: Rc<crate::windows::media_inspector::ApplicationInspector>,
    pub(crate) logs: Rc<crate::log_viewer::ApplicationLogViewer>,
    pub(crate) debug_information: Rc<crate::debug_dump::ApplicationDebugInformation>,
    window_menu: platform::window_menu::Application,
    pub(crate) highlights: Rc<crate::windows::release_highlights::ReleaseHighlights>,
    pub(crate) open_url: Rc<crate::windows::open_url::ApplicationOpenUrl>,
    pub(crate) about: Rc<crate::about_window::ApplicationAbout>,
    recents: Rc<recent_activity::ApplicationRecents>,
    pub(crate) updates: Rc<crate::updates::ApplicationUpdates>,
    power: Rc<crate::playback_power::ApplicationPower>,
    profile_playback: Rc<profile_playback::ProfilePlayback>,
    touch_bars: platform::player_touch_bar::Application,
    pub(crate) settings: platform::application_settings::Owner,
}

impl ApplicationServices {
    pub(crate) fn present_release_highlights(
        &self,
        app: &PlayerWindow,
        welcome: Option<&WelcomeWindow>,
    ) {
        self.highlights.show_first_launch(app, welcome);
    }

    fn connect_preferences(
        &self,
        app: &PlayerWindow,
        context: &PlayerContext,
        session: &startup::PreparedSession,
        presentation: Option<&platform::NativePresentation>,
    ) {
        self.updates
            .connect(app, presentation, context.preferences_path());
        let preferences = context.preferences().snapshot();
        self.power.initialize(preferences.playback_power);
        self.logs.connect(
            app,
            context.preferences_path(),
            preferences.advanced.log_level,
        );
        self.inspector.connect(
            app,
            context.playback().clone(),
            context.commands().clone(),
            context.preferences_path(),
        );
        self.highlights.connect(app);
        self.about.connect(app, context.playback().clone());
        self.settings
            .font_picker()
            .connect_player(app, context.commands());
        let profile = self.profile_playback.connect(app, context);
        session_preferences::connect(
            app,
            context,
            &session.history,
            &session.recent.activity,
            self,
            profile,
        );
    }

    fn bind_playback(
        &self,
        app: &PlayerWindow,
        observer: &observer_state::ObserverRegistration,
        timers: features::PlaybackTimers,
        native_engine: bool,
        volume: &Rc<volume_preference::VolumePreference>,
    ) {
        observer.bind_volume(volume);
        let power = native_engine.then(|| self.power.attach(app));
        observer.bind_playback(timers, power);
    }
}

pub(crate) fn configure_player(
    app: &PlayerWindow,
    native: Option<platform::NativePresentation>,
    media: InitialMedia,
    welcome: Option<&WelcomeWindow>,
    loaded: LoadedPreferences,
    media_geometry: crate::window_aspect::MediaGeometry,
    application: ApplicationServices,
) -> Result<UiSession, Box<dyn std::error::Error>> {
    configure_with_engine(
        app,
        native,
        media,
        welcome,
        loaded,
        SessionStartup {
            engine: EngineStart::Native,
            media_geometry,
            application,
        },
    )
}

fn configure_with_engine(
    app: &PlayerWindow,
    native: Option<platform::NativePresentation>,
    media: startup::InitialMedia,
    welcome: Option<&WelcomeWindow>,
    loaded: LoadedPreferences,
    SessionStartup {
        engine: engine_start,
        media_geometry,
        application,
    }: SessionStartup,
) -> Result<UiSession, Box<dyn std::error::Error>> {
    let startup::PreparedStartup {
        mut session,
        engine_options,
        observation,
    } = startup::prepare(
        app,
        native.as_ref(),
        welcome,
        &engine_start,
        loaded,
        media_geometry.clone(),
        media.startup_context(&application),
    )?;
    let prefs = &session.prefs;
    let prefs_path = &session.prefs_path;
    let volume = application.profile_playback.volume(prefs_path, prefs);
    let opening_volume =
        startup::initial_volume(prefs, media.launch.volume, volume.pending_level());
    let latest_playback = &session.projection.latest_playback;
    let observation_dispatch::PreparedObservation {
        notification,
        automatic_music,
        crop_editor,
        observer_state,
        feedback_video_osd,
        screenshot_scratch,
        transition_release_generation,
    } = observation;
    let notify = move |state| notification.publish(state);
    let capture_transition_first = engine_start.captures_before_navigation(native.is_some());
    let (player, command_sender) = engine_start
        .start(native.as_ref(), engine_options, notify)
        .inspect_err(|error| app.set_status(error.clone().into()))
        .map_err(std::io::Error::other)?;
    startup::apply_player_preferences(&command_sender, prefs, media.launch.pause)?;
    if let Some(host) = &mut session.plugin_host {
        host.attach_player(command_sender.clone());
    }
    let resize_filter = native
        .is_none()
        .then(|| crate::window_aspect::connect_with_target(app, None, &media_geometry));
    let display = crate::player::surface::PlayerDisplay {
        welcome: welcome.map(slint::ComponentHandle::as_weak),
        host: native.as_ref().map(platform::NativePresentation::host),
    };
    let window_menu = application.window_menu.register(app, &display);
    let features::Installed {
        guards,
        timers,
        live_text_timer,
        pending_navigation,
        scroll_monitor,
        subtitle_assignments,
        timeline_samples,
    } = features::install(
        app,
        features::Inputs {
            native: native.as_ref(),
            display,
            commands: &command_sender,
            playback: latest_playback,
            prefs_path,
            history: &session.history,
            history_window: &application.history,
            plugin_host: session.plugin_host_handle.as_ref(),
            plugin_ui: &session.plugin_ui,
            filters: &session.filters,
            automatic_music: &automatic_music,
            transition_release_generation: &transition_release_generation,
            picture_in_picture: &session.picture_in_picture,
            system_media: &session.projection.system_media,
            capture_transition_first,
            resize_filter,
        },
    );
    application.bind_playback(app, &observer_state, timers, player.is_some(), &volume);
    let preference_model = PreferenceModel::new(prefs_path.clone(), prefs.clone());
    let context = PlayerContext::new(
        app,
        command_sender.clone(),
        latest_playback.clone(),
        preference_model.clone(),
        session.plugin_host_handle.clone(),
        prefs_path.clone(),
    );
    control_actions::install(
        app,
        control_actions::Inputs {
            context: &context,
            native: native.as_ref(),
            crop_editor: &crop_editor,
            screenshot_scratch,
            feedback_video_osd,
            audio_equalizer: &session.audio_equalizer,
            pending_navigation: &pending_navigation,
            capture_transition_first,
            subtitle_assignments,
        },
    );
    application.connect_media_actions(app, native.as_ref(), welcome, &session, &command_sender);
    observer_state.bind_open_url(&application.open_url);
    let idle_media_available = startup::activate_output(
        app,
        native.is_some(),
        player.as_ref(),
        &command_sender,
        opening_volume,
        startup::InitialOpen {
            media,
            plugins: session.plugin_host_handle.clone(),
            observer: observer_state.id(),
        },
        &session.picture_in_picture,
    )?;
    application.connect_preferences(app, &context, &session, native.as_ref());
    observer_state.bind_touch_bar(
        application
            .touch_bars
            .install(app, &context, &timeline_samples),
    );
    let debug_information = application.debug_information.connect(app, latest_playback);
    let (platform_extras, plugin_runtime) = session.plugin_ui.finish(
        session.plugin_host,
        session.projection.system_media,
        session.picture_in_picture,
        scroll_monitor,
    );
    Ok(UiSession {
        player: player.map(SessionResource::Live),
        idle_media_available,
        platform_extras,
        _plugins: plugin_runtime,
        application,
        debug_information,
        window_menu,
        _presentation: native.map(SessionResource::Live),
        _live_text_timer: live_text_timer,
        _preference_recovery_notice: session.recovery.and_then(|recovery| recovery.announce(app)),
        observer_state,
        _appearance_timer: session.projection.appearance_timer,
        features: guards,
        preference_model,
        preference_state: session_preferences::SessionState {
            volume,
            audio_equalizer: session.audio_equalizer,
            recent: session.recent,
            history: session.history,
            latest_playback: session.projection.latest_playback,
        },
    })
}

impl ApplicationServices {
    fn connect_media_actions(
        &self,
        app: &PlayerWindow,
        native: Option<&platform::NativePresentation>,
        welcome: Option<&WelcomeWindow>,
        session: &startup::PreparedSession,
        commands: &riina_mpv::CommandSender,
    ) {
        media_actions::install(
            app,
            media_actions::Context {
                open_url: &self.open_url,
                commands,
                open_host: native.map(platform::NativePresentation::host),
                welcome,
                recent: &session.recent,
                prefs_path: &session.prefs_path,
                latest_playback: &session.projection.latest_playback,
            },
        );
        window_actions::install_lifecycle(app, welcome);
    }
}

#[cfg(test)]
mod welcome_tests {
    use super::{
        media_actions::recent::{resumable, visible_documents},
        startup::resume_engine_options,
    };
    use riina_core::RecentMedia;

    #[test]
    fn resume_item_is_not_repeated_in_recent_documents() {
        let directory = riina_test_support::TempDir::new("resume-selection").unwrap();
        let file = directory.path().join("resume.mkv");
        std::fs::write(&file, b"fixture").unwrap();
        let history = [RecentMedia {
            path: file.to_string_lossy().into_owned(),
            title: "Resume item".into(),
            position: 12.0,
        }];
        let documents = [
            RecentMedia {
                path: "different.mkv".into(),
                title: "Newest document".into(),
                position: 0.0,
            },
            history[0].clone(),
            RecentMedia {
                path: url::Url::from_file_path(&file).unwrap().to_string(),
                ..history[0].clone()
            },
        ];
        let resume = resumable(&history, true);
        assert_eq!(resume, Some(&history[0]));
        assert_eq!(
            visible_documents(&documents, resume).collect::<Vec<_>>(),
            [&documents[0]]
        );
        assert!(resumable(&history, false).is_none());
        std::fs::remove_file(file).unwrap();
        assert!(resumable(&history, true).is_none());
        assert_eq!(visible_documents(&documents, None).count(), 3);
    }

    #[test]
    fn startup_options_configure_resume_and_subtitle_autoload() {
        let directory = std::path::Path::new("profile/watch-later");
        let enabled = resume_engine_options(
            directory,
            true,
            &riina_core::network_preferences::Connection::default(),
            &riina_core::network_preferences::Cache::default(),
            &riina_core::network_preferences::OnlineVideos::default(),
        )
        .unwrap();
        assert_eq!(enabled[0].0, "watch-later-directory");
        assert_eq!(enabled[0].1, directory.to_string_lossy());
        assert_eq!(enabled[1], ("save-position-on-quit".into(), "yes".into()));
        assert_eq!(enabled[2], ("resume-playback".into(), "yes".into()));
        assert_eq!(enabled[3], ("sub-auto".into(), "no".into()));
        assert_eq!(enabled[4], ("rtsp-transport".into(), "tcp".into()));
        assert_eq!(enabled[5], ("cache".into(), "yes".into()));
        assert_eq!(enabled[8], ("cache-secs".into(), "36000".into()));
        assert_eq!(enabled[10], ("ytdl".into(), "yes".into()));
        assert_eq!(enabled[11], ("ytdl-raw-options".into(), "".into()));

        let disabled = resume_engine_options(
            directory,
            false,
            &riina_core::network_preferences::Connection::default(),
            &riina_core::network_preferences::Cache::default(),
            &riina_core::network_preferences::OnlineVideos::default(),
        )
        .unwrap();
        assert_eq!(disabled[1].1, "no");
        assert_eq!(disabled[2].1, "no");
    }
}

#[cfg(test)]
mod window_video_geometry_tests {
    use super::{media_geometry_requires_resize, window_video_geometry};
    use riina_core::{Phase, Playback, VideoParameters};

    fn video(width: u32, height: u32, display: Option<(u32, u32)>) -> VideoParameters {
        VideoParameters {
            pixelformat: None,
            w: width,
            h: height,
            dw: display.map(|(width, _)| width),
            dh: display.map(|(_, height)| height),
            rotate: Some(90),
            colormatrix: None,
            colorlevels: None,
            primaries: None,
            gamma: None,
            max_luma: None,
            min_luma: None,
            signal_peak: None,
        }
    }

    #[test]
    fn natural_size_uses_display_dimensions_and_observed_rotation() {
        let state = Playback {
            phase: Phase::Playing,
            video: Some(video(640, 360, Some((1280, 720)))),
            video_rotation: Some(180),
            ..Playback::default()
        };
        assert_eq!(window_video_geometry(&state), Some((1280.0, 720.0, 180)));
    }

    #[test]
    fn natural_size_falls_back_to_source_dimensions_when_display_size_is_zero() {
        let state = Playback {
            phase: Phase::Playing,
            video: Some(video(640, 360, Some((0, 0)))),
            ..Playback::default()
        };
        assert_eq!(window_video_geometry(&state), Some((640.0, 360.0, 90)));
    }

    #[test]
    fn subtitle_canvas_does_not_size_the_main_window_for_plain_audio() {
        let state = Playback {
            phase: Phase::Paused,
            video_target: Some(video(960, 600, None)),
            tracks: vec![
                serde_json::from_value(serde_json::json!({"id": 1, "type": "audio"})).unwrap(),
            ]
            .into(),
            ..Playback::default()
        };
        assert_eq!(window_video_geometry(&state), None);
        let mut discovering = state;
        discovering.tracks = Default::default();
        assert_eq!(window_video_geometry(&discovering), None);
    }

    #[test]
    fn coalesced_file_change_resizes_even_without_a_loading_snapshot() {
        let state = Playback {
            generation: 2,
            phase: Phase::Playing,
            video: Some(video(640, 480, None)),
            ..Playback::default()
        };
        assert!(media_geometry_requires_resize(Some(1), false, &state));
        assert!(!media_geometry_requires_resize(Some(2), false, &state));
    }

    #[test]
    fn late_geometry_resizes_once_in_the_same_media_generation() {
        let mut state = Playback {
            generation: 2,
            phase: Phase::Playing,
            ..Playback::default()
        };
        assert!(!media_geometry_requires_resize(Some(1), true, &state));
        state.video = Some(video(640, 480, None));
        assert!(media_geometry_requires_resize(Some(2), true, &state));
        assert!(!media_geometry_requires_resize(Some(2), false, &state));
        state.phase = Phase::Loading;
        assert!(!media_geometry_requires_resize(Some(1), true, &state));
    }

    #[test]
    fn missing_or_zero_geometry_does_not_enable_video_size_commands() {
        assert_eq!(window_video_geometry(&Playback::default()), None);
        let state = Playback {
            phase: Phase::Playing,
            video: Some(video(0, 360, None)),
            ..Playback::default()
        };
        assert_eq!(window_video_geometry(&state), None);

        for phase in [Phase::Loading, Phase::Stopped, Phase::Failed] {
            let state = Playback {
                phase,
                video: Some(video(640, 360, None)),
                ..Playback::default()
            };
            assert_eq!(window_video_geometry(&state), None);
        }
    }
}
