//! The UI settles a delivered track catalog once, even when playback is paused.
use crate::PlayerWindow;
use riina_core::{
    Phase, Playback,
    music_mode::{AudioStatus, State},
};
use slint::ComponentHandle;
use std::{cell::Cell, rc::Rc, time::Instant};

pub struct Controller {
    app: slint::Weak<PlayerWindow>,
    state: Cell<State>,
    catalog: Cell<(u64, AudioStatus)>,
    timer: slint::Timer,
    scheduled: Cell<Option<Instant>>,
}

impl Controller {
    pub fn install(app: &PlayerWindow, enabled: bool) -> Rc<Self> {
        let owner = Rc::new(Self {
            app: app.as_weak(),
            state: Cell::new(State::new(enabled)),
            catalog: Cell::new((0, AudioStatus::Unknown)),
            timer: slint::Timer::default(),
            scheduled: Cell::new(None),
        });
        let weak = Rc::downgrade(&owner);
        app.on_auto_switch_music_mode_changed(move |enabled| {
            if let Some(owner) = weak.upgrade() {
                let mut state = owner.state.get();
                state.set_auto_switch(enabled);
                owner.state.set(state);
                owner.evaluate(Instant::now());
            }
        });
        let weak = Rc::downgrade(&owner);
        app.on_automatic_music_observation_changed(move |_| {
            if let Some(owner) = weak.upgrade() {
                owner.evaluate(Instant::now());
            }
        });
        owner
    }

    pub fn observe(self: &Rc<Self>, playback: &Playback) {
        let ready = matches!(
            playback.phase,
            Phase::Playing | Phase::Paused | Phase::Buffering | Phase::Ended
        );
        // Seeking within the item does not interrupt a catalog already delivered.
        if playback.phase == Phase::Seeking && self.catalog.get().0 == playback.generation {
            return;
        }
        let status = if ready {
            playback.audio_status()
        } else {
            AudioStatus::Unknown
        };
        let generation = self.catalog.replace((playback.generation, status)).0;
        if generation != playback.generation {
            let mut state = self.state.get();
            state.reset_candidate();
            self.state.set(state);
        }
        self.evaluate(Instant::now());
    }

    pub fn toggle_manually(self: &Rc<Self>, app: &PlayerWindow) {
        let mut state = self.state.get();
        let active = state.toggle_manually();
        self.state.set(state);
        super::presentation::set_music_mode(app, active);
        self.evaluate(Instant::now());
    }

    fn evaluate(self: &Rc<Self>, now: Instant) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let mut state = self.state.get();
        let (_, status) = self.catalog.get();
        let fullscreen = app.get_fullscreen() || app.get_entering_fullscreen();
        let status = if app.get_loaded() {
            status
        } else {
            AudioStatus::Unknown
        };
        let active = state.observe_at(status, fullscreen, now);
        self.state.set(state);
        self.schedule(state.deadline(), now);
        if let Some(active) = active {
            super::presentation::set_music_mode(&app, active);
        }
    }

    fn schedule(self: &Rc<Self>, deadline: Option<Instant>, now: Instant) {
        if self.scheduled.replace(deadline) == deadline {
            return;
        }
        self.timer.stop();
        if let Some(deadline) = deadline {
            let weak = Rc::downgrade(self);
            self.timer.start(
                slint::TimerMode::SingleShot,
                deadline.saturating_duration_since(now),
                move || {
                    if let Some(owner) = weak.upgrade() {
                        owner.scheduled.set(None);
                        owner.evaluate(Instant::now().max(deadline));
                    }
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use riina_ui::PlayerMusicModeState;
    use std::time::Duration;

    #[test]
    fn only_the_latest_catalog_arms_a_deadline_and_stable_playback_stays_idle()
    -> Result<(), slint::PlatformError> {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new()?;
        app.set_loaded(true);
        let mode = Controller::install(&app, true);
        assert!(!mode.timer.running());
        let mut audio = Playback {
            generation: 1,
            phase: Phase::Paused,
            tracks: vec![
                serde_json::from_value(serde_json::json!({"id": 1, "type": "audio"})).unwrap(),
            ]
            .into(),
            ..Default::default()
        };
        mode.observe(&audio);
        assert!(mode.timer.running());
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(200));
        audio.generation = 2;
        mode.observe(&audio);
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(120));
        let music = app.global::<PlayerMusicModeState>();
        assert!(!music.get_music_mode());
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(181));
        assert!(music.get_music_mode());
        assert!(!mode.timer.running());

        mode.toggle_manually(&app);
        mode.observe(&audio);
        assert!(!mode.timer.running());
        Ok(())
    }
}
