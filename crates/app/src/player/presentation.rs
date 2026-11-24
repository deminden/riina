//! Publish presentation changes at the point where the player commits them.
use crate::PlayerWindow;
use riina_ui::{PlayerMusicModeState, PlayerWindowMode};
use slint::ComponentHandle;

pub(crate) fn set_music_mode(app: &PlayerWindow, active: bool) {
    let state = app.global::<PlayerMusicModeState>();
    if state.get_music_mode() != active {
        state.set_music_mode(active);
        app.invoke_window_mode_confirmed(PlayerWindowMode::MusicMode, active);
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
pub(crate) fn set_picture_in_picture(app: &PlayerWindow, active: bool) {
    if app.get_picture_in_picture_active() != active {
        app.set_picture_in_picture_active(active);
        app.invoke_window_mode_confirmed(PlayerWindowMode::PictureInPicture, active);
    }
}
