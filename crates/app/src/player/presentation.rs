//! Publish presentation changes at the point where the player commits them.
use crate::PlayerWindow;
use riina_ui::{PlayerMusicModeState, PlayerOsdState, PlayerPreferences, PlayerWindowMode};
use slint::ComponentHandle;

pub(crate) fn set_music_mode(app: &PlayerWindow, active: bool) {
    let state = app.global::<PlayerMusicModeState>();
    if state.get_music_mode() != active {
        state.set_music_mode(active);
        if active {
            // Retire the main player's queued feedback before Music Mode can
            // produce its own inline readout.
            app.invoke_osd_enabled_changed(false);
            app.global::<PlayerOsdState>()
                .set_plugin_osd_message("".into());
        }
        app.invoke_window_mode_confirmed(PlayerWindowMode::MusicMode, active);
    }
}

pub(super) fn playback_osd_allowed(app: &PlayerWindow) -> bool {
    !app.global::<PlayerMusicModeState>().get_music_mode()
        && app.global::<PlayerPreferences>().get_osd_enabled()
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
pub(crate) fn set_picture_in_picture(app: &PlayerWindow, active: bool) {
    if app.get_picture_in_picture_active() != active {
        app.set_picture_in_picture_active(active);
        app.invoke_window_mode_confirmed(PlayerWindowMode::PictureInPicture, active);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};

    #[test]
    fn entering_music_mode_retires_feedback_without_editing_the_osd_preference()
    -> Result<(), slint::PlatformError> {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new()?;
        let preferences = app.global::<PlayerPreferences>();
        preferences.set_osd_enabled(true);
        let plugin = app.global::<PlayerOsdState>();
        plugin.set_plugin_osd_message("Active plugin message".into());
        let retired = Rc::new(Cell::new(false));
        let observed = retired.clone();
        app.on_osd_enabled_changed(move |enabled| observed.set(!enabled));

        assert!(playback_osd_allowed(&app));
        set_music_mode(&app, true);
        assert!(retired.get());
        assert!(!playback_osd_allowed(&app));
        assert!(plugin.get_plugin_osd_message().is_empty());
        assert!(preferences.get_osd_enabled());
        set_music_mode(&app, false);
        assert!(playback_osd_allowed(&app));
        Ok(())
    }
}
