//! Build initial engine options and issue the saved player commands in startup order.
use super::{
    audio_controls::set_audio_equalizer_controls,
    engine_start::EngineStart,
    hdr_output,
    history_session::{LoadedHistory, Tracking},
    initial_projection, live_text, observation_dispatch, plugins,
    recent_activity::RecentActivity,
};
use crate::{
    PlayerWindow, WelcomeWindow, platform,
    plugin_host::{PluginHost, PluginHostHandle},
    preference_load::{LoadedPreferences, Recovery},
    window_aspect::MediaGeometry,
};
use riina_core::{Command, Preferences, audio_equalizer, sync::lock};
use riina_mpv::{CommandSender, Player, StartupOptions};
use slint::ComponentHandle;
use std::{
    cell::Cell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, Mutex},
};

type EngineOptions = Vec<(String, String)>;

pub(super) struct PreparedSettings {
    pub(super) prefs: Preferences,
    pub(super) recovery: Option<Recovery>,
    pub(super) audio_equalizer: Arc<Mutex<audio_equalizer::Settings>>,
    pub(super) history: LoadedHistory,
}

pub(super) fn prepare_settings(
    app: &PlayerWindow,
    prefs_path: &Path,
    loaded: LoadedPreferences,
    use_system_services: bool,
) -> PreparedSettings {
    let plugin_menu_order = std::fs::read_to_string(prefs_path.with_file_name("plugin-order.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<Vec<String>>(&text).ok())
        .unwrap_or_default();
    app.set_plugin_menu_order(slint::ModelRc::new(slint::VecModel::from(
        plugin_menu_order
            .into_iter()
            .map(slint::SharedString::from)
            .collect::<Vec<_>>(),
    )));
    let LoadedPreferences {
        preferences: mut prefs,
        recovery,
    } = loaded;
    if use_system_services
        && let Err(error) = crate::online_subtitles_credentials::restore_preferences(&mut prefs)
    {
        crate::diagnostics::report(app, format!("OpenSubtitles credential storage: {error}"));
    }
    let audio_equalizer = Arc::new(Mutex::new(prefs.audio_equalizer.clone()));
    set_audio_equalizer_controls(app, &lock(&audio_equalizer));
    let history = LoadedHistory::load(app, prefs_path);
    PreparedSettings {
        prefs,
        recovery,
        audio_equalizer,
        history,
    }
}

pub(super) fn options_for_session(
    app: &PlayerWindow,
    prefs: &Preferences,
    prefs_path: &Path,
    launch_options: EngineOptions,
) -> std::io::Result<StartupOptions> {
    let watch_later_directory = prefs_path.with_file_name("watch-later");
    std::fs::create_dir_all(&watch_later_directory)?;
    let ytdl_plugin_active = crate::plugin_host::is_enabled_plugin(prefs_path, "io.iina.ytdl");
    app.global::<riina_ui::PlayerPreferences>()
        .set_network_ytdl_plugin_active(ytdl_plugin_active);
    let advanced_options = crate::advanced_preferences::engine_options(&prefs.advanced, prefs_path)
        .map(|mut options| {
            options.extend(launch_options);
            options
        });
    let configured_device = advanced_options.as_ref().is_ok_and(|options| {
        options
            .iter()
            .any(|(name, value)| name == "audio-device" || (name == "config" && value == "yes"))
    });
    let mut mpv = engine_options(
        prefs,
        &watch_later_directory,
        OptionsFlags {
            ytdl_plugin_active,
            platform_manages_icc: cfg!(target_os = "macos"),
        },
        advanced_options,
    )
    .map_err(std::io::Error::other)?;
    let preferred_audio_device = (!configured_device && prefs.audio.device != "auto").then(|| {
        mpv.retain(|(name, _)| name != "audio-device");
        let driver = mpv.iter().rev().find(|(name, _)| name == "ao");
        driver
            .and_then(|(_, driver)| {
                riina_core::audio_preferences::device_alias(&prefs.audio.device, driver)
            })
            .unwrap_or_else(|| prefs.audio.device.clone())
    });
    Ok(StartupOptions {
        mpv,
        preferred_audio_device,
    })
}

pub(super) struct OptionsFlags {
    pub(super) ytdl_plugin_active: bool,
    pub(super) platform_manages_icc: bool,
}

pub(super) fn engine_options(
    prefs: &Preferences,
    watch_later_directory: &Path,
    flags: OptionsFlags,
    advanced_options: Result<EngineOptions, String>,
) -> Result<EngineOptions, String> {
    let mut network_online_videos = prefs.network_online_videos.clone();
    if flags.ytdl_plugin_active {
        network_online_videos.enabled = false;
    }
    let mut engine_options = resume_engine_options(
        watch_later_directory,
        prefs.resume_last_position,
        &prefs.network,
        &prefs.network_cache,
        &network_online_videos,
    )?;
    engine_options.push((
        "keep-open".into(),
        riina_core::playlist_end_policy(prefs.keep_open_on_file_end, prefs.playlist_auto_play_next)
            .into(),
    ));
    engine_options.extend(prefs.audio.engine_options()?);
    engine_options.push(("vd-lavc-threads".into(), prefs.video_threads.to_string()));
    engine_options.extend(prefs.hdr_tone_mapping.engine_options()?);
    if !prefs.hdr_enabled {
        engine_options.extend(riina_core::hdr_preferences::disabled_output_options());
    }
    if flags.platform_manages_icc {
        engine_options.retain(|(name, _)| name != "icc-profile-auto");
        engine_options.push(("icc-profile-auto".into(), "no".into()));
    }
    engine_options.extend(advanced_options?);
    // Playback power is app-wide, including sessions without a native video output.
    engine_options.push(("stop-screensaver".into(), "no".into()));
    Ok(engine_options)
}

pub(super) fn resume_engine_options(
    watch_later_directory: &std::path::Path,
    resume_last_position: bool,
    network: &riina_core::network_preferences::Connection,
    network_cache: &riina_core::network_preferences::Cache,
    network_online_videos: &riina_core::network_preferences::OnlineVideos,
) -> Result<Vec<(String, String)>, String> {
    let enabled = if resume_last_position { "yes" } else { "no" }.to_owned();
    let mut options = vec![
        (
            "watch-later-directory".into(),
            watch_later_directory.to_string_lossy().into_owned(),
        ),
        ("save-position-on-quit".into(), enabled.clone()),
        ("resume-playback".into(), enabled),
    ];
    options.push(("sub-auto".into(), "no".into()));
    options.extend(network.engine_options()?);
    options.extend(network_cache.engine_options()?);
    options.extend(network_online_videos.engine_options()?);
    Ok(options)
}

pub(super) fn apply_player_preferences(
    command_sender: &CommandSender,
    prefs: &Preferences,
    launch_pause: Option<bool>,
) -> std::io::Result<()> {
    if prefs
        .audio_equalizer
        .gains_db
        .iter()
        .any(|gain| *gain != 0.0)
    {
        command_sender
            .send(Command::AudioEqualizer(prefs.audio_equalizer.gains_db))
            .map_err(std::io::Error::other)?;
    }
    command_sender
        .send(Command::SetVolumeBoost(prefs.volume_boost))
        .map_err(std::io::Error::other)?;
    command_sender
        .send(Command::SetPauseWhenOpen(
            launch_pause.unwrap_or(prefs.pause_when_open),
        ))
        .map_err(std::io::Error::other)?;
    command_sender
        .send(Command::HardwareDecoding(prefs.hardware_decoder))
        .map_err(std::io::Error::other)?;
    command_sender
        .send(Command::SubtitleAssOverride {
            lane: riina_core::subtitle_controls::SubtitleLane::Primary,
            level: prefs.subtitle_ass_override,
        })
        .map_err(std::io::Error::other)?;
    command_sender
        .send(Command::SubtitleAssOverride {
            lane: riina_core::subtitle_controls::SubtitleLane::Secondary,
            level: prefs.secondary_subtitle_ass_override,
        })
        .map_err(std::io::Error::other)?;
    command_sender
        .send(Command::SubtitleFont(prefs.subtitle_font.clone()))
        .map_err(std::io::Error::other)?;
    command_sender
        .send(Command::SubtitleFontSize(prefs.subtitle_font_size))
        .map_err(std::io::Error::other)?;
    for command in crate::subtitle_preferences::commands(prefs.subtitle_text_style) {
        command_sender
            .send(command)
            .map_err(std::io::Error::other)?;
    }
    for command in prefs.subtitle_position.commands() {
        command_sender
            .send(command)
            .map_err(std::io::Error::other)?;
    }
    for command in prefs.subtitle_other.commands() {
        command_sender
            .send(command)
            .map_err(std::io::Error::other)?;
    }
    Ok(())
}

pub(super) fn install_toolbar_order(app: &PlayerWindow) {
    let toolbar_settings_window = app.as_weak();
    app.on_toolbar_settings_changed(move || {
        if let Some(app) = toolbar_settings_window.upgrade() {
            crate::osc_preferences::refresh_visible_order(&app);
        }
    });
    crate::osc_preferences::refresh_visible_order(app);
}

#[derive(Default)]
pub(crate) struct InitialMedia {
    pub(crate) files: Vec<std::ffi::OsString>,
    pub(crate) stream: Option<crate::windows::open_url::PendingStream>,
    pub(crate) resume_position: Option<f64>,
    pub(crate) explicit_playlist: bool,
    pub(crate) managed: Option<Arc<crate::plugins::plugin_players::Ticket>>,
    pub(crate) launch: crate::startup_arguments::LaunchOptions,
}

impl InitialMedia {
    pub(crate) fn requests_playback(&self) -> bool {
        !self.files.is_empty() || self.stream.is_some() || self.launch.stdin
    }

    pub(crate) fn can_use_open_engine(&self) -> bool {
        self.managed.is_none()
            && self.requests_playback()
            && self.launch == crate::startup_arguments::LaunchOptions::default()
    }

    fn external_stream(&self) -> Option<&str> {
        if self.explicit_playlist
            || self.launch.shuffle
            || self.launch.stdin
            || self.managed.is_some()
        {
            return None;
        }
        let [file] = self.files.as_slice() else {
            return None;
        };
        crate::media_open::validate_url(file.to_str()?).ok()
    }

    pub(super) fn startup_context<'a>(
        &'a self,
        application: &'a super::ApplicationServices,
    ) -> StartupContext<'a> {
        StartupContext {
            application,
            managed: self.managed.as_ref(),
            launch_options: &self.launch.engine,
        }
    }

    pub(crate) fn for_plugin(ticket: Arc<crate::plugins::plugin_players::Ticket>) -> Self {
        let files = ticket
            .options
            .source
            .iter()
            .map(std::ffi::OsString::from)
            .collect();
        Self {
            files,
            managed: Some(ticket),
            ..Self::default()
        }
    }
}

pub(super) struct InitialOpen {
    pub(super) media: InitialMedia,
    pub(super) plugins: Option<PluginHostHandle>,
    pub(super) observer: super::observer_state::ObserverId,
}

impl InitialOpen {
    pub(super) fn open(self, app: &PlayerWindow, sender: CommandSender) {
        if !self.media.requests_playback() {
            return;
        }
        let Self {
            media,
            plugins,
            observer,
        } = self;
        let presents_window = media.external_stream().is_some();
        let open = move |app: &PlayerWindow| open_initial_media(app, media, sender, observer);
        if plugins.is_none() && !presents_window {
            open(app);
            return;
        }

        let player = app.as_weak();
        let generation = app.get_media_request_generation();
        let when_ready = move || {
            // Showing a separate progress window must run outside the render callback.
            let _ = player.upgrade_in_event_loop(move |app| {
                if app.get_media_request_generation() == generation {
                    open(&app);
                }
            });
        };
        if let Some(plugins) = plugins {
            plugins.after_initial_plugins(when_ready);
        } else {
            when_ready();
        }
    }
}

pub(super) fn open_initial_media(
    app: &PlayerWindow,
    mut media: InitialMedia,
    sender: CommandSender,
    observer: super::observer_state::ObserverId,
) {
    let previous_request = app.get_media_request_generation();
    let presentation = media.launch.presentation;
    if let Some(stream) = media.stream.take() {
        stream.start(app, &sender);
    } else if media.launch.stdin {
        crate::media_open::startup_stdin(app, &sender);
    } else if let Some(address) = media.external_stream()
        && let Some(open_url) = observer.open_url()
    {
        if let Err(error) =
            open_url.open_initial_stream(app, &sender, address, media.resume_position)
        {
            crate::diagnostics::report(app, error);
        }
    } else {
        crate::media_open::startup_at(app, media, sender, observer);
    }
    let request = app.get_media_request_generation();
    if let Some(presentation) = presentation
        && request != previous_request
        && let Some(observer) = observer.lookup()
    {
        #[cfg(debug_assertions)]
        crate::diagnostics::log_args(
            crate::diagnostics::Level::Debug,
            "launch",
            format_args!("Queued {presentation:?} for initial playback request {request}"),
        );
        observer.borrow_mut().startup_presentation = Some((request, presentation));
    }
}

pub(super) fn activate_output(
    app: &PlayerWindow,
    native_presentation: bool,
    player: Option<&Player>,
    command_sender: &CommandSender,
    volume: f64,
    opening: InitialOpen,
    picture_in_picture: &crate::platform::PictureInPictureSession,
) -> Result<Option<Rc<Cell<bool>>>, Box<dyn std::error::Error>> {
    let idle_media_available = (!opening.media.requests_playback()
        && opening.media.managed.is_none())
    .then(|| Rc::new(Cell::new(true)));
    crate::fullscreen_info::connect(app);
    if native_presentation {
        app.set_engine_ready(true);
        command_sender
            .send(Command::RestoreVolume(volume))
            .map_err(std::io::Error::other)?;
        opening.open(app, command_sender.clone());
    } else if let Some(player) = player {
        super::rendering::install(
            app,
            player,
            opening,
            volume,
            picture_in_picture.render_extras(),
        )?;
    }
    Ok(idle_media_available)
}

pub(super) fn initial_volume(
    prefs: &Preferences,
    requested: Option<f64>,
    recent: Option<f64>,
) -> f64 {
    let profile_volume = if prefs.audio.initial_volume_enabled {
        f64::from(prefs.audio.initial_volume)
    } else {
        recent.unwrap_or(prefs.volume)
    };
    requested
        .unwrap_or(profile_volume)
        .clamp(0.0, prefs.volume_limit())
}

pub(super) struct SessionRecents {
    pub(super) activity: Rc<RecentActivity>,
    pub(super) resume_revision: Rc<Cell<u64>>,
}

pub(super) struct PreparedStartup {
    pub(super) observation: observation_dispatch::PreparedObservation,
    pub(super) engine_options: StartupOptions,
    pub(super) session: PreparedSession,
}

pub(super) struct PreparedSession {
    pub(super) filters: Rc<crate::video_filter_window::Profile>,
    pub(super) projection: initial_projection::SessionProjection,
    pub(super) history: Tracking,
    pub(super) recent: SessionRecents,
    pub(super) plugin_ui: plugins::InstalledPluginUi,
    pub(super) plugin_host_handle: Option<PluginHostHandle>,
    pub(super) plugin_host: Option<PluginHost>,
    pub(super) audio_equalizer: Arc<Mutex<audio_equalizer::Settings>>,
    pub(super) recovery: Option<Recovery>,
    pub(super) prefs: Preferences,
    pub(super) prefs_path: PathBuf,
    pub(super) picture_in_picture: platform::PictureInPictureSession,
}

pub(super) struct StartupContext<'a> {
    pub(super) application: &'a super::ApplicationServices,
    pub(super) managed: Option<&'a Arc<crate::plugins::plugin_players::Ticket>>,
    pub(super) launch_options: &'a [(String, String)],
}

pub(super) fn prepare(
    app: &PlayerWindow,
    native: Option<&platform::NativePresentation>,
    welcome: Option<&WelcomeWindow>,
    engine_start: &EngineStart,
    loaded: LoadedPreferences,
    media_geometry: MediaGeometry,
    startup: StartupContext<'_>,
) -> Result<PreparedStartup, Box<dyn std::error::Error>> {
    let picture_in_picture = platform::PictureInPictureSession::new(app);
    let prefs_path = engine_start.preferences_path()?;
    let use_system_services = engine_start.uses_system_services();
    let PreparedSettings {
        prefs,
        recovery,
        audio_equalizer,
        history,
    } = prepare_settings(app, &prefs_path, loaded, use_system_services);
    let filters = startup.application.filters.profile(&prefs_path, &prefs);
    if let Some(ticket) = startup.managed {
        app.global::<riina_ui::PlayerControlsState>()
            .set_plugin_ui_disabled(ticket.options.disable_ui);
    }
    let activity =
        startup
            .application
            .recents
            .for_profile(&prefs_path, &prefs, use_system_services);
    let resume_revision = activity.revision();
    let recent_documents = lock(activity.documents()).clone();
    let plugin_host = match startup
        .application
        .plugins
        .host(&prefs_path)
        .and_then(|global| {
            PluginHost::prepare_for_application(
                &prefs_path,
                &recent_documents,
                history.entries(),
                global,
                startup.managed.cloned(),
            )
        }) {
        Ok(host) => Some(host),
        Err(error) => {
            app.set_status(error.into());
            None
        }
    };
    let plugin_host_handle = plugin_host.as_ref().map(PluginHost::handle);
    let plugin_ui = plugins::install(
        app,
        native,
        plugin_host_handle.clone(),
        &startup.application.plugins,
    );
    live_text::initialize(app, prefs.live_text_enabled, use_system_services);
    hdr_output::initialize_display(app, &prefs, native);
    let recent = SessionRecents {
        resume_revision: Rc::new(Cell::new(resume_revision)),
        activity,
    };
    let history = history.start_tracking(&prefs_path, &prefs, recent.activity.documents().clone());
    history.install_recording_action(app);
    recent
        .activity
        .bind_history(&startup.application.history, &history.state);
    let engine_options =
        options_for_session(app, &prefs, &prefs_path, startup.launch_options.to_vec())?;
    let projection = initial_projection::install_session(app, &prefs, &prefs_path, native);
    if let Some(welcome) = welcome {
        welcome.set_app_version(env!("CARGO_PKG_VERSION").into());
    }
    recent.activity.attach(app, welcome);
    let observation = observation_dispatch::prepare_observation(
        app,
        observation_dispatch::Inputs {
            prefs: &prefs,
            media_geometry,
            native_presentation: native.is_some(),
            reported_playback: projection.latest_playback.clone(),
            history: history.observe(plugin_host_handle.clone()),
            saved_video_filters: filters.settings(crate::video_filter_window::Kind::Video),
            saved_audio_filters: filters.settings(crate::video_filter_window::Kind::Audio),
            picture_in_picture_state_dirty: picture_in_picture.notification_flag(),
            managed_visibility: startup
                .managed
                .filter(|ticket| ticket.is_waiting_for_media())
                .map(|ticket| observation_dispatch::ManagedVisibility {
                    ticket: ticket.clone(),
                    native: native.map(platform::NativePresentation::host),
                }),
        },
    );
    Ok(PreparedStartup {
        observation,
        engine_options,
        session: PreparedSession {
            filters,
            projection,
            history,
            recent,
            plugin_ui,
            plugin_host_handle,
            plugin_host,
            audio_equalizer,
            recovery,
            prefs,
            prefs_path,
            picture_in_picture,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_reuse_requires_playback_and_default_startup_options() {
        let mut media = InitialMedia::default();
        assert!(!media.can_use_open_engine());

        media.files = vec!["clip.mkv".into()];
        media.resume_position = Some(3.5);
        media.explicit_playlist = true;
        assert!(media.can_use_open_engine());

        media.launch.engine.push(("speed".into(), "1.5".into()));
        assert!(!media.can_use_open_engine());
        media.launch.engine.clear();
        media.launch.presentation = Some(crate::startup_arguments::Presentation::Fullscreen);
        assert!(!media.can_use_open_engine());
    }

    #[test]
    fn initial_volume_uses_the_profile_or_explicit_override_with_the_active_cap() {
        let mut prefs = Preferences {
            volume: 46.0,
            ..Preferences::default()
        };
        assert_eq!(initial_volume(&prefs, None, None), 46.0);
        assert_eq!(initial_volume(&prefs, None, Some(52.0)), 52.0);
        prefs.audio.initial_volume_enabled = true;
        prefs.audio.initial_volume = 65;
        assert_eq!(initial_volume(&prefs, None, Some(52.0)), 65.0);
        assert_eq!(initial_volume(&prefs, Some(24.0), Some(52.0)), 24.0);
        prefs.audio.initial_volume = 235;
        assert_eq!(initial_volume(&prefs, None, None), 100.0);
        prefs.volume_boost = true;
        assert_eq!(initial_volume(&prefs, None, None), 200.0);
        assert_eq!(initial_volume(&prefs, Some(160.0), None), 160.0);
    }
}
