#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod advanced_preferences;
mod appearance_preferences;
mod audio_preferences;
mod auto_playlist;
mod codec_preferences;
mod control_preferences;
mod crop_editor;
mod file_selection;
mod file_types;
mod font_catalog;
mod font_picker;
mod fullscreen_info;
mod key_binding_preferences;
pub mod key_binding_profiles;
mod language_names;
mod layout_preferences;
mod liquid_glass_preferences;
mod media_open;
mod music_mode_layout;
mod network_cache_preferences;
mod network_connection_preferences;
mod network_ytdl_preferences;
mod online_subtitles_credentials;
mod online_subtitles_preferences;
mod open_subtitles;
mod opening_fullscreen;
mod osc_preferences;
mod osd_preferences;
mod pip_preferences;
mod platform;
mod player_manager;
mod player_ui;
mod plugin_github;
mod plugin_host;
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
mod subtitle_discovery;
mod subtitle_other_preferences;
mod subtitle_position_preferences;
mod subtitle_preferences;
mod subtitle_priority;
mod thumbnail_cache;
mod thumbnail_preferences;
mod thumbnail_preview;
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
    let (arguments, resume_position) = take_resume_argument(std::env::args_os().skip(1))?;
    let (_shared_gl, _native_video, media) = startup_arguments(arguments.into_iter());
    let supplied_media_argument = !media.is_empty();
    #[cfg(target_os = "windows")]
    // Native D3D11 preserves the Windows HDR presentation path. Its control
    // overlay restores compositor alpha after Slint realizes the window.
    if !_shared_gl {
        return platform::native_window::run(media, resume_position);
    }
    #[cfg(not(target_os = "windows"))]
    if _native_video {
        return Err("Native video presentation is currently available on Windows; use --shared-gl on this platform".into());
    }
    let backend = slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("femtovg".into());
    #[cfg(target_os = "macos")]
    let backend = {
        use slint::winit_030::winit::platform::macos::WindowAttributesExtMacOS;

        let accepts_first_mouse = player_ui::accepts_first_mouse_at_launch()?;
        backend.with_winit_window_attributes_hook(move |attributes| {
            attributes.with_accepts_first_mouse(accepts_first_mouse)
        })
    };
    backend.select()?;
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
    let session = configure_player(&app, None, media, welcome.as_ref(), resume_position)?;
    player_manager.attach(
        &app,
        welcome.as_ref().map(|window| window.clone_strong()),
        session,
    );
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
    player_manager.save_all()
}

fn take_resume_argument(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(Vec<std::ffi::OsString>, Option<f64>), Box<dyn std::error::Error>> {
    // Keep the resume position separate from the media reference passed to a new player.
    let mut args = args.peekable();
    let mut media = Vec::new();
    let mut resume_position = None;
    while let Some(argument) = args.next() {
        if argument == "--riina-resume-at" {
            let value = args.next().ok_or("Missing resume position")?;
            let value = value
                .to_str()
                .ok_or("Resume position is not valid Unicode")?
                .parse::<f64>()?;
            if !value.is_finite() || value < 0.0 {
                return Err("Resume position must be a finite non-negative number".into());
            }
            resume_position = Some(value);
        } else {
            media.push(argument);
        }
    }
    Ok((media, resume_position))
}

fn startup_arguments(
    mut args: impl Iterator<Item = std::ffi::OsString>,
) -> (bool, bool, Vec<std::ffi::OsString>) {
    let first = args.next();
    let shared_gl = first.as_deref() == Some(std::ffi::OsStr::new("--shared-gl"));
    let native_video = first.as_deref() == Some(std::ffi::OsStr::new("--native-video"));
    let media = if shared_gl || native_video {
        args.collect()
    } else {
        first.into_iter().chain(args).collect()
    };
    (shared_gl, native_video, media)
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

#[cfg(test)]
mod tests {
    use super::startup_arguments;
    use std::ffi::OsString;

    #[test]
    fn startup_keeps_every_media_argument_after_an_internal_renderer_switch() {
        let (shared_gl, native_video, media) = startup_arguments(
            ["--shared-gl", "first file.mkv", "second.mkv"]
                .into_iter()
                .map(OsString::from),
        );

        assert!(shared_gl);
        assert!(!native_video);
        assert_eq!(media, ["first file.mkv", "second.mkv"].map(OsString::from));
    }

    #[test]
    fn startup_treats_all_arguments_as_media_without_a_renderer_switch() {
        let (shared_gl, native_video, media) =
            startup_arguments(["first.mkv", "second.mkv"].into_iter().map(OsString::from));

        assert!(!shared_gl);
        assert!(!native_video);
        assert_eq!(media, ["first.mkv", "second.mkv"].map(OsString::from));
    }
}
