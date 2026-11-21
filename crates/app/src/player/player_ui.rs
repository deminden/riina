mod audio_controls;
mod catalog;
mod context;
mod dialogs;
mod discovery;
mod engine_start;
mod feedback;
#[cfg(test)]
pub(crate) mod glue_tests;
pub(crate) mod hdr_output;
mod history_session;
mod lifecycle;
#[cfg(any(target_os = "windows", target_os = "macos", test))]
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
mod input;
mod subtitle_controls;
mod video_controls;
use audio_controls::set_audio_equalizer_controls;
mod materials;
mod observation_dispatch;
mod observations;
mod observer_state;
mod plugin_menus;
mod plugins;
use catalog::install_playlist_metadata_refresh;
use context::{PlayerContext, bind};
use engine_start::EngineStart;
#[cfg(target_os = "windows")]
use materials::{install_native_video_material_sampler, install_windows_material_policy_monitor};
mod session_preferences;

use plugin_menus::reorder_plugin_menu_rows;

use crate::{
    MenuCatalogRow, PlayerWindow, RecentMediaRow, WelcomeWindow, media_open, platform,
    preference_load::LoadedPreferences, preferences::model::PreferenceModel,
};
use riina_core::{Command, Phase, Playback, Preferences, TrackKind, sync::lock, time_label};
use riina_mpv::Player;
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
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

fn queue_playlist_navigation(
    sender: &riina_mpv::CommandSender,
    pending: &PendingNavigation,
    command: Command,
    capture_transition_first: bool,
) {
    if !capture_transition_first {
        if let Err(error) = sender.send(command) {
            crate::diagnostics::warn!("Could not navigate the playlist: {error}");
        }
        return;
    }

    let needs_capture = {
        let mut pending = pending.borrow_mut();
        if let Some((commands, _)) = pending.as_mut() {
            commands.push(command);
            false
        } else {
            *pending = Some((vec![command], Instant::now()));
            true
        }
    };
    if needs_capture && let Err(error) = sender.send(Command::CaptureTransitionFrame) {
        let commands = pending.borrow_mut().take().map(|(commands, _)| commands);
        crate::diagnostics::warn!("Could not capture the outgoing video frame: {error}");
        if let Some(commands) = commands {
            for command in commands {
                if let Err(error) = sender.send(command) {
                    crate::diagnostics::warn!("Could not navigate the playlist: {error}");
                }
            }
        }
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

fn set_native_video_host_visible(
    host: Option<&slint::Weak<crate::NativeVideoHost>>,
    visible: bool,
) {
    let Some(host) = host.and_then(slint::Weak::upgrade) else {
        return;
    };
    // The native presentation retains this Winit handle for mpv's HWND
    // lifetime. Toggle the OS surface directly so Slint does not keep it shown.
    host.window()
        .with_winit_window(|window| window.set_visible(visible));
}

fn refresh_video_menu(app: &PlayerWindow) {
    // Native check items toggle before dispatch. A no-op or rejected command
    // cannot dirty the observed selection, so renew the model identity to make
    // Slint rebuild the menu from confirmed state rather than that local toggle.
    app.set_video_crop_menu_labels(slint::ModelRc::new(slint::VecModel::from(
        riina_core::crop::MENU_LABELS
            .map(slint::SharedString::from)
            .to_vec(),
    )));
    app.set_video_aspect_menu_labels(slint::ModelRc::new(slint::VecModel::from(
        riina_core::video_controls::ASPECT_MENU_PRESETS
            .map(slint::SharedString::from)
            .to_vec(),
    )));
}

fn recent_media_rows(media: &[riina_core::RecentMedia]) -> Vec<RecentMediaRow> {
    media
        .iter()
        .map(|item| {
            let title = if item.title.trim().is_empty() {
                PathBuf::from(&item.path).file_name().map_or_else(
                    || item.path.clone(),
                    |name| name.to_string_lossy().into_owned(),
                )
            } else {
                item.title.clone()
            };
            RecentMediaRow {
                title: title.into(),
                position_label: time_label(Some(item.position)).into(),
            }
        })
        .collect()
}

fn recent_open_menu_rows(media: &[riina_core::RecentMedia]) -> Vec<MenuCatalogRow> {
    media
        .iter()
        .map(|item| {
            let label = if item.title.trim().is_empty() {
                PathBuf::from(&item.path).file_name().map_or_else(
                    || item.path.clone(),
                    |name| name.to_string_lossy().into_owned(),
                )
            } else {
                item.title.clone()
            };
            MenuCatalogRow {
                token: item.path.clone().into(),
                label: label.into(),
                selected: false,
            }
        })
        .collect()
}

fn remember_recent_open_document(
    app: &PlayerWindow,
    documents: &Rc<RefCell<Vec<riina_core::RecentMedia>>>,
    preferences_path: &std::path::Path,
    path: String,
    title: String,
) {
    if !app.get_record_recent_files() || path.is_empty() {
        return;
    }
    let native_path = path.clone();
    let mut documents = documents.borrow_mut();
    documents.retain(|document| document.path != path);
    documents.insert(
        0,
        riina_core::RecentMedia {
            path,
            title,
            position: 0.0,
        },
    );
    documents.truncate(10);
    let recent = documents.clone();
    drop(documents);
    app.set_recent_open_menu_rows(slint::ModelRc::new(slint::VecModel::from(
        recent_open_menu_rows(&recent),
    )));
    app.invoke_video_menu_changed();
    if let Err(error) = persist_recent_open_documents(preferences_path, &recent) {
        app.set_status(format!("Could not save Open Recent: {error}").into());
    }
    if let Err(error) = platform::remember_recent_document(&native_path) {
        app.set_status(format!("Could not update the system recent documents: {error}").into());
    }
}

fn persist_recent_open_documents(
    preferences_path: &std::path::Path,
    documents: &[riina_core::RecentMedia],
) -> std::io::Result<()> {
    crate::preference_store::update_automatically(
        preferences_path,
        &Preferences::default(),
        |preferences| {
            preferences.recent_open_documents = documents.to_vec();
            Ok(())
        },
    )
    .map_err(std::io::Error::other)
}

fn resume_media_rows(
    media: &[riina_core::RecentMedia],
    resume_last_position: bool,
) -> Vec<RecentMediaRow> {
    if resume_last_position {
        recent_media_rows(media.get(..1).unwrap_or_default())
    } else {
        Vec::new()
    }
}

fn recent_document_rows(
    media: &[riina_core::RecentMedia],
    resume_last_position: bool,
) -> Vec<RecentMediaRow> {
    // IINA removes the last-played item from the document table only while its
    // separate Resume action is enabled.
    let offset = if resume_last_position { 1 } else { 0 };
    recent_media_rows(media.get(offset..).unwrap_or_default())
}

fn refresh_welcome_history(
    welcome: &WelcomeWindow,
    media: &[riina_core::RecentMedia],
    resume_last_position: bool,
) {
    welcome.set_recent_media(slint::ModelRc::new(slint::VecModel::from(
        resume_media_rows(media, resume_last_position),
    )));
    welcome.set_recent_documents(slint::ModelRc::new(slint::VecModel::from(
        recent_document_rows(media, resume_last_position),
    )));
    welcome.set_recent_document_offset(if resume_last_position { 1 } else { 0 });
    welcome.set_selection_index(if resume_last_position || media.is_empty() {
        -1
    } else {
        0
    });
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
    _thumbnail_preview_timer: Rc<slint::Timer>,
    _transition_frame_timer: Option<Rc<slint::Timer>>,
    _native_material_timer: Option<Rc<slint::Timer>>,
    _windows_materials_policy_timer: Option<Rc<slint::Timer>>,
    _subtitle_priority_timer: Rc<slint::Timer>,
    _online_subtitles_window: Rc<RefCell<Option<crate::OnlineSubtitlesWindow>>>,
    _online_subtitles_timer: Rc<slint::Timer>,
    _playback_history_window: crate::playback_history_window::Guard,
    _video_filters_window: Rc<RefCell<Option<crate::VideoFiltersWindow>>>,
    _video_filters_timer: Rc<slint::Timer>,
    _audio_filters_window: Rc<RefCell<Option<crate::VideoFiltersWindow>>>,
    _audio_filters_timer: Rc<slint::Timer>,
    _playlist_metadata_timer: Rc<slint::Timer>,
    _automatic_music_mode_timer: Rc<slint::Timer>,
    _file_drop_timer: Rc<slint::Timer>,
    _player_window_focus: input::PlayerWindowFocusRegistration,
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

enum PendingWelcomeOpen {
    Url(String),
    Recent(riina_core::RecentMedia, bool),
}

fn dispatch_welcome_open(
    app: &PlayerWindow,
    request: PendingWelcomeOpen,
    sender: &riina_mpv::CommandSender,
) {
    match request {
        PendingWelcomeOpen::Url(value) => media_open::url(app, &value, sender.clone()),
        PendingWelcomeOpen::Recent(media, resume) => {
            media_open::open_recent(app, media, resume, sender.clone());
        }
    }
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
    let use_system_services = engine_start.uses_system_services();
    if use_system_services
        && let Err(error) = crate::online_subtitles_credentials::restore_preferences(
            &mut prefs,
            source_text.as_deref(),
            &prefs_path,
            recovery.is_none(),
        )
    {
        app.set_status(format!("OpenSubtitles credential storage: {error}").into());
    }
    let audio_equalizer = Arc::new(std::sync::Mutex::new(prefs.audio_equalizer.clone()));
    let saved_video_filters = Arc::new(std::sync::Mutex::new(prefs.video_filters.clone()));
    let saved_audio_filters = Arc::new(std::sync::Mutex::new(prefs.audio_filters.clone()));
    set_audio_equalizer_controls(app, &lock(&audio_equalizer));
    let history = history_session::LoadedHistory::load(app, &prefs_path, &mut prefs);
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
    app.set_live_text_enabled(prefs.live_text_enabled);
    app.set_live_text_regions(slint::ModelRc::new(slint::VecModel::from(Vec::new())));
    app.set_live_text_available(false);
    #[cfg(target_os = "macos")]
    app.set_live_text_selectable(true);
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    if use_system_services {
        let weak = app.as_weak();
        let _ = platform::text_recognition::check_availability(move |available| {
            let _ = crate::background::to_ui(weak, move |app| {
                app.set_live_text_available(available);
                if !available {
                    app.set_live_text_enabled(false);
                }
            });
        });
    }
    app.set_hdr_enabled(prefs.hdr_enabled);
    app.set_default_hdr_mode_enabled(prefs.hdr_enabled);
    app.set_load_icc_profile(prefs.load_icc_profile);
    app.set_hdr_display_active(
        native
            .as_ref()
            .is_some_and(|presentation| presentation.hdr_display_active()),
    );
    let hdr_display_timer = native.as_ref().map(|presentation| {
        let timer = Rc::new(slint::Timer::default());
        let display = presentation.clone();
        let weak = app.as_weak();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_secs(2),
            move || {
                if let Some(app) = weak.upgrade() {
                    hdr_output::update_display(&app, display.hdr_display_active());
                }
            },
        );
        timer
    });
    let recent_media = Rc::new(RefCell::new(prefs.recent_media.clone()));
    let recent_open_documents = Rc::new(RefCell::new(prefs.recent_open_documents.clone()));
    let recent_open_cleared = Rc::new(Cell::new(false));
    let history = history.start_tracking(&prefs_path, &prefs);
    let watch_later_directory = prefs_path.with_file_name("watch-later");
    std::fs::create_dir_all(&watch_later_directory)?;
    let ytdl_plugin_active = crate::plugin_host::is_enabled_plugin(&prefs_path, "io.iina.ytdl");
    app.set_network_ytdl_plugin_active(ytdl_plugin_active);
    let advanced_options =
        crate::advanced_preferences::engine_options(&prefs.advanced, &prefs_path);
    let engine_options = startup::engine_options(
        &prefs,
        &watch_later_directory,
        startup::OptionsFlags {
            ytdl_plugin_active,
            platform_manages_icc: cfg!(target_os = "macos"),
        },
        advanced_options,
    )
    .map_err(std::io::Error::other)?;
    initial_projection::install_platform(app, &prefs);
    let latest_playback = crate::PlaybackStore::default();
    crate::about_window::connect(app, latest_playback.clone());
    let pause_resume_timer = lifecycle::install_pause_on_minimize(
        app,
        native.as_ref().map(|presentation| presentation.host()),
        latest_playback.clone(),
    );
    crate::preferences::model::install_player_sections(&prefs, app);
    crate::key_binding_preferences::install_player(
        app,
        &prefs.key_bindings,
        latest_playback.clone(),
    );
    #[cfg(target_os = "macos")]
    let lifecycle::SystemMedia {
        sender: system_media_sender,
        controls: system_media_controls,
        timer: system_media_controls_timer,
    } = lifecycle::install_system_media(app, latest_playback.clone());
    initial_projection::install_media(app, &prefs, welcome, &recent_open_documents);
    let appearance_timer = initial_projection::install_layout(app, &prefs, &prefs_path);
    let app_weak = app.as_weak();
    // Only one pending state snapshot is retained; a decoder can produce updates
    // faster than the UI consumes them without growing the application event queue.
    // State and capture completions share one handoff. A newer completion must
    // never be cleaned up while an older snapshot is still being rendered.
    let latest = Arc::new(playback_mailbox::Mailbox::default());
    let transition_overlay = Arc::new(AtomicU64::new(u64::MAX));
    let transition_release_generation = Arc::new(AtomicU64::new(u64::MAX));
    let notify_transition_overlay = transition_overlay.clone();
    #[cfg(target_os = "windows")]
    let notify_transition_release_generation = transition_release_generation.clone();
    let observed_playback = latest_playback.clone();
    let music_mode_state = Arc::new(std::sync::Mutex::new(riina_core::music_mode::State::new(
        prefs.auto_switch_to_music_mode,
    )));
    if native.is_none() {
        music_mode::install_window_resize(app);
    }
    let queued = Arc::new(AtomicBool::new(false));
    let crop_editor = crate::crop_editor::Editor::default();
    let observed_crop_editor = crop_editor.clone();
    let mut video_osd =
        crate::video_osd::VideoOsd::new(prefs.disable_osd_speed, prefs.disable_osd_pause_resume);
    video_osd.set_volume_limit(prefs.volume_limit());
    let observer_state = observer_state::UiObserverState::new(video_osd, media_geometry);
    let feedback_video_osd = observer_state.video.clone();
    let observer_state = observer_state::ObserverRegistration::new(observer_state);
    let observer_id = observer_state.id();
    let screenshot_scratch = Arc::new(crate::screenshot_output::Scratch::default());
    let opening_fullscreen = Arc::new(std::sync::Mutex::new(
        crate::opening_fullscreen::OpeningFullscreen::default(),
    ));
    let notification = observation_dispatch::PlaybackNotification {
        reported_playback: observed_playback,
        queued,
        latest,
        history: history.observe(plugin_host_handle.clone()),
        delivery: observation_dispatch::WindowDelivery {
            app: app_weak,
            observer_id,
            crop_editor: observed_crop_editor,
            saved_video_filters: saved_video_filters.clone(),
            saved_audio_filters: saved_audio_filters.clone(),
            audio_device_sender: plugin_command_sender.clone(),
            opening_fullscreen,
            music_mode_state: music_mode_state.clone(),
            transition_overlay: notify_transition_overlay,
            #[cfg(target_os = "windows")]
            transition_release_generation: notify_transition_release_generation,
        },
        picture_in_picture_state_dirty: picture_in_picture.notification_flag(),
        _screenshot_scratch: screenshot_scratch.clone(),
    };
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
    let playback_history_window = history.install_window(
        app,
        &command_sender,
        &prefs_path,
        plugin_host_handle.clone(),
        latest_playback.clone(),
    );
    let ((video_filters_window, video_filters_timer), (audio_filters_window, audio_filters_timer)) =
        crate::video_filter_window::install(
            app,
            latest_playback.clone(),
            saved_video_filters.clone(),
            saved_audio_filters.clone(),
            command_sender.clone(),
            prefs_path.clone(),
        );
    let playlist_metadata_timer = install_playlist_metadata_refresh(app, latest_playback.clone());
    let automatic_music_mode_timer = crate::automatic_music_mode::install(
        app,
        latest_playback.clone(),
        music_mode_state.clone(),
    );
    *lock(&plugin_command_sender) = Some(command_sender.clone());
    let subtitle_discovery_ready = Arc::new(crate::subtitle_discovery::Readiness::default());
    let (online_subtitles_window, online_subtitles_timer) = crate::open_subtitles::connect_player(
        app,
        command_sender.clone(),
        latest_playback.clone(),
        subtitle_discovery_ready.clone(),
    );
    let subtitle_priority_timer = discovery::install(
        app,
        &command_sender,
        latest_playback.clone(),
        subtitle_discovery_ready,
    );
    picture_in_picture.bind(app, &command_sender, &latest_playback);
    let input::InstalledInput {
        file_drop_timer,
        player_window_focus,
        #[cfg(target_os = "macos")]
        mac_scroll_event_monitor,
    } = input::install(app, &command_sender, latest_playback.clone());
    let thumbnail_preview_timer =
        thumbnail_actions::install(app, &latest_playback, plugin_host_handle.clone());
    let pending_navigation: PendingNavigation = Rc::new(RefCell::new(None));
    let transition_frame_timer = transitions::install(
        capture_transition_first,
        native.as_ref(),
        &command_sender,
        &latest_playback,
        &pending_navigation,
        &transition_release_generation,
    );
    #[cfg(target_os = "windows")]
    let native_material_timer = native.is_some().then(|| {
        install_native_video_material_sampler(
            app,
            command_sender.clone(),
            latest_playback.clone(),
            native
                .as_ref()
                .and_then(platform::NativePresentation::material_capture),
        )
    });
    #[cfg(not(target_os = "windows"))]
    let native_material_timer = None;
    #[cfg(target_os = "windows")]
    let windows_materials_policy_timer = Some(install_windows_material_policy_monitor(app));
    #[cfg(not(target_os = "windows"))]
    let windows_materials_policy_timer = None;
    #[cfg(target_os = "macos")]
    {
        *system_media_sender.borrow_mut() = Some(command_sender.clone());
    }
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    let live_text_timer = Some(live_text::install(
        app,
        command_sender.clone(),
        latest_playback.clone(),
    ));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let live_text_timer = None;
    let weak = app.as_weak();
    let toolbar_settings_window = weak.clone();
    app.on_toolbar_settings_changed(move || {
        if let Some(app) = toolbar_settings_window.upgrade() {
            crate::osc_preferences::refresh_visible_order(&app);
        }
    });
    crate::osc_preferences::refresh_visible_order(app);
    let clipboard_parent = weak.clone();
    app.on_copy_live_text(move |text| {
        if let Err(error) = crate::platform::clipboard::copy_text(text.as_str())
            && let Some(app) = clipboard_parent.upgrade()
        {
            app.set_status(format!("Could not copy recognized text: {error}").into());
        }
    });
    let live_text_parent = weak.clone();
    let live_text_path = prefs_path.clone();
    app.on_toggle_live_text(move || {
        let Some(app) = live_text_parent.upgrade() else {
            return;
        };
        if !app.get_live_text_available() {
            app.set_status("Live Text is unavailable on this system".into());
            return;
        }
        let enabled = !app.get_live_text_enabled();
        let result = crate::preference_store::update(
            &live_text_path,
            &Preferences::default(),
            |preferences| {
                preferences.live_text_enabled = enabled;
                Ok(())
            },
        );
        match result {
            Ok(()) => app.set_live_text_enabled(enabled),
            Err(error) => {
                app.set_status(format!("Could not save Live Text preference: {error}").into())
            }
        }
    });
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
    let opening_player = command_sender.clone();
    let opening_app = app.as_weak();
    let resume_player = command_sender.clone();
    let resume_app = app.as_weak();
    let resume_welcome = welcome.map(WelcomeWindow::as_weak);
    let recent_media_for_resume = recent_media.clone();
    app.on_resume_last_position_changed(move |enabled| {
        if let Err(error) = resume_player.send(Command::SetResumeLastPosition(enabled))
            && let Some(app) = resume_app.upgrade()
        {
            app.set_status(error.into());
        }
        if let Some(welcome) = resume_welcome.as_ref().and_then(slint::Weak::upgrade) {
            refresh_welcome_history(&welcome, &recent_media_for_resume.borrow(), enabled);
        }
    });
    history.install_recording_action(app);
    app.on_pause_when_open_changed(move |pause| {
        if let Err(error) = opening_player.send(Command::SetPauseWhenOpen(pause))
            && let Some(app) = opening_app.upgrade()
        {
            app.set_status(error.into());
        }
    });
    crate::window_controls::install(app, native.as_ref().map(platform::NativePresentation::host));
    crop_editor.install(app, command_sender.clone());
    let reflection_player = command_sender.clone();
    let weak = app.as_weak();
    app.on_toggle_video_reflection(move |horizontal| {
        if let Some(app) = weak.upgrade() {
            refresh_video_menu(&app);
            if !app.get_loaded()
                || !app.get_reflection_known()
                || app.get_crop_editing()
                || app.get_dialog_open()
            {
                return;
            }
            let axis = if horizontal {
                riina_core::video_reflection::Axis::Horizontal
            } else {
                riina_core::video_reflection::Axis::Vertical
            };
            if let Err(error) = reflection_player.send(Command::ToggleVideoReflection(axis)) {
                app.set_status(error.into());
            }
        }
    });
    feedback::install(app, &command_sender, screenshot_scratch, feedback_video_osd);
    let deinterlace_player = command_sender.clone();
    let weak = app.as_weak();
    app.on_toggle_deinterlace(move || {
        let Some(app) = weak.upgrade() else { return };
        refresh_video_menu(&app);
        if !app.get_loaded()
            || !app.get_deinterlace_known()
            || app.get_crop_editing()
            || app.get_dialog_open()
        {
            return;
        }
        app.invoke_activity();
        if let Err(error) = deinterlace_player.send(Command::ToggleDeinterlace) {
            app.set_status(error.into());
        }
    });
    transport::install_pause(app, &context);
    input::install_mpv_input_action(app, &context);
    bind!(context, app, on_toggle_mute, Command::ToggleMute);
    let hdr_player = command_sender.clone();
    hdr_output::install(app, move |command| hdr_player.send(command));
    transport::install_stop_and_steps(app, &context);
    let playlist_navigation_sender = command_sender.clone();
    let sender = playlist_navigation_sender.clone();
    let pending = pending_navigation.clone();
    let weak = app.as_weak();
    app.on_playlist_next(move || {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
        }
        queue_playlist_navigation(
            &sender,
            &pending,
            Command::PlaylistNext,
            capture_transition_first,
        );
    });
    let sender = playlist_navigation_sender.clone();
    let pending = pending_navigation.clone();
    let weak = app.as_weak();
    app.on_playlist_previous(move || {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
        }
        queue_playlist_navigation(
            &sender,
            &pending,
            Command::PlaylistPrevious,
            capture_transition_first,
        );
    });
    transport::install_speed(app, &context);
    video_controls::install(app, &command_sender);
    bind!(
        context,
        app,
        on_reset_subtitle_delay,
        Command::SubtitleDelay(0.0)
    );
    audio_controls::install(app, &command_sender, audio_equalizer.clone());
    subtitle_controls::install(app, &command_sender);
    bind!(context, app, on_reset_audio_delay, Command::AudioDelay(0.0));
    transport::install_loops(app, &context);
    bind!(
        context,
        app,
        on_toggle_secondary_subtitle_visibility,
        Command::ToggleSecondarySubtitleVisibility
    );
    bind!(
        context,
        app,
        on_cycle_audio_track,
        Command::CycleTrack(TrackKind::Audio)
    );
    bind!(
        context,
        app,
        on_cycle_subtitle_track,
        Command::CycleTrack(TrackKind::Sub)
    );
    bind!(
        context,
        app,
        on_cycle_video_track,
        Command::CycleTrack(TrackKind::Video)
    );
    let audio_device_player = command_sender.clone();
    let audio_device_state = latest_playback.clone();
    let audio_device_app = app.as_weak();
    app.on_set_audio_device(move |device| {
        let Some(app) = audio_device_app.upgrade() else {
            return;
        };
        let device = device.to_string();
        let available = lock(&audio_device_state)
            .audio_devices
            .iter()
            .any(|available| available.name == device);
        if !available {
            app.set_status("That audio device is no longer available".into());
            return;
        }
        app.invoke_activity();
        if let Err(error) = audio_device_player.send(Command::AudioDevice(device)) {
            app.set_status(error.into());
        }
    });
    bind!(
        context,
        app,
        on_toggle_subtitle_visibility,
        Command::ToggleSubtitleVisibility
    );
    let subtitle_encoding_player = command_sender.clone();
    let weak = app.as_weak();
    app.on_set_subtitle_encoding(move |encoding| {
        let Some(app) = weak.upgrade() else { return };
        let encoding = encoding.to_string();
        if !riina_core::subtitle_controls::SUBTITLE_ENCODING_CHOICES
            .iter()
            .any(|(_, code)| *code == encoding.as_str())
        {
            app.set_status("Unsupported subtitle encoding".into());
            return;
        }
        app.invoke_activity();
        match subtitle_encoding_player.send(Command::SubtitleEncoding(encoding.clone())) {
            Ok(()) => app.set_subtitle_default_encoding(encoding.into()),
            Err(error) => app.set_status(error.into()),
        }
    });
    catalog::install_catalog_selection(app, &context);
    let delay_player = command_sender.clone();
    let preview_weak = app.as_weak();
    app.on_preview_audio_delay(move |value| {
        if let Some(app) = preview_weak.upgrade() {
            app.set_audio_delay_preview_text(
                crate::localized_number::format_delay(f64::from(value)).into(),
            );
        }
    });
    let weak = app.as_weak();
    app.on_set_delay(move |category, value| {
        let command = match category {
            1 => Some(Command::AudioDelay(value as f64)),
            2 => Some(Command::SubtitleDelay(value as f64)),
            5 => Some(Command::SecondarySubtitleDelay(value as f64)),
            _ => None,
        };
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            if let Some(command) = command
                && let Err(error) = delay_player.send(command)
            {
                app.set_status(error.into());
            }
        }
    });
    playlist_actions::install(
        app,
        &command_sender,
        &pending_navigation,
        capture_transition_first,
    );
    dialogs::install_browse(
        app,
        &command_sender,
        native.as_ref().map(platform::NativePresentation::host),
    );
    transport::install_seek(app, &command_sender);
    dialogs::install_time_and_url(app, &command_sender);
    audio_controls::install_volume(app, &command_sender);
    transport::install_relative_speed(app, &command_sender);
    audio_controls::install_volume_adjustment(app, &command_sender);
    window_actions::install_fullscreen(
        app,
        native.as_ref().map(platform::NativePresentation::host),
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
            #[cfg(target_os = "macos")]
            _system_media_controls: system_media_controls,
            #[cfg(target_os = "macos")]
            _system_media_controls_timer: system_media_controls_timer,
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
        _thumbnail_preview_timer: thumbnail_preview_timer,
        _transition_frame_timer: transition_frame_timer,
        _native_material_timer: native_material_timer,
        _windows_materials_policy_timer: windows_materials_policy_timer,
        _subtitle_priority_timer: subtitle_priority_timer,
        _online_subtitles_window: online_subtitles_window,
        _online_subtitles_timer: online_subtitles_timer,
        _playback_history_window: playback_history_window,
        _video_filters_window: video_filters_window,
        _video_filters_timer: video_filters_timer,
        _audio_filters_window: audio_filters_window,
        _audio_filters_timer: audio_filters_timer,
        _playlist_metadata_timer: playlist_metadata_timer,
        _automatic_music_mode_timer: automatic_music_mode_timer,
        _file_drop_timer: file_drop_timer,
        _player_window_focus: player_window_focus,
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
        recent_document_rows, recent_media_rows, resume_media_rows, startup::resume_engine_options,
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
