//! Resolve the visible insertion gap before directory discovery and command dispatch.
use crate::{
    PlayerWindow, media_open, platform,
    player::playlist_edits::{self, Insertion},
};
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
    if app.get_media_request_generation() != generation {
        return;
    }
    let result = playlist_edits::prepare(app, sender, Insertion::Before(before_id), move || {
        files
            .into_iter()
            .map(OsString::into_string)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "A media path contains invalid Unicode".to_owned())
    });
    if let Err(error) = result {
        app.set_status(error.into());
    }
}

pub(super) fn paste(
    app: &PlayerWindow,
    sender: &CommandSender,
    before_id: Option<i64>,
    paths: platform::clipboard::PlaylistPaste,
) -> Result<(), String> {
    playlist_edits::prepare(app, sender, Insertion::Before(before_id), move || {
        Ok(paths.into_paths())
    })
}
