//! Keep the active macOS display profile bound to the current GL surface.
use crate::{PlayerWindow, platform};
use riina_core::Command;
use riina_mpv::{Player, video::VideoSurface};
use slint::ComponentHandle;

#[derive(Default)]
pub(super) struct DisplayColor {
    profile: Option<(usize, Option<Vec<u8>>, bool)>,
    active: Option<bool>,
}

impl DisplayColor {
    pub(super) fn update(&mut self, app: &PlayerWindow, video: &mut VideoSurface, player: &Player) {
        let player_preferences = app.global::<riina_ui::PlayerPreferences>();
        if let Some(display) = platform::display_identity(app.window()) {
            if !player_preferences.get_load_icc_profile()
                && let Some((cached_display, profile, _)) = self.profile.take()
            {
                self.profile = Some((cached_display, profile, false));
            }
            let needs_profile =
                self.profile
                    .as_ref()
                    .is_none_or(|(cached_display, _, attempted)| {
                        *cached_display != display
                            || (player_preferences.get_load_icc_profile() && !attempted)
                    });
            if needs_profile {
                let attempted = player_preferences.get_load_icc_profile();
                let mut profile = attempted
                    .then(|| platform::icc_profile_for_window(app.window()))
                    .flatten();
                if let Some(bytes) = profile.as_deref() {
                    // SAFETY: this callback runs on Slint's current GL render context.
                    if let Err(error) = unsafe { video.set_icc_profile(bytes) } {
                        crate::diagnostics::warn!(
                            "Could not install the macOS display ICC profile: {error}"
                        );
                        profile = None;
                    }
                }
                self.profile = Some((display, profile, attempted));
            }
            let profile_available = self
                .profile
                .as_ref()
                .is_some_and(|(_, profile, _)| profile.is_some());
            let video = app.global::<riina_ui::PlaybackVideoState>();
            let hdr_output_active = video.get_hdr_source_detected()
                && video.get_hdr_enabled()
                && video.get_hdr_display_active();
            let should_use_icc = player_preferences.get_load_icc_profile()
                && profile_available
                && !hdr_output_active;
            if self.active != Some(should_use_icc) {
                if let Err(error) = player.send(Command::SetIccProfileAuto(should_use_icc)) {
                    crate::diagnostics::warn!(
                        "Could not update macOS display ICC processing: {error}"
                    );
                } else {
                    self.active = Some(should_use_icc);
                }
            }
        }
    }
}
