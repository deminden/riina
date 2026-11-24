//! Coordinate idle power requests across the players of one application.
mod settings;

use crate::{PlayerWindow, PreferencesWindow, platform::playback_power::Controller};
use riina_core::playback_power::{Mode, Settings};
use riina_ui::PlaybackPowerActivity;
use slint::Weak as WeakComponent;
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::{Rc, Weak},
};

#[derive(Default)]
pub(crate) struct ApplicationPower {
    settings: Cell<Settings>,
    initialized: Cell<bool>,
    windows: RefCell<Vec<WeakComponent<PreferencesWindow>>>,
    players: RefCell<BTreeMap<u64, PlaybackPowerActivity>>,
    next: Cell<u64>,
    current: Cell<Mode>,
    controller: RefCell<Controller>,
}

impl ApplicationPower {
    pub(crate) fn initialize(&self, settings: Settings) {
        if !self.initialized.replace(true) {
            self.settings.set(settings);
            self.reconcile();
        }
    }

    pub(crate) fn attach(self: &Rc<Self>, app: &PlayerWindow) -> Registration {
        let id = self.next.get();
        self.next.set(id + 1);
        self.players
            .borrow_mut()
            .insert(id, PlaybackPowerActivity::Idle);
        let owner = Rc::downgrade(self);
        app.on_playback_power_changed(move |activity| {
            if let Some(owner) = owner.upgrade() {
                owner.observe(id, activity);
            }
        });
        Registration {
            id,
            owner: Rc::downgrade(self),
        }
    }

    fn observe(&self, id: u64, activity: PlaybackPowerActivity) {
        if let Some(previous) = self.players.borrow_mut().get_mut(&id) {
            if *previous == activity {
                return;
            }
            *previous = activity;
        } else {
            return;
        }
        self.reconcile();
    }

    fn reconcile(&self) {
        let next = request(self.settings.get(), self.players.borrow().values().copied());
        if self.current.replace(next) != next {
            self.controller.borrow_mut().set(next);
        }
    }
}

fn request(settings: Settings, mut players: impl Iterator<Item = PlaybackPowerActivity>) -> Mode {
    // IINA chooses the first actively playing player in creation order.
    settings.request(players.find_map(|activity| match activity {
        PlaybackPowerActivity::Idle => None,
        PlaybackPowerActivity::Audio => Some(true),
        PlaybackPowerActivity::Video => Some(false),
    }))
}

pub(crate) struct Registration {
    id: u64,
    owner: Weak<ApplicationPower>,
}

impl Drop for Registration {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.upgrade() {
            owner.players.borrow_mut().remove(&self.id);
            owner.reconcile();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use PlaybackPowerActivity::{Audio, Idle, Video};

    #[test]
    fn first_playing_window_controls_the_idle_request() {
        let default = Settings::default();
        let audio_allowed = Settings {
            allow_screen_saver_for_audio: true,
            ..default
        };
        let disabled = Settings {
            prevent_screen_saver: false,
            ..audio_allowed
        };
        assert_eq!(request(default, [Idle, Audio].into_iter()), Mode::Display);
        assert_eq!(
            request(audio_allowed, [Audio, Video].into_iter()),
            Mode::System
        );
        assert_eq!(
            request(audio_allowed, [Idle, Video].into_iter()),
            Mode::Display
        );
        assert_eq!(request(audio_allowed, [Idle].into_iter()), Mode::Idle);
        assert_eq!(request(disabled, [Video].into_iter()), Mode::Idle);
    }

    #[test]
    fn a_closed_player_cannot_restore_its_request_with_a_late_callback() {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new().unwrap();
        let service = Rc::new(ApplicationPower::default());
        service.initialize(Settings {
            prevent_screen_saver: false,
            ..Settings::default()
        });
        let registration = service.attach(&app);
        app.invoke_playback_power_changed(Video);
        assert_eq!(service.players.borrow()[&registration.id], Video);

        drop(registration);
        app.invoke_playback_power_changed(Audio);
        assert!(service.players.borrow().is_empty());
        assert_eq!(service.current.get(), Mode::Idle);
    }
}
