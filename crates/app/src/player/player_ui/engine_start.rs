use crate::platform;
use riina_core::Playback;
use riina_mpv::{CommandSender, Player};
use std::{path::PathBuf, sync::Arc};

pub(super) type EngineOptions = Vec<(String, String)>;

pub(super) enum EngineStart {
    Native,
    #[cfg(test)]
    Recording {
        engine: std::rc::Rc<riina_mpv::RecordingEngine>,
        commands: CommandSender,
        profile: PathBuf,
        capture_before_navigation: bool,
        startup_options: std::rc::Rc<std::cell::RefCell<EngineOptions>>,
    },
}

impl EngineStart {
    pub(super) fn preferences_path(&self) -> std::io::Result<PathBuf> {
        match self {
            Self::Native => platform::preferences_path(),
            #[cfg(test)]
            Self::Recording { profile, .. } => Ok(profile.clone()),
        }
    }

    pub(super) fn uses_system_services(&self) -> bool {
        match self {
            Self::Native => true,
            #[cfg(test)]
            // Keep tests independent of the user's credential store and installed OCR.
            Self::Recording { .. } => false,
        }
    }

    pub(super) fn captures_before_navigation(&self, native_window: bool) -> bool {
        match self {
            Self::Native => cfg!(target_os = "windows") && native_window,
            #[cfg(test)]
            Self::Recording {
                capture_before_navigation,
                ..
            } => *capture_before_navigation,
        }
    }

    pub(super) fn start(
        self,
        native: Option<&platform::NativePresentation>,
        options: EngineOptions,
        notify: impl Fn(Arc<Playback>) + Send + 'static,
    ) -> Result<(Option<Player>, CommandSender), String> {
        match self {
            Self::Native => {
                let player = if let Some(presentation) = native {
                    presentation.start_engine(options, notify)
                } else {
                    Player::start_with_options(
                        &platform::playback_library(),
                        false,
                        options,
                        notify,
                    )
                }?;
                let commands = player.command_sender();
                Ok((Some(player), commands))
            }
            #[cfg(test)]
            Self::Recording {
                engine,
                commands,
                startup_options,
                ..
            } => {
                startup_options.replace(options);
                engine.set_observer(notify)?;
                Ok((None, commands))
            }
        }
    }
}
