use super::{Command, CommandError, cmd};
use crate::valid_path;
use crate::{Phase, Playback};

impl Command {
    pub(super) fn capture_args(&self) -> Result<Vec<String>, CommandError> {
        let args = match self {
            Self::CaptureFrame
            | Self::CaptureThumbnailFrame
            | Self::CaptureTransitionFrame
            | Self::CaptureIncomingTransitionFrame
            | Self::CaptureMaterialFrame => cmd(["screenshot-raw", "video", "rgba"]),
            Self::ScreenshotToFile { path, content } if valid_path(path) => vec![
                "screenshot-to-file".into(),
                path.clone(),
                content.native_value().into(),
            ],
            _ => {
                return Err(CommandError::InvalidArgument {
                    what: "The requested value is out of range or non-finite",
                });
            }
        };
        Ok(args)
    }
}

impl Command {
    pub(super) fn validate_capture(&self, state: &Playback) -> Result<(), CommandError> {
        if matches!(self, Self::CaptureFrame) && state.phase != Phase::Paused {
            return Err(CommandError::InvalidState {
                reason: "Text recognition is available only while video is paused",
            });
        }
        // A paused thumbnail decoder can reach EOF while retaining its final frame.
        // Live Text still requires the interactive player to be fully paused.
        if matches!(self, Self::CaptureThumbnailFrame)
            && (!state.paused
                || matches!(state.phase, Phase::Stopped | Phase::Loading | Phase::Failed))
        {
            return Err(CommandError::InvalidState {
                reason: "Thumbnail extraction requires an active, paused decoder",
            });
        }
        if matches!(
            self,
            Self::CaptureTransitionFrame
                | Self::CaptureIncomingTransitionFrame
                | Self::CaptureMaterialFrame
        ) && (state.video.is_none()
            || matches!(state.phase, Phase::Loading | Phase::Stopped | Phase::Failed))
        {
            return Err(CommandError::InvalidState {
                reason: "A transition image requires an active video",
            });
        }
        Ok(())
    }
}
