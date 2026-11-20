//! Shared fullscreen policy follows an accepted platform transition.
use crate::{NativeVideoHost, PlayerWindow};
use slint::ComponentHandle;

#[cfg(test)]
mod tests;

pub(super) fn install_fullscreen(
    app: &PlayerWindow,
    fullscreen_host: Option<slint::Weak<NativeVideoHost>>,
) {
    let weak = app.as_weak();
    app.on_toggle_fullscreen(move || {
        if let Some(app) = weak.upgrade() {
            app.invoke_activity();
            let fullscreen = !app.get_fullscreen();
            match fullscreen_host.as_ref() {
                Some(host) => {
                    let Some(host) = host.upgrade() else { return };
                    if !host.invoke_set_fullscreen_window(fullscreen) {
                        return;
                    }
                }
                None => app.window().set_fullscreen(fullscreen),
            }
            app.set_fullscreen(fullscreen);
            if fullscreen
                && app.get_loaded()
                && app.get_paused()
                && app.get_play_when_entering_fullscreen()
            {
                app.invoke_set_paused(false);
            } else if !fullscreen
                && app.get_loaded()
                && !app.get_paused()
                && app.get_pause_when_leaving_fullscreen()
            {
                app.invoke_set_paused(true);
            }
        }
    });
}
