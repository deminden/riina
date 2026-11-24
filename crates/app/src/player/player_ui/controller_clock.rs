//! Share saved controller options among players using the same profile.

use super::PlayerContext;
use crate::{PlayerWindow, osc_preferences};
use riina_core::{
    Playback,
    osc_preferences::{ControllerOptions, TimePrecision},
};
use riina_ui::{PlaybackTimelineState, PlayerControlActions};
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    path::Path,
    rc::{Rc, Weak},
};

#[derive(Default)]
pub(super) struct ApplicationClocks {
    players: RefCell<Vec<Weak<PlayerContext>>>,
    pub(super) form: crate::platform::preferences::ControllerOptionsForm,
}

impl ApplicationClocks {
    pub(super) fn connect(self: &Rc<Self>, app: &PlayerWindow, context: &PlayerContext) {
        let player = Rc::new(context.clone());
        let mut players = self.players.borrow_mut();
        players.retain(|target| target.strong_count() > 0);
        players.push(Rc::downgrade(&player));
        drop(players);

        let actions = app.global::<PlayerControlActions>();
        crate::platform::bind_controller_alignment_feedback(app);
        let clocks = Rc::clone(self);
        actions.on_publish_controller_options(move || {
            if let Some(app) = player.app() {
                clocks.publish(
                    player.preferences_path(),
                    osc_preferences::player_controller_options(&app),
                );
            }
        });
        let toggle = context.clone();
        actions.on_toggle_time_display(move || {
            save(&toggle, |options| {
                options.show_remaining_time = !options.show_remaining_time;
            });
        });
        let select = context.clone();
        actions.on_select_time_precision(move |index| {
            if let Some(precision) = TimePrecision::from_index(index) {
                save(&select, |options| options.time_precision = precision);
            }
        });
    }

    pub(super) fn publish(&self, path: &Path, options: ControllerOptions) {
        self.publish_players(path, options);
        self.form.publish(path, options);
    }

    pub(super) fn publish_players(&self, path: &Path, options: ControllerOptions) {
        self.players.borrow_mut().retain(|target| {
            let Some(target) = target.upgrade() else {
                return false;
            };
            let Some(app) = target.app() else {
                return false;
            };
            if target.preferences_path() == path {
                target
                    .preferences()
                    .adopt_committed(|saved| saved.control_bar.controller = options);
                osc_preferences::install_player_controller_options(&app, options);
                let state = riina_core::sync::lock(target.playback()).clone();
                update_labels(&app, &state);
                app.global::<PlayerControlActions>()
                    .invoke_refresh_controller_options();
            }
            true
        });
    }
}

pub(super) fn update_labels(app: &PlayerWindow, state: &Playback) {
    let options = osc_preferences::player_controller_options(app);
    let timeline = app.global::<PlaybackTimelineState>();
    timeline.set_position_label(options.elapsed_label(state.position).into());
    timeline.set_duration_label(options.end_label(state).into());
}

fn save(context: &PlayerContext, edit: impl FnOnce(&mut ControllerOptions)) {
    let Some(app) = context.app() else {
        return;
    };
    let model = context.preferences();
    let mut candidate = model.snapshot();
    edit(&mut candidate.control_bar.controller);
    let options = candidate.control_bar.controller;
    if let Err(error) = model.save(candidate) {
        context.report(error);
        return;
    }
    osc_preferences::install_player_controller_options(&app, options);
    app.global::<PlayerControlActions>()
        .invoke_publish_controller_options();
    context.activity();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preferences::model::PreferenceModel;
    use riina_core::Preferences;
    use std::sync::{Arc, Mutex};

    #[test]
    fn controller_gestures_update_paused_players_and_merge_with_other_edits()
    -> Result<(), Box<dyn std::error::Error>> {
        i_slint_backend_testing::init_no_event_loop();
        let directory = riina_test_support::TempDir::new("controller-clock")?;
        let path = directory.path().join("preferences.json");
        let clocks = Rc::new(ApplicationClocks::default());
        let first = PlayerWindow::new()?;
        let second = PlayerWindow::new()?;
        let (commands, _engine) = riina_mpv::CommandSender::recording(8);
        let playback = Arc::new(Mutex::new(Arc::new(Playback {
            duration: Some(60.0),
            position: Some(30.0),
            remaining_time: Some(30.0),
            scaled_remaining_time: Some(15.0),
            paused: true,
            ..Default::default()
        })));
        let attach = |app| {
            let model = PreferenceModel::new(path.clone(), Preferences::default());
            let context = PlayerContext::new(
                app,
                commands.clone(),
                playback.clone(),
                model.clone(),
                None,
                path.clone(),
            );
            clocks.connect(app, &context);
            model
        };
        let first_model = attach(&first);
        let second_model = attach(&second);
        first
            .global::<PlayerControlActions>()
            .invoke_toggle_time_display();
        second
            .global::<PlayerControlActions>()
            .invoke_select_time_precision(3);
        for app in [&first, &second] {
            let timeline = app.global::<PlaybackTimelineState>();
            assert_eq!(timeline.get_position_label(), "00:30.000");
            assert_eq!(timeline.get_duration_label(), "-00:30.000");
        }

        let mut edited = first_model.snapshot();
        edited.volume = 37.0;
        first_model.save(edited)?;
        let saved = Preferences::parse(&std::fs::read_to_string(&path)?)?;
        assert_eq!(
            saved.control_bar.controller,
            second_model.snapshot().control_bar.controller
        );
        assert!(saved.control_bar.controller.show_remaining_time);
        assert_eq!(
            saved.control_bar.controller.time_precision,
            TimePrecision::Milliseconds
        );
        assert!((saved.volume - 37.0).abs() < f64::EPSILON);
        Ok(())
    }
}
