#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod advanced_preferences;
mod appearance_preferences;
mod audio_preferences;
mod codec_preferences;
mod control_preferences;
mod crop_editor;
mod file_selection;
mod font_catalog;
mod font_picker;
mod fullscreen_info;
mod key_binding_preferences;
pub mod key_binding_profiles;
mod language_names;
mod layout_preferences;
mod liquid_glass_preferences;
mod media_open;
mod network_cache_preferences;
mod network_connection_preferences;
mod network_ytdl_preferences;
mod opening_fullscreen;
mod osd_preferences;
mod pip_preferences;
mod platform;
mod player_ui;
mod plugin_packages;
mod plugin_preferences;
pub mod plugin_runtime;
mod screenshot_osd;
mod screenshot_output;
mod screenshot_preferences;
mod seek_osd;
mod settings_search;
mod sidebar_preferences;
mod subtitle_auto_load_preferences;
mod subtitle_other_preferences;
mod subtitle_position_preferences;
mod subtitle_preferences;
mod tone_mapping_preferences;
mod ui_dialogs;
mod utilities_preferences;
mod video_osd;
mod window_aspect;
mod window_controls;
mod window_resize_preferences;
#[cfg(target_os = "windows")]
use platform::file_dialog;
use player_ui::configure_player;
use slint::ComponentHandle;
slint::include_modules!();

fn main() -> Result<(), Box<dyn std::error::Error>> {
    prepare_online_video_path()?;
    let action_after_launch = player_ui::action_after_launch()?;
    let mut args = std::env::args_os().skip(1);
    let first = args.next();
    let shared_gl = first.as_deref() == Some(std::ffi::OsStr::new("--shared-gl"));
    let native_video = first.as_deref() == Some(std::ffi::OsStr::new("--native-video"));
    let media = if shared_gl || native_video {
        args.next()
    } else {
        first
    };
    let supplied_media_argument = media.is_some();
    #[cfg(target_os = "windows")]
    // Native D3D11 preserves the Windows HDR presentation path. Its control
    // overlay restores compositor alpha after Slint realizes the window.
    if !shared_gl {
        return platform::native_window::run(media);
    }
    #[cfg(not(target_os = "windows"))]
    if native_video {
        return Err("Native video presentation is currently available on Windows; use --shared-gl on this platform".into());
    }
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("femtovg".into())
        .select()?;
    let app = PlayerWindow::new()?;
    // This entry point renders video in the Slint window and needs its menu bar.
    // Native-video mode keeps the transparent surface menu-free at initialization.
    app.set_native_video(false);
    let welcome = if action_after_launch.show_welcome(supplied_media_argument) {
        Some(WelcomeWindow::new()?)
    } else {
        None
    };
    let session = configure_player(&app, None, media, welcome.as_ref())?;
    window_aspect::connect(&app);
    if let Some(welcome) = &welcome {
        welcome.show()?;
    } else {
        app.show()?;
        if action_after_launch.open_file_panel(supplied_media_argument) {
            app.invoke_open_file();
        }
    }
    slint::run_event_loop()?;
    session.save(&app)
}

fn prepare_online_video_path() -> Result<(), Box<dyn std::error::Error>> {
    let path = platform::preferences_path()?;
    let preferences = match std::fs::read_to_string(path) {
        Ok(text) => riina_core::Preferences::parse(&text).map_err(std::io::Error::other)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            riina_core::Preferences::default()
        }
        Err(error) => return Err(error.into()),
    };
    let executable = std::env::current_exe()?;
    let path = preferences
        .network_online_videos
        .process_search_path(&executable, std::env::var_os("PATH").as_deref())
        .map_err(std::io::Error::other)?;
    // This runs before Slint or the playback worker creates threads, so no thread
    // can observe a partially updated environment while mpv resolves yt-dlp.
    unsafe { std::env::set_var("PATH", path) };
    Ok(())
}
