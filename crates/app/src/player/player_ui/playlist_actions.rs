//! Playlist selection, context actions and drag ordering.
use super::{
    PendingNavigation,
    catalog::{
        install_playlist_selection, playlist_paste_index, selected_playlist_ids,
        selected_playlist_paths,
    },
    show_osd_message,
};
use crate::PlayerWindow;
use riina_core::Command;
use riina_mpv::CommandSender;
use riina_ui::PlayerMenuActions;
use slint::{ComponentHandle, Model};
use std::{cell::RefCell, rc::Rc, time::Instant};

pub(super) fn install(
    app: &PlayerWindow,
    command_sender: &CommandSender,
    pending_navigation: &PendingNavigation,
    capture_transition_first: bool,
) {
    install_actions(
        app,
        command_sender,
        pending_navigation,
        capture_transition_first,
    );
    install_selection(app, command_sender);
}

fn install_actions(
    app: &PlayerWindow,
    command_sender: &CommandSender,
    pending_navigation: &PendingNavigation,
    capture_transition_first: bool,
) {
    let playlist_player = command_sender.clone();
    let playlist_pending_navigation = pending_navigation.clone();
    let playlist_capture_transition_first = capture_transition_first;
    let weak = app.as_weak();
    app.global::<PlayerMenuActions>()
        .on_playlist_action(move |action, token| {
            let Some(window) = weak.upgrade() else {
                return;
            };
            if action == 12 {
                window
                    .global::<riina_ui::PlayerControlActions>()
                    .invoke_activity();
                let rows = window
                    .global::<riina_ui::PlayerCatalogState>()
                    .get_playlist();
                let exists = (0..rows.row_count()).any(|index| {
                    rows.row_data(index)
                        .is_some_and(|row| row.token.as_str() == token.as_str())
                });
                if !exists {
                    window.set_status("The playlist item is no longer available".into());
                    return;
                }
                match token.parse::<i64>() {
                    Ok(id) => queue_playlist_navigation(
                        &playlist_player,
                        &playlist_pending_navigation,
                        Command::SelectPlaylistId(id),
                        playlist_capture_transition_first,
                    ),
                    Err(error) => {
                        crate::diagnostics::report(
                            &window,
                            format!("Invalid playlist item: {error}"),
                        );
                    }
                }
                return;
            }
            if matches!(action, 6..=11) {
                handle_playlist_external_action(&window, &playlist_player, action, token.as_str());
                return;
            }
            let command = match action {
                1 => selected_playlist_ids(&window, token.as_str()).map(Command::RemovePlaylistIds),
                2 | 3 => token
                    .parse()
                    .map(|id| Command::MovePlaylistId {
                        id,
                        direction: if action == 2 { -1 } else { 1 },
                    })
                    .map_err(|error: std::num::ParseIntError| error.to_string()),
                4 => Ok(Command::ClearPlaylist),
                5 => Ok(Command::CycleLoop),
                _ => Err("Unknown playlist action".into()),
            };
            window
                .global::<riina_ui::PlayerControlActions>()
                .invoke_activity();
            if let Err(error) =
                command.and_then(|command| playlist_player.send(command).map_err(String::from))
            {
                window.set_status(error.into());
            } else if action == 4 {
                let message = crate::video_osd::action_message(
                    crate::video_osd::ActionFeedback::PlaylistCleared,
                );
                show_osd_message(&window, message.text, false);
            } else if action == 1 {
                window
                    .global::<riina_ui::PlaylistInteraction>()
                    .set_selected_playlist_token("".into());
                window
                    .global::<riina_ui::PlaylistInteraction>()
                    .set_selected_playlist_tokens(slint::ModelRc::default());
                install_playlist_selection(&window, &[]);
            }
        });
}

fn handle_playlist_external_action(
    window: &PlayerWindow,
    playlist_player: &CommandSender,
    action: i32,
    token: &str,
) {
    window
        .global::<riina_ui::PlayerControlActions>()
        .invoke_activity();
    let rows = window
        .global::<riina_ui::PlayerCatalogState>()
        .get_playlist();
    let selected_index = (0..rows.row_count())
        .find(|index| rows.row_data(*index).is_some_and(|row| row.token == token));
    let result = match action {
        6 | 7 => {
            let paths = selected_playlist_paths(window, token);
            if paths.is_empty() {
                Err("Select a playlist item first".into())
            } else {
                crate::platform::clipboard::copy_playlist_references(&paths).and_then(|()| {
                    if action == 7 {
                        selected_playlist_ids(window, token).and_then(|ids| {
                            playlist_player
                                .send(Command::RemovePlaylistIds(ids))
                                .map_err(String::from)
                        })
                    } else {
                        Ok(())
                    }
                })
            }
        }
        8 => crate::platform::clipboard::read_playlist_references().and_then(|paths| {
            if paths.is_empty() {
                return Err("Clipboard contains no playlist references".into());
            }
            let index = playlist_paste_index(window, token);
            let count = paths.len();
            playlist_player
                .send(Command::InsertPlaylistFiles { paths, index })
                .map_err(String::from)
                .map(|()| {
                    let message = crate::video_osd::action_message(
                        crate::video_osd::ActionFeedback::PlaylistAdded(count),
                    );
                    show_osd_message(window, message.text, false);
                })
        }),
        9 => selected_index
            .and_then(|index| rows.row_data(index))
            .ok_or_else(|| "Select a playlist item first".to_owned())
            .and_then(|row| crate::media_open::open_in_new_player(window, row.path.as_str())),
        10 => {
            let urls = selected_playlist_paths(window, token)
                .into_iter()
                .filter(|path| crate::platform::is_http_url(path))
                .collect::<Vec<_>>();
            if urls.is_empty() {
                Err("Select an HTTP or HTTPS playlist entry first".into())
            } else {
                urls.iter().try_for_each(|url| {
                    crate::platform::open_http_url_in_browser(url)
                        .map_err(|error| error.to_string())
                })
            }
        }
        11 => {
            let urls = selected_playlist_paths(window, token)
                .into_iter()
                .filter(|path| crate::platform::is_http_url(path))
                .collect::<Vec<_>>();
            if urls.is_empty() {
                Err("Select an HTTP or HTTPS playlist entry first".into())
            } else {
                crate::platform::clipboard::copy_text(&urls.join("\n"))
            }
        }
        _ => unreachable!(),
    };
    if let Err(error) = result {
        window.set_status(error.into());
    } else if action == 7 {
        window
            .global::<riina_ui::PlaylistInteraction>()
            .set_selected_playlist_token("".into());
        window
            .global::<riina_ui::PlaylistInteraction>()
            .set_selected_playlist_tokens(slint::ModelRc::default());
        install_playlist_selection(window, &[]);
    }
}

fn install_selection(app: &PlayerWindow, command_sender: &CommandSender) {
    let playlist_selection_anchor = Rc::new(RefCell::new(None::<String>));
    let selection_anchor = playlist_selection_anchor.clone();
    let weak = app.as_weak();
    app.global::<riina_ui::PlayerSidebarActions>()
        .on_select_playlist_row(move |index, extend, range| {
            let Some(app) = weak.upgrade() else {
                return slint::ModelRc::default();
            };
            let Ok(index) = usize::try_from(index) else {
                return slint::ModelRc::default();
            };
            let rows = app.global::<riina_ui::PlayerCatalogState>().get_playlist();
            let Some(clicked) = rows.row_data(index) else {
                return app
                    .global::<riina_ui::PlaylistInteraction>()
                    .get_selected_playlist_tokens();
            };
            let clicked_token = clicked.token.to_string();
            let previous = app
                .global::<riina_ui::PlaylistInteraction>()
                .get_selected_playlist_tokens()
                .iter()
                .map(|token| token.to_string())
                .collect::<Vec<_>>();
            let selected = if range {
                let anchor = selection_anchor
                    .borrow()
                    .as_deref()
                    .and_then(|anchor| {
                        (0..rows.row_count()).find(|row| {
                            rows.row_data(*row)
                                .is_some_and(|entry| entry.token.as_str() == anchor)
                        })
                    })
                    .unwrap_or(index);
                let mut selected = if extend { previous } else { Vec::new() };
                for row in anchor.min(index)..=anchor.max(index) {
                    if let Some(entry) = rows.row_data(row)
                        && !selected.iter().any(|token| token == entry.token.as_str())
                    {
                        selected.push(entry.token.to_string());
                    }
                }
                selected
            } else if extend {
                let mut selected = previous;
                if let Some(position) = selected.iter().position(|token| token == &clicked_token) {
                    selected.remove(position);
                    if selection_anchor.borrow().as_deref() == Some(clicked_token.as_str()) {
                        *selection_anchor.borrow_mut() = None;
                    }
                } else {
                    selected.push(clicked_token.clone());
                    *selection_anchor.borrow_mut() = Some(clicked_token.clone());
                }
                selected
            } else {
                *selection_anchor.borrow_mut() = Some(clicked_token.clone());
                vec![clicked_token]
            };
            install_playlist_selection(&app, &selected);
            slint::ModelRc::new(slint::VecModel::from(
                selected.into_iter().map(Into::into).collect::<Vec<_>>(),
            ))
        });

    let reorder_player = command_sender.clone();
    let weak = app.as_weak();
    app.global::<riina_ui::PlayerSidebarActions>()
        .on_reorder_playlist(move |tokens, insertion_slot| {
            let Some(app) = weak.upgrade() else { return };
            app.global::<riina_ui::PlayerControlActions>()
                .invoke_activity();
            let rows = app.global::<riina_ui::PlayerCatalogState>().get_playlist();
            let entries = (0..rows.row_count())
                .filter_map(|index| rows.row_data(index))
                .map(|row| row.token.parse::<i64>().ok())
                .collect::<Option<Vec<_>>>();
            let selected = tokens
                .iter()
                .map(|token| token.parse::<i64>().ok())
                .collect::<Option<Vec<_>>>();
            let plan =
                entries
                    .as_deref()
                    .zip(selected.as_deref())
                    .and_then(|(entries, selected)| {
                        usize::try_from(insertion_slot).ok().and_then(|slot| {
                            riina_core::playlist::reorder_selection(entries, selected, slot)
                        })
                    });
            let Some(plan) = plan else {
                app.set_status("The playlist drop position is no longer available".into());
                return;
            };
            for (from, to) in plan {
                if let Err(error) = reorder_player.send(Command::MovePlaylistEntry { from, to }) {
                    crate::diagnostics::report(&app, error);
                    break;
                }
            }
        });
    let shuffle_player = command_sender.clone();
    let weak = app.as_weak();
    app.global::<riina_ui::PlayerSidebarActions>()
        .on_toggle_playlist_shuffle(move || {
            if let Some(app) = weak.upgrade() {
                app.global::<riina_ui::PlayerControlActions>()
                    .invoke_activity();
                if let Err(error) = shuffle_player.send(Command::TogglePlaylistShuffle) {
                    crate::diagnostics::report(&app, error);
                }
            }
        });
    let sort_player = command_sender.clone();
    let weak = app.as_weak();
    app.global::<riina_ui::PlayerSidebarActions>()
        .on_sort_playlist(move |sort| {
            let sort = match sort {
                0 => Some(riina_core::playlist::PlaylistSort::FilenameAscending),
                1 => Some(riina_core::playlist::PlaylistSort::FilenameDescending),
                2 => Some(riina_core::playlist::PlaylistSort::PathAscending),
                3 => Some(riina_core::playlist::PlaylistSort::PathDescending),
                _ => None,
            };
            if let Some(app) = weak.upgrade() {
                app.global::<riina_ui::PlayerControlActions>()
                    .invoke_activity();
                let result = sort.map_or_else(
                    || Err("Unknown playlist sort order".to_owned()),
                    |sort| {
                        sort_player
                            .send(Command::SortPlaylist(sort))
                            .map_err(String::from)
                    },
                );
                if let Err(error) = result {
                    app.set_status(error.into());
                }
            }
        });
}

fn queue_playlist_navigation(
    sender: &riina_mpv::CommandSender,
    pending: &PendingNavigation,
    command: Command,
    capture_transition_first: bool,
) {
    if !capture_transition_first {
        if let Err(error) = sender.send(command) {
            crate::diagnostics::warn!("Could not navigate the playlist: {error}");
        }
        return;
    }

    let needs_capture = {
        let mut pending = pending.requests.borrow_mut();
        if let Some((commands, _)) = pending.as_mut() {
            commands.push(command);
            false
        } else {
            *pending = Some((vec![command], Instant::now()));
            true
        }
    };
    if needs_capture && let Err(error) = sender.send(Command::CaptureTransitionFrame) {
        let commands = pending
            .requests
            .borrow_mut()
            .take()
            .map(|(commands, _)| commands);
        crate::diagnostics::warn!("Could not capture the outgoing video frame: {error}");
        if let Some(commands) = commands {
            for command in commands {
                if let Err(error) = sender.send(command) {
                    crate::diagnostics::warn!("Could not navigate the playlist: {error}");
                }
            }
        }
    } else {
        pending.wake();
    }
}

pub(super) fn install_navigation(
    app: &PlayerWindow,
    player: &CommandSender,
    pending_navigation: &PendingNavigation,
    capture_transition_first: bool,
) {
    let sender = player.clone();
    let pending = pending_navigation.clone();
    let weak = app.as_weak();
    app.global::<PlayerMenuActions>().on_playlist_next(move || {
        if let Some(app) = weak.upgrade() {
            app.global::<riina_ui::PlayerControlActions>()
                .invoke_activity();
        }
        queue_playlist_navigation(
            &sender,
            &pending,
            Command::PlaylistNext,
            capture_transition_first,
        );
    });
    let sender = player.clone();
    let pending = pending_navigation.clone();
    let weak = app.as_weak();
    app.global::<PlayerMenuActions>()
        .on_playlist_previous(move || {
            if let Some(app) = weak.upgrade() {
                app.global::<riina_ui::PlayerControlActions>()
                    .invoke_activity();
            }
            queue_playlist_navigation(
                &sender,
                &pending,
                Command::PlaylistPrevious,
                capture_transition_first,
            );
        });
}
