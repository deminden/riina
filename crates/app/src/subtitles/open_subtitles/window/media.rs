//! Keep a provider's result on the media that requested it.
use crate::PlayerWindow;
use riina_core::{Command, Phase, Playback};
use riina_mpv::{CommandSender, EngineError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Media {
    pub(super) generation: u64,
    open_request: i32,
}

impl Media {
    pub(super) fn capture(app: &PlayerWindow, state: &Playback) -> Option<Self> {
        active(state).then(|| Self {
            generation: state.generation,
            open_request: app.get_media_request_generation(),
        })
    }

    pub(super) fn is_current(self, app: &PlayerWindow, state: &Playback) -> bool {
        self.is_observed(state) && self.open_request == app.get_media_request_generation()
    }

    pub(super) fn is_observed(self, state: &Playback) -> bool {
        self.generation == state.generation && active(state)
    }

    pub(super) fn attach(
        self,
        sender: &CommandSender,
        paths: Vec<String>,
    ) -> Result<(), EngineError> {
        sender.send_for_media(
            self.generation,
            Command::AddSubtitlesForMedia {
                generation: self.generation,
                paths,
            },
        )
    }
}

fn active(state: &Playback) -> bool {
    !matches!(state.phase, Phase::Loading | Phase::Stopped | Phase::Failed)
}
