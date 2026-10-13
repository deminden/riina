//! Own independent shared-renderer player windows for the application lifetime.

use crate::{PlayerWindow, WelcomeWindow, player_ui::UiSession};
use slint::{CloseRequestResponse, ComponentHandle, Model, winit_030::WinitWindowAccessor};
use std::{
    cell::RefCell,
    ffi::OsString,
    rc::{Rc, Weak},
};

struct ManagedPlayer {
    id: u64,
    session: UiSession,
    welcome: Option<WelcomeWindow>,
    app: PlayerWindow,
}

#[derive(Default)]
struct ManagerState {
    next_id: u64,
    players: Vec<ManagedPlayer>,
}

pub(crate) const fn should_quit_after_last_window_closed(
    quit_when_no_opened_window: bool,
    windows_remaining: usize,
) -> bool {
    quit_when_no_opened_window && windows_remaining == 0
}

#[derive(Clone, Default)]
pub(crate) struct PlayerManager {
    state: Rc<RefCell<ManagerState>>,
}

impl PlayerManager {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn attach(
        &self,
        app: &PlayerWindow,
        welcome: Option<WelcomeWindow>,
        session: UiSession,
    ) -> u64 {
        let managed_welcome = welcome.as_ref().map(WelcomeWindow::clone_strong);
        let id = {
            let mut state = self.state.borrow_mut();
            state.next_id = state.next_id.wrapping_add(1).max(1);
            let id = state.next_id;
            state.players.push(ManagedPlayer {
                id,
                session,
                welcome: managed_welcome,
                app: app.clone_strong(),
            });
            id
        };

        app.set_in_process_player_manager(true);
        let state = Rc::downgrade(&self.state);
        let owner = app.as_weak();
        app.on_open_player_requested(move |paths, position| {
            let Some(state) = state.upgrade() else { return };
            let Some(owner) = owner.upgrade() else { return };
            let paths = paths
                .iter()
                .map(|path| OsString::from(path.as_str()))
                .collect::<Vec<_>>();
            let resume_position =
                (position.is_finite() && position >= 0.0).then_some(f64::from(position));
            if let Err(error) = open_player(&state, &owner, paths, resume_position) {
                owner.set_status(error.into());
            }
        });

        let state = Rc::downgrade(&self.state);
        app.on_close_player(move || schedule_close(state.clone(), id, false));
        let state = Rc::downgrade(&self.state);
        app.on_quit_riina(move || schedule_quit(state.clone()));
        let state = Rc::downgrade(&self.state);
        let owner = app.as_weak();
        app.on_bring_all_to_front(move || {
            let (Some(state), Some(owner)) = (state.upgrade(), owner.upgrade()) else {
                return;
            };
            bring_all_to_front(&state, &owner);
        });
        if let Some(welcome) = welcome {
            let state = Rc::downgrade(&self.state);
            welcome.window().on_close_requested(move || {
                schedule_close(state.clone(), id, true);
                CloseRequestResponse::HideWindow
            });
        }
        id
    }

    pub(crate) fn save_all(&self) -> Result<(), Box<dyn std::error::Error>> {
        for player in &self.state.borrow().players {
            player.session.save(&player.app)?;
        }
        Ok(())
    }
}

fn bring_all_to_front(state: &Rc<RefCell<ManagerState>>, owner: &PlayerWindow) {
    let windows = state
        .borrow()
        .players
        .iter()
        .map(|player| {
            (
                player.app.clone_strong(),
                player.welcome.as_ref().map(WelcomeWindow::clone_strong),
            )
        })
        .collect::<Vec<_>>();
    for (app, welcome) in windows {
        show_and_focus(&app, owner);
        if let Some(welcome) = welcome {
            show_and_focus(&welcome, owner);
        }
    }
}

fn show_and_focus(window: &impl ComponentHandle, owner: &PlayerWindow) {
    if let Err(error) = window.show() {
        owner.set_status(format!("Could not show a player window: {error}").into());
        return;
    }
    window.window().with_winit_window(|window| {
        window.set_minimized(false);
        window.focus_window();
    });
}

fn open_player(
    state: &Rc<RefCell<ManagerState>>,
    owner: &PlayerWindow,
    media: Vec<OsString>,
    resume_position: Option<f64>,
) -> Result<(), String> {
    if media.is_empty() {
        return Err("No media was selected for the new player".into());
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
    let grouped_request = crate::media_open::grouped_open_request(
        media.len(),
        preferences.group_simultaneous_opens_in_playlist,
    );
    let existing_playlists = state
        .borrow()
        .players
        .iter()
        .map(|player| {
            let playlist = player.app.get_playlist();
            (0..playlist.row_count())
                .filter_map(|index| playlist.row_data(index))
                .map(|row| row.path.to_string())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let (selected, existing_player) = select_media_for_new_player(
        media,
        &existing_playlists,
        preferences.allow_duplicate_players,
        grouped_request,
    );
    if selected.is_empty() {
        if let Some(player) = existing_player.and_then(|index| {
            state
                .borrow()
                .players
                .get(index)
                .map(|player| player.app.clone_strong())
        }) {
            player.show().map_err(|error| error.to_string())?;
            player.window().with_winit_window(|window| {
                window.set_minimized(false);
                window.focus_window();
            });
            return Ok(());
        }
        return Err("The selected media is already open.".into());
    }
    let app = PlayerWindow::new().map_err(|error| error.to_string())?;
    app.set_native_video(false);
    let session = crate::player_ui::configure_player(&app, None, selected, None, resume_position)
        .map_err(|error| error.to_string())?;
    let manager = PlayerManager {
        state: state.clone(),
    };
    let id = manager.attach(&app, None, session);
    crate::window_aspect::connect(&app);
    if let Err(error) = app.show() {
        close_player(state, id, false);
        return Err(error.to_string());
    }
    crate::platform::center_player_window_for_owner(
        &app,
        owner,
        app.get_initial_window_position_enabled(),
    );
    Ok(())
}

fn select_media_for_new_player(
    media: Vec<OsString>,
    existing_playlists: &[Vec<String>],
    allow_duplicate_players: bool,
    grouped_request: bool,
) -> (Vec<OsString>, Option<usize>) {
    let filter_duplicates = !allow_duplicate_players && !grouped_request;
    let mut selected = Vec::<OsString>::new();
    let mut existing_player = None;
    for path in media {
        let candidate = path.to_string_lossy();
        if filter_duplicates {
            if let Some(index) = existing_playlists.iter().position(|playlist| {
                playlist
                    .iter()
                    .any(|open| crate::media_open::same_media(open, &candidate))
            }) {
                existing_player = Some(index);
                continue;
            }
            if selected
                .iter()
                .any(|open| crate::media_open::same_media(&open.to_string_lossy(), &candidate))
            {
                continue;
            }
        }
        selected.push(path);
    }
    (selected, existing_player)
}

fn schedule_close(state: Weak<RefCell<ManagerState>>, id: u64, from_welcome: bool) {
    let _ = slint::spawn_local(async move {
        if let Some(state) = state.upgrade() {
            close_player(&state, id, from_welcome);
        }
    });
}

fn schedule_quit(state: Weak<RefCell<ManagerState>>) {
    let _ = slint::spawn_local(async move {
        let Some(state) = state.upgrade() else { return };
        let players = std::mem::take(&mut state.borrow_mut().players);
        for player in players {
            if let Err(error) = player.session.save(&player.app) {
                eprintln!("Could not save player state before quitting: {error}");
            }
            if let Some(welcome) = &player.welcome {
                let _ = welcome.hide();
            }
            let _ = player.app.hide();
            drop(player);
        }
        let _ = slint::quit_event_loop();
    });
}

fn close_player(state: &Rc<RefCell<ManagerState>>, id: u64, from_welcome: bool) {
    let removed = {
        let mut state = state.borrow_mut();
        state
            .players
            .iter()
            .position(|player| player.id == id)
            .map(|index| state.players.remove(index))
    };
    let Some(player) = removed else { return };
    if let Err(error) = player.session.save(&player.app) {
        player.app.set_status(error.to_string().into());
    }
    if let Some(welcome) = &player.welcome {
        let _ = welcome.hide();
    }
    let empty = state.borrow().players.is_empty();
    let quit_when_empty = should_quit_after_last_window_closed(
        player.app.get_quit_when_no_opened_window(),
        state.borrow().players.len(),
    );
    drop(player);
    if empty {
        if quit_when_empty {
            let _ = slint::quit_event_loop();
        } else if !from_welcome && let Err(error) = show_welcome(state) {
            eprintln!("Could not reopen the welcome window: {error}");
            let _ = slint::quit_event_loop();
        }
    }
}

fn show_welcome(state: &Rc<RefCell<ManagerState>>) -> Result<(), Box<dyn std::error::Error>> {
    let app = PlayerWindow::new()?;
    app.set_native_video(false);
    let welcome = WelcomeWindow::new()?;
    let session = crate::player_ui::configure_player(&app, None, Vec::new(), Some(&welcome), None)?;
    let manager = PlayerManager {
        state: state.clone(),
    };
    manager.attach(&app, Some(welcome.clone_strong()), session);
    crate::window_aspect::connect(&app);
    crate::platform::center_auxiliary_window(&welcome, &app);
    welcome.show()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{select_media_for_new_player, should_quit_after_last_window_closed};
    use std::ffi::OsString;

    #[test]
    fn duplicate_media_resolves_to_the_existing_player() {
        let existing = vec![
            vec!["https://example.com/first.mkv".to_owned()],
            vec!["https://example.com/current.mkv".to_owned()],
        ];

        let (selected, existing_player) = select_media_for_new_player(
            vec![OsString::from("https://example.com/current.mkv")],
            &existing,
            false,
            false,
        );

        assert!(selected.is_empty());
        assert_eq!(existing_player, Some(1));
    }

    #[test]
    fn duplicate_media_is_preserved_when_duplicates_are_allowed() {
        let (selected, existing_player) = select_media_for_new_player(
            vec![
                OsString::from("https://example.com/current.mkv"),
                OsString::from("https://example.com/current.mkv"),
            ],
            &[vec!["https://example.com/current.mkv".to_owned()]],
            true,
            false,
        );

        assert_eq!(
            selected,
            [
                OsString::from("https://example.com/current.mkv"),
                OsString::from("https://example.com/current.mkv"),
            ]
        );
        assert_eq!(existing_player, None);
    }

    #[test]
    fn grouped_open_keeps_duplicate_items_even_when_duplicates_are_disabled() {
        let (selected, existing_player) = select_media_for_new_player(
            vec![
                OsString::from("https://example.com/current.mkv"),
                OsString::from("https://example.com/current.mkv"),
            ],
            &[vec!["https://example.com/current.mkv".to_owned()]],
            false,
            true,
        );

        assert_eq!(selected.len(), 2);
        assert_eq!(existing_player, None);
    }

    #[test]
    fn application_quits_only_when_requested_after_the_last_window_closes() {
        assert!(should_quit_after_last_window_closed(true, 0));
        assert!(!should_quit_after_last_window_closed(false, 0));
        assert!(!should_quit_after_last_window_closed(true, 1));
    }
}
