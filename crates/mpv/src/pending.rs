//! Classify native replies while retaining the media generation at submission.
use riina_core::{Command, Playback};

pub(super) struct PendingCommand {
    pub(super) generation: u64,
    pub(super) label: String,
    pub(super) kind: ReplyKind,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum ReplyKind {
    Plain,
    Screenshot { path: Option<String> },
    FrameCapture,
    TransitionCapture,
    IncomingTransitionCapture,
    MaterialCapture,
}

impl ReplyKind {
    pub(super) fn for_command(command: &Command, native_command: Option<&str>) -> Self {
        match command {
            Command::ScreenshotToFile { path, .. } => Self::Screenshot {
                path: Some(path.clone()),
            },
            Command::Screenshot { .. } if native_command == Some("screenshot") => {
                Self::Screenshot { path: None }
            }
            Command::CaptureFrame => Self::FrameCapture,
            Command::CaptureTransitionFrame => Self::TransitionCapture,
            Command::CaptureIncomingTransitionFrame => Self::IncomingTransitionCapture,
            Command::CaptureMaterialFrame => Self::MaterialCapture,
            _ => Self::Plain,
        }
    }
}

impl PendingCommand {
    pub(super) fn belongs_to(&self, state: &Playback) -> bool {
        self.generation == state.generation
            || (self.kind == ReplyKind::TransitionCapture
                && state.generation.saturating_sub(self.generation) <= 1
                && state
                    .transition_frame_generation
                    .is_none_or(|generation| generation <= self.generation))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screenshot_setup_commands_have_plain_replies() {
        let command = Command::Screenshot {
            directory: "fixtures".into(),
            content: riina_core::screenshot::Content::Video,
            encoding: Default::default(),
        };
        assert_eq!(
            ReplyKind::for_command(&command, Some("set")),
            ReplyKind::Plain
        );
        assert_eq!(
            ReplyKind::for_command(&command, Some("screenshot")),
            ReplyKind::Screenshot { path: None },
        );
    }

    #[test]
    fn only_an_outgoing_transition_can_complete_after_a_source_change() {
        let mut state = Playback {
            generation: 8,
            ..Default::default()
        };
        let mut reply = PendingCommand {
            generation: 7,
            label: "screenshot-raw".into(),
            kind: ReplyKind::FrameCapture,
        };
        assert!(!reply.belongs_to(&state));
        reply.kind = ReplyKind::IncomingTransitionCapture;
        assert!(!reply.belongs_to(&state));
        reply.kind = ReplyKind::TransitionCapture;
        assert!(reply.belongs_to(&state));
        state.transition_frame_generation = Some(8);
        assert!(!reply.belongs_to(&state));
        state.transition_frame_generation = None;
        state.generation = 9;
        assert!(!reply.belongs_to(&state));
    }
}
