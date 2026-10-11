//! Filesystem preparation never runs inside a rendering callback or GUI event.
use crate::{PlayerWindow, file_selection};
use riina_core::Command;
use riina_mpv::CommandSender;
use slint::ComponentHandle;
use std::ffi::OsString;

pub fn open_in_new_player(path: &str) -> Result<(), String> {
    if path.is_empty() || path.contains('\0') {
        return Err("The media reference is invalid".into());
    }
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    // Pass the media reference as one OS argument so spaces and Unicode stay intact.
    std::process::Command::new(executable)
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

pub fn startup(app: &PlayerWindow, path: OsString, sender: CommandSender) {
    match path.into_string() {
        Ok(path) => selection(app, vec![path], false, sender),
        Err(_) => app.set_status("The path contains invalid Unicode".into()),
    }
}

pub fn url(app: &PlayerWindow, value: &str, sender: CommandSender) {
    let value = match validate_url(value) {
        Ok(value) => value,
        Err(error) => {
            app.set_status(error.into());
            return;
        }
    };
    invalidate(app);
    app.set_status("Opening…".into());
    if let Err(error) = sender.send(Command::Load(value.to_owned())) {
        app.set_status(error.into());
    }
}

pub fn resume(app: &PlayerWindow, media: riina_core::RecentMedia, sender: CommandSender) {
    invalidate(app);
    app.set_status("Resuming…".into());
    if let Err(error) = sender.send(Command::LoadAtPosition {
        path: media.path,
        position: media.position,
    }) {
        app.set_status(error.into());
    }
}

pub fn open_recent(
    app: &PlayerWindow,
    media: riina_core::RecentMedia,
    resume_last_position: bool,
    sender: CommandSender,
) {
    if resume_last_position {
        resume(app, media, sender);
        return;
    }
    invalidate(app);
    app.set_status("Opening…".into());
    if let Err(error) = sender.send(Command::Load(media.path)) {
        app.set_status(error.into());
    }
}

pub fn validate_url(value: &str) -> Result<&str, &'static str> {
    let value = value.trim();
    let Some((scheme, remainder)) = value.split_once("://") else {
        return Err("Enter a complete http, https, rtsp or rtsps URL");
    };
    let host = remainder.split(['/', '?', '#']).next().unwrap_or_default();
    let supported_scheme = matches!(
        scheme.to_ascii_lowercase().as_str(),
        "http" | "https" | "rtsp" | "rtsps"
    );
    if value.contains('\0')
        || value.chars().any(char::is_whitespace)
        || host.is_empty()
        || !supported_scheme
    {
        return Err("Enter a valid http, https, rtsp or rtsps URL");
    }
    Ok(value)
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

#[cfg(test)]
mod tests {
    use super::validate_url;

    #[test]
    fn accepts_only_absolute_network_media_urls() {
        assert_eq!(
            validate_url(" https://example.test/movie.mkv ").unwrap(),
            "https://example.test/movie.mkv"
        );
        assert_eq!(
            validate_url("RTSP://camera.test/live").unwrap(),
            "RTSP://camera.test/live"
        );
        for rejected in [
            "",
            "movie.mkv",
            "file:///movie.mkv",
            "https://",
            "https://bad host/x",
            "https://x\0y",
        ] {
            assert!(validate_url(rejected).is_err(), "accepted {rejected:?}");
        }
    }
}
