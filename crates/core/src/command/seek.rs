use super::AbLoopPoint;
use super::{Command, CommandError, set};
use crate::Playback;
use crate::control_preferences;

impl Command {
    pub(super) fn seek_args(&self) -> Result<Vec<String>, CommandError> {
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
            Self::SeekTimeline(fraction) if fraction.is_finite() => {
                // Seeking to 100% can advance the playlist. Match the reference's
                // half-open slider range and leave absolute timestamp seeks intact.
                let percent = (fraction * 100.0).clamp(0.0, 100.0_f64.next_down());
                vec![
                    "seek".into(),
                    percent.to_string(),
                    "absolute-percent+exact".into(),
                ]
            }
            Self::SelectChapter(index) => set("chapter", index.to_string()),
            Self::AdjustChapter(delta) => {
                // Relative navigation keeps native restart-threshold and boundary
                // semantics; selecting a sidebar chapter intentionally resumes.
                vec!["add".into(), "chapter".into(), delta.to_string()]
            }
            _ => return Err(CommandError::OUT_OF_RANGE),
        };
        Ok(args)
    }
}

pub(super) fn set_loop_points(
    a: Option<f64>,
    b: Option<f64>,
) -> Result<Vec<Vec<String>>, CommandError> {
    Ok(vec![
        set("ab-loop-a", loop_value(a)?),
        set("ab-loop-b", loop_value(b)?),
    ])
}

pub(super) fn set_loop_point(
    point: AbLoopPoint,
    value: Option<f64>,
) -> Result<Vec<Vec<String>>, CommandError> {
    let name = match point {
        AbLoopPoint::A => "ab-loop-a",
        AbLoopPoint::B => "ab-loop-b",
    };
    Ok(vec![set(name, loop_value(value)?)])
}

fn loop_value(value: Option<f64>) -> Result<String, CommandError> {
    match value {
        None => Ok("no".into()),
        Some(value) if value.is_finite() && value >= 0.0 => {
            // IINA preserves a point at the beginning using a nonzero loop timestamp.
            Ok(value.max(0.000001).to_string())
        }
        _ => Err(CommandError::InvalidArgument {
            what: "Invalid A–B loop point",
        }),
    }
}

pub(super) fn select_chapter(
    state: &Playback,
    index: usize,
    command: &Command,
) -> Result<Vec<Vec<String>>, CommandError> {
    let chapter = state.chapters.get(index);
    // A menu can remain open when the current file's chapter list changes.
    if let Command::ActivateChapter { start, .. } = command
        && chapter.is_none_or(|chapter| chapter.time.to_bits() != start.to_bits())
    {
        return Ok(Vec::new());
    }
    let chapter = chapter.ok_or(CommandError::Unavailable {
        what: "The chapter is no longer available",
    })?;
    if !chapter.time.is_finite() || chapter.time < 0.0 {
        return Err(CommandError::InvalidArgument {
            what: "Invalid chapter time",
        });
    }
    // PlayerCore.playChapter seeks to the observed time and always resumes.
    Ok(vec![
        vec!["seek".into(), chapter.time.to_string(), "absolute".into()],
        set("pause", "no"),
    ])
}
