//! Filesystem preparation never runs inside a rendering callback or GUI event.
use crate::{PlayerWindow, file_selection};
use riina_core::Command;
use riina_mpv::CommandSender;
use slint::ComponentHandle;
use std::ffi::OsString;

pub fn startup(app: &PlayerWindow, path: OsString, sender: CommandSender) {
    match path.into_string() {
        Ok(path) => selection(app, vec![path], false, sender),
        Err(_) => app.set_status("The path contains invalid Unicode".into()),
    }
}

pub fn invalidate(app: &PlayerWindow) -> i32 {
    let generation = app.get_media_request_generation().wrapping_add(1);
    app.set_media_request_generation(generation);
    generation
}

pub fn selection(app: &PlayerWindow, paths: Vec<String>, append: bool, sender: CommandSender) {
    if paths.is_empty() {
        return;
    }
    let generation = invalidate(app);
    let weak = app.as_weak();
    let failure = weak.clone();
    app.set_status("Opening…".into());
    let started = std::thread::Builder::new()
        .name("media-discovery".into())
        .spawn(move || {
            let result = file_selection::prepare(paths, !append).and_then(|paths| {
                if paths.is_empty() {
                    Err("No playable files were found".into())
                } else {
                    Ok(if append {
                        Command::AppendFiles(paths)
                    } else {
                        Command::OpenFiles(paths)
                    })
                }
            });
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(app) = weak.upgrade()
                    // A newer accepted selection or Stop owns the player now.
                    && app.get_media_request_generation() == generation
                    && let Err(error) = result.and_then(|command| sender.send(command))
                {
                    app.set_status(error.into());
                }
            });
        });
    if let Err(error) = started
        && let Some(app) = failure.upgrade()
        && app.get_media_request_generation() == generation
    {
        app.set_status(error.to_string().into());
    }
}
