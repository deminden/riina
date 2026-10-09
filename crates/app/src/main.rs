#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod file_dialog;
mod file_selection;
mod media_open;
mod native_window;
mod platform;
mod player_ui;
mod ui_dialogs;
use player_ui::configure_player;
use slint::ComponentHandle;
slint::include_modules!();

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let first = args.next();
    let native = first.as_deref() == Some(std::ffi::OsStr::new("--native-video"));
    let media = if native { args.next() } else { first };
    if native {
        return native_window::run(media);
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
