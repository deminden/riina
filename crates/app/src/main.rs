#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod codec_preferences;
mod crop_editor;
mod file_selection;
mod font_catalog;
mod font_picker;
mod language_names;
mod layout_preferences;
mod media_open;
mod network_cache_preferences;
mod network_connection_preferences;
mod network_ytdl_preferences;
mod opening_fullscreen;
mod platform;
mod player_ui;
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
mod ui_dialogs;
mod video_osd;
mod window_controls;
#[cfg(target_os = "windows")]
use platform::file_dialog;
use player_ui::configure_player;
use slint::ComponentHandle;
slint::include_modules!();

fn main() -> Result<(), Box<dyn std::error::Error>> {
    prepare_online_video_path()?;
    let mut args = std::env::args_os().skip(1);
    let first = args.next();
    let native = first.as_deref() == Some(std::ffi::OsStr::new("--native-video"));
    let media = if native { args.next() } else { first };
    if native {
        #[cfg(target_os = "windows")]
        return platform::native_window::run(media);
        #[cfg(not(target_os = "windows"))]
        return Err("Native video presentation is currently available on Windows; run without --native-video for shared OpenGL playback".into());
    }
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("femtovg".into())
        .select()?;
    let app = PlayerWindow::new()?;
    let welcome = if media.is_none() {
        Some(WelcomeWindow::new()?)
    } else {
        None
    };
    let session = configure_player(&app, None, media, welcome.as_ref())?;
    if let Some(welcome) = &welcome {
        welcome.show()?;
    } else {
        app.show()?;
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
