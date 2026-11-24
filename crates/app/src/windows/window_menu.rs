//! One window catalog for every player menu in the application.
use crate::{
    PlayerWindow,
    player::surface::{PlayerDisplay, SwitchableWindow},
};
use riina_ui::{MenuCatalogRow, PlayerCatalogState, PlayerMenuActions};
use slint::{ComponentHandle, ModelRc, SharedString, Timer, TimerMode, VecModel};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    time::Duration,
};

#[derive(Default)]
pub(crate) struct ApplicationWindowMenu {
    next_id: Cell<u64>,
    players: RefCell<Vec<PlayerEntry>>,
    projected: RefCell<Vec<WindowEntry>>,
    monitor: Timer,
}

struct PlayerEntry {
    id: u64,
    window: slint::Weak<PlayerWindow>,
    display: PlayerDisplay,
}

#[derive(PartialEq, Eq)]
struct WindowEntry {
    id: u64,
    title: SharedString,
}

pub(crate) struct Registration {
    owner: Weak<ApplicationWindowMenu>,
    id: Cell<Option<u64>>,
}

impl ApplicationWindowMenu {
    pub(crate) fn register(
        self: &Rc<Self>,
        app: &PlayerWindow,
        display: PlayerDisplay,
    ) -> Registration {
        let id = self.next_id.get().wrapping_add(1);
        self.next_id.set(id);
        let mut players = self.players.borrow_mut();
        let first = players.is_empty();
        players.push(PlayerEntry {
            id,
            window: app.as_weak(),
            display,
        });
        drop(players);

        let owner = Rc::downgrade(self);
        let source = app.as_weak();
        app.global::<PlayerMenuActions>()
            .on_select_player_window(move |token| {
                let (Some(owner), Some(app)) = (owner.upgrade(), source.upgrade()) else {
                    return;
                };
                if let Ok(destination) = token.parse()
                    && let Some(target) = owner.destination(destination)
                {
                    target.focus_from_menu(&app);
                }
                // Native checkable items toggle on activation. The check
                // identifies the owning player and stays in its original row.
                publish(&app, id, &owner.projected.borrow());
            });
        // Window managers can hide or restore a surface independently of the
        // playback model. The low-frequency projection leaves unchanged menus alone.
        if first {
            let owner = Rc::downgrade(self);
            self.monitor
                .start(TimerMode::Repeated, Duration::from_millis(220), move || {
                    if let Some(owner) = owner.upgrade() {
                        owner.refresh();
                    }
                });
        }
        if !self.refresh() {
            publish(app, id, &self.projected.borrow());
        }
        Registration {
            owner: Rc::downgrade(self),
            id: Cell::new(Some(id)),
        }
    }

    fn destination(&self, id: u64) -> Option<SwitchableWindow> {
        let players = self.players.borrow();
        let player = players.iter().find(|player| player.id == id)?;
        player.display.switchable(&player.window.upgrade()?)
    }

    fn refresh(&self) -> bool {
        let players = self.players.borrow();
        let windows = players
            .iter()
            .filter_map(|player| {
                let app = player.window.upgrade()?;
                let surface = player.display.switchable(&app)?;
                Some(WindowEntry {
                    id: player.id,
                    title: surface.label(&app),
                })
            })
            .collect::<Vec<_>>();
        if *self.projected.borrow() == windows {
            return false;
        }
        for player in players.iter() {
            if let Some(app) = player.window.upgrade() {
                publish(&app, player.id, &windows);
            }
        }
        self.projected.replace(windows);
        true
    }
}

fn publish(app: &PlayerWindow, id: u64, windows: &[WindowEntry]) {
    let rows = windows
        .iter()
        .map(|window| MenuCatalogRow {
            token: window.id.to_string().into(),
            label: menu_label(&window.title),
            selected: window.id == id,
        })
        .collect::<Vec<_>>();
    app.global::<PlayerCatalogState>()
        .set_window_menu_rows(ModelRc::new(VecModel::from(rows)));
}

fn menu_label(title: &SharedString) -> SharedString {
    if cfg!(target_os = "windows") && title.contains('&') {
        // Windows interprets a single ampersand as a mnemonic.
        title.replace('&', "&&").into()
    } else {
        title.clone()
    }
}

impl Registration {
    pub(crate) fn retire(&self) {
        if let Some(id) = self.id.take()
            && let Some(owner) = self.owner.upgrade()
        {
            owner.players.borrow_mut().retain(|player| player.id != id);
            if owner.players.borrow().is_empty() {
                owner.monitor.stop();
            }
            owner.refresh();
        }
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        self.retire();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WelcomeWindow;
    use slint::Model;

    #[test]
    fn menus_follow_visible_surfaces_titles_and_session_ownership()
    -> Result<(), Box<dyn std::error::Error>> {
        i_slint_backend_testing::init_no_event_loop();
        let owner = Rc::new(ApplicationWindowMenu::default());
        let first = PlayerWindow::new()?;
        let second = PlayerWindow::new()?;
        let welcome = WelcomeWindow::new()?;
        first.set_window_title("First movie".into());
        second.set_window_title("Second movie".into());
        first.show()?;
        let initial = owner.register(&first, PlayerDisplay::default());
        let other = owner.register(
            &second,
            PlayerDisplay {
                welcome: Some(welcome.as_weak()),
                host: None,
            },
        );
        let first_menu = first.global::<PlayerCatalogState>();
        let second_menu = second.global::<PlayerCatalogState>();
        assert_eq!(second_menu.get_window_menu_rows().row_count(), 1);
        welcome.show()?;
        owner.refresh();
        let before = first_menu.get_window_menu_rows();
        assert_eq!(before.row_count(), 2);
        assert!(before.row_data(0).unwrap().selected);
        assert_eq!(before.row_data(1).unwrap().label, "Riina");
        assert!(
            second_menu
                .get_window_menu_rows()
                .row_data(1)
                .unwrap()
                .selected
        );
        let second_id = before.row_data(1).unwrap().token.parse()?;
        assert!(matches!(
            owner.destination(second_id),
            Some(SwitchableWindow::Welcome(_))
        ));

        welcome.hide()?;
        second.show()?;
        owner.refresh();
        let loaded = first_menu.get_window_menu_rows();
        assert_eq!(loaded.row_data(1).unwrap().label, "Second movie");
        owner.refresh();
        assert_eq!(loaded, first_menu.get_window_menu_rows());

        second.set_window_title("Renamed & edited movie".into());
        owner.refresh();
        let changed = first_menu.get_window_menu_rows();
        let renamed = if cfg!(target_os = "windows") {
            "Renamed && edited movie"
        } else {
            "Renamed & edited movie"
        };
        assert_eq!(changed.row_data(1).unwrap().label, renamed);
        assert_eq!(
            changed.row_data(1).unwrap().token,
            loaded.row_data(1).unwrap().token
        );
        first
            .global::<PlayerMenuActions>()
            .invoke_select_player_window(changed.row_data(1).unwrap().token);
        let selected = first_menu.get_window_menu_rows();
        assert_ne!(selected, changed);
        assert!(selected.row_data(0).unwrap().selected);
        assert!(!selected.row_data(1).unwrap().selected);
        other.retire();
        assert!(owner.destination(second_id).is_none());
        assert_eq!(first_menu.get_window_menu_rows().row_count(), 1);
        assert!(second.window().is_visible());
        initial.retire();
        assert!(!owner.monitor.running());
        first.hide()?;
        second.hide()?;
        Ok(())
    }
}
