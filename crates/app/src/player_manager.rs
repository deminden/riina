//! Own independent shared-renderer player windows for the application lifetime.

use crate::{PlayerWindow, WelcomeWindow, player_ui::UiSession};
use slint::{CloseRequestResponse, ComponentHandle, Model};
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
            let paths = paths
                .iter()
                .map(|path| OsString::from(path.as_str()))
                .collect::<Vec<_>>();
            let resume_position =
                (position.is_finite() && position >= 0.0).then_some(f64::from(position));
            if let Err(error) = open_player(&state, paths, resume_position)
                && let Some(owner) = owner.upgrade()
            {
                owner.set_status(error.into());
            }
        });

        let state = Rc::downgrade(&self.state);
        app.on_close_player(move || schedule_close(state.clone(), id, false));
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

fn open_player(
    state: &Rc<RefCell<ManagerState>>,
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
    let mut selected = Vec::<OsString>::new();
    let mut existing_player = None;
    for path in media {
        let candidate = path.to_string_lossy();
        if !preferences.allow_duplicate_players && !grouped_request {
            let duplicate = state.borrow().players.iter().find_map(|player| {
                let playlist = player.app.get_playlist();
                (0..playlist.row_count())
                    .filter_map(|index| playlist.row_data(index))
                    .any(|row| crate::media_open::same_media(row.path.as_str(), &candidate))
                    .then(|| player.app.clone_strong())
            });
            if let Some(player) = duplicate {
                existing_player = Some(player);
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
    if selected.is_empty() {
        if let Some(player) = existing_player {
            player.show().map_err(|error| error.to_string())?;
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
    Ok(())
}

fn schedule_close(state: Weak<RefCell<ManagerState>>, id: u64, from_welcome: bool) {
    let _ = slint::spawn_local(async move {
        if let Some(state) = state.upgrade() {
            close_player(&state, id, from_welcome);
        }
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
    let quit_when_empty = player.app.get_quit_when_no_opened_window();
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
    welcome.show()?;
    Ok(())
}
