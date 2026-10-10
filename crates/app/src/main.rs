#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod crop_editor;
mod file_selection;
mod font_catalog;
mod font_picker;
mod language_names;
mod media_open;
mod platform;
mod player_ui;
mod screenshot_osd;
mod seek_osd;
mod ui_dialogs;
mod video_osd;
mod window_controls;
#[cfg(target_os = "windows")]
use platform::file_dialog;
use player_ui::configure_player;
use slint::ComponentHandle;
slint::include_modules!();

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    let session = match configure_player(&app, None, media) {
        Ok(session) => session,
        Err(error) => {
            app.set_status(error.to_string().into());
            return app.run().map_err(Into::into);
        }
    };
    app.run()?;
    session.save(&app)
}
