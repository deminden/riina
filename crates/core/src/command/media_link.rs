use super::{Command, CommandError, cmd, flag, playback, set};
use crate::{Playback, valid_path};

/// Only playback controls that can be represented as typed values cross the
/// browser boundary. In particular, links cannot supply arbitrary mpv options.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MediaLink {
    pub path: String,
    pub append: bool,
    pub start: Option<f64>,
    pub pause: Option<bool>,
    pub volume: Option<f64>,
    pub speed: Option<f64>,
    pub mute: Option<bool>,
}

pub(super) fn plan(state: &Playback, link: &MediaLink) -> Result<Vec<Vec<String>>, CommandError> {
    if !valid_path(&link.path)
        || link
            .start
            .is_some_and(|start| !start.is_finite() || start < 0.0)
        || link
            .volume
            .is_some_and(|volume| !(0.0..=200.0).contains(&volume))
        || link
            .speed
            .is_some_and(|speed| !(0.01..=100.0).contains(&speed))
    {
        return Err(CommandError::InvalidArgument {
            what: "The media link has an invalid path or playback option",
        });
    }
    let mut plan = if link.append {
        let mut load = vec!["loadfile".into(), link.path.clone(), "append".into()];
        if let Some(start) = link.start {
            load.extend(["-1".into(), format!("start={start}")]);
        }
        vec![load]
    } else {
        let load = if let Some(position) = link.start {
            Command::LoadAtPosition {
                path: link.path.clone(),
                position,
            }
        } else {
            Command::Load(link.path.clone())
        };
        playback::load(state, &load)?
    };
    if let Some(pause) = link.pause {
        if !link.append
            && let Some(entry) = plan
                .iter_mut()
                .find(|entry| entry.starts_with(&cmd(["set", "pause"])))
        {
            *entry = set("pause", flag(pause));
        } else {
            plan.push(set("pause", flag(pause)));
        }
    }
    plan.extend(link.volume.map(|value| set("volume", value.to_string())));
    plan.extend(link.speed.map(|value| set("speed", value.to_string())));
    plan.extend(link.mute.map(|value| set("mute", flag(value))));
    Ok(plan)
}
