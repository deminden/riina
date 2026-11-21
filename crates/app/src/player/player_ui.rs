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
#[cfg(test)]
pub(crate) mod glue_tests;
pub(crate) mod hdr_output;
mod history_session;
pub(crate) mod lifecycle;
mod live_text;
mod media_actions;
mod music_mode;
mod playback_mailbox;
mod playlist_actions;
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
mod session_preferences;

use plugin_menus::reorder_plugin_menu_rows;

use crate::{
    PlayerWindow, WelcomeWindow, media_open, platform, preference_load::LoadedPreferences,
    preferences::model::PreferenceModel,
};
use riina_core::{Command, Phase, Playback, Preferences};
use riina_mpv::Player;
use slint::ComponentHandle;
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
    sync::{Arc, atomic::Ordering},
    time::Instant,
};

type PendingNavigation = Rc<RefCell<Option<(Vec<Command>, Instant)>>>;

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

pub(crate) fn show_osd_message(
    app: &PlayerWindow,
    text: impl Into<slint::SharedString>,
    keep_stopped: bool,
) {
    if !app.get_osd_enabled() || app.get_crop_editing() {
        return;
    }
    app.set_video_osd_request_feedback(keep_stopped);
    app.set_video_osd_text(text.into());
    app.set_video_osd_progress(-1.0);
    app.invoke_show_video_osd();
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
    _plugin_host: Option<crate::plugin_host::PluginHost>,
    _plugin_host_timer: Option<Rc<slint::Timer>>,
    _plugin_osd_timer: Option<Rc<slint::Timer>>,
    _presentation: Option<SessionResource<platform::NativePresentation>>,
    _hdr_display_timer: Option<Rc<slint::Timer>>,
    _live_text_timer: Option<Rc<slint::Timer>>,
    _preference_recovery_notice: Option<Rc<slint::Timer>>,
    _observer_state: observer_state::ObserverRegistration,
    // The repeated sampler is held for the same lifetime as the player window.
    _appearance_timer: Rc<slint::Timer>,
    _pause_resume_timer: Rc<slint::Timer>,
    _features: features::PlaybackFeatures,
    prefs_path: PathBuf,
    pub(crate) preference_model: PreferenceModel,
    audio_equalizer: Arc<std::sync::Mutex<riina_core::audio_equalizer::Settings>>,
    saved_video_filters: Arc<std::sync::Mutex<riina_core::video_filters::Settings>>,
    saved_audio_filters: Arc<std::sync::Mutex<riina_core::video_filters::Settings>>,
    recent_media: Rc<RefCell<Vec<riina_core::RecentMedia>>>,
    recent_open_documents: Rc<RefCell<Vec<riina_core::RecentMedia>>>,
    recent_open_cleared: Rc<Cell<bool>>,
    history: history_session::Tracking,
    latest_playback: crate::PlaybackStore,
}

impl UiSession {
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
        self.platform_extras.prepare_to_close();
        // Release session ownership before its presentation on every platform.
        // Native playback joins here; GL callbacks retain their player until the
        // renderer has released its surface during RenderingTeardown.
        self.release_player();
    }
}

struct SessionStartup {
    engine: EngineStart,
    media_geometry: crate::window_aspect::MediaGeometry,
}

pub(crate) fn configure_player(
    app: &PlayerWindow,
    native: Option<platform::NativePresentation>,
    media: Vec<std::ffi::OsString>,
    welcome: Option<&WelcomeWindow>,
    startup_resume_position: Option<f64>,
    loaded: LoadedPreferences,
    media_geometry: crate::window_aspect::MediaGeometry,
) -> Result<UiSession, Box<dyn std::error::Error>> {
    configure_with_engine(
        app,
        native,
        media,
        welcome,
        startup_resume_position,
        loaded,
        SessionStartup {
            engine: EngineStart::Native,
            media_geometry,
        },
    )
}

#[expect(
    clippy::too_many_lines,
    reason = "decomposition tracked in docs/review"
)]
fn configure_with_engine(
    app: &PlayerWindow,
    native: Option<platform::NativePresentation>,
    media: Vec<std::ffi::OsString>,
    welcome: Option<&WelcomeWindow>,
    startup_resume_position: Option<f64>,
    loaded: LoadedPreferences,
    SessionStartup {
        engine: engine_start,
        media_geometry,
    }: SessionStartup,
) -> Result<UiSession, Box<dyn std::error::Error>> {
    let picture_in_picture = platform::PictureInPictureSession::new(app);
    let prefs_path = engine_start.preferences_path()?;
    let use_system_services = engine_start.uses_system_services();
    let startup::PreparedSettings {
        prefs,
        recovery,
        audio_equalizer,
        saved_video_filters,
        saved_audio_filters,
        history,
    } = startup::prepare_settings(app, &prefs_path, loaded, use_system_services);
    let plugin_host = match crate::plugin_host::PluginHost::start(
        &prefs_path,
        &prefs.recent_media,
        history.entries(),
    ) {
        Ok(host) => Some(host),
        Err(error) => {
            app.set_status(error.into());
            None
        }
    };
    let plugin_host_handle = plugin_host
        .as_ref()
        .map(crate::plugin_host::PluginHost::handle);
    let plugins::InstalledPluginUi {
        #[cfg(target_os = "windows")]
        plugin_overlay_presenter,
        #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
        plugin_sidebar_presenter,
        #[cfg(target_os = "macos")]
        mac_plugin_menu_observer,
        plugin_host_timer,
        plugin_osd_timer,
        plugin_command_sender,
    } = plugins::install(app, native.as_ref(), plugin_host_handle.clone());
    live_text::initialize(app, prefs.live_text_enabled, use_system_services);
    let hdr_display_timer = hdr_output::install_display_observer(app, &prefs, native.as_ref());
    let recent_media = Rc::new(RefCell::new(prefs.recent_media.clone()));
    let recent_open_documents = Rc::new(RefCell::new(prefs.recent_open_documents.clone()));
    let recent_open_cleared = Rc::new(Cell::new(false));
    let history = history.start_tracking(&prefs_path, &prefs);
    let engine_options = startup::options_for_session(app, &prefs, &prefs_path)?;
    let initial_projection::SessionProjection {
        latest_playback,
        pause_resume_timer,
        system_media,
        appearance_timer,
    } = initial_projection::install_session(
        app,
        &prefs,
        &prefs_path,
        native.as_ref(),
        welcome,
        &recent_open_documents,
    );
    let observation_dispatch::PreparedObservation {
        notification,
        music_mode_state,
        crop_editor,
        observer_state,
        feedback_video_osd,
        screenshot_scratch,
        transition_release_generation,
    } = observation_dispatch::prepare_observation(
        app,
        observation_dispatch::Inputs {
            prefs: &prefs,
            media_geometry,
            native_presentation: native.is_some(),
            reported_playback: latest_playback.clone(),
            history: history.observe(plugin_host_handle.clone()),
            saved_video_filters: saved_video_filters.clone(),
            saved_audio_filters: saved_audio_filters.clone(),
            audio_device_sender: plugin_command_sender.clone(),
            picture_in_picture_state_dirty: picture_in_picture.notification_flag(),
        },
    );
    let notify = move |state| notification.publish(state);
    let capture_transition_first = engine_start.captures_before_navigation(native.is_some());
    let (player, command_sender) = match engine_start.start(native.as_ref(), engine_options, notify)
    {
        Ok(engine) => engine,
        Err(error) => {
            app.set_status(error.clone().into());
            return Err(std::io::Error::other(error).into());
        }
    };
    let features::Installed {
        guards,
        live_text_timer,
        pending_navigation,
        #[cfg(target_os = "macos")]
        mac_scroll_event_monitor,
    } = features::install(
        app,
        features::Inputs {
            native: native.as_ref(),
            commands: &command_sender,
            playback: &latest_playback,
            prefs_path: &prefs_path,
            history: &history,
            plugin_host: plugin_host_handle.as_ref(),
            plugin_commands: &plugin_command_sender,
            saved_video_filters: &saved_video_filters,
            saved_audio_filters: &saved_audio_filters,
            music_mode: &music_mode_state,
            transition_release_generation: &transition_release_generation,
            picture_in_picture: &picture_in_picture,
            system_media: &system_media,
            capture_transition_first,
        },
    );
    let weak = app.as_weak();
    let toolbar_settings_window = weak.clone();
    app.on_toolbar_settings_changed(move || {
        if let Some(app) = toolbar_settings_window.upgrade() {
            crate::osc_preferences::refresh_visible_order(&app);
        }
    });
    crate::osc_preferences::refresh_visible_order(app);
    live_text::install_actions(app, &prefs_path);
    music_mode::install_actions(app, &music_mode_state);
    let preference_model = PreferenceModel::new(prefs_path.clone(), prefs.clone());
    let context = PlayerContext::new(
        app,
        command_sender.clone(),
        latest_playback.clone(),
        preference_model.clone(),
        plugin_host_handle,
        prefs_path.clone(),
    );
    crate::preferences_window::connect(
        app,
        context.commands().clone(),
        context.preferences_path().to_path_buf(),
        context.preferences().clone(),
        context.playback().clone(),
        context.plugin_host().cloned(),
        history.clear_hook(
            prefs_path.clone(),
            context.plugin_host().cloned(),
            latest_playback.clone(),
        ),
    );
    media_actions::install_resume_preference(app, &command_sender, welcome, &recent_media);
    history.install_recording_action(app);
    media_actions::install_pause_on_open(app, &command_sender);
    control_actions::install(
        app,
        control_actions::Inputs {
            context: &context,
            native: native.as_ref(),
            crop_editor: &crop_editor,
            screenshot_scratch,
            feedback_video_osd,
            audio_equalizer: &audio_equalizer,
            pending_navigation: &pending_navigation,
            capture_transition_first,
        },
    );
    media_actions::install(
        app,
        media_actions::Context {
            commands: &command_sender,
            open_host: native.as_ref().map(platform::NativePresentation::host),
            welcome,
            recent_media: &recent_media,
            recent_open_documents: &recent_open_documents,
            recent_open_cleared: &recent_open_cleared,
            prefs_path: &prefs_path,
            latest_playback: &latest_playback,
        },
    );
    window_actions::install_lifecycle(app, welcome);

    startup::apply_player_preferences(&command_sender, &prefs)?;
    crate::fullscreen_info::connect(app);
    if native.is_some() {
        app.set_engine_ready(true);
        command_sender
            .send(Command::Volume(prefs.volume))
            .map_err(std::io::Error::other)?;
        if !media.is_empty() {
            media_open::startup_at(app, media, startup_resume_position, command_sender.clone());
        }
    } else if let Some(player) = &player {
        rendering::install(
            app,
            player,
            media,
            startup_resume_position,
            prefs.volume,
            #[cfg(target_os = "macos")]
            picture_in_picture.render_manager(),
        )?;
    }
    Ok(UiSession {
        player: player.map(SessionResource::Live),
        platform_extras: platform::SessionExtras {
            #[cfg(target_os = "windows")]
            _plugin_overlay_presenter: plugin_overlay_presenter,
            _plugin_sidebar_presenter: plugin_sidebar_presenter,
            _system_media: system_media,
            _picture_in_picture: picture_in_picture,
            #[cfg(target_os = "macos")]
            _mac_scroll_event_monitor: mac_scroll_event_monitor,
            #[cfg(target_os = "macos")]
            _mac_plugin_menu_observer: mac_plugin_menu_observer,
        },
        _presentation: native.map(SessionResource::Live),
        _hdr_display_timer: hdr_display_timer,
        _live_text_timer: live_text_timer,
        _preference_recovery_notice: recovery.and_then(|recovery| recovery.announce(app)),
        _observer_state: observer_state,
        _appearance_timer: appearance_timer,
        _pause_resume_timer: pause_resume_timer,
        _features: guards,
        _plugin_host: plugin_host,
        _plugin_host_timer: plugin_host_timer,
        _plugin_osd_timer: plugin_osd_timer,
        prefs_path,
        preference_model,
        audio_equalizer,
        saved_video_filters,
        saved_audio_filters,
        recent_media,
        recent_open_documents,
        recent_open_cleared,
        history,
        latest_playback,
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
            &riina_core::subtitle_controls::AutoLoadPreferences::default(),
            &riina_core::network_preferences::Connection::default(),
            &riina_core::network_preferences::Cache::default(),
            &riina_core::network_preferences::OnlineVideos::default(),
        )
        .unwrap();
        assert_eq!(enabled[0].0, "watch-later-directory");
        assert_eq!(enabled[0].1, directory.to_string_lossy());
        assert_eq!(enabled[1], ("save-position-on-quit".into(), "yes".into()));
        assert_eq!(enabled[2], ("resume-playback".into(), "yes".into()));
        assert_eq!(enabled[3], ("sub-auto".into(), "exact".into()));
        assert_eq!(enabled[4], ("sub-file-paths".into(), "".into()));
        assert_eq!(enabled[5], ("rtsp-transport".into(), "tcp".into()));
        assert_eq!(enabled[6], ("cache".into(), "yes".into()));
        assert_eq!(enabled[9], ("cache-secs".into(), "36000".into()));
        assert_eq!(enabled[11], ("ytdl".into(), "yes".into()));
        assert_eq!(enabled[12], ("ytdl-raw-options".into(), "".into()));

        let disabled = resume_engine_options(
            directory,
            false,
            &riina_core::subtitle_controls::AutoLoadPreferences::default(),
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
