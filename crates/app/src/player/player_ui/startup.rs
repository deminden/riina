//! Build initial engine options and issue the saved player commands in startup order.
use super::{
    audio_controls::set_audio_equalizer_controls,
    engine_start::{EngineOptions, EngineStart},
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
use riina_core::{Command, Preferences, audio_equalizer, sync::lock, video_filters};
use riina_mpv::{CommandSender, Player};
use slint::ComponentHandle;
use std::{
    cell::Cell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, Mutex},
};

pub(super) struct PreparedSettings {
    pub(super) prefs: Preferences,
    pub(super) recovery: Option<Recovery>,
    pub(super) audio_equalizer: Arc<Mutex<audio_equalizer::Settings>>,
    pub(super) saved_video_filters: Arc<Mutex<video_filters::Settings>>,
    pub(super) saved_audio_filters: Arc<Mutex<video_filters::Settings>>,
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
        source_text,
        recovery,
    } = loaded;
    if use_system_services
        && let Err(error) = crate::online_subtitles_credentials::restore_preferences(
            &mut prefs,
            source_text.as_deref(),
            prefs_path,
            recovery.is_none(),
        )
    {
        crate::diagnostics::report(app, format!("OpenSubtitles credential storage: {error}"));
    }
    let audio_equalizer = Arc::new(Mutex::new(prefs.audio_equalizer.clone()));
    let saved_video_filters = Arc::new(Mutex::new(prefs.video_filters.clone()));
    let saved_audio_filters = Arc::new(Mutex::new(prefs.audio_filters.clone()));
    set_audio_equalizer_controls(app, &lock(&audio_equalizer));
    let history = LoadedHistory::load(app, prefs_path, &mut prefs);
    PreparedSettings {
        prefs,
        recovery,
        audio_equalizer,
        saved_video_filters,
        saved_audio_filters,
        history,
    }
}

pub(super) fn options_for_session(
    app: &PlayerWindow,
    prefs: &Preferences,
    prefs_path: &Path,
    launch_options: EngineOptions,
) -> std::io::Result<EngineOptions> {
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
    engine_options(
        prefs,
        &watch_later_directory,
        OptionsFlags {
            ytdl_plugin_active,
            platform_manages_icc: cfg!(target_os = "macos"),
        },
        advanced_options,
    )
    .map_err(std::io::Error::other)
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
        if self.media.files.is_empty() && self.media.stream.is_none() {
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

fn open_initial_media(
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
    prefs: &Preferences,
    opening: InitialOpen,
    picture_in_picture: &crate::platform::PictureInPictureSession,
) -> Result<(), Box<dyn std::error::Error>> {
    crate::fullscreen_info::connect(app);
    let volume = opening.media.launch.volume.unwrap_or(prefs.volume);
    if native_presentation {
        app.set_engine_ready(true);
        command_sender
            .send(Command::Volume(volume))
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
    Ok(())
}

pub(super) struct SessionRecents {
    pub(super) activity: Rc<RecentActivity>,
    pub(super) resume_revision: Rc<Cell<u64>>,
}

pub(super) struct PreparedStartup {
    pub(super) observation: observation_dispatch::PreparedObservation,
    pub(super) engine_options: EngineOptions,
    pub(super) session: PreparedSession,
}

pub(super) struct PreparedSession {
    pub(super) projection: initial_projection::SessionProjection,
    pub(super) history: Tracking,
    pub(super) recent: SessionRecents,
    pub(super) plugin_ui: plugins::InstalledPluginUi,
    pub(super) plugin_host_handle: Option<PluginHostHandle>,
    pub(super) plugin_host: Option<PluginHost>,
    pub(super) saved_audio_filters: Arc<Mutex<video_filters::Settings>>,
    pub(super) saved_video_filters: Arc<Mutex<video_filters::Settings>>,
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
        saved_video_filters,
        saved_audio_filters,
        history,
    } = prepare_settings(app, &prefs_path, loaded, use_system_services);
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
            saved_video_filters: saved_video_filters.clone(),
            saved_audio_filters: saved_audio_filters.clone(),
            audio_device_sender: plugin_ui.plugin_command_sender.clone(),
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
            projection,
            history,
            recent,
            plugin_ui,
            plugin_host_handle,
            plugin_host,
            saved_audio_filters,
            saved_video_filters,
            audio_equalizer,
            recovery,
            prefs,
            prefs_path,
            picture_in_picture,
        },
    })
}
