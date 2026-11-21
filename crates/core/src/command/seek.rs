use super::AbLoopPoint;
use super::{Command, set};
use crate::Playback;
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

pub(super) fn set_loop_points(a: Option<f64>, b: Option<f64>) -> Result<Vec<Vec<String>>, String> {
    Ok(vec![
        set("ab-loop-a", loop_value(a)?),
        set("ab-loop-b", loop_value(b)?),
    ])
}

pub(super) fn set_loop_point(
    point: AbLoopPoint,
    value: Option<f64>,
) -> Result<Vec<Vec<String>>, String> {
    let name = match point {
        AbLoopPoint::A => "ab-loop-a",
        AbLoopPoint::B => "ab-loop-b",
    };
    Ok(vec![set(name, loop_value(value)?)])
}

fn loop_value(value: Option<f64>) -> Result<String, String> {
    match value {
        None => Ok("no".into()),
        Some(value) if value.is_finite() && value >= 0.0 => {
            // IINA preserves a point at the beginning using a nonzero loop timestamp.
            Ok(value.max(0.000001).to_string())
        }
        _ => Err("Invalid A–B loop point".into()),
    }
}

pub(super) fn select_chapter(state: &Playback, index: usize) -> Result<Vec<Vec<String>>, String> {
    let chapter = state
        .chapters
        .get(index)
        .ok_or("The chapter is no longer available")?;
    if !chapter.time.is_finite() || chapter.time < 0.0 {
        return Err("Invalid chapter time".into());
    }
    // PlayerCore.playChapter seeks to the observed time and always resumes.
    Ok(vec![
        vec!["seek".into(), chapter.time.to_string(), "absolute".into()],
        set("pause", "no"),
    ])
}
