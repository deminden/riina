//! Application orchestration for the Riina executable.

mod background;
mod browser_links;
mod debug_dump;
mod diagnostics;
mod file_actions;
mod file_selection;
mod file_types;
mod font_catalog;
mod http;
mod localized_number;
mod platform;
mod playback_history;
mod playback_history_store;
mod playback_power;
mod player;
mod plugins;
mod preferences;
mod series_names;
mod settings_search;
mod startup_arguments;
mod subtitles;
mod ui_dialogs;
mod ui_models;
mod updates;
mod windows;
#[doc(hidden)]
pub use player::automatic_music_mode;
use player::{
    auto_playlist, fullscreen_info, media_open, music_mode_layout, opening_fullscreen,
    player_manager, player_ui, playlist_export, playlist_metadata, screenshot_osd,
    screenshot_output, seek_osd, thumbnail_cache, thumbnail_preview, transition_frame, video_blur,
    video_osd, window_aspect, window_controls,
};
use player_ui::{InitialMedia, configure_player};
use plugins::{
    plugin_files, plugin_github, plugin_host, plugin_http, plugin_packages, plugin_runtime,
    plugin_snapshot,
};
use preferences::{
    advanced_preferences, appearance_preferences, audio_preferences, codec_preferences,
    control_preferences, key_binding_preferences, key_binding_profiles, language_token_preferences,
    layout_preferences, liquid_glass_preferences, network_cache_preferences,
    network_connection_preferences, network_ytdl_preferences, online_subtitles_preferences,
    osc_preferences, osd_preferences, pip_preferences, plugin_preferences, preference_load,
    preference_store, preferences_window, screenshot_preferences, sidebar_preferences,
    subtitle_auto_load_preferences, subtitle_other_preferences, subtitle_position_preferences,
    subtitle_preferences, thumbnail_preferences, tone_mapping_preferences, utilities_preferences,
    window_resize_preferences,
};
use riina_ui::*;
use slint::ComponentHandle;
use startup_arguments::StartupArguments;
use subtitles::{
    assrt_subtitles, online_subtitles_credentials, open_subtitles, shooter_subtitles,
    subtitle_discovery, subtitle_priority, subtitle_series,
};
use windows::{
    about_window, crop_editor, font_picker, log_viewer, playback_history_window,
    video_filter_window,
};

type PlaybackStore = std::sync::Arc<std::sync::Mutex<std::sync::Arc<riina_core::Playback>>>;

/// Start the application using the process command line.
///
/// # Safety
/// Call at most once on the initial thread before starting any other threads.
/// Startup updates the process environment before initializing the UI or engine.
pub unsafe fn run_from_command_line() -> std::process::ExitCode {
    diagnostics::install_panic_hook();
    riina_mpv::set_diagnostic_sink(|message| {
        diagnostics::log(diagnostics::Level::Info, "mpv", message);
    });
    // SAFETY: the caller establishes single-threaded process startup.
    let result = unsafe { run() };
    background::report_running();
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            diagnostics::error!("Could not start or run Riina: {error}");
            platform::show_fatal_error("Riina could not continue", &error.to_string());
            std::process::ExitCode::FAILURE
        }
    }
}

unsafe fn run() -> Result<(), Box<dyn std::error::Error>> {
    let preferences_path = platform::preferences_path()?;
    if let Some(profile) = preferences_path.parent()
        && let Err(error) = diagnostics::init(profile)
    {
        diagnostics::warn!("Could not open the profile log: {error}");
    }
    platform::initialize_recent_documents();
    let loaded = preference_load::load(&preferences_path)?;
    // SAFETY: the application is still in single-threaded process startup.
    unsafe { prepare_online_video_path(&loaded.preferences) }?;
    let (arguments, browser_link) = StartupArguments::invocation(std::env::args_os().skip(1))?;
    let activation = match platform::browser_activation::Activation::prepare(
        &preferences_path,
        browser_link.as_deref(),
    ) {
        Ok(Some(activation)) => activation,
        Ok(None) => return Ok(()),
        Err(error) if browser_link.is_none() => {
            diagnostics::warn!("Browser links are unavailable for this application: {error}");
            Default::default()
        }
        Err(error) => return Err(error.into()),
    };
    platform::startup::run_player(arguments, loaded, &activation.inbox)
}

fn run_shared_player(
    mut players: impl Iterator<Item = InitialMedia>,
    loaded: preference_load::LoadedPreferences,
    browser: &browser_links::Inbox,
) -> Result<(), Box<dyn std::error::Error>> {
    let action_after_launch = loaded.preferences.action_after_launch;
    let media = players.next().ok_or("Missing initial player")?;
    let supplied_media_argument = !media.files.is_empty();
    platform::startup::select_shared_backend(&loaded.preferences)?;
    let app = PlayerWindow::new()?;
    let player_manager = player_manager::PlayerManager::new();
    // This entry point renders video in the Slint window and needs its menu bar.
    // Native-video mode keeps the transparent surface menu-free at initialization.
    app.set_native_video(false);
    let welcome = if action_after_launch.show_welcome(supplied_media_argument) {
        Some(WelcomeWindow::new()?)
    } else {
        None
    };
    if let Some(welcome) = welcome.as_ref() {
        crate::appearance_preferences::install_system_typography(welcome);
    }
    let media_geometry = window_aspect::MediaGeometry::default();
    let session = configure_player(
        &app,
        None,
        media,
        welcome.as_ref(),
        loaded.clone(),
        media_geometry,
        player_manager.services(),
    )?;
    player_manager.attach(
        &app,
        welcome.as_ref().map(|window| window.clone_strong()),
        session,
    );
    let application_menu = platform::startup::ApplicationMenu::new();
    if let Some(welcome) = &welcome {
        platform::center_auxiliary_window(welcome, &app);
        platform::show_window(welcome)?;
        application_menu.normalize();
    } else {
        platform::show_window(&app)?;
        application_menu.normalize();
        if action_after_launch.open_file_panel(supplied_media_argument) {
            app.global::<PlayerMenuActions>().invoke_open_file();
        }
        platform::center_player_window(
            &app,
            app.global::<riina_ui::PlayerPreferences>()
                .get_initial_window_position_enabled(),
        );
    }
    player_manager.open_startup_players(&app, players, &loaded)?;
    let _browser = player_manager.register_browser_links(browser);
    #[cfg(target_os = "macos")]
    let _browser_events = platform::browser_activation::BrowserEvents::new(browser.clone());
    slint::run_event_loop()?;
    player_manager.save_all()
}

unsafe fn prepare_online_video_path(
    preferences: &riina_core::Preferences,
) -> Result<(), Box<dyn std::error::Error>> {
    let executable = std::env::current_exe()?;
    let path = preferences
        .network_online_videos
        .process_search_path(&executable, std::env::var_os("PATH").as_deref())
        .map_err(std::io::Error::other)?;
    // SAFETY: main runs this before initializing Slint, loading libmpv or
    // starting workers that might read the shared process environment.
    unsafe { std::env::set_var("PATH", path) };
    Ok(())
}
