//! Resolve the visible insertion gap before directory discovery and command dispatch.
use crate::{PlayerWindow, background, file_selection, media_open, platform, video_osd};
use riina_core::Command;
use riina_mpv::CommandSender;
use riina_ui::{PlayerCatalogState, PlayerSidebarState};
use slint::{ComponentHandle, Model};
use std::ffi::OsString;

pub(super) enum Target {
    Player,
    Playlist {
        before_id: Option<i64>,
        generation: i32,
    },
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub(super) fn register_desktop(
    app: &PlayerWindow,
    sender: &CommandSender,
    hover: std::rc::Rc<super::playlist_hover::PlaylistHover>,
) -> platform::desktop_drop::Registration {
    let weak = app.as_weak();
    let sender = sender.clone();
    platform::desktop_drop::register(
        app,
        move |event| hover.on_desktop(event),
        move |position| {
            let app = weak.upgrade()?;
            let target = target_at(&app, position);
            let generation = app.get_media_request_generation();
            let weak = weak.clone();
            let sender = sender.clone();
            Some(Box::new(move |paths| {
                if let Some(app) = weak.upgrade()
                    && app.window().is_visible()
                    && app.get_media_request_generation() == generation
                {
                    handle(&app, &sender, target, paths);
                }
            }))
        },
    )
}

pub(super) fn target(app: &PlayerWindow, window: &slint::Window) -> Result<Target, String> {
    let Some(position) = platform::file_drop_position(window) else {
        return Ok(Target::Player);
    };
    target_at(app, position)
}

pub(super) fn target_at(
    app: &PlayerWindow,
    position: slint::LogicalPosition,
) -> Result<Target, String> {
    let sidebar = app.global::<PlayerSidebarState>();
    let Ok(index) = usize::try_from(sidebar.invoke_playlist_drop_slot(position.x, position.y))
    else {
        return Ok(Target::Player);
    };
    let rows = app.global::<PlayerCatalogState>().get_playlist();
    let before_id = if index == rows.row_count() {
        None
    } else {
        Some(
            rows.row_data(index)
                .and_then(|row| row.token.parse().ok())
                .ok_or("The playlist drop position is no longer available")?,
        )
    };
    Ok(Target::Playlist {
        before_id,
        generation: app.get_media_request_generation(),
    })
}

pub(super) fn handle(
    app: &PlayerWindow,
    sender: &CommandSender,
    target: Result<Target, String>,
    files: Vec<OsString>,
) {
    let (before_id, generation) = match target {
        Ok(Target::Player) => {
            media_open::dropped(app, files, sender.clone());
            return;
        }
        Ok(Target::Playlist {
            before_id,
            generation,
        }) => (before_id, generation),
        Err(error) => {
            app.set_status(error.into());
            return;
        }
    };
    insert_prepared(app, sender, before_id, generation, move || {
        files
            .into_iter()
            .map(OsString::into_string)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "A media path contains invalid Unicode".to_owned())
            .and_then(prepare)
    });
}

pub(super) fn paste(
    app: &PlayerWindow,
    sender: &CommandSender,
    before_id: Option<i64>,
    paths: Vec<String>,
) {
    insert_prepared(
        app,
        sender,
        before_id,
        app.get_media_request_generation(),
        move || prepare(platform::clipboard::resolve_playlist_paste_paths(paths)),
    );
}

fn insert_prepared(
    app: &PlayerWindow,
    sender: &CommandSender,
    before_id: Option<i64>,
    generation: i32,
    prepare_paths: impl FnOnce() -> Result<Vec<String>, String> + Send + 'static,
) {
    let window = app.as_weak();
    let sender = sender.clone();
    let started = background::spawn_named("riina-playlist-insertion", move || {
        let result = prepare_paths();
        let _ = background::to_ui(window, move |app| {
            if app.get_media_request_generation() != generation {
                return;
            }
            let added = result.and_then(|paths| {
                let count = paths.len();
                sender.send(Command::InsertPlaylistFilesBefore { paths, before_id })?;
                Ok(count)
            });
            match added {
                Ok(count) => super::show_osd_message(
                    &app,
                    video_osd::action_message(video_osd::ActionFeedback::PlaylistAdded(count)).text,
                    false,
                ),
                Err(error) => app.set_status(error.into()),
            }
        });
    });
    if let Err(error) = started {
        app.set_status(error.to_string().into());
    }
}

fn prepare(paths: Vec<String>) -> Result<Vec<String>, String> {
    let paths = file_selection::prepare(media_open::resolve_dropped_url_shortcuts(paths)?, false)?;
    if paths.is_empty() {
        return Err("No playable files were found for the playlist".into());
    }
    Ok(paths)
}
