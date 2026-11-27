//! Classify native replies while retaining the media generation at submission.
use riina_core::{Command, FilterChange, Playback, TrackKind};

pub(super) struct PendingCommand {
    pub(super) generation: u64,
    pub(super) label: String,
    pub(super) kind: ReplyKind,
    pub(super) changes_media: bool,
    pub(super) changes_watch_later: bool,
}

#[derive(Debug, PartialEq)]
pub(super) enum ReplyKind {
    Plain,
    RememberVolume(f64),
    RememberAdjustedVolume,
    PlaylistEdit { batch: u64, lead: bool },
    ChapterSelection { index: usize, start: f64 },
    FilterEdit(FilterChange),
    TrackAttachment(TrackKind),
    MatchedSubtitle,
    Screenshot { path: Option<String> },
    FrameCapture,
    TransitionCapture,
    IncomingTransitionCapture,
    MaterialCapture,
}

impl ReplyKind {
    pub(super) fn for_filter_edit(args: &[String], name: String) -> Option<Self> {
        match args {
            [chain, edit, _] if matches!(chain.as_str(), "vf" | "af") => match edit.as_str() {
                "add" => Some(Self::FilterEdit(FilterChange::Added(name.into()))),
                "set" => Some(Self::FilterEdit(FilterChange::Removed)),
                _ => None,
            },
            _ => None,
        }
    }

    pub(super) fn for_command(command: &Command, native_command: Option<&str>) -> Self {
        match command {
            Command::AddAudio(_) => Self::TrackAttachment(TrackKind::Audio),
            Command::AddVideo(_) => Self::TrackAttachment(TrackKind::Video),
            Command::AddSubtitle(_) if native_command == Some("sub-add") => {
                Self::TrackAttachment(TrackKind::Sub)
            }
            Command::AddMatchedSubtitles { .. } if native_command == Some("sub-add") => {
                Self::MatchedSubtitle
            }
            Command::ScreenshotToFile { path, .. } => Self::Screenshot {
                path: Some(path.clone()),
            },
            Command::Screenshot { .. } if native_command == Some("screenshot") => {
                Self::Screenshot { path: None }
            }
            Command::ScreenshotBinding { binding, .. }
                if native_command == Some("screenshot") && binding.reports_completion() =>
            {
                Self::Screenshot { path: None }
            }
            Command::AdjustVolume { .. } if native_command == Some("add") => {
                Self::RememberAdjustedVolume
            }
            Command::ActivateChapter {
                index,
                start,
                notify: true,
            } if native_command == Some("seek") => Self::ChapterSelection {
                index: *index,
                start: *start,
            },
            Command::CaptureFrame | Command::CaptureThumbnailFrame => Self::FrameCapture,
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
            || matches!(
                self.kind,
                ReplyKind::RememberVolume(_) | ReplyKind::RememberAdjustedVolume
            )
            || (self.kind == ReplyKind::TransitionCapture
                && state.generation.saturating_sub(self.generation) <= 1
                && state
                    .transition_frame
                    .generation
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
            changes_media: false,
            changes_watch_later: false,
        };
        assert!(!reply.belongs_to(&state));
        reply.kind = ReplyKind::IncomingTransitionCapture;
        assert!(!reply.belongs_to(&state));
        reply.kind = ReplyKind::TransitionCapture;
        assert!(reply.belongs_to(&state));
        state.transition_frame.generation = Some(8);
        assert!(!reply.belongs_to(&state));
        state.transition_frame.generation = None;
        state.generation = 9;
        assert!(!reply.belongs_to(&state));
    }
}
