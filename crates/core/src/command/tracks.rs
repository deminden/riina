use super::{Command, CommandError, set};
use crate::valid_path;

impl Command {
    pub(super) fn tracks_args(&self) -> Result<Vec<String>, CommandError> {
        let args = match self {
            Self::SelectTrack { kind, id } if id.is_none_or(|id| id > 0) => set(
                kind.property(),
                id.map_or_else(|| "no".into(), |id| id.to_string()),
            ),
            Self::CycleTrack(kind) => {
                // Native cycling owns track ordering and the disabled sentinel;
                // a catalog snapshot can lag external-track insertion/removal.
                vec!["cycle".into(), kind.property().into()]
            }
            Self::SelectSecondarySubtitle(id) if id.is_none_or(|id| id > 0) => set(
                "secondary-sid",
                id.map_or_else(|| "no".into(), |id| id.to_string()),
            ),
            Self::AddSubtitle(path) | Self::AddAudio(path) | Self::AddVideo(path)
                if valid_path(path) =>
            {
                let args = vec![
                    match self {
                        Self::AddAudio(_) => "audio-add",
                        Self::AddVideo(_) => "video-add",
                        _ => "sub-add",
                    }
                    .into(),
                    path.clone(),
                ];
                args
            }
            _ => {
                return Err(CommandError::InvalidArgument {
                    what: "The requested value is out of range or non-finite",
                });
            }
        };
        Ok(args)
    }
}
