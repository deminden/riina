//! A paused catalog must settle even when the engine emits no further events.
use crate::PlayerWindow;
use riina_core::{
    Phase, Playback,
    music_mode::{AudioStatus, State},
};
use slint::ComponentHandle;
use std::{
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

pub(crate) fn install(
    app: &PlayerWindow,
    playback: Arc<Mutex<Playback>>,
    mode: Arc<Mutex<State>>,
) -> Rc<slint::Timer> {
    let timer = Rc::new(slint::Timer::default());
    let weak = app.as_weak();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(100),
        move || {
            let Some(app) = weak.upgrade() else { return };
            if let Ok(playback) = playback.lock() {
                observe(&app, &playback, &mode);
            }
        },
    );
    timer
}

// Notifications still apply mode changes before media-window geometry. The
// timer supplies the missing observation when a paused catalog becomes quiet.
pub(crate) fn observe(app: &PlayerWindow, playback: &Playback, mode: &Mutex<State>) {
    let transition = {
        let Ok(mut mode) = mode.lock() else { return };
        let status = if matches!(
            playback.phase,
            Phase::Playing | Phase::Paused | Phase::Buffering | Phase::Ended
        ) {
            playback.audio_status()
        } else {
            AudioStatus::Unknown
        };
        mode.observe(status, app.get_fullscreen())
    };
    if let Some(active) = transition {
        app.set_music_mode(active);
    }
}
