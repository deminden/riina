use super::{Command, set};
use crate::control_preferences;

impl Command {
    pub(super) fn seek_args(&self) -> Result<Vec<String>, String> {
        let args = match self {
            Self::Seek { seconds, absolute } if seconds.is_finite() => vec![
                "seek".into(),
                seconds.to_string(),
                if *absolute {
                    "absolute+exact"
                } else {
                    "relative"
                }
                .into(),
            ],
            Self::SeekWithType { seconds, seek_type } if seconds.is_finite() => vec![
                "seek".into(),
                seconds.to_string(),
                match seek_type {
                    control_preferences::SeekType::Keyframe
                    | control_preferences::SeekType::Auto => "relative",
                    control_preferences::SeekType::Exact => "relative+exact",
                }
                .into(),
            ],
            Self::SelectChapter(index) => set("chapter", index.to_string()),
            Self::AdjustChapter(delta) => {
                // Relative navigation keeps native restart-threshold and boundary
                // semantics; selecting a sidebar chapter intentionally resumes.
                vec!["add".into(), "chapter".into(), delta.to_string()]
            }
            _ => return Err("The requested value is out of range or non-finite".into()),
        };
        Ok(args)
    }
}
