//! Native focus ownership and optional gesture routing for one player window.
use crate::PlayerWindow;
use riina_mpv::CommandSender;
use std::cell::Cell;

#[cfg(target_os = "macos")]
mod mac_gestures;

#[cfg(target_os = "macos")]
thread_local! {
    static FOCUSED_PLAYER_WINDOWS: Cell<usize> = const { Cell::new(0) };
}

pub(crate) struct PlayerFocus(Cell<bool>);

impl PlayerFocus {
    pub(crate) fn new(focused: bool) -> Self {
        let focus = Self(Cell::new(false));
        focus.update(focused);
        focus
    }

    pub(crate) fn update(&self, focused: bool) {
        let changed = self.0.replace(focused) != focused;
        #[cfg(target_os = "macos")]
        if changed {
            FOCUSED_PLAYER_WINDOWS.with(|count| {
                count.set(if focused {
                    count.get().saturating_add(1)
                } else {
                    count.get().saturating_sub(1)
                });
            });
        }
        #[cfg(not(target_os = "macos"))]
        let _ = changed;
    }
}

impl Drop for PlayerFocus {
    fn drop(&mut self) {
        self.update(false);
    }
}

#[cfg(target_os = "macos")]
pub(super) fn some_player_window_is_focused() -> bool {
    FOCUSED_PLAYER_WINDOWS.with(|count| count.get() > 0)
}

pub(crate) fn install_player_gestures(
    app: &PlayerWindow,
    player: &CommandSender,
    dispatch: fn(&PlayerWindow, &CommandSender, i32),
) {
    #[cfg(target_os = "macos")]
    mac_gestures::install(app, player.clone(), dispatch);
    #[cfg(not(target_os = "macos"))]
    let _ = (app, player, dispatch);
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct NativeScrollEvent {
    pub(crate) delta_x: f64,
    pub(crate) delta_y: f64,
    pub(crate) precise: bool,
    pub(crate) natural_direction: bool,
    pub(crate) phase_empty: bool,
    pub(crate) phase_began: bool,
    pub(crate) phase_ended: bool,
    pub(crate) momentum_phase_empty: bool,
}
// The monitor is dropped with the session; other hosts deliver wheel input directly.
pub(crate) struct PlayerScrollMonitor {
    #[cfg(target_os = "macos")]
    _native: Option<super::macos::MacScrollEventMonitor>,
}

impl PlayerScrollMonitor {
    pub(crate) fn install(
        app: &PlayerWindow,
        callback: impl Fn(NativeScrollEvent) + 'static,
    ) -> Self {
        #[cfg(target_os = "macos")]
        {
            let monitor = match super::macos::MacScrollEventMonitor::new(callback) {
                Ok(monitor) => Some(monitor),
                Err(error) => {
                    app.set_status(format!("Native trackpad input unavailable: {error}").into());
                    None
                }
            };
            Self { _native: monitor }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (app, callback);
            Self {}
        }
    }
}
