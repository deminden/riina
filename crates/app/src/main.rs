#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod about_window;
mod advanced_preferences;
mod appearance_preferences;
mod assrt_subtitles;
mod audio_preferences;
mod auto_playlist;
mod codec_preferences;
mod control_preferences;
mod crop_editor;
mod file_actions;
mod file_selection;
mod file_types;
mod font_catalog;
mod font_picker;
mod fullscreen_info;
mod key_binding_preferences;
pub mod key_binding_profiles;
mod language_names;
mod language_token_preferences;
mod layout_preferences;
mod liquid_glass_preferences;
mod localized_number;
mod log_viewer;
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
mod playback_history_store;
mod playback_history_window;
mod player_manager;
mod player_ui;
mod playlist_export;
mod playlist_metadata;
mod plugin_files;
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
mod shooter_subtitles;
mod sidebar_preferences;
mod subtitle_auto_load_preferences;
mod subtitle_discovery;
mod subtitle_other_preferences;
mod subtitle_position_preferences;
mod subtitle_preferences;
mod subtitle_priority;
mod subtitle_series;
mod thumbnail_cache;
mod thumbnail_preferences;
mod thumbnail_preview;
mod tone_mapping_preferences;
mod transition_frame;
mod ui_dialogs;
mod ui_models;
mod utilities_preferences;
mod video_filter_window;
mod video_osd;
mod window_aspect;
mod window_controls;
mod window_resize_preferences;
#[cfg(target_os = "windows")]
use platform::file_dialog;
use player_ui::configure_player;
use slint::ComponentHandle;
slint::include_modules!();

struct StartupArguments {
    media: Vec<std::ffi::OsString>,
    resume_position: Option<f64>,
    owner_center: Option<(i32, i32)>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    prepare_online_video_path()?;
    let action_after_launch = player_ui::action_after_launch()?;
    let startup = take_startup_arguments(std::env::args_os().skip(1))?;
    let (_shared_gl, _native_video, media) = startup_arguments(startup.media.into_iter());
    let resume_position = startup.resume_position;
    let owner_center = startup.owner_center;
    let supplied_media_argument = !media.is_empty();
    #[cfg(target_os = "windows")]
    // Native D3D11 preserves the Windows HDR presentation path. Its control
    // overlay restores compositor alpha after Slint realizes the window.
    if !_shared_gl {
        return platform::native_window::run(media, resume_position, owner_center);
    }
    #[cfg(not(target_os = "windows"))]
    if _native_video {
        return Err("Native video presentation is currently available on Windows; use --shared-gl on this platform".into());
    }
    #[cfg(not(target_os = "windows"))]
    let _ = owner_center;
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
        platform::center_auxiliary_window(welcome, &app);
        welcome.show()?;
    } else {
        app.show()?;
        if action_after_launch.open_file_panel(supplied_media_argument) {
            app.invoke_open_file();
        }
        platform::center_player_window(&app, app.get_initial_window_position_enabled());
    }
    slint::run_event_loop()?;
    player_manager.save_all()
}

fn take_startup_arguments(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<StartupArguments, Box<dyn std::error::Error>> {
    // Keep internal launch hints separate from media references passed to a player.
    let mut args = args.peekable();
    let mut media = Vec::new();
    let mut resume_position = None;
    let mut owner_center = None;
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
        } else if argument == "--riina-owner-center" {
            let x = args.next().ok_or("Missing owner-center x coordinate")?;
            let y = args.next().ok_or("Missing owner-center y coordinate")?;
            owner_center = Some((
                x.to_str()
                    .ok_or("Owner-center x coordinate is not Unicode")?
                    .parse()?,
                y.to_str()
                    .ok_or("Owner-center y coordinate is not Unicode")?
                    .parse()?,
            ));
        } else {
            media.push(argument);
        }
    }
    Ok(StartupArguments {
        media,
        resume_position,
        owner_center,
    })
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
    use super::{startup_arguments, take_startup_arguments};
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

    #[test]
    fn child_player_keeps_owner_monitor_hint_out_of_media_arguments() {
        let arguments = take_startup_arguments(
            [
                "--riina-owner-center",
                "1440",
                "520",
                "--riina-resume-at",
                "3.5",
                "movie.mkv",
            ]
            .into_iter()
            .map(OsString::from),
        )
        .unwrap();

        assert_eq!(arguments.media, ["movie.mkv"].map(OsString::from));
        assert_eq!(arguments.resume_position, Some(3.5));
        assert_eq!(arguments.owner_center, Some((1440, 520)));
    }
}
