use super::{Command, cmd};
use crate::valid_path;

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
