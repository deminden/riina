use super::{Command, cmd, flag, set};
use crate::{playlist_end_policy, valid_path};

impl Command {
    pub(super) fn playback_args(&self) -> Result<Vec<String>, String> {
        let args = match self {
            Self::Load(path) if !path.is_empty() && !path.contains('\0') => {
                vec!["loadfile".into(), path.clone(), "replace".into()]
            }
            Self::Load(_) => return Err("The media path is empty or contains a NUL byte".into()),
            Self::LoadAtPosition { path, position }
                if valid_path(path) && position.is_finite() && *position >= 0.0 =>
            {
                vec![
                    "loadfile".into(),
                    path.clone(),
                    "replace".into(),
                    "-1".into(),
                    format!("start={position}"),
                ]
            }
            Self::LoadAtPosition { .. } => {
                return Err("The resume file or position is invalid".into());
            }
            Self::SetPaused(paused) => set("pause", flag(*paused)),
            Self::TogglePause => cmd(["cycle", "pause"]),
            Self::SetPlaybackEndPolicy {
                keep_open_on_file_end,
                playlist_auto_play_next,
            } => set(
                "keep-open",
                playlist_end_policy(*keep_open_on_file_end, *playlist_auto_play_next),
            ),
            Self::Stop => cmd(["stop"]),
            // The reference's custom field has no upper bound. Submit finite
            // requests above the engine limit so native rejection remains an
            // observed failure instead of inventing a narrower input contract.
            Self::Speed(value) if value.is_finite() && *value >= 0.01 => {
                set("speed", value.to_string())
            }
            Self::MultiplySpeed(factor) if factor.is_finite() && *factor > 0.0 => {
                vec!["multiply".into(), "speed".into(), factor.to_string()]
            }
            Self::FrameStep => cmd(["frame-step"]),
            Self::FrameBackStep => cmd(["frame-back-step"]),
            Self::CycleAbLoop => cmd(["ab-loop"]),
            Self::ToggleFileLoop => {
                // This shortcut changes only file repeat; a playlist repeat
                // setting survives while file repeat temporarily takes priority.
                cmd(["cycle-values", "loop-file", "inf", "no"])
            }
            _ => return Err("The requested value is out of range or non-finite".into()),
        };
        Ok(args)
    }
}
