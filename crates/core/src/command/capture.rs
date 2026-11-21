use super::{Command, cmd};
use crate::valid_path;
use crate::{Phase, Playback};

impl Command {
    pub(super) fn capture_args(&self) -> Result<Vec<String>, String> {
        let args = match self {
            Self::CaptureFrame => cmd(["screenshot-raw", "video", "rgba"]),
            Self::CaptureTransitionFrame => cmd(["screenshot-raw", "video", "rgba"]),
            Self::CaptureIncomingTransitionFrame => cmd(["screenshot-raw", "video", "rgba"]),
            Self::CaptureMaterialFrame => cmd(["screenshot-raw", "video", "rgba"]),
            Self::ScreenshotToFile { path, content } if valid_path(path) => vec![
                "screenshot-to-file".into(),
                path.clone(),
                content.native_value().into(),
            ],
            _ => return Err("The requested value is out of range or non-finite".into()),
        };
        Ok(args)
    }
}

impl Command {
    pub(super) fn validate_capture(&self, state: &Playback) -> Result<(), String> {
        if matches!(self, Self::CaptureFrame) && state.phase != Phase::Paused {
            return Err("Text recognition is available only while video is paused".into());
        }
        if matches!(
            self,
            Self::CaptureTransitionFrame
                | Self::CaptureIncomingTransitionFrame
                | Self::CaptureMaterialFrame
        ) && (state.video.is_none()
            || matches!(state.phase, Phase::Loading | Phase::Stopped | Phase::Failed))
        {
            return Err("A transition image requires an active video".into());
        }
        Ok(())
    }
}
