//! Playlist selection, context actions and drag ordering.
use super::{
    PendingNavigation,
    catalog::{
        install_playlist_selection, playlist_paste_anchor, selected_playlist_ids,
        selected_playlist_paths, selected_playlist_rows,
    },
    show_osd_message,
};
use crate::{
    PlayerWindow,
    file_actions::{self, PlaylistFile},
};
use riina_core::Command;
use riina_mpv::CommandSender;
use riina_ui::{PlayerMenuActions, PlaylistInteraction};
use slint::{ComponentHandle, Model};
use std::{cell::RefCell, collections::HashSet, rc::Rc, time::Instant};

pub(super) fn install(
    app: &PlayerWindow,
    command_sender: &CommandSender,
    pending_navigation: &PendingNavigation,
    capture_transition_first: bool,
) {
    let interaction = app.global::<PlaylistInteraction>();
    interaction
        .set_reveal_action_label(format!("Show in {}", app.get_current_file_manager_name()).into());
    install_actions(
        app,
        command_sender,
        pending_navigation,
        capture_transition_first,
    );
    install_selection(app, command_sender);
    install_keyboard_editing(app, command_sender);
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
            if matches!(action, 6..=11 | 14..=15) {
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
                13 => selected_playlist_ids(&window, token.as_str())
                    .map(Command::QueuePlaylistIdsNext),
                _ => Err("Unknown playlist action".into()),
            };
            window
                .global::<riina_ui::PlayerControlActions>()
                .invoke_activity();
            if let Err(error) =
                command.and_then(|command| playlist_player.send(command).map_err(String::from))
            {
                window.set_status(error.into());
                return;
            }
            if action == 4 {
                let message = crate::video_osd::action_message(
                    crate::video_osd::ActionFeedback::PlaylistCleared,
                );
                show_osd_message(&window, message.text, false);
            } else if matches!(action, 1 | 13) {
                window
                    .global::<riina_ui::PlaylistInteraction>()
                    .invoke_clear_selection();
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
        8 => crate::platform::clipboard::read_playlist_references()
            .and_then(|paths| paste_playlist(window, playlist_player, paths)),
        9 => {
            let paths = selected_playlist_paths(window, token);
            if paths.is_empty() {
                Err("Select a playlist item first".into())
            } else {
                crate::media_open::playlist_in_new_player(window, paths);
                Ok(())
            }
        }
        10 | 11 => handle_playlist_url_action(window, token, action),
        14 | 15 => {
            let files = selected_playlist_rows(window, token)
                .into_iter()
                .filter_map(|row| {
                    let id = row.token.parse().ok()?;
                    let path = file_actions::local_media_path(row.path.as_str())?;
                    Some(PlaylistFile::new(id, row.path.to_string(), path))
                })
                .collect::<Vec<_>>();
            if files.is_empty() {
                Err("Select a local playlist file first".into())
            } else {
                if action == 14 {
                    file_actions::trash_playlist_files(window, files, playlist_player.clone());
                } else {
                    window
                        .global::<PlaylistInteraction>()
                        .invoke_clear_selection();
                    file_actions::reveal_playlist_files(window, files);
                }
                Ok(())
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

fn paste_playlist(
    app: &PlayerWindow,
    sender: &CommandSender,
    paths: Vec<String>,
) -> Result<(), String> {
    if paths.is_empty() {
        return Err("Clipboard contains no playlist references".into());
    }
    super::playlist_drop::paste(app, sender, playlist_paste_anchor(app)?, paths);
    Ok(())
}

fn install_keyboard_editing(app: &PlayerWindow, commands: &CommandSender) {
    let window = app.as_weak();
    let commands = commands.clone();
    app.global::<PlaylistInteraction>()
        .on_edit(move |edit, apply| {
            window
                .upgrade()
                .is_some_and(|window| edit_focused_playlist(&window, &commands, edit, apply))
        });
}

fn edit_focused_playlist(
    app: &PlayerWindow,
    commands: &CommandSender,
    edit: riina_ui::PlaylistEdit,
    apply: bool,
) -> bool {
    use riina_ui::PlaylistEdit::{Copy, Cut, None as NoEdit, Paste, Remove, SelectAll};
    let interaction = app.global::<PlaylistInteraction>();
    let focused = interaction.get_focused_playlist_token();
    let rows = app.global::<riina_ui::PlayerCatalogState>().get_playlist();
    if focused.is_empty() || !rows.iter().any(|row| row.token == focused) {
        return false;
    }
    let selected = interaction.get_selected_playlist_tokens();
    let action = app.global::<PlayerMenuActions>();
    match edit {
        NoEdit => return false,
        Paste if !apply => return crate::platform::clipboard::playlist_paste_available(),
        Paste => {
            let Ok(paths) = crate::platform::clipboard::read_playlist_references() else {
                return false;
            };
            if paths.is_empty() {
                return false;
            }
            if let Err(error) = paste_playlist(app, commands, paths) {
                crate::diagnostics::report(app, error);
            }
        }
        SelectAll => {
            if apply {
                let tokens = rows.iter().map(|row| row.token).collect::<Vec<_>>();
                install_playlist_selection(
                    app,
                    &tokens.iter().map(ToString::to_string).collect::<Vec<_>>(),
                );
                interaction.set_selected_playlist_token(focused);
                interaction.set_selected_playlist_tokens(slint::ModelRc::new(
                    slint::VecModel::from(tokens),
                ));
            }
        }
        Cut | Copy | Remove => {
            let Some(token) = selected.row_data(0) else {
                return false;
            };
            if apply {
                if matches!(edit, Cut | Remove)
                    && let Some(target) = focus_after_removal(&rows, &selected, &focused)
                {
                    interaction.set_edit_focus_target(target);
                }
                let code = match edit {
                    Cut => 7,
                    Copy => 6,
                    _ => 1,
                };
                action.invoke_playlist_action(code, token);
            }
        }
    }
    true
}

fn focus_after_removal(
    rows: &slint::ModelRc<crate::CatalogRow>,
    selected: &slint::ModelRc<slint::SharedString>,
    focused: &slint::SharedString,
) -> Option<slint::SharedString> {
    let selected = selected.iter().collect::<HashSet<_>>();
    if !selected.contains(focused) {
        return None;
    }
    let mut preceding = None;
    let mut found_focus = false;
    for row in rows.iter() {
        if row.token == *focused {
            found_focus = true;
        } else if !selected.contains(&row.token) {
            if found_focus {
                return Some(row.token);
            }
            preceding = Some(row.token);
        }
    }
    preceding
}

pub(super) fn playlist_network_url(row: &crate::CatalogRow) -> Option<&str> {
    riina_core::media_reference::validate_network_url(row.path.as_str()).ok()
}

fn handle_playlist_url_action(
    window: &PlayerWindow,
    token: &str,
    action: i32,
) -> Result<(), String> {
    let rows = selected_playlist_rows(window, token);
    let urls = rows.iter().filter_map(playlist_network_url);
    if action == 11 {
        let urls = urls.collect::<Vec<_>>();
        return if urls.is_empty() {
            Err("Select a playlist URL first".into())
        } else {
            crate::platform::clipboard::copy_text(&urls.join("\n"))
        };
    }
    open_selected_browser_urls(urls, crate::platform::open_http_url_in_browser)
}

fn open_selected_browser_urls<'a>(
    urls: impl Iterator<Item = &'a str>,
    mut open: impl FnMut(&str) -> std::io::Result<()>,
) -> Result<(), String> {
    let mut opened = 0;
    let mut failed = 0;
    for url in urls.filter(|url| crate::platform::is_http_url(url)) {
        if open(url).is_ok() {
            opened += 1;
        } else {
            failed += 1;
        }
    }
    match (opened, failed) {
        (0, 0) => Err("Select an HTTP or HTTPS playlist entry first".into()),
        (_, 0) => Ok(()),
        (_, 1) => Err("Could not open one selected URL in the browser".into()),
        (_, failed) => Err(format!(
            "Could not open {failed} selected URLs in the browser"
        )),
    }
}

fn install_selection(app: &PlayerWindow, command_sender: &CommandSender) {
    let playlist_selection_anchor = Rc::new(RefCell::new(None::<String>));
    let selection_anchor = playlist_selection_anchor.clone();
    let weak = app.as_weak();
    app.global::<riina_ui::PlayerSidebarActions>()
        .on_clear_playlist_selection(move || {
            *playlist_selection_anchor.borrow_mut() = None;
            if let Some(app) = weak.upgrade() {
                install_playlist_selection(&app, &[]);
            }
        });
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
                let previous_tokens = selected.iter().map(String::as_str).collect::<HashSet<_>>();
                let mut added = Vec::new();
                for row in anchor.min(index)..=anchor.max(index) {
                    if let Some(entry) = rows.row_data(row)
                        && !previous_tokens.contains(entry.token.as_str())
                    {
                        added.push(entry.token.to_string());
                    }
                }
                selected.extend(added);
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
            let result = (|| {
                let slot = usize::try_from(insertion_slot).map_err(|error| error.to_string())?;
                let before_id = if slot == rows.row_count() {
                    None
                } else {
                    let row = rows
                        .row_data(slot)
                        .ok_or("The playlist drop position is no longer available")?;
                    Some(
                        row.token
                            .parse::<i64>()
                            .map_err(|error| error.to_string())?,
                    )
                };
                let ids = tokens
                    .iter()
                    .map(|token| token.parse::<i64>())
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| error.to_string())?;
                reorder_player
                    .send(Command::MovePlaylistIdsBefore { ids, before_id })
                    .map_err(String::from)
            })();
            if let Err(error) = result {
                crate::diagnostics::report(&app, error);
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

#[cfg(test)]
mod browser_tests {
    use super::open_selected_browser_urls;

    #[test]
    fn browser_launch_tries_each_web_url_in_order_and_reports_partial_failure() {
        let first = "http://first.example.test/movie";
        let last = "https://last.example.test/video";
        let mut attempted = Vec::new();
        let result = open_selected_browser_urls(
            [first, "rtsp://camera.example.test/live", last].into_iter(),
            |url| {
                attempted.push(url.to_owned());
                if url == first {
                    Err(std::io::Error::other("Desktop launcher failed"))
                } else {
                    Ok(())
                }
            },
        );
        assert_eq!(attempted, [first, last]);
        assert_eq!(
            result,
            Err("Could not open one selected URL in the browser".into())
        );
    }
}
