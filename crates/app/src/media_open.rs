//! Filesystem preparation never runs inside a rendering callback or GUI event.
use crate::{PlayerWindow, file_selection};
use riina_core::Command;
use riina_mpv::CommandSender;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::ffi::OsString;

fn apply_manual_open_repeat(app: &PlayerWindow, sender: &CommandSender) -> Result<(), String> {
    if let Some(command) =
        manual_open_repeat_command(app.get_auto_repeat(), app.get_default_repeat_mode_index())?
    {
        sender.send(command).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn manual_open_repeat_command(enabled: bool, mode_index: i32) -> Result<Option<Command>, String> {
    if !enabled {
        return Ok(None);
    }
    let mode = riina_core::DefaultRepeatMode::from_index(mode_index)
        .ok_or_else(|| "The saved default repeat mode is invalid".to_owned())?;
    Ok(Some(Command::SetLoopMode(mode.loop_mode())))
}

pub fn open_in_new_player(app: &PlayerWindow, path: &str) -> Result<(), String> {
    if path.is_empty() || path.contains('\0') {
        return Err("The media reference is invalid".into());
    }
    open_paths_in_new_player(app, &[path.to_owned()])
}

pub fn selection_in_new_player(app: &PlayerWindow, paths: Vec<String>) {
    if paths.is_empty() {
        return;
    }
    if paths
        .iter()
        .any(|path| path.is_empty() || path.contains('\0'))
    {
        app.set_status("The media selection contains an invalid reference".into());
        return;
    }
    app.invoke_activity();
    app.set_status("Preparing media for a new player…".into());
    let weak = app.as_weak();
    let failure = weak.clone();
    let started = std::thread::Builder::new()
        .name("new-player-media-discovery".into())
        .spawn(move || {
            let result = file_selection::prepare(paths, true);
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(app) = weak.upgrade()
                    && let Err(error) = result.and_then(|paths| {
                        if paths.is_empty() {
                            return Err("No playable files were found".into());
                        }
                        open_paths_in_new_player(&app, &paths)
                    })
                {
                    app.set_status(error.into());
                }
            });
        });
    if let Err(error) = started
        && let Some(app) = failure.upgrade()
    {
        app.set_status(error.to_string().into());
    }
}

fn open_paths_in_new_player(app: &PlayerWindow, paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    if app.get_in_process_player_manager() {
        app.invoke_open_player_requested(
            ModelRc::new(VecModel::from(
                paths
                    .iter()
                    .cloned()
                    .map(SharedString::from)
                    .collect::<Vec<_>>(),
            )),
            -1.0,
        );
    } else {
        spawn_player(app, paths, None)?;
    }
    app.set_status(if paths.len() == 1 {
        "Opened the media in a new player.".into()
    } else {
        format!(
            "Opened {} media files in a new player playlist.",
            paths.len()
        )
        .into()
    });
    Ok(())
}

fn spawn_player(
    app: &PlayerWindow,
    paths: &[String],
    resume_position: Option<f64>,
) -> Result<(), String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let mut process = std::process::Command::new(executable);
    if let Some((x, y)) = crate::platform::player_window_center(app.window()) {
        process
            .arg("--riina-owner-center")
            .arg(x.to_string())
            .arg(y.to_string());
    }
    if let Some(position) = resume_position.filter(|value| value.is_finite() && *value >= 0.0) {
        process.arg("--riina-resume-at").arg(position.to_string());
    }
    process
        .args(paths)
        .spawn()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

pub(crate) fn same_media(left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }
    if left.contains("://") || right.contains("://") {
        return false;
    }
    std::path::Path::new(left)
        .canonicalize()
        .ok()
        .zip(std::path::Path::new(right).canonicalize().ok())
        .is_some_and(|(left, right)| left == right)
}

fn select_open_paths(
    paths: &[String],
    existing_paths: &[String],
    allow_duplicate_players: bool,
    group_simultaneous_opens: bool,
) -> Vec<String> {
    let grouped_request = grouped_open_request(paths.len(), group_simultaneous_opens);
    let filter_duplicates = !allow_duplicate_players && !grouped_request;
    let mut selected = Vec::<String>::new();
    for path in paths {
        if filter_duplicates
            && (existing_paths.iter().any(|open| same_media(open, path))
                || selected.iter().any(|open| same_media(open, path)))
        {
            continue;
        }
        selected.push(path.clone());
    }
    selected
}

pub(crate) const fn grouped_open_request(
    path_count: usize,
    group_simultaneous_opens: bool,
) -> bool {
    group_simultaneous_opens && path_count > 1
}

fn open_in_separate_players(
    app: &PlayerWindow,
    paths: &[String],
    resume_position: Option<f64>,
) -> Result<bool, String> {
    if app.get_playlist().row_count() == 0 {
        return Ok(false);
    }
    let preferences_path =
        crate::platform::preferences_path().map_err(|error| error.to_string())?;
    let preferences = match std::fs::read_to_string(preferences_path) {
        Ok(text) => riina_core::Preferences::parse(&text).map_err(|error| error.to_string())?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            riina_core::Preferences::default()
        }
        Err(error) => return Err(error.to_string()),
    };
    if !preferences.allow_multiple_windows {
        return Ok(false);
    }

    let existing = app.get_playlist();
    let open_paths = (0..existing.row_count())
        .filter_map(|index| existing.row_data(index))
        .map(|row| row.path.to_string())
        .collect::<Vec<_>>();
    let grouped = grouped_open_request(
        paths.len(),
        preferences.group_simultaneous_opens_in_playlist,
    );
    let selected = select_open_paths(
        paths,
        &open_paths,
        preferences.allow_duplicate_players,
        preferences.group_simultaneous_opens_in_playlist,
    );
    if selected.is_empty() {
        app.set_status("The selected media is already open.".into());
        return Ok(true);
    }

    let groups = if grouped {
        vec![selected]
    } else {
        selected.into_iter().map(|path| vec![path]).collect()
    };
    for group in &groups {
        if app.get_in_process_player_manager() {
            let position = if groups.len() == 1 {
                resume_position
            } else {
                None
            };
            app.invoke_open_player_requested(
                ModelRc::new(VecModel::from(
                    group
                        .iter()
                        .cloned()
                        .map(SharedString::from)
                        .collect::<Vec<_>>(),
                )),
                position.map_or(-1.0, |value| value as f32),
            );
        } else {
            spawn_player(
                app,
                group,
                if groups.len() == 1 {
                    resume_position
                } else {
                    None
                },
            )?;
        }
    }
    app.set_status(if grouped {
        "Opened the media in a new player playlist.".into()
    } else if groups.len() == 1 {
        "Opened the media in a new player.".into()
    } else {
        format!("Opened {} new players.", groups.len()).into()
    });
    Ok(true)
}

pub fn dropped(app: &PlayerWindow, paths: Vec<OsString>, sender: CommandSender) {
    startup_selection(app, paths, None, sender, true);
}

pub fn startup_at(
    app: &PlayerWindow,
    paths: Vec<OsString>,
    resume_position: Option<f64>,
    sender: CommandSender,
) {
    startup_selection(app, paths, resume_position, sender, false);
}

fn startup_selection(
    app: &PlayerWindow,
    paths: Vec<OsString>,
    resume_position: Option<f64>,
    sender: CommandSender,
    subtitle_fallback: bool,
) {
    let paths = paths
        .into_iter()
        .map(OsString::into_string)
        .collect::<Result<Vec<_>, _>>();
    match paths {
        Ok(paths) => selection_at(
            app,
            paths,
            false,
            resume_position,
            sender,
            subtitle_fallback,
        ),
        Err(_) => app.set_status("A media path contains invalid Unicode".into()),
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
    match open_in_separate_players(app, &[value.to_owned()], None) {
        Ok(true) => return,
        Ok(false) => {}
        Err(error) => {
            app.set_status(error.into());
            return;
        }
    }
    invalidate(app);
    app.set_status("Opening…".into());
    if let Err(error) = sender.send(Command::Load(value.to_owned())) {
        app.set_status(error.into());
    } else if let Err(error) = apply_manual_open_repeat(app, &sender) {
        app.set_status(error.into());
    }
}

pub fn resume(app: &PlayerWindow, media: riina_core::RecentMedia, sender: CommandSender) {
    match open_in_separate_players(app, std::slice::from_ref(&media.path), Some(media.position)) {
        Ok(true) => return,
        Ok(false) => {}
        Err(error) => {
            app.set_status(error.into());
            return;
        }
    }
    invalidate(app);
    app.set_status("Resuming…".into());
    if let Err(error) = sender.send(Command::LoadAtPosition {
        path: media.path,
        position: media.position,
    }) {
        app.set_status(error.into());
    } else if let Err(error) = apply_manual_open_repeat(app, &sender) {
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
    match open_in_separate_players(app, std::slice::from_ref(&media.path), None) {
        Ok(true) => return,
        Ok(false) => {}
        Err(error) => {
            app.set_status(error.into());
            return;
        }
    }
    invalidate(app);
    app.set_status("Opening…".into());
    if let Err(error) = sender.send(Command::Load(media.path)) {
        app.set_status(error.into());
    } else if let Err(error) = apply_manual_open_repeat(app, &sender) {
        app.set_status(error.into());
    }
}

pub fn validate_url(value: &str) -> Result<&str, &'static str> {
    riina_core::media_reference::validate_network_url(value)
}

pub fn invalidate(app: &PlayerWindow) -> i32 {
    let generation = app.get_media_request_generation().wrapping_add(1);
    app.set_media_request_generation(generation);
    generation
}

pub fn selection(app: &PlayerWindow, paths: Vec<String>, append: bool, sender: CommandSender) {
    selection_at(app, paths, append, None, sender, false);
}

fn selection_at(
    app: &PlayerWindow,
    paths: Vec<String>,
    append: bool,
    resume_position: Option<f64>,
    sender: CommandSender,
    subtitle_fallback: bool,
) {
    if paths.is_empty() {
        return;
    }
    let generation = invalidate(app);
    let weak = app.as_weak();
    let failure = weak.clone();
    let original_paths = paths.clone();
    app.set_status("Opening…".into());
    let started = std::thread::Builder::new()
        .name("media-discovery".into())
        .spawn(move || {
            let subtitles = if subtitle_fallback && !append {
                original_paths
                    .iter()
                    .map(|path| crate::platform::resolve_alias_path(path))
                    .filter(|path| riina_core::file_selection::is_supported_subtitle(path))
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            let result = (if subtitle_fallback {
                resolve_dropped_url_shortcuts(paths)
            } else {
                Ok(paths)
            })
            .and_then(|paths| file_selection::prepare(paths, !append))
            .and_then(|paths| {
                if paths.is_empty() {
                    if !subtitles.is_empty() {
                        Ok(PreparedSelection::Subtitles(subtitles))
                    } else {
                        Err("No playable files were found".into())
                    }
                } else if let Some(position) = resume_position
                    && position.is_finite()
                    && position >= 0.0
                    && !append
                    && paths.len() == 1
                {
                    Ok(PreparedSelection::Command(Command::LoadAtPosition {
                        path: paths[0].clone(),
                        position,
                    }))
                } else {
                    let command = if append {
                        Command::AppendFiles(paths)
                    } else {
                        Command::OpenFiles(paths)
                    };
                    if !subtitles.is_empty() {
                        let paths = match command {
                            Command::OpenFiles(paths) => paths,
                            _ => {
                                return Err(
                                    "Dropped subtitles require an opened media batch".into()
                                );
                            }
                        };
                        Ok(PreparedSelection::CommandWithSubtitles(paths, subtitles))
                    } else {
                        Ok(PreparedSelection::Command(command))
                    }
                }
            });
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(app) = weak.upgrade()
                    // A newer accepted selection or Stop owns the player now.
                    && app.get_media_request_generation() == generation
                    && let Err(error) = result.and_then(|prepared| match prepared {
                        PreparedSelection::Subtitles(paths) => {
                            let count = paths.len();
                            for path in paths {
                                sender.send(Command::AddSubtitle(path))?;
                            }
                            app.set_status(if count == 1 {
                                "Added dropped subtitle file".into()
                            } else {
                                format!("Added {count} dropped subtitle files").into()
                            });
                            Ok(())
                        }
                        PreparedSelection::Command(Command::OpenFiles(paths)) if !append => {
                            if open_in_separate_players(&app, &paths, resume_position)? {
                                Ok(())
                            } else {
                                sender.send(Command::OpenFiles(paths))?;
                                apply_manual_open_repeat(&app, &sender)
                            }
                        }
                        PreparedSelection::CommandWithSubtitles(paths, subtitles) => {
                            // A dropped subtitle belongs to the dropped media batch. Keep
                            // both in the current player rather than opening media in a
                            // separate window and attaching subtitles to the wrong player.
                            sender.send(Command::OpenFilesWithSubtitles { paths, subtitles })?;
                            apply_manual_open_repeat(&app, &sender)
                        }
                        PreparedSelection::Command(command) => {
                            let added = match &command {
                                Command::AppendFiles(paths) => Some(paths.len()),
                                _ => None,
                            };
                            sender.send(command)?;
                            if added.is_none() && !append {
                                apply_manual_open_repeat(&app, &sender)?;
                            }
                            if let Some(count) = added {
                                crate::player_ui::show_osd_message(
                                    &app,
                                    crate::video_osd::action_message(
                                        crate::video_osd::ActionFeedback::PlaylistAdded(count),
                                    )
                                    .text,
                                    false,
                                );
                            }
                            Ok(())
                        }
                    })
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

enum PreparedSelection {
    Command(Command),
    CommandWithSubtitles(Vec<String>, Vec<String>),
    Subtitles(Vec<String>),
}

fn resolve_dropped_url_shortcuts(paths: Vec<String>) -> Result<Vec<String>, String> {
    paths
        .into_iter()
        .map(|path| {
            if riina_core::media_reference::is_network_url_candidate(&path)
                || !std::path::Path::new(&path)
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("url"))
            {
                return Ok(path);
            }
            let metadata = std::fs::metadata(&path).map_err(|error| error.to_string())?;
            if !metadata.is_file() || metadata.len() > 16 * 1024 {
                return Err("The dropped internet shortcut is not a small regular file".into());
            }
            let contents = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
            parse_internet_shortcut(&contents).map(str::to_owned)
        })
        .collect()
}

fn parse_internet_shortcut(contents: &str) -> Result<&str, String> {
    let mut in_internet_shortcut = false;
    let mut url = None;
    for line in contents.lines().map(str::trim) {
        if line.starts_with('[') && line.ends_with(']') {
            in_internet_shortcut = line.eq_ignore_ascii_case("[InternetShortcut]");
            continue;
        }
        if in_internet_shortcut
            && let Some((key, value)) = line.split_once('=')
            && key.trim().eq_ignore_ascii_case("URL")
        {
            url = Some(value.trim());
            break;
        }
    }
    let url = url.ok_or_else(|| "The dropped internet shortcut has no URL".to_owned())?;
    validate_url(url).map_err(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::manual_open_repeat_command;
    use super::{parse_internet_shortcut, select_open_paths, validate_url};
    use riina_core::{Command, LoopMode};

    #[test]
    fn manual_open_repeat_is_opt_in() {
        assert_eq!(manual_open_repeat_command(false, 0), Ok(None));
    }

    #[test]
    fn manual_open_repeat_maps_the_selected_loop_mode() {
        assert_eq!(
            manual_open_repeat_command(true, 0),
            Ok(Some(Command::SetLoopMode(LoopMode::Playlist)))
        );
        assert_eq!(
            manual_open_repeat_command(true, 1),
            Ok(Some(Command::SetLoopMode(LoopMode::File)))
        );
    }

    #[test]
    fn invalid_manual_repeat_mode_is_rejected_only_when_enabled() {
        assert!(manual_open_repeat_command(false, 99).is_ok());
        assert!(manual_open_repeat_command(true, 99).is_err());
    }

    #[test]
    fn internet_shortcuts_resolve_only_validated_network_urls() {
        assert_eq!(
            parse_internet_shortcut(
                "[Other]\nURL=https://ignored.test/\n[InternetShortcut]\nURL=RTSP://camera.test/live\n"
            )
            .unwrap(),
            "RTSP://camera.test/live"
        );
        for contents in [
            "URL=https://outside-section.test/\n",
            "[InternetShortcut]\nURL=file:///secret\n",
            "[InternetShortcut]\nURL=https://bad host/\n",
            "[InternetShortcut]\nIconFile=shortcut.ico\n",
        ] {
            assert!(parse_internet_shortcut(contents).is_err());
        }
    }

    #[test]
    fn grouped_simultaneous_opens_preserve_items_even_when_duplicates_are_disabled() {
        let existing = vec!["already-open.mkv".into()];
        let incoming = vec![
            "already-open.mkv".into(),
            "selected.mkv".into(),
            "selected.mkv".into(),
        ];

        assert_eq!(
            select_open_paths(&incoming, &existing, false, true),
            incoming
        );
    }

    #[test]
    fn separate_player_opens_filter_existing_and_repeated_media() {
        let existing = vec!["already-open.mkv".into()];
        let incoming = vec![
            "already-open.mkv".into(),
            "selected.mkv".into(),
            "selected.mkv".into(),
        ];

        assert_eq!(
            select_open_paths(&incoming, &existing, false, false),
            ["selected.mkv"]
        );
    }

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
