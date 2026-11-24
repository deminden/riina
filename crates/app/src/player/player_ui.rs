mod audio_controls;
mod catalog;
mod context;
mod control_actions;
mod delays;
mod dialogs;
mod discovery;
mod engine_start;
mod features;
mod feedback;
mod fullscreen;
#[cfg(test)]
pub(crate) mod glue_tests;
mod hdr_output;
mod history_session;
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
mod plugin_menus;
mod plugins;
mod subtitle_controls;
mod video_controls;
use context::{PlayerContext, bind};
use engine_start::EngineStart;
use transitions::PendingNavigation;
mod session_preferences;

pub(crate) use plugin_menus::refresh_plugin_menu_shortcuts;
use plugin_menus::reorder_plugin_menu_rows;

use crate::{
    PlayerWindow, WelcomeWindow, platform, preference_load::LoadedPreferences,
    preferences::model::PreferenceModel,
};
use riina_core::{Phase, Playback};
use riina_mpv::Player;
use slint::ComponentHandle;
use std::rc::Rc;

pub(crate) fn help_menu_url(destination: &str) -> Option<&'static str> {
    match destination {
        "help" => Some("https://github.com/deminden/riina#status"),
        "release-highlights" => Some("https://github.com/deminden/riina/releases/latest"),
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
            help_menu_url("release-highlights"),
            Some("https://github.com/deminden/riina/releases/latest")
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

pub(crate) fn refresh_playlist_resume_positions() {
    crate::playlist_metadata::resume::invalidate();
    for app in input::open_player_windows() {
        app.invoke_refresh_playlist_metadata();
    }
}

pub(crate) fn show_osd_message(
    app: &PlayerWindow,
    text: impl Into<slint::SharedString>,
    keep_stopped: bool,
) {
    if !app
        .global::<riina_ui::PlayerPreferences>()
        .get_osd_enabled()
        || app.global::<riina_ui::PlayerCropState>().get_crop_editing()
    {
        return;
    }
    app.set_video_osd_request_feedback(keep_stopped);
    let osd = app.global::<riina_ui::PlayerOsdState>();
    osd.set_video_osd_text(text.into());
    osd.set_video_osd_progress(-1.0);
    app.global::<riina_ui::PlayerControlActions>()
        .invoke_show_video_osd();
}

fn window_video_geometry(state: &Playback) -> Option<(f32, f32, i32)> {
    if matches!(state.phase, Phase::Loading | Phase::Stopped | Phase::Failed) {
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
    platform_extras: platform::SessionExtras,
    _plugins: plugins::RuntimeSession,
    _application: ApplicationServices,
    _presentation: Option<SessionResource<platform::NativePresentation>>,
    _live_text_timer: Option<Rc<slint::Timer>>,
    _preference_recovery_notice: Option<Rc<slint::Timer>>,
    _observer_state: observer_state::ObserverRegistration,
    // The repeated sampler is held for the same lifetime as the player window.
    _appearance_timer: Rc<slint::Timer>,
    _features: features::PlaybackFeatures,
    pub(crate) preference_model: PreferenceModel,
    preference_state: session_preferences::SessionState,
}

impl UiSession {
    pub(super) fn prepare_to_close(&self) {
        self.platform_extras.prepare_to_close();
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn player(&self) -> Option<&Player> {
        match self.player.as_ref()? {
            SessionResource::Live(player) => Some(player),
            #[cfg(test)]
            SessionResource::Recorded(_) => None,
        }
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
    pub(crate) history: Rc<crate::playback_history_window::ApplicationHistory>,
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
        media.plugin_startup(&application.plugins),
    )?;
    let prefs = &session.prefs;
    let prefs_path = &session.prefs_path;
    let latest_playback = &session.projection.latest_playback;
    let observation_dispatch::PreparedObservation {
        notification,
        music_mode_state,
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
    startup::apply_player_preferences(&command_sender, prefs)?;
    if let Some(host) = &mut session.plugin_host {
        host.attach_player(command_sender.clone());
    }
    let resize_filter = native
        .is_none()
        .then(|| crate::window_aspect::connect_with_target(app, None, &media_geometry));
    let features::Installed {
        guards,
        timers,
        live_text_timer,
        pending_navigation,
        scroll_monitor,
        subtitle_assignments,
    } = features::install(
        app,
        features::Inputs {
            native: native.as_ref(),
            welcome,
            commands: &command_sender,
            playback: latest_playback,
            prefs_path,
            history: &session.history,
            history_window: &application.history,
            plugin_host: session.plugin_host_handle.as_ref(),
            plugin_commands: &session.plugin_ui.plugin_command_sender,
            saved_video_filters: &session.saved_video_filters,
            saved_audio_filters: &session.saved_audio_filters,
            music_mode: &music_mode_state,
            transition_release_generation: &transition_release_generation,
            picture_in_picture: &session.picture_in_picture,
            system_media: &session.projection.system_media,
            capture_transition_first,
            resize_filter,
        },
    );
    observer_state.bind_timers(timers);
    let preference_model = PreferenceModel::new(prefs_path.clone(), prefs.clone());
    let context = PlayerContext::new(
        app,
        command_sender.clone(),
        latest_playback.clone(),
        preference_model.clone(),
        session.plugin_host_handle.clone(),
        prefs_path.clone(),
    );
    session_preferences::connect(app, &context, &session.history);
    media_actions::install_resume_preference(app, &command_sender, welcome, &session.recent.media);
    session.history.install_recording_action(app);
    media_actions::install_pause_on_open(app, &command_sender);
    control_actions::install(
        app,
        control_actions::Inputs {
            context: &context,
            music_mode_state: &music_mode_state,
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
    media_actions::install(
        app,
        media_actions::Context {
            commands: &command_sender,
            open_host: native.as_ref().map(platform::NativePresentation::host),
            welcome,
            recent_media: &session.recent.media,
            recent_open_documents: &session.recent.documents,
            recent_open_cleared: &session.recent.documents_cleared,
            prefs_path,
            latest_playback,
        },
    );
    window_actions::install_lifecycle(app, welcome);

    startup::activate_output(
        app,
        native.is_some(),
        player.as_ref(),
        &command_sender,
        prefs,
        startup::InitialOpen {
            media,
            plugins: session.plugin_host_handle.clone(),
        },
        &session.picture_in_picture,
    )?;
    let (platform_extras, plugin_runtime) = session.plugin_ui.finish(
        session.plugin_host,
        session.projection.system_media,
        session.picture_in_picture,
        scroll_monitor,
    );
    Ok(UiSession {
        player: player.map(SessionResource::Live),
        platform_extras,
        _plugins: plugin_runtime,
        _application: application,
        _presentation: native.map(SessionResource::Live),
        _live_text_timer: live_text_timer,
        _preference_recovery_notice: session.recovery.and_then(|recovery| recovery.announce(app)),
        _observer_state: observer_state,
        _appearance_timer: session.projection.appearance_timer,
        _features: guards,
        preference_model,
        preference_state: session_preferences::SessionState {
            prefs_path: session.prefs_path,
            audio_equalizer: session.audio_equalizer,
            saved_video_filters: session.saved_video_filters,
            saved_audio_filters: session.saved_audio_filters,
            recent: session.recent,
            history: session.history,
            latest_playback: session.projection.latest_playback,
        },
    })
}

#[cfg(test)]
mod welcome_tests {
    use super::{
        media_actions::recent::{recent_document_rows, recent_media_rows, resume_media_rows},
        startup::resume_engine_options,
    };
    use riina_core::RecentMedia;

    #[test]
    fn resume_item_is_not_repeated_in_recent_documents() {
        let history = [
            RecentMedia {
                path: "resume.mkv".into(),
                title: "Resume item".into(),
                position: 12.0,
            },
            RecentMedia {
                path: "older.mkv".into(),
                title: "Older item".into(),
                position: 3.0,
            },
        ];

        let resume = recent_media_rows(&history);
        let resume_action = resume_media_rows(&history, true);
        let documents = recent_document_rows(&history, true);

        assert_eq!(resume[0].title.as_str(), "Resume item");
        assert_eq!(resume_action[0].title.as_str(), "Resume item");
        assert_eq!(documents.len(), 1);
        assert_eq!(documents[0].title.as_str(), "Older item");

        assert!(resume_media_rows(&history, false).is_empty());
        let all_documents = recent_document_rows(&history, false);
        assert_eq!(all_documents.len(), 2);
        assert_eq!(all_documents[0].title.as_str(), "Resume item");
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
