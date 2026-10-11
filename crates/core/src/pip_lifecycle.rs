//! Cross-platform decisions for entering and leaving Picture-in-Picture.

use crate::pip_preferences::{Settings, WindowBehavior};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MainWindowState {
    pub fullscreen: bool,
    pub minimized: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EntryEffects {
    pub window_behavior: WindowBehavior,
    pub pause_playback: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExitEffects {
    pub restore_main_window: bool,
    pub pause_playback: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Session {
    restore_main_window: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Off,
    Entering,
    Active,
    Exiting,
}

impl Session {
    /// IINA avoids hiding/minimizing an already fullscreen or minimized window.
    pub const fn enter(settings: Settings, window: MainWindowState) -> (Self, EntryEffects) {
        let window_behavior = if window.fullscreen || window.minimized {
            WindowBehavior::DoNothing
        } else {
            settings.window_behavior
        };
        let restore_main_window = matches!(
            window_behavior,
            WindowBehavior::Hide | WindowBehavior::Minimize
        );

        (
            Self {
                restore_main_window,
            },
            EntryEffects {
                window_behavior,
                pause_playback: settings.pause_when_entering,
            },
        )
    }

    /// Only undo the main-window action this PiP session initiated.
    pub const fn should_restore_main_window(self) -> bool {
        self.restore_main_window
    }

    /// IINA pauses the player once its Picture-in-Picture surface closes.
    pub const fn exit(self) -> ExitEffects {
        ExitEffects {
            restore_main_window: self.restore_main_window,
            pause_playback: true,
        }
    }
}

/// Return the PiP state requested by a minimize/restore event, if this setting
/// applies to the current media. `None` leaves unrelated window events alone.
pub const fn minimize_toggle_target(
    settings: Settings,
    is_video: bool,
    minimized: bool,
    phase: Phase,
) -> Option<bool> {
    if !settings.toggle_by_minimizing || (settings.minimize_toggle_video_only && !is_video) {
        return None;
    }

    if minimized && matches!(phase, Phase::Off) {
        Some(true)
    } else if !minimized && matches!(phase, Phase::Active) {
        Some(false)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{MainWindowState, Phase, Session, minimize_toggle_target};
    use crate::pip_preferences::{Settings, WindowBehavior};

    #[test]
    fn configured_window_policy_is_overridden_for_fullscreen_or_minimized_windows() {
        let settings = Settings {
            window_behavior: WindowBehavior::Hide,
            ..Settings::default()
        };

        for window in [
            MainWindowState {
                fullscreen: true,
                minimized: false,
            },
            MainWindowState {
                fullscreen: false,
                minimized: true,
            },
        ] {
            let (session, effects) = Session::enter(settings, window);
            assert_eq!(effects.window_behavior, WindowBehavior::DoNothing);
            assert!(!session.should_restore_main_window());
        }
    }

    #[test]
    fn session_restores_only_windows_hidden_or_minimized_by_its_policy() {
        for behavior in [WindowBehavior::Hide, WindowBehavior::Minimize] {
            let (session, effects) = Session::enter(
                Settings {
                    window_behavior: behavior,
                    pause_when_entering: true,
                    ..Settings::default()
                },
                MainWindowState::default(),
            );
            assert_eq!(effects.window_behavior, behavior);
            assert!(effects.pause_playback);
            assert!(session.should_restore_main_window());
        }

        let (session, effects) = Session::enter(
            Settings {
                window_behavior: WindowBehavior::DoNothing,
                ..Settings::default()
            },
            MainWindowState::default(),
        );
        assert!(!effects.pause_playback);
        assert!(!session.should_restore_main_window());
    }

    #[test]
    fn exiting_picture_in_picture_pauses_and_restores_only_session_hidden_windows() {
        for behavior in [WindowBehavior::Hide, WindowBehavior::Minimize] {
            let (session, _) = Session::enter(
                Settings {
                    window_behavior: behavior,
                    ..Settings::default()
                },
                MainWindowState::default(),
            );
            assert_eq!(
                session.exit(),
                super::ExitEffects {
                    restore_main_window: true,
                    pause_playback: true,
                }
            );
        }

        let (session, _) = Session::enter(
            Settings {
                window_behavior: WindowBehavior::DoNothing,
                ..Settings::default()
            },
            MainWindowState::default(),
        );
        assert_eq!(
            session.exit(),
            super::ExitEffects {
                restore_main_window: false,
                pause_playback: true,
            }
        );
    }

    #[test]
    fn minimize_toggle_respects_enablement_video_only_and_current_pip_state() {
        let settings = Settings {
            toggle_by_minimizing: true,
            minimize_toggle_video_only: true,
            ..Settings::default()
        };

        assert_eq!(
            minimize_toggle_target(settings, false, true, Phase::Off),
            None
        );
        assert_eq!(
            minimize_toggle_target(settings, true, true, Phase::Off),
            Some(true)
        );
        assert_eq!(
            minimize_toggle_target(settings, true, false, Phase::Active),
            Some(false)
        );
        assert_eq!(
            minimize_toggle_target(settings, true, false, Phase::Off),
            None
        );
        assert_eq!(
            minimize_toggle_target(settings, true, true, Phase::Active),
            None
        );
        for phase in [Phase::Entering, Phase::Exiting] {
            assert_eq!(minimize_toggle_target(settings, true, true, phase), None);
            assert_eq!(minimize_toggle_target(settings, true, false, phase), None);
        }
        assert_eq!(
            minimize_toggle_target(Settings::default(), true, true, Phase::Off),
            None
        );
    }
}
