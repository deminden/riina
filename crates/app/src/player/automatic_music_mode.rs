//! A paused catalog must settle even when the engine emits no further events.
use crate::PlayerWindow;
use riina_core::{
    Phase, Playback,
    music_mode::{AudioStatus, State},
    sync::lock,
};
use slint::ComponentHandle;
use std::{
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

pub fn install(
    app: &PlayerWindow,
    playback: crate::PlaybackStore,
    mode: Arc<Mutex<State>>,
) -> Rc<slint::Timer> {
    let timer = Rc::new(slint::Timer::default());
    let weak = app.as_weak();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(100),
        move || {
            let Some(app) = weak.upgrade() else { return };
            observe(&app, &lock(&playback), &mode);
        },
    );
    if !app.get_observe_automatic_music_mode() {
        timer.stop();
    }
    let weak_timer = Rc::downgrade(&timer);
    app.on_automatic_music_observation_changed(move |enabled| {
        if let Some(timer) = weak_timer.upgrade() {
            if enabled && !timer.running() {
                timer.restart();
            } else if !enabled {
                timer.stop();
            }
        }
    });
    timer
}

// Notifications still apply mode changes before media-window geometry. The
// timer supplies the missing observation when a paused catalog becomes quiet.
pub(crate) fn observe(app: &PlayerWindow, playback: &Playback, mode: &Mutex<State>) {
    let transition = {
        let mut mode = lock(mode);
        let status = if matches!(
            playback.phase,
            Phase::Playing | Phase::Paused | Phase::Buffering | Phase::Ended
        ) {
            playback.audio_status()
        } else {
            AudioStatus::Unknown
        };
        let fullscreen = app.get_fullscreen() || app.get_entering_fullscreen();
        mode.observe(status, fullscreen)
    };
    if let Some(active) = transition {
        super::presentation::set_music_mode(app, active);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_music_timer_runs_only_for_loaded_media_while_enabled()
    -> Result<(), slint::PlatformError> {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new()?;
        let mode = Arc::new(Mutex::new(State::new(true)));
        let timer = install(&app, crate::PlaybackStore::default(), mode);
        assert!(!timer.running());

        app.global::<riina_ui::PlayerMusicModeState>()
            .set_auto_switch_to_music_mode(true);
        app.set_loaded(true);
        i_slint_backend_testing::mock_elapsed_time(Duration::ZERO);
        assert!(timer.running());

        app.global::<riina_ui::PlayerMusicModeState>()
            .set_auto_switch_to_music_mode(false);
        i_slint_backend_testing::mock_elapsed_time(Duration::ZERO);
        assert!(!timer.running());

        app.global::<riina_ui::PlayerMusicModeState>()
            .set_auto_switch_to_music_mode(true);
        i_slint_backend_testing::mock_elapsed_time(Duration::ZERO);
        assert!(timer.running());
        app.set_loaded(false);
        i_slint_backend_testing::mock_elapsed_time(Duration::ZERO);
        assert!(!timer.running());
        Ok(())
    }
}
