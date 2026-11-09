mod audio_controls;
mod catalog;
mod discovery;
mod feedback;
pub(crate) mod hdr_output;
mod lifecycle;
#[cfg(any(target_os = "windows", target_os = "macos", test))]
mod live_text;
mod media_actions;
mod playback_mailbox;
mod rendering;
mod window_actions;
use feedback::clear_screenshot_preview;
mod input;
mod subtitle_controls;
mod video_controls;
use audio_controls::set_audio_equalizer_controls;
mod materials;
mod observations;
mod plugin_menus;
mod plugins;
use catalog::{
    install_playlist_metadata_refresh, install_playlist_selection, playlist_paste_index,
    selected_playlist_ids, selected_playlist_paths,
};
#[cfg(target_os = "windows")]
use materials::{install_native_video_material_sampler, install_windows_material_policy_monitor};
use observations::{UiUpdateResources, update_ui};
mod session_preferences;

use plugin_menus::reorder_plugin_menu_rows;

use crate::seek_osd::SeekOsd;
use crate::{
    MenuCatalogRow, PlayerWindow, RecentMediaRow, WelcomeWindow, media_open, platform,
    ui_dialogs::{self, Purpose},
};
use riina_core::{Command, Phase, Playback, Preferences, TrackKind, time_label};
use riina_mpv::Player;
use slint::{
    ComponentHandle, Model,
    winit_030::{WinitWindowAccessor, winit},
};
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

const INCOMING_SURFACE_STABLE_FOR: Duration = Duration::from_millis(300);
const MANUAL_TRANSITION_CAPTURE_TIMEOUT: Duration = Duration::from_millis(500);
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
            eprintln!("Could not navigate the playlist: {error}");
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
        eprintln!("Could not capture the outgoing video frame: {error}");
        if let Some(commands) = commands {
            for command in commands {
                if let Err(error) = sender.send(command) {
                    eprintln!("Could not navigate the playlist: {error}");
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
    crate::preference_store::update(preferences_path, &Preferences::default(), |preferences| {
        preferences.recent_open_documents = documents.to_vec();
        Ok(())
    })
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

fn resume_engine_options(
    watch_later_directory: &std::path::Path,
    resume_last_position: bool,
    subtitle_auto_load: &riina_core::subtitle_controls::AutoLoadPreferences,
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
    options.push((
        "sub-auto".into(),
        subtitle_auto_load.mpv_matching_mode().into(),
    ));
    options.push((
        "sub-file-paths".into(),
        subtitle_auto_load.mpv_search_paths().unwrap_or_default(),
    ));
    options.extend(network.engine_options()?);
    options.extend(network_cache.engine_options()?);
    options.extend(network_online_videos.engine_options()?);
    Ok(options)
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

pub(crate) struct UiSession {
    // Native callbacks retain senders only. Field order keeps the target alive
    // until this sole native player owner has joined its decoder worker.
    pub(crate) player: Option<Player>,
    #[cfg(target_os = "windows")]
    _plugin_overlay_presenter: Rc<RefCell<platform::PluginOverlayPresenter>>,
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    _plugin_sidebar_presenter: Rc<RefCell<platform::PluginSidebarPresenter>>,
    #[cfg(target_os = "macos")]
    _system_media_controls: Rc<RefCell<platform::SystemMediaControls>>,
    #[cfg(target_os = "macos")]
    _system_media_controls_timer: Rc<slint::Timer>,
    #[cfg(target_os = "macos")]
    _picture_in_picture: Rc<RefCell<Option<platform::MacPictureInPicture>>>,
    #[cfg(target_os = "macos")]
    _picture_in_picture_timer: Rc<slint::Timer>,
    #[cfg(target_os = "macos")]
    _mac_scroll_event_monitor: Option<platform::MacScrollEventMonitor>,
    #[cfg(target_os = "macos")]
    _mac_plugin_menu_observer: Option<platform::MacPluginMenuObserver>,
    _plugin_host: Option<crate::plugin_host::PluginHost>,
    _plugin_host_timer: Option<Rc<slint::Timer>>,
    _plugin_osd_timer: Option<Rc<slint::Timer>>,
    _presentation: Option<platform::NativePresentation>,
    _hdr_display_timer: Option<Rc<slint::Timer>>,
    _live_text_timer: Option<Rc<slint::Timer>>,
    // The repeated sampler is held for the same lifetime as the player window.
    _appearance_timer: Rc<slint::Timer>,
    _pause_resume_timer: Rc<slint::Timer>,
    _thumbnail_preview_timer: Rc<slint::Timer>,
    _transition_frame_timer: Option<Rc<slint::Timer>>,
    _native_material_timer: Option<Rc<slint::Timer>>,
    #[cfg(target_os = "windows")]
    _windows_materials_policy_timer: Rc<slint::Timer>,
    _subtitle_priority_timer: Rc<slint::Timer>,
    _online_subtitles_window: Rc<RefCell<Option<crate::OnlineSubtitlesWindow>>>,
    _online_subtitles_timer: Rc<slint::Timer>,
    _playback_history_window: Rc<RefCell<Option<crate::PlaybackHistoryWindow>>>,
    _playback_history_timer: Rc<slint::Timer>,
    _video_filters_window: Rc<RefCell<Option<crate::VideoFiltersWindow>>>,
    _video_filters_timer: Rc<slint::Timer>,
    _audio_filters_window: Rc<RefCell<Option<crate::VideoFiltersWindow>>>,
    _audio_filters_timer: Rc<slint::Timer>,
    _playlist_metadata_timer: Rc<slint::Timer>,
    _automatic_music_mode_timer: Rc<slint::Timer>,
    _file_drop_timer: Rc<slint::Timer>,
    _player_window_focus: input::PlayerWindowFocusRegistration,
    prefs_path: PathBuf,
    prefs: Preferences,
    preference_writer: crate::preference_store::Writer,
    audio_equalizer: Arc<std::sync::Mutex<riina_core::audio_equalizer::Settings>>,
    saved_video_filters: Arc<std::sync::Mutex<riina_core::video_filters::Settings>>,
    saved_audio_filters: Arc<std::sync::Mutex<riina_core::video_filters::Settings>>,
    recent_media: Rc<RefCell<Vec<riina_core::RecentMedia>>>,
    recent_open_documents: Rc<RefCell<Vec<riina_core::RecentMedia>>>,
    recent_open_cleared: Rc<Cell<bool>>,
    playback_history: Arc<std::sync::Mutex<crate::playback_history::PlaybackHistoryState>>,
    record_playback_history: Arc<AtomicBool>,
    history_cleared: Arc<AtomicBool>,
    latest_playback: Arc<std::sync::Mutex<Playback>>,
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

impl Drop for UiSession {
    fn drop(&mut self) {
        #[cfg(target_os = "macos")]
        self._system_media_controls
            .borrow_mut()
            .synchronize(false, "", None, None);
        // Release session ownership before its presentation on every platform.
        // Native playback joins here; GL callbacks retain their player until the
        // renderer has released its surface during RenderingTeardown.
        self.player.take();
    }
}

pub(crate) fn action_after_launch()
-> Result<riina_core::ActionAfterLaunch, Box<dyn std::error::Error>> {
    Ok(load_preferences()?.action_after_launch)
}

#[cfg(target_os = "windows")]
pub(crate) fn windows_initial_theme()
-> Result<Option<winit::window::Theme>, Box<dyn std::error::Error>> {
    Ok(match load_preferences()?.effective_theme_material() {
        riina_core::ThemeMaterial::Dark => Some(winit::window::Theme::Dark),
        riina_core::ThemeMaterial::Light => Some(winit::window::Theme::Light),
        riina_core::ThemeMaterial::System => None,
    })
}

fn load_preferences() -> Result<Preferences, Box<dyn std::error::Error>> {
    let path = platform::preferences_path()?;
    match std::fs::read_to_string(path) {
        Ok(text) => Preferences::parse(&text)
            .map_err(std::io::Error::other)
            .map_err(Into::into),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Preferences::default()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn accepts_first_mouse_at_launch() -> Result<bool, Box<dyn std::error::Error>> {
    let path = platform::preferences_path()?;
    let preferences = match std::fs::read_to_string(path) {
        Ok(text) => Preferences::parse(&text).map_err(std::io::Error::other)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Preferences::default(),
        Err(error) => return Err(error.into()),
    };
    Ok(preferences.controls.accepts_first_mouse)
}

pub(crate) fn configure_player(
    app: &PlayerWindow,
    native: Option<platform::NativePresentation>,
    media: Vec<std::ffi::OsString>,
    welcome: Option<&WelcomeWindow>,
    startup_resume_position: Option<f64>,
) -> Result<UiSession, Box<dyn std::error::Error>> {
    #[cfg(target_os = "linux")]
    {
        let weak = app.as_weak();
        app.on_toggle_picture_in_picture(move || {
            if let Some(app) = weak.upgrade() {
                app.set_status(
                    "Picture-in-Picture is unavailable on this presentation path".into(),
                );
            }
        });
    }
    let prefs_path = platform::preferences_path()?;
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
    let saved_preferences = match std::fs::read_to_string(&prefs_path) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    let mut prefs = match saved_preferences.as_deref() {
        Some(text) => Preferences::parse(text).map_err(std::io::Error::other)?,
        None => Preferences::default(),
    };
    if let Err(error) = crate::online_subtitles_credentials::restore_preferences(
        &mut prefs,
        saved_preferences.as_deref(),
        &prefs_path,
    ) {
        app.set_status(format!("OpenSubtitles credential storage: {error}").into());
    }
    let audio_equalizer = Arc::new(std::sync::Mutex::new(prefs.audio_equalizer.clone()));
    let saved_video_filters = Arc::new(std::sync::Mutex::new(prefs.video_filters.clone()));
    let saved_audio_filters = Arc::new(std::sync::Mutex::new(prefs.audio_filters.clone()));
    if let Ok(settings) = audio_equalizer.lock() {
        set_audio_equalizer_controls(app, &settings);
    }
    let playback_history_path = crate::playback_history_store::path_for_preferences(&prefs_path);
    let mut playback_history_write_path = Some(playback_history_path.clone());
    let playback_history_entries = match crate::playback_history_store::load(
        &playback_history_path,
        &prefs.playback_history,
    ) {
        Ok(entries) => {
            prefs.playback_history.clone_from(&entries);
            if !playback_history_path.exists()
                && let Err(error) =
                    crate::playback_history_store::save(&playback_history_path, &entries)
            {
                app.set_status(format!("Could not migrate playback history: {error}").into());
                playback_history_write_path = None;
            }
            entries
        }
        Err(error) => {
            app.set_status(format!("Could not load playback history: {error}").into());
            playback_history_write_path = None;
            prefs.playback_history.clone()
        }
    };
    let plugin_host = match crate::plugin_host::PluginHost::start(
        &prefs_path,
        &prefs.recent_media,
        &playback_history_entries,
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
    {
        let weak = app.as_weak();
        let _ = platform::text_recognition::check_availability(move |available| {
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(app) = weak.upgrade() {
                    app.set_live_text_available(available);
                    if !available {
                        app.set_live_text_enabled(false);
                    }
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
    let plugin_recent_media = Arc::new(std::sync::Mutex::new(prefs.recent_media.clone()));
    let playback_history_session_id = crate::playback_history::next_history_session_id();
    let playback_history = crate::playback_history::shared_playback_history_state(
        playback_history_path,
        playback_history_entries.clone(),
        playback_history_write_path,
        Some(prefs_path.with_file_name("watch-later")),
    );
    let plugin_playback_history = playback_history.clone();
    let record_playback_history = Arc::new(AtomicBool::new(prefs.record_playback_history));
    let notify_record_playback_history = record_playback_history.clone();
    let history_cleared = crate::playback_history::shared_history_merge_flag(&prefs_path);
    let watch_later_directory = prefs_path.with_file_name("watch-later");
    std::fs::create_dir_all(&watch_later_directory)?;
    let ytdl_plugin_active = crate::plugin_host::is_enabled_plugin(&prefs_path, "io.iina.ytdl");
    app.set_network_ytdl_plugin_active(ytdl_plugin_active);
    let mut network_online_videos = prefs.network_online_videos.clone();
    if ytdl_plugin_active {
        network_online_videos.enabled = false;
    }
    let mut engine_options = resume_engine_options(
        &watch_later_directory,
        prefs.resume_last_position,
        &prefs.subtitle_auto_load,
        &prefs.network,
        &prefs.network_cache,
        &network_online_videos,
    )
    .map_err(std::io::Error::other)?;
    engine_options.push((
        "keep-open".into(),
        riina_core::playlist_end_policy(prefs.keep_open_on_file_end, prefs.playlist_auto_play_next)
            .into(),
    ));
    engine_options.extend(
        prefs
            .audio
            .engine_options()
            .map_err(std::io::Error::other)?,
    );
    engine_options.push(("vd-lavc-threads".into(), prefs.video_threads.to_string()));
    engine_options.extend(
        prefs
            .hdr_tone_mapping
            .engine_options()
            .map_err(std::io::Error::other)?,
    );
    if !prefs.hdr_enabled {
        engine_options.extend(riina_core::hdr_preferences::disabled_output_options());
    }
    #[cfg(target_os = "macos")]
    {
        engine_options.retain(|(name, _)| name != "icc-profile-auto");
        engine_options.push(("icc-profile-auto".into(), "no".into()));
    }
    engine_options.extend(
        crate::advanced_preferences::engine_options(&prefs.advanced, &prefs_path)
            .map_err(std::io::Error::other)?,
    );
    app.set_volume_boost(prefs.volume_boost);
    app.set_auto_switch_to_music_mode(prefs.auto_switch_to_music_mode);
    app.set_disable_animations(prefs.disable_animations);
    app.set_system_reduce_motion(crate::platform::system_reduce_motion_enabled());
    app.set_platform_macos(cfg!(target_os = "macos"));
    app.set_platform_windows(cfg!(target_os = "windows"));
    #[cfg(target_os = "windows")]
    {
        app.set_system_materials_enabled(crate::platform::native_materials_enabled());
        if let Some(accent) = crate::platform::windows_accent_color() {
            app.set_windows_accent_color(accent);
        }
        if let Some(foreground) = crate::platform::windows_accent_foreground() {
            app.set_windows_accent_foreground(foreground);
        }
    }
    crate::appearance_preferences::install_system_typography(app);
    app.set_music_mode_show_playlist(prefs.music_mode_show_playlist);
    app.set_music_mode_show_album_art(prefs.music_mode_show_album_art);
    let latest_playback = Arc::new(std::sync::Mutex::new(Playback::default()));
    crate::about_window::connect(app, latest_playback.clone());
    let pause_resume_timer = lifecycle::install_pause_on_minimize(
        app,
        native.as_ref().map(|presentation| presentation.host()),
        latest_playback.clone(),
    );
    crate::control_preferences::install_player(app, &prefs.controls);
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
    app.set_hardware_decoder_index(crate::codec_preferences::index(prefs.hardware_decoder));
    app.set_subtitle_ass_override_index(crate::subtitle_preferences::index(
        prefs.subtitle_ass_override,
    ));
    app.set_secondary_subtitle_ass_override_index(crate::subtitle_preferences::index(
        prefs.secondary_subtitle_ass_override,
    ));
    crate::subtitle_position_preferences::install_player(app, prefs.subtitle_position);
    crate::subtitle_other_preferences::install_player(app, &prefs.subtitle_other);
    crate::subtitle_auto_load_preferences::install_player(app, &prefs.subtitle_auto_load);
    crate::online_subtitles_preferences::install_player(app, &prefs.online_subtitles);
    crate::network_cache_preferences::install_player(app, &prefs.network_cache);
    crate::network_ytdl_preferences::install_player(app, &prefs.network_online_videos);
    app.set_subtitle_font(prefs.subtitle_font.clone().into());
    app.set_subtitle_font_size_preference(prefs.subtitle_font_size as f32);
    crate::subtitle_preferences::install_style(app, prefs.subtitle_text_style);
    if let Some(welcome) = welcome {
        welcome.set_app_version(env!("CARGO_PKG_VERSION").into());
        refresh_welcome_history(welcome, &prefs.recent_media, prefs.resume_last_position);
    }
    app.set_video_aspect_presets(slint::ModelRc::new(slint::VecModel::from(
        riina_core::video_controls::ASPECT_PRESETS
            .map(slint::SharedString::from)
            .to_vec(),
    )));
    app.set_osd_enabled(prefs.enable_osd);
    app.set_disable_osd_file_start(prefs.disable_osd_file_start);
    app.set_disable_osd_pause_resume(prefs.disable_osd_pause_resume);
    app.set_disable_osd_speed(prefs.disable_osd_speed);
    app.set_disable_osd_seek(prefs.disable_osd_seek);
    app.set_osd_timeout_secs(prefs.osd_timeout_secs as f32);
    app.set_osd_text_size(prefs.osd_text_size as f32);
    app.set_display_time_and_battery_in_fullscreen(prefs.display_time_and_battery_in_fullscreen);
    crate::osc_preferences::install_player(app, prefs.control_bar);
    crate::screenshot_preferences::install(app, &prefs.screenshot);
    crate::thumbnail_preferences::install_player(app, &prefs.thumbnail_preview);
    app.set_pause_when_open(prefs.pause_when_open);
    app.set_play_when_entering_fullscreen(prefs.play_when_entering_fullscreen);
    app.set_pause_when_leaving_fullscreen(prefs.pause_when_leaving_fullscreen);
    app.set_pause_when_inactive(prefs.pause_when_inactive);
    app.set_pause_when_minimized(prefs.pause_when_minimized);
    app.set_pause_when_goes_to_sleep(prefs.pause_when_goes_to_sleep);
    app.set_quit_when_no_opened_window(prefs.quit_when_no_opened_window);
    app.set_keep_open_on_file_end(prefs.keep_open_on_file_end);
    app.set_playlist_auto_play_next(prefs.playlist_auto_play_next);
    app.set_playlist_auto_add(prefs.playlist_auto_add);
    app.set_playlist_show_metadata(prefs.playlist_show_metadata);
    app.set_playlist_show_metadata_in_music_mode(prefs.playlist_show_metadata_in_music_mode);
    app.set_auto_repeat(prefs.auto_repeat);
    app.set_default_repeat_mode_index(prefs.default_repeat_mode.index());
    app.set_fullscreen_when_open(prefs.fullscreen_when_open);
    app.set_resume_last_position(prefs.resume_last_position);
    app.set_record_playback_history(prefs.record_playback_history);
    app.set_record_recent_files(prefs.record_recent_files);
    app.set_track_all_files_in_recent_open_menu(prefs.track_all_files_in_recent_open_menu);
    app.set_recent_open_menu_rows(slint::ModelRc::new(slint::VecModel::from(
        recent_open_menu_rows(&recent_open_documents.borrow()),
    )));
    app.set_launch_action_index(prefs.action_after_launch.index());
    app.set_leading_sidebar_width(prefs.leading_sidebar_width as f32);
    app.set_trailing_sidebar_width(prefs.trailing_sidebar_width as f32);
    app.set_settings_at_leading(prefs.settings_at_leading);
    app.set_playlist_at_leading(prefs.playlist_at_leading);
    app.set_plugins_at_leading(prefs.plugins_at_leading);
    app.set_edge_to_edge_video(prefs.edge_to_edge_video);
    app.set_docked_controls(prefs.docked_controls);
    app.set_unlock_window_aspect_ratio(prefs.unlock_window_aspect_ratio);
    crate::window_resize_preferences::install_player(app, prefs.window_resize);
    app.set_compact_ui(prefs.compact_ui);
    app.set_always_float_on_top(prefs.always_float_on_top);
    app.set_always_show_on_top_icon(prefs.always_show_on_top_icon);
    crate::liquid_glass_preferences::install_player(app, prefs.liquid_glass);
    let appearance = prefs.effective_theme_material();
    app.set_appearance_mode(appearance.index());
    app.set_surface_material_title(crate::platform::surface_material_title().into());
    app.set_dark_theme(appearance.dark_for(prefs.dark));
    let appearance_timer = crate::appearance_preferences::connect(app);
    app.set_osc_position(i32::from(prefs.osc_position));
    app.set_control_bar_position_horizontal(prefs.control_bar_position_horizontal as f32);
    app.set_control_bar_position_vertical(prefs.control_bar_position_vertical as f32);
    app.set_show_chapter_pos(prefs.show_chapter_pos);
    crate::pip_preferences::install_player(app, prefs.picture_in_picture);
    crate::sidebar_preferences::connect(app, prefs_path.clone(), prefs.clone());
    crate::layout_preferences::connect(app, prefs_path.clone(), prefs.clone());
    refresh_video_menu(app);
    app.set_video_crop_presets(slint::ModelRc::new(slint::VecModel::from(
        riina_core::crop::LABELS
            .map(slint::SharedString::from)
            .to_vec(),
    )));
    app.set_primary_shortcut_is_meta(cfg!(target_os = "macos"));
    app.set_current_file_manager_name(
        if cfg!(target_os = "macos") {
            "Finder"
        } else if cfg!(target_os = "windows") {
            "Explorer"
        } else {
            "File Manager"
        }
        .into(),
    );
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
        let original_size = Rc::new(Cell::new(None::<slint::PhysicalSize>));
        let saved_size = original_size.clone();
        let weak = app.as_weak();
        app.on_music_mode_changed(move || {
            if let Some(app) = weak.upgrade() {
                let window = app.window();
                if app.get_music_mode() {
                    if saved_size.get().is_none() {
                        saved_size.set(Some(window.size()));
                    }
                    let scale = f64::from(window.scale_factor());
                    let height = crate::music_mode_layout::window_height(
                        app.get_music_mode_show_album_art(),
                        app.get_music_mode_show_playlist(),
                        f64::from(app.get_music_mode_artwork_aspect_ratio()),
                    );
                    let target = slint::PhysicalSize::new(
                        (crate::music_mode_layout::WINDOW_WIDTH * scale).round() as u32,
                        (height * scale).round() as u32,
                    );
                    crate::window_aspect::resize_centered(
                        window,
                        riina_core::window_aspect::PixelSize {
                            width: window.size().width,
                            height: window.size().height,
                        },
                        riina_core::window_aspect::PixelSize {
                            width: target.width,
                            height: target.height,
                        },
                        None,
                    );
                } else if let Some(size) = saved_size.take() {
                    crate::window_aspect::resize_centered(
                        window,
                        riina_core::window_aspect::PixelSize {
                            width: window.size().width,
                            height: window.size().height,
                        },
                        riina_core::window_aspect::PixelSize {
                            width: size.width,
                            height: size.height,
                        },
                        None,
                    );
                }
            }
        });
    }
    let queued = Arc::new(AtomicBool::new(false));
    let displayed = Arc::new(std::sync::Mutex::new(None));
    let crop_editor = crate::crop_editor::Editor::default();
    let observed_crop_editor = crop_editor.clone();
    let last_seek = Arc::new(std::sync::Mutex::new(SeekOsd::default()));
    let mut video_osd =
        crate::video_osd::VideoOsd::new(prefs.disable_osd_speed, prefs.disable_osd_pause_resume);
    video_osd.set_volume_limit(prefs.volume_limit());
    let last_video = Arc::new(std::sync::Mutex::new(video_osd));
    let feedback_video_osd = last_video.clone();
    let last_screenshot = Arc::new(std::sync::Mutex::new(
        crate::screenshot_osd::ScreenshotOsd::default(),
    ));
    let screenshot_scratch = Arc::new(crate::screenshot_output::Scratch::default());
    let opening_fullscreen = Arc::new(std::sync::Mutex::new(
        crate::opening_fullscreen::OpeningFullscreen::default(),
    ));
    #[cfg(target_os = "macos")]
    let picture_in_picture = Rc::new(RefCell::new(None::<platform::MacPictureInPicture>));
    #[cfg(target_os = "macos")]
    let picture_in_picture_state_dirty = Arc::new(AtomicBool::new(false));
    let notify_music_mode_state = music_mode_state.clone();
    let worker_scratch = screenshot_scratch.clone();
    let notify_plugin_host = plugin_host_handle.clone();
    let notify_plugin_recent_media = plugin_recent_media.clone();
    let notify_video_filter_settings = saved_video_filters.clone();
    let notify_audio_filter_settings = saved_audio_filters.clone();
    let notify_audio_device_sender = plugin_command_sender.clone();
    let notified_plugin_file_context = Arc::new(std::sync::Mutex::new(
        crate::plugin_files::PluginPlayerFileContext::default(),
    ));
    let last_notified_plugin_file_context = notified_plugin_file_context.clone();
    #[cfg(target_os = "macos")]
    let notify_picture_in_picture_state_dirty = picture_in_picture_state_dirty.clone();
    let notify = move |state: Playback| {
        // This closure remains owned by the engine until native destruction.
        let _retain_until_native_teardown = &worker_scratch;
        if let Ok(mut latest_playback) = observed_playback.lock() {
            *latest_playback = state.clone();
        }
        let (plugin_context, history_error) = crate::plugin_snapshot::observe_playback(
            &state,
            &notify_plugin_recent_media,
            &plugin_playback_history,
            notify_record_playback_history.load(Ordering::Acquire),
            playback_history_session_id,
        );
        if let Some(error) = history_error {
            let weak = app_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(app) = weak.upgrade() {
                    app.set_status(format!("Could not save playback history: {error}").into());
                }
            });
        }
        if let Ok(mut last_context) = last_notified_plugin_file_context.lock()
            && *last_context != plugin_context
            && let Some(host) = &notify_plugin_host
            && host
                .update_player_file_context(plugin_context.clone())
                .is_ok()
        {
            *last_context = plugin_context;
        }
        #[cfg(target_os = "macos")]
        notify_picture_in_picture_state_dirty.store(true, Ordering::Release);
        latest.publish(state);
        if queued.swap(true, Ordering::AcqRel) {
            return;
        }
        let latest = latest.clone();
        let queued = queued.clone();
        let weak = app_weak.clone();
        let displayed = displayed.clone();
        let last_seek = last_seek.clone();
        let last_video = last_video.clone();
        let last_screenshot = last_screenshot.clone();
        let crop_editor = observed_crop_editor.clone();
        let notify_saved_video_filters = notify_video_filter_settings.clone();
        let notify_saved_audio_filters = notify_audio_filter_settings.clone();
        let audio_device_sender = notify_audio_device_sender.clone();
        let opening_fullscreen = opening_fullscreen.clone();
        let music_mode_state = notify_music_mode_state.clone();
        let transition_overlay = notify_transition_overlay.clone();
        #[cfg(target_os = "windows")]
        let transition_release_generation = notify_transition_release_generation.clone();
        let _ = slint::invoke_from_event_loop(move || {
            queued.store(false, Ordering::Release);
            if let Some(app) = weak.upgrade()
                && let Some(playback_mailbox::Delivery { state, captures }) = latest.take()
                && let Ok(mut displayed) = displayed.lock()
                && let Ok(mut last_seek) = last_seek.lock()
                && let Ok(mut last_video) = last_video.lock()
                && let Ok(mut last_screenshot) = last_screenshot.lock()
            {
                let first_loaded = (!app.get_loaded()
                    || app.get_window_geometry_source().parse::<u64>().ok()
                        != Some(state.generation))
                    && matches!(
                        state.phase,
                        Phase::Playing | Phase::Paused | Phase::Buffering | Phase::Ended
                    );
                let enter_fullscreen = opening_fullscreen.lock().is_ok_and(|mut opening| {
                    opening.observe(
                        &state,
                        app.get_fullscreen_when_open(),
                        app.get_fullscreen(),
                        false,
                    )
                });
                let clipboard_error = crate::screenshot_output::copy(&app, &captures);
                crop_editor.observe(&app, &state);
                let audio_device_sender = audio_device_sender
                    .lock()
                    .ok()
                    .and_then(|sender| sender.clone());
                crate::automatic_music_mode::observe(&app, &state, &music_mode_state);
                let resize_for_media = update_ui(
                    &app,
                    state,
                    UiUpdateResources {
                        audio_device_player: audio_device_sender.as_ref(),
                        saved_video_filters: &notify_saved_video_filters,
                        saved_audio_filters: &notify_saved_audio_filters,
                        transition_overlay: transition_overlay.as_ref(),
                        #[cfg(target_os = "windows")]
                        transition_release_generation: transition_release_generation.as_ref(),
                    },
                    &mut displayed,
                    &mut last_seek,
                    &mut last_video,
                    &mut last_screenshot,
                );
                if resize_for_media {
                    // Apply the complete geometry after automatic music mode has
                    // restored the video window. Coalesced snapshots may omit
                    // Loading, so media identity owns this transition.
                    app.invoke_resize_window_for_media();
                }
                if first_loaded {
                    app.invoke_initial_media_loaded();
                }
                crate::screenshot_output::cleanup(&app, &captures);
                if enter_fullscreen {
                    app.invoke_toggle_fullscreen();
                }
                if let Some(error) = clipboard_error {
                    app.set_status(format!("Could not copy screenshot: {error}").into());
                }
            }
        });
    };
    let player = if let Some(presentation) = &native {
        presentation.start_engine(engine_options, notify)
    } else {
        Player::start_with_options(&platform::playback_library(), false, engine_options, notify)
    };
    let player = match player {
        Ok(player) => player,
        Err(error) => {
            app.set_status(error.clone().into());
            return Err(std::io::Error::other(error).into());
        }
    };
    let history_plugin = plugin_host_handle.clone();
    let history_latest_playback = latest_playback.clone();
    let history_recent_media = plugin_recent_media.clone();
    let history_sync: crate::playback_history_window::HistorySync = Arc::new(move |history| {
        let Some(plugin) = &history_plugin else {
            return Ok(());
        };
        let playback = history_latest_playback
            .lock()
            .map_err(|_| "latest playback lock poisoned".to_owned())?
            .clone();
        let context = crate::plugin_snapshot::player_file_context(
            &playback,
            &mut history_recent_media
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            &history,
        );
        plugin.update_player_file_context(context)
    });
    let (playback_history_window, playback_history_timer) = crate::playback_history_window::install(
        app,
        playback_history.clone(),
        player.command_sender(),
        prefs_path.clone(),
        history_cleared.clone(),
        history_sync,
    );
    let ((video_filters_window, video_filters_timer), (audio_filters_window, audio_filters_timer)) =
        crate::video_filter_window::install(
            app,
            latest_playback.clone(),
            saved_video_filters.clone(),
            saved_audio_filters.clone(),
            player.command_sender(),
            prefs_path.clone(),
        );
    let playlist_metadata_timer = install_playlist_metadata_refresh(app, latest_playback.clone());
    let automatic_music_mode_timer = crate::automatic_music_mode::install(
        app,
        latest_playback.clone(),
        music_mode_state.clone(),
    );
    if let Ok(mut sender) = plugin_command_sender.lock() {
        *sender = Some(player.command_sender());
    }
    let subtitle_discovery_ready = Arc::new(crate::subtitle_discovery::Readiness::default());
    let (online_subtitles_window, online_subtitles_timer) = crate::open_subtitles::connect_player(
        app,
        player.command_sender(),
        latest_playback.clone(),
        subtitle_discovery_ready.clone(),
    );
    let subtitle_priority_timer = discovery::install(
        app,
        &player.command_sender(),
        latest_playback.clone(),
        subtitle_discovery_ready,
    );
    #[cfg(target_os = "macos")]
    let picture_in_picture = {
        let manager = objc2::MainThreadMarker::new()
            .ok_or_else(|| std::io::Error::other("PiP setup must run on the macOS main thread"))
            .and_then(|mtm| {
                platform::MacPictureInPicture::new(
                    player.command_sender(),
                    latest_playback.clone(),
                    mtm,
                )
                .map_err(std::io::Error::other)
            });
        let manager = match manager {
            Ok(manager) => Some(manager),
            Err(error) => {
                app.set_status(format!("macOS PiP unavailable: {error}").into());
                None
            }
        };
        *picture_in_picture.borrow_mut() = manager;
        let weak_app = app.as_weak();
        let toggle_manager = picture_in_picture.clone();
        app.on_toggle_picture_in_picture(move || {
            let result = toggle_manager
                .borrow_mut()
                .as_mut()
                .ok_or_else(|| "macOS PiP is unavailable on this system".to_owned())
                .and_then(platform::MacPictureInPicture::toggle);
            if let Err(error) = result
                && let Some(app) = weak_app.upgrade()
            {
                app.set_status(error.into());
            }
        });
        picture_in_picture.clone()
    };
    #[cfg(target_os = "macos")]
    let picture_in_picture_timer = {
        let manager = picture_in_picture.clone();
        let dirty = picture_in_picture_state_dirty.clone();
        let weak_app = app.as_weak();
        let timer = slint::Timer::default();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(100),
            move || {
                let mut active = false;
                if let Some(manager) = manager.borrow_mut().as_mut() {
                    active = manager.is_active();
                    if dirty.swap(false, Ordering::AcqRel) {
                        manager.synchronize_playback();
                    }
                }
                if let Some(app) = weak_app.upgrade()
                    && app.get_picture_in_picture_active() != active
                {
                    app.set_picture_in_picture_active(active);
                }
            },
        );
        Rc::new(timer)
    };
    let input::InstalledInput {
        file_drop_timer,
        player_window_focus,
        #[cfg(target_os = "macos")]
        mac_scroll_event_monitor,
    } = input::install(app, &player.command_sender(), latest_playback.clone());
    let thumbnail_preview_timer = Rc::new(slint::Timer::default());
    let thumbnail_controller =
        Rc::new(RefCell::new(crate::thumbnail_preview::Controller::default()));
    let thumbnail_hover = thumbnail_controller.clone();
    let thumbnail_weak = app.as_weak();
    app.on_timeline_hovered(move |seconds| {
        if let Some(app) = thumbnail_weak.upgrade() {
            thumbnail_hover.borrow_mut().hover(&app, f64::from(seconds));
        }
    });
    let thumbnail_weak = app.as_weak();
    let thumbnail_playback = latest_playback.clone();
    let thumbnail_tick = thumbnail_controller.clone();
    let thumbnail_library = platform::playback_library();
    let thumbnail_plugin_host = plugin_host_handle.clone();
    thumbnail_preview_timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(80),
        move || {
            let Some(app) = thumbnail_weak.upgrade() else {
                return;
            };
            let Ok(settings) = crate::thumbnail_preferences::current_player(&app) else {
                return;
            };
            let Ok(state) = thumbnail_playback.lock().map(|state| state.clone()) else {
                return;
            };
            let ready =
                thumbnail_tick
                    .borrow_mut()
                    .observe(&app, &state, &settings, &thumbnail_library);
            if ready && let Some(host) = &thumbnail_plugin_host {
                let _ = host.notify_thumbnails_ready();
            }
        },
    );
    let pending_navigation: PendingNavigation = Rc::new(RefCell::new(None));
    let transition_frame_timer = if cfg!(target_os = "windows") && native.is_some() {
        let timer = Rc::new(slint::Timer::default());
        let playback = latest_playback.clone();
        let sender = player.command_sender();
        let captured_generation = Rc::new(Cell::new(None::<u64>));
        let outgoing_capture_attempt = Rc::new(Cell::new(None::<(u64, Instant)>));
        let incoming_capture_attempt = Rc::new(Cell::new(None::<(u64, Instant)>));
        let incoming_capture_stability = Rc::new(Cell::new(None::<(u64, u64, u8, Instant)>));
        let surface_host = native.as_ref().map(|presentation| presentation.host());
        let surface_geometry = Rc::new(Cell::new(None::<(u64, slint::PhysicalSize, u64, Instant)>));
        let timer_generation = captured_generation.clone();
        let timer_outgoing_attempt = outgoing_capture_attempt.clone();
        let timer_incoming_attempt = incoming_capture_attempt.clone();
        let timer_incoming_stability = incoming_capture_stability.clone();
        let timer_transition_release_generation = transition_release_generation.clone();
        let timer_pending_navigation = pending_navigation.clone();
        let timer_surface_host = surface_host.clone();
        let timer_surface_geometry = surface_geometry.clone();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(80),
            move || {
                let Ok(state) = playback.lock().map(|state| state.clone()) else {
                    return;
                };
                let navigation = {
                    let mut pending = timer_pending_navigation.borrow_mut();
                    pending
                        .as_ref()
                        .is_some_and(|(_, requested_at)| {
                            state.transition_frame_generation == Some(state.generation)
                                && state
                                    .transition_frame_captured_at
                                    .is_some_and(|captured_at| captured_at >= *requested_at)
                                || requested_at.elapsed() >= MANUAL_TRANSITION_CAPTURE_TIMEOUT
                        })
                        .then(|| pending.take().map(|(commands, _)| commands))
                        .flatten()
                };
                if let Some(commands) = navigation {
                    for command in commands {
                        if let Err(error) = sender.send(command) {
                            eprintln!("Could not navigate the playlist: {error}");
                        }
                    }
                }
                if state.transition_frame_generation == Some(state.generation) {
                    // Only a successful asynchronous reply confirms that mpv
                    // retained a frame. Queue acceptance alone is not enough:
                    // the D3D11 output can reject a capture while reconfiguring.
                    timer_generation.set(Some(state.generation));
                    timer_outgoing_attempt.set(None);
                }
                let now = Instant::now();
                if crate::transition_frame::outgoing_capture_due(
                    &state,
                    timer_generation.get(),
                    timer_outgoing_attempt.get(),
                    now,
                ) {
                    // Capture before mpv tears down and reconfigures its
                    // D3D11 output. Retry rejected or failed captures while
                    // there is still time before the playlist advances.
                    match sender.send(Command::CaptureTransitionFrame) {
                        Ok(()) => timer_outgoing_attempt.set(Some((state.generation, now))),
                        Err(error) => {
                            timer_outgoing_attempt.set(None);
                            eprintln!("Could not retain the outgoing video frame: {error}");
                        }
                    }
                }
                if state
                    .transition_frame_generation
                    .is_some_and(|generation| generation < state.generation)
                    && crate::transition_frame::incoming_has_video(&state)
                    && !matches!(state.phase, Phase::Loading | Phase::Stopped | Phase::Failed)
                    && crate::transition_frame::incoming_capture_window_active(
                        &state,
                        Instant::now(),
                    )
                    && crate::transition_frame::incoming_capture_ready(&state)
                {
                    let now = Instant::now();
                    // Decoder captures can remain healthy while mpv's native
                    // child HWND is hidden or changing size. Check the actual
                    // presentation surface before considering cover release;
                    // doing this after the capture-stability branch let a
                    // stale but valid decoder frame uncover a blank swap chain.
                    let surface_stable = timer_surface_host
                        .as_ref()
                        .and_then(slint::Weak::upgrade)
                        .is_some_and(|host| {
                            let size = host.window().size();
                            let Some(surface_signature) =
                                crate::platform::native_video_surface_signature(host.window())
                            else {
                                timer_surface_geometry.set(None);
                                return false;
                            };
                            match timer_surface_geometry.get() {
                                Some((generation, previous_size, previous_surface, since))
                                    if generation == state.generation
                                        && previous_size == size
                                        && previous_surface == surface_signature =>
                                {
                                    now.duration_since(since) >= INCOMING_SURFACE_STABLE_FOR
                                }
                                _ => {
                                    timer_surface_geometry.set(Some((
                                        state.generation,
                                        size,
                                        surface_signature,
                                        now,
                                    )));
                                    false
                                }
                            }
                        });
                    let capture_is_usable = state.incoming_transition_frame_generation
                        == Some(state.generation)
                        && crate::transition_frame::incoming_frame_is_usable(&state);
                    if capture_is_usable {
                        let revision = state.incoming_transition_frame_revision;
                        let stable_capture = match timer_incoming_stability.get() {
                            Some((generation, last_revision, count, since))
                                if generation == state.generation && last_revision == revision =>
                            {
                                (generation, last_revision, count, since)
                            }
                            Some((generation, _, count, since))
                                if generation == state.generation =>
                            {
                                (generation, revision, count.saturating_add(1), since)
                            }
                            _ => (state.generation, revision, 1, now),
                        };
                        timer_incoming_stability.set(Some(stable_capture));
                        let (_, _, count, since) = stable_capture;
                        if crate::transition_frame::incoming_cover_release_ready(
                            count,
                            since,
                            now,
                            surface_stable,
                        ) {
                            timer_transition_release_generation
                                .store(state.generation, Ordering::Release);
                            return;
                        }
                    } else {
                        timer_incoming_stability.set(None);
                    }
                    let retry_wait_elapsed =
                        timer_incoming_attempt
                            .get()
                            .is_none_or(|(generation, attempted_at)| {
                                generation != state.generation
                                    || now.duration_since(attempted_at)
                                        >= Duration::from_millis(240)
                            });
                    if !retry_wait_elapsed {
                        return;
                    }
                    if !surface_stable {
                        return;
                    }
                    // Do not uncover the native child just because time-pos or
                    // track metadata appeared. Wait for host geometry to settle,
                    // then require a successful raw capture before uncovering.
                    if let Err(error) = sender.send(Command::CaptureIncomingTransitionFrame) {
                        eprintln!("Could not capture the incoming video frame: {error}");
                    } else {
                        timer_incoming_attempt.set(Some((state.generation, now)));
                    }
                }
            },
        );
        Some(timer)
    } else {
        None
    };
    #[cfg(target_os = "windows")]
    let native_material_timer = native.is_some().then(|| {
        install_native_video_material_sampler(
            app,
            player.command_sender(),
            latest_playback.clone(),
            native
                .as_ref()
                .and_then(platform::NativePresentation::material_capture),
        )
    });
    #[cfg(not(target_os = "windows"))]
    let native_material_timer = None;
    #[cfg(target_os = "windows")]
    let windows_materials_policy_timer = install_windows_material_policy_monitor(app);
    #[cfg(target_os = "macos")]
    {
        *system_media_sender.borrow_mut() = Some(player.command_sender());
    }
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    let live_text_timer = Some(live_text::install(
        app,
        player.command_sender(),
        latest_playback.clone(),
    ));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let live_text_timer = None;
    let manual_music_mode = music_mode_state.clone();
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
    app.on_toggle_music_mode(move || {
        if let (Some(app), Ok(mut mode)) = (weak.upgrade(), manual_music_mode.lock()) {
            let active = mode.toggle_manually();
            if active && app.get_fullscreen() {
                app.invoke_toggle_fullscreen();
            }
            app.set_music_mode(active);
        }
    });
    let automatic_music_mode = music_mode_state.clone();
    app.on_auto_switch_music_mode_changed(move |enabled| {
        if let Ok(mut mode) = automatic_music_mode.lock() {
            mode.set_auto_switch(enabled);
        }
    });
    let clear_history_plugin_host = plugin_host_handle.clone();
    let clear_history_latest_playback = latest_playback.clone();
    let clear_history_recent_media = plugin_recent_media.clone();
    crate::preferences_window::connect(
        app,
        player.command_sender(),
        prefs_path.clone(),
        prefs.clone(),
        latest_playback.clone(),
        plugin_host_handle,
        {
            let path = prefs_path.clone();
            let playback_history = playback_history.clone();
            let history_cleared = history_cleared.clone();
            let plugin_host = clear_history_plugin_host;
            let latest_playback = clear_history_latest_playback;
            let recent_media = clear_history_recent_media;
            move || {
                let mut history = playback_history
                    .lock()
                    .map_err(|_| "playback history lock poisoned".to_owned())?;
                let removed = crate::utilities_preferences::clear_playback_history(&path)?;
                history.clear();
                history_cleared.store(true, Ordering::Release);
                let entries = history.entries().to_vec();
                drop(history);
                if let Some(host) = &plugin_host {
                    let playback = latest_playback
                        .lock()
                        .map_err(|_| "latest playback lock poisoned".to_owned())?
                        .clone();
                    let context = crate::plugin_snapshot::player_file_context(
                        &playback,
                        &mut recent_media
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner),
                        &entries,
                    );
                    host.update_player_file_context(context)
                        .map_err(|error| error.to_string())?;
                }
                Ok(removed)
            }
        },
    );
    let opening_player = player.command_sender();
    let opening_app = app.as_weak();
    let resume_player = player.command_sender();
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
    let record_history = record_playback_history.clone();
    app.on_record_playback_history_changed(move |enabled| {
        record_history.store(enabled, Ordering::Release);
    });
    app.on_pause_when_open_changed(move |pause| {
        if let Err(error) = opening_player.send(Command::SetPauseWhenOpen(pause))
            && let Some(app) = opening_app.upgrade()
        {
            app.set_status(error.into());
        }
    });
    crate::window_controls::install(app, native.as_ref().map(platform::NativePresentation::host));
    crop_editor.install(app, player.command_sender());
    let reflection_player = player.command_sender();
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
    feedback::install(
        app,
        &player.command_sender(),
        screenshot_scratch,
        feedback_video_osd,
    );
    let deinterlace_player = player.command_sender();
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
    macro_rules! connect {
        ($method:ident, $command:expr) => {{
            let player = player.command_sender();
            let weak = app.as_weak();
            app.$method(move || {
                let command = $command;
                let show_stop = matches!(&command, Command::Stop);
                if let Some(app) = weak.upgrade() {
                    app.invoke_activity();
                    if matches!(command, Command::Stop) {
                        media_open::invalidate(&app);
                    }
                }
                match player.send(command) {
                    Err(error) => {
                        if let Some(app) = weak.upgrade() {
                            app.set_status(error.into());
                        }
                    }
                    Ok(()) if show_stop => {
                        if let Some(app) = weak.upgrade() {
                            let message = crate::video_osd::action_message(
                                crate::video_osd::ActionFeedback::Stop,
                            );
                            show_osd_message(&app, message.text, true);
                        }
                    }
                    Ok(()) => {}
                }
            });
        }};
    }
    // Let mpv toggle its current state: UI pause snapshots can lag an in-flight
    // transition, while the playback worker serializes this action with engine events.
    let pause_player = player.command_sender();
    let pause_app = app.as_weak();
    app.on_toggle_pause(move || {
        let Some(app) = pause_app.upgrade() else {
            return;
        };
        app.invoke_activity();
        if let Err(error) = pause_player.toggle_pause() {
            app.set_status(error.into());
        }
    });
    let set_pause_player = player.command_sender();
    let set_pause_app = app.as_weak();
    app.on_set_paused(move |paused| {
        let Some(app) = set_pause_app.upgrade() else {
            return;
        };
        app.invoke_activity();
        if let Err(error) = set_pause_player.send(Command::SetPaused(paused)) {
            app.set_status(error.into());
        }
    });
    let input_action_player = player.command_sender();
    let input_action_app = app.as_weak();
    app.on_execute_mpv_input_action(move |action| {
        let Some(app) = input_action_app.upgrade() else {
            return;
        };
        app.invoke_activity();
        if let Err(error) = input_action_player.send(Command::MpvInputAction(action.to_string())) {
            app.set_status(error.into());
        }
    });
    connect!(on_toggle_mute, Command::ToggleMute);
    let hdr_player = player.command_sender();
    hdr_output::install(app, move |command| hdr_player.send(command));
    connect!(on_stop, Command::Stop);
    connect!(on_frame_step, Command::FrameStep);
    connect!(on_frame_back_step, Command::FrameBackStep);
    let chapter_player = player.command_sender();
    let weak = app.as_weak();
    app.on_adjust_chapter(move |direction| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
        }
        if let Err(error) = chapter_player.send(Command::AdjustChapter(direction))
            && let Some(app) = weak.upgrade()
        {
            app.set_status(error.into());
        }
    });
    let subtitle_player = player.command_sender();
    let weak = app.as_weak();
    app.on_subtitle_seek(move |direction| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
        }
        if let Err(error) = subtitle_player.send(Command::SubtitleSeek(direction))
            && let Some(app) = weak.upgrade()
        {
            app.set_status(error.into());
        }
    });
    let capture_transition_first = cfg!(target_os = "windows") && native.is_some();
    let playlist_navigation_sender = player.command_sender();
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
    connect!(on_reset_speed, Command::Speed(1.0));
    let speed_slider_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_speed_slider(move |position| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            let result = riina_core::speed_controls::from_slider(position.into())
                .map_err(str::to_owned)
                .and_then(|command| speed_slider_player.send(command));
            if let Err(error) = result {
                app.set_status(error.into());
            }
        }
    });
    let speed_text_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_speed_text(move |text| {
        let result = riina_core::speed_controls::from_text(text.as_str())
            .map_err(str::to_owned)
            .and_then(|command| speed_text_player.send(command));
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            if let Err(error) = result {
                app.set_status(error.into());
                return false;
            }
        }
        true
    });
    video_controls::install(app, &player.command_sender());
    connect!(on_reset_subtitle_delay, Command::SubtitleDelay(0.0));
    audio_controls::install(app, &player.command_sender(), audio_equalizer.clone());
    subtitle_controls::install(app, &player.command_sender());
    connect!(on_reset_audio_delay, Command::AudioDelay(0.0));
    connect!(on_cycle_ab_loop, Command::CycleAbLoop);
    let ab_loop_player = player.command_sender();
    let ab_loop_app = app.as_weak();
    app.on_set_ab_loop_point(move |point, value| {
        let Some(app) = ab_loop_app.upgrade() else {
            return;
        };
        if !app.get_loaded() || !value.is_finite() {
            return;
        }
        let point = match point {
            0 => riina_core::AbLoopPoint::A,
            1 => riina_core::AbLoopPoint::B,
            _ => return,
        };
        app.invoke_activity();
        if let Err(error) = ab_loop_player.send(Command::SetAbLoopPoint {
            point,
            value: Some(f64::from(value)),
        }) {
            app.set_status(error.into());
        }
    });
    connect!(on_toggle_file_loop, Command::ToggleFileLoop);
    connect!(
        on_toggle_secondary_subtitle_visibility,
        Command::ToggleSecondarySubtitleVisibility
    );
    connect!(on_cycle_audio_track, Command::CycleTrack(TrackKind::Audio));
    connect!(on_cycle_subtitle_track, Command::CycleTrack(TrackKind::Sub));
    connect!(on_cycle_video_track, Command::CycleTrack(TrackKind::Video));
    let audio_device_player = player.command_sender();
    let audio_device_state = latest_playback.clone();
    let audio_device_app = app.as_weak();
    app.on_set_audio_device(move |device| {
        let Some(app) = audio_device_app.upgrade() else {
            return;
        };
        let device = device.to_string();
        let available = audio_device_state.lock().ok().is_some_and(|state| {
            state
                .audio_devices
                .iter()
                .any(|available| available.name == device)
        });
        if !available {
            app.set_status("That audio device is no longer available".into());
            return;
        }
        app.invoke_activity();
        if let Err(error) = audio_device_player.send(Command::AudioDevice(device)) {
            app.set_status(error.into());
        }
    });
    connect!(
        on_toggle_subtitle_visibility,
        Command::ToggleSubtitleVisibility
    );
    let subtitle_encoding_player = player.command_sender();
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
    let selection_player = player.command_sender();
    let selection_state = latest_playback.clone();
    let weak = app.as_weak();
    app.on_select_item(move |category, token| {
        // IDs stay strings in the UI: Slint's float numbers cannot preserve every i64.
        let command = match category {
            5 => token
                .as_str()
                .parse::<i64>()
                .map(Some)
                .or_else(|error| if token == "no" { Ok(None) } else { Err(error) })
                .map(Command::SelectSecondarySubtitle)
                .map_err(|error| error.to_string()),
            1 | 2 | 6 => token
                .as_str()
                .parse::<i64>()
                .map(Some)
                .or_else(|error| if token == "no" { Ok(None) } else { Err(error) })
                .map(|id| Command::SelectTrack {
                    kind: match category {
                        1 => TrackKind::Audio,
                        6 => TrackKind::Video,
                        _ => TrackKind::Sub,
                    },
                    id,
                })
                .map_err(|error| error.to_string()),
            3 => token
                .parse::<usize>()
                .map(Command::SelectChapter)
                .map_err(|error| error.to_string()),
            4 => token
                .parse::<i64>()
                .map(Command::SelectPlaylistId)
                .map_err(|error| error.to_string()),
            _ => Err("Unknown selection category".into()),
        };
        let chapter_title = if category == 3 {
            token
                .parse::<usize>()
                .ok()
                .and_then(|index| selection_state.lock().ok()?.chapters.get(index).cloned())
                .and_then(|chapter| chapter.title)
        } else {
            None
        };
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            match command.and_then(|command| selection_player.send(command)) {
                Err(error) => app.set_status(error.into()),
                Ok(()) => {
                    if let Some(title) = chapter_title {
                        let message = crate::video_osd::action_message(
                            crate::video_osd::ActionFeedback::Chapter(title),
                        );
                        show_osd_message(&app, message.text, false);
                    }
                }
            }
        }
    });
    let delay_player = player.command_sender();
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
    let playlist_player = player.command_sender();
    let playlist_pending_navigation = pending_navigation.clone();
    let playlist_capture_transition_first = capture_transition_first;
    let weak = app.as_weak();
    app.on_playlist_action(move |action, token| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        if action == 12 {
            window.invoke_activity();
            let rows = window.get_playlist();
            let exists = (0..rows.row_count()).any(|index| {
                rows.row_data(index)
                    .is_some_and(|row| row.token.as_str() == token.as_str())
            });
            if !exists {
                window.set_status("The playlist item is no longer available".into());
                return;
            }
            match token.parse::<i64>() {
                Ok(id) => queue_playlist_navigation(
                    &playlist_player,
                    &playlist_pending_navigation,
                    Command::SelectPlaylistId(id),
                    playlist_capture_transition_first,
                ),
                Err(error) => {
                    window.set_status(format!("Invalid playlist item: {error}").into());
                }
            }
            return;
        }
        if matches!(action, 6..=11) {
            window.invoke_activity();
            let rows = window.get_playlist();
            let selected_index = (0..rows.row_count()).find(|index| {
                rows.row_data(*index)
                    .is_some_and(|row| row.token.as_str() == token.as_str())
            });
            let result = match action {
                6 | 7 => {
                    let paths = selected_playlist_paths(&window, token.as_str());
                    if paths.is_empty() {
                        Err("Select a playlist item first".into())
                    } else {
                        crate::platform::clipboard::copy_playlist_references(&paths).and_then(
                            |()| {
                                if action == 7 {
                                    selected_playlist_ids(&window, token.as_str()).and_then(|ids| {
                                        playlist_player.send(Command::RemovePlaylistIds(ids))
                                    })
                                } else {
                                    Ok(())
                                }
                            },
                        )
                    }
                }
                8 => crate::platform::clipboard::read_playlist_references().and_then(|paths| {
                    if paths.is_empty() {
                        return Err("Clipboard contains no playlist references".into());
                    }
                    let index = playlist_paste_index(&window, token.as_str());
                    let count = paths.len();
                    playlist_player
                        .send(Command::InsertPlaylistFiles { paths, index })
                        .map(|()| {
                            let message = crate::video_osd::action_message(
                                crate::video_osd::ActionFeedback::PlaylistAdded(count),
                            );
                            show_osd_message(&window, message.text, false);
                        })
                }),
                9 => selected_index
                    .and_then(|index| rows.row_data(index))
                    .ok_or_else(|| "Select a playlist item first".to_owned())
                    .and_then(|row| {
                        crate::media_open::open_in_new_player(&window, row.path.as_str())
                    }),
                10 => {
                    let urls = selected_playlist_paths(&window, token.as_str())
                        .into_iter()
                        .filter(|path| crate::platform::is_http_url(path))
                        .collect::<Vec<_>>();
                    if urls.is_empty() {
                        Err("Select an HTTP or HTTPS playlist entry first".into())
                    } else {
                        urls.iter().try_for_each(|url| {
                            crate::platform::open_http_url_in_browser(url)
                                .map_err(|error| error.to_string())
                        })
                    }
                }
                11 => {
                    let urls = selected_playlist_paths(&window, token.as_str())
                        .into_iter()
                        .filter(|path| crate::platform::is_http_url(path))
                        .collect::<Vec<_>>();
                    if urls.is_empty() {
                        Err("Select an HTTP or HTTPS playlist entry first".into())
                    } else {
                        crate::platform::clipboard::copy_text(&urls.join("\n"))
                    }
                }
                _ => unreachable!(),
            };
            if let Err(error) = result {
                window.set_status(error.into());
            } else if action == 7 {
                window.set_selected_playlist_token("".into());
                window.set_selected_playlist_tokens(slint::ModelRc::default());
                install_playlist_selection(&window, &[]);
            }
            return;
        }
        let command = match action {
            1 => selected_playlist_ids(&window, token.as_str()).map(Command::RemovePlaylistIds),
            2 | 3 => token
                .parse()
                .map(|id| Command::MovePlaylistId {
                    id,
                    direction: if action == 2 { -1 } else { 1 },
                })
                .map_err(|error: std::num::ParseIntError| error.to_string()),
            4 => Ok(Command::ClearPlaylist),
            5 => Ok(Command::CycleLoop),
            _ => Err("Unknown playlist action".into()),
        };
        window.invoke_activity();
        if let Err(error) = command.and_then(|command| playlist_player.send(command)) {
            window.set_status(error.into());
        } else if action == 4 {
            let message =
                crate::video_osd::action_message(crate::video_osd::ActionFeedback::PlaylistCleared);
            show_osd_message(&window, message.text, false);
        } else if action == 1 {
            window.set_selected_playlist_token("".into());
            window.set_selected_playlist_tokens(slint::ModelRc::default());
            install_playlist_selection(&window, &[]);
        }
    });
    let playlist_selection_anchor = Rc::new(RefCell::new(None::<String>));
    let selection_anchor = playlist_selection_anchor.clone();
    let weak = app.as_weak();
    app.on_select_playlist_row(move |index, extend, range| {
        let Some(app) = weak.upgrade() else {
            return slint::ModelRc::default();
        };
        let Ok(index) = usize::try_from(index) else {
            return slint::ModelRc::default();
        };
        let rows = app.get_playlist();
        let Some(clicked) = rows.row_data(index) else {
            return app.get_selected_playlist_tokens();
        };
        let clicked_token = clicked.token.to_string();
        let previous = app
            .get_selected_playlist_tokens()
            .iter()
            .map(|token| token.to_string())
            .collect::<Vec<_>>();
        let selected = if range {
            let anchor = selection_anchor
                .borrow()
                .as_deref()
                .and_then(|anchor| {
                    (0..rows.row_count()).find(|row| {
                        rows.row_data(*row)
                            .is_some_and(|entry| entry.token.as_str() == anchor)
                    })
                })
                .unwrap_or(index);
            let mut selected = if extend { previous } else { Vec::new() };
            for row in anchor.min(index)..=anchor.max(index) {
                if let Some(entry) = rows.row_data(row)
                    && !selected.iter().any(|token| token == entry.token.as_str())
                {
                    selected.push(entry.token.to_string());
                }
            }
            selected
        } else if extend {
            let mut selected = previous;
            if let Some(position) = selected.iter().position(|token| token == &clicked_token) {
                selected.remove(position);
                if selection_anchor.borrow().as_deref() == Some(clicked_token.as_str()) {
                    *selection_anchor.borrow_mut() = None;
                }
            } else {
                selected.push(clicked_token.clone());
                *selection_anchor.borrow_mut() = Some(clicked_token.clone());
            }
            selected
        } else {
            *selection_anchor.borrow_mut() = Some(clicked_token.clone());
            vec![clicked_token]
        };
        install_playlist_selection(&app, &selected);
        slint::ModelRc::new(slint::VecModel::from(
            selected.into_iter().map(Into::into).collect::<Vec<_>>(),
        ))
    });

    let reorder_player = player.command_sender();
    let weak = app.as_weak();
    app.on_reorder_playlist(move |tokens, insertion_slot| {
        let Some(app) = weak.upgrade() else { return };
        app.invoke_activity();
        let rows = app.get_playlist();
        let entries = (0..rows.row_count())
            .filter_map(|index| rows.row_data(index))
            .map(|row| row.token.parse::<i64>().ok())
            .collect::<Option<Vec<_>>>();
        let selected = tokens
            .iter()
            .map(|token| token.parse::<i64>().ok())
            .collect::<Option<Vec<_>>>();
        let plan = entries
            .as_deref()
            .zip(selected.as_deref())
            .and_then(|(entries, selected)| {
                usize::try_from(insertion_slot).ok().and_then(|slot| {
                    riina_core::playlist::reorder_selection(entries, selected, slot)
                })
            });
        let Some(plan) = plan else {
            app.set_status("The playlist drop position is no longer available".into());
            return;
        };
        for (from, to) in plan {
            if let Err(error) = reorder_player.send(Command::MovePlaylistEntry { from, to }) {
                app.set_status(error.into());
                break;
            }
        }
    });
    let shuffle_player = player.command_sender();
    let weak = app.as_weak();
    app.on_toggle_playlist_shuffle(move || {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            if let Err(error) = shuffle_player.send(Command::TogglePlaylistShuffle) {
                app.set_status(error.into());
            }
        }
    });
    let sort_player = player.command_sender();
    let weak = app.as_weak();
    app.on_sort_playlist(move |sort| {
        let sort = match sort {
            0 => Some(riina_core::playlist::PlaylistSort::FilenameAscending),
            1 => Some(riina_core::playlist::PlaylistSort::FilenameDescending),
            2 => Some(riina_core::playlist::PlaylistSort::PathAscending),
            3 => Some(riina_core::playlist::PlaylistSort::PathDescending),
            _ => None,
        };
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            let result = sort.map_or_else(
                || Err("Unknown playlist sort order".to_owned()),
                |sort| sort_player.send(Command::SortPlaylist(sort)),
            );
            if let Err(error) = result {
                app.set_status(error.into());
            }
        }
    });
    let browse_player = player.command_sender();
    let weak = app.as_weak();
    let browse_host = native.as_ref().map(platform::NativePresentation::host);
    app.on_browse_files(move |category| {
        let purpose = match category {
            1 => Purpose::Audio,
            2 => Purpose::Subtitle,
            4 => Purpose::Playlist,
            _ => return,
        };
        if let Some(app) = weak.upgrade() {
            ui_dialogs::open(
                &app,
                browse_host.clone(),
                purpose,
                browse_player.clone(),
                None,
            );
        }
    });
    for absolute in [true, false] {
        let player = player.command_sender();
        let weak = app.as_weak();
        let seek = move |seconds: f32| {
            if let Some(app) = weak.upgrade() {
                app.invoke_activity();
            }
            if let Err(error) = player.send(Command::Seek {
                seconds: seconds as f64,
                absolute,
            }) && let Some(app) = weak.upgrade()
            {
                app.set_status(error.into());
            }
        };
        if absolute {
            app.on_seek(seek);
        } else {
            app.on_seek_relative(seek);
        }
    }
    let weak = app.as_weak();
    app.on_open_jump_time_dialog(move || {
        if let Some(app) = weak.upgrade() {
            app.set_jump_time_draft(app.get_position_label());
            app.set_jump_time_error("".into());
            app.set_jump_time_dialog_visible(true);
        }
    });
    let weak = app.as_weak();
    app.on_open_url_dialog(move || {
        if let Some(app) = weak.upgrade() {
            app.set_url_dialog_draft("".into());
            app.set_url_dialog_visible(true);
        }
    });
    let jump_player = player.command_sender();
    let weak = app.as_weak();
    app.on_jump_to_time(move |input| {
        let Some(app) = weak.upgrade() else { return };
        let Some(seconds) = riina_core::time_input::parse_video_time(input.as_str()) else {
            app.set_jump_time_error("Enter a valid time such as 20:35 or 1:02:03.500.".into());
            return;
        };
        app.invoke_activity();
        match jump_player.send(Command::Seek {
            seconds,
            absolute: true,
        }) {
            Ok(()) => {
                app.set_jump_time_error("".into());
                app.set_jump_time_dialog_visible(false);
            }
            Err(error) => app.set_jump_time_error(error.into()),
        }
    });
    let boost_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_volume_boost(move |enabled| {
        if let Some(app) = weak.upgrade() {
            let volume = app.get_volume().min(if enabled { 200.0 } else { 100.0 });
            match boost_player.send(Command::SetVolumeBoost(enabled)) {
                Ok(()) => {
                    app.set_volume_boost(enabled);
                    app.set_volume(volume);
                }
                Err(error) => app.set_status(error.into()),
            }
            app.invoke_activity();
        }
    });
    let volume_player = player.command_sender();
    let weak = app.as_weak();
    app.on_set_volume(move |volume| {
        let Some(app) = weak.upgrade() else { return };
        app.invoke_activity();
        let limit = if app.get_volume_boost() { 200.0 } else { 100.0 };
        if let Err(error) = volume_player.send(Command::Volume((volume as f64).min(limit)))
            && let Some(app) = weak.upgrade()
        {
            app.set_status(error.into());
        }
    });
    let speed_player = player.command_sender();
    let weak = app.as_weak();
    app.on_speed(move |factor| {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
        }
        if let Err(error) = speed_player.send(Command::MultiplySpeed(factor as f64))
            && let Some(app) = weak.upgrade()
        {
            app.set_status(error.into());
        }
    });
    let adjustment_player = player.command_sender();
    let weak = app.as_weak();
    app.on_adjust_volume(move |delta| {
        let Some(app) = weak.upgrade() else { return };
        app.invoke_activity();
        if let Err(error) = adjustment_player.send(Command::AdjustVolume {
            delta: delta as f64,
            boost: app.get_volume_boost(),
        }) {
            app.set_status(error.into());
        }
    });
    window_actions::install_fullscreen(
        app,
        native.as_ref().map(platform::NativePresentation::host),
    );
    media_actions::install(
        app,
        media_actions::Context {
            player: &player,
            open_host: native.as_ref().map(platform::NativePresentation::host),
            welcome,
            recent_media: &recent_media,
            recent_open_documents: &recent_open_documents,
            recent_open_cleared: &recent_open_cleared,
            prefs_path: &prefs_path,
            latest_playback: &latest_playback,
        },
    );
    let weak = app.as_weak();
    app.window().on_close_requested(move || {
        if weak.upgrade().is_some_and(|app| app.get_dialog_open()) {
            slint::CloseRequestResponse::KeepWindowShown
        } else {
            if let Some(app) = weak.upgrade() {
                app.invoke_dismiss_font_picker();
                // Remove the borrowed texture before hide suspends its context.
                // Teardown runs inside the backend's window borrow and cannot
                // safely dirty a property that schedules another redraw.
                app.set_video_frame(slint::Image::default());
                app.set_engine_ready(false);
                app.invoke_close_player();
            }
            slint::CloseRequestResponse::HideWindow
        }
    });
    let minimize_window = app.as_weak();
    let hide_app = app.as_weak();
    app.on_hide_riina(move || {
        if let Some(app) = hide_app.upgrade()
            && let Err(error) = platform::hide_application(app.window())
        {
            app.set_status(error.into());
        }
    });
    app.on_minimize_window(move || {
        if let Some(app) = minimize_window.upgrade() {
            platform::minimize_window(app.window());
        }
    });
    let zoom_window = app.as_weak();
    app.on_zoom_window(move || {
        if let Some(app) = zoom_window.upgrade() {
            platform::toggle_zoom_window(app.window());
        }
    });
    let close_player = app.as_weak();
    app.on_close_player(move || {
        if let Some(app) = close_player.upgrade() {
            app.invoke_window_will_close();
            app.invoke_stop();
        }
        let _ = slint::quit_event_loop();
    });

    if let Some(welcome) = welcome {
        let weak = app.as_weak();
        welcome.on_open_file(move || {
            if let Some(app) = weak.upgrade() {
                app.invoke_open_file();
            }
        });
        let weak = app.as_weak();
        welcome.on_open_url(move |value| {
            if let Some(app) = weak.upgrade() {
                app.invoke_open_url(value);
            }
        });
        let weak = app.as_weak();
        welcome.on_open_recent(move |index| {
            if let Some(app) = weak.upgrade() {
                app.invoke_open_recent(index);
            }
        });
    }

    if prefs
        .audio_equalizer
        .gains_db
        .iter()
        .any(|gain| *gain != 0.0)
    {
        player
            .send(Command::AudioEqualizer(prefs.audio_equalizer.gains_db))
            .map_err(std::io::Error::other)?;
    }
    player
        .send(Command::SetVolumeBoost(prefs.volume_boost))
        .map_err(std::io::Error::other)?;
    player
        .send(Command::SetPauseWhenOpen(prefs.pause_when_open))
        .map_err(std::io::Error::other)?;
    player
        .send(Command::HardwareDecoding(prefs.hardware_decoder))
        .map_err(std::io::Error::other)?;
    player
        .send(Command::SubtitleAssOverride {
            lane: riina_core::subtitle_controls::SubtitleLane::Primary,
            level: prefs.subtitle_ass_override,
        })
        .map_err(std::io::Error::other)?;
    player
        .send(Command::SubtitleAssOverride {
            lane: riina_core::subtitle_controls::SubtitleLane::Secondary,
            level: prefs.secondary_subtitle_ass_override,
        })
        .map_err(std::io::Error::other)?;
    player
        .send(Command::SubtitleFont(prefs.subtitle_font.clone()))
        .map_err(std::io::Error::other)?;
    player
        .send(Command::SubtitleFontSize(prefs.subtitle_font_size))
        .map_err(std::io::Error::other)?;
    for command in crate::subtitle_preferences::commands(prefs.subtitle_text_style) {
        player.send(command).map_err(std::io::Error::other)?;
    }
    for command in prefs.subtitle_position.commands() {
        player.send(command).map_err(std::io::Error::other)?;
    }
    for command in prefs.subtitle_other.commands() {
        player.send(command).map_err(std::io::Error::other)?;
    }
    crate::fullscreen_info::connect(app);
    if native.is_some() {
        app.set_engine_ready(true);
        player
            .send(Command::Volume(prefs.volume))
            .map_err(std::io::Error::other)?;
        if !media.is_empty() {
            media_open::startup_at(app, media, startup_resume_position, player.command_sender());
        }
    } else {
        rendering::install(
            app,
            &player,
            media,
            startup_resume_position,
            prefs.volume,
            #[cfg(target_os = "macos")]
            picture_in_picture.clone(),
        )?;
    }
    Ok(UiSession {
        player: Some(player),
        #[cfg(target_os = "windows")]
        _plugin_overlay_presenter: plugin_overlay_presenter,
        #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
        _plugin_sidebar_presenter: plugin_sidebar_presenter,
        #[cfg(target_os = "macos")]
        _system_media_controls: system_media_controls,
        #[cfg(target_os = "macos")]
        _system_media_controls_timer: system_media_controls_timer,
        #[cfg(target_os = "macos")]
        _picture_in_picture: picture_in_picture,
        #[cfg(target_os = "macos")]
        _picture_in_picture_timer: picture_in_picture_timer,
        #[cfg(target_os = "macos")]
        _mac_scroll_event_monitor: mac_scroll_event_monitor,
        #[cfg(target_os = "macos")]
        _mac_plugin_menu_observer: mac_plugin_menu_observer,
        _presentation: native,
        _hdr_display_timer: hdr_display_timer,
        _live_text_timer: live_text_timer,
        _appearance_timer: appearance_timer,
        _pause_resume_timer: pause_resume_timer,
        _thumbnail_preview_timer: thumbnail_preview_timer,
        _transition_frame_timer: transition_frame_timer,
        _native_material_timer: native_material_timer,
        #[cfg(target_os = "windows")]
        _windows_materials_policy_timer: windows_materials_policy_timer,
        _subtitle_priority_timer: subtitle_priority_timer,
        _online_subtitles_window: online_subtitles_window,
        _online_subtitles_timer: online_subtitles_timer,
        _playback_history_window: playback_history_window,
        _playback_history_timer: playback_history_timer,
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
        preference_writer: crate::preference_store::Writer::new(prefs_path.clone(), prefs.clone()),
        prefs_path,
        prefs,
        audio_equalizer,
        saved_video_filters,
        saved_audio_filters,
        recent_media,
        recent_open_documents,
        recent_open_cleared,
        playback_history,
        record_playback_history,
        history_cleared,
        latest_playback,
    })
}

#[cfg(test)]
mod welcome_tests {
    use super::{
        recent_document_rows, recent_media_rows, resume_engine_options, resume_media_rows,
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
