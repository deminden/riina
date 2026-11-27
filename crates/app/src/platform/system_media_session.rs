//! Keep Now Playing controls and application focus events with the player session.
use crate::{NativeVideoHost, PlaybackStore, PlayerWindow};
use riina_mpv::CommandSender;

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows", test))]
fn pause_for_suspend(app: &PlayerWindow, playback: &PlaybackStore) {
    use riina_core::{Phase, sync::lock};
    use slint::ComponentHandle;

    if !app
        .global::<riina_ui::PlayerPreferences>()
        .get_pause_when_goes_to_sleep()
    {
        return;
    }
    let should_pause = {
        let state = lock(playback);
        !state.engine_shutdown
            && !state.idle_active
            && !state.paused
            && (state.phase == Phase::Loading || state.phase.has_loaded_file())
    };
    if should_pause {
        app.global::<riina_ui::PlayerControlActions>()
            .invoke_set_paused(true);
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows", test))]
pub(crate) mod key_actions;

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub(crate) struct SystemMediaSession;

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
impl SystemMediaSession {
    pub(crate) fn new(
        _: &PlayerWindow,
        _: PlaybackStore,
        _: Option<slint::Weak<NativeVideoHost>>,
    ) -> Self {
        Self
    }
    pub(crate) fn bind(&self, _: &CommandSender) {}
    pub(crate) fn prepare_to_close(&self) {}
}

#[cfg(test)]
mod suspend_tests {
    use super::*;
    use riina_core::{Phase, Playback, sync::lock};
    use slint::ComponentHandle;
    use std::{cell::Cell, rc::Rc, sync::Arc};

    #[test]
    fn suspend_pauses_only_active_unpaused_players_with_the_setting_enabled()
    -> Result<(), slint::PlatformError> {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new()?;
        let latest = PlaybackStore::default();
        let requests = Rc::new(Cell::new(0));
        let watched = requests.clone();
        app.global::<riina_ui::PlayerControlActions>()
            .on_set_paused(move |paused| {
                assert!(paused);
                watched.set(watched.get() + 1);
            });
        let prefs = app.global::<riina_ui::PlayerPreferences>();
        for phase in [
            Phase::Playing,
            Phase::Buffering,
            Phase::Seeking,
            Phase::Loading,
            Phase::Stopped,
        ] {
            for (enabled, paused) in [(false, false), (true, true), (true, false)] {
                prefs.set_pause_when_goes_to_sleep(enabled);
                *lock(&latest) = Arc::new(Playback {
                    phase,
                    paused,
                    ..Playback::default()
                });
                pause_for_suspend(&app, &latest);
            }
        }
        *lock(&latest) = Arc::new(Playback {
            phase: Phase::Playing,
            engine_shutdown: true,
            ..Playback::default()
        });
        pause_for_suspend(&app, &latest);
        *lock(&latest) = Arc::new(Playback {
            phase: Phase::Playing,
            idle_active: true,
            ..Playback::default()
        });
        pause_for_suspend(&app, &latest);
        assert_eq!(requests.get(), 4);
        Ok(())
    }
}

#[cfg(any(target_os = "macos", test))]
pub(super) mod ownership;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub(crate) use macos::SystemMediaSession;
#[cfg(target_os = "macos")]
pub(super) use macos::wake_application;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::SystemMediaSession;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub(crate) use linux::SystemMediaSession;

impl SystemMediaSession {
    pub(crate) fn bind_timeline_samples(&self, samples: &super::player_thumbnails::Samples) {
        #[cfg(target_os = "macos")]
        self.store_timeline_samples(samples);
        #[cfg(not(target_os = "macos"))]
        let _ = samples;
    }
}
