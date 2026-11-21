use super::{Command, cmd, flag, set};
use crate::{Playback, audio_equalizer};

impl Command {
    pub(super) fn audio_args(&self) -> Result<Vec<String>, String> {
        let args = match self {
            Self::Volume(value) if value.is_finite() && (0.0..=200.0).contains(value) => {
                set("volume", value.to_string())
            }
            Self::AudioDevice(name) if !name.is_empty() && !name.contains('\0') => {
                set("audio-device", name.clone())
            }
            Self::AudioDevice(_) => {
                return Err("The audio device name is empty or contains a NUL byte".into());
            }
            Self::ToggleMute => cmd(["cycle", "mute"]),
            Self::SetMuted(muted) => set("mute", flag(*muted)),
            _ => return Err("The requested value is out of range or non-finite".into()),
        };
        Ok(args)
    }
}

pub(super) fn equalizer(state: &Playback, gains: &[f64; 10]) -> Result<Vec<Vec<String>>, String> {
    let filter = audio_equalizer::lavfi_filter(gains).map_err(str::to_owned)?;
    let mut commands = Vec::new();
    if state
        .audio_equalizer
        .is_some_and(|current| current.iter().any(|gain| *gain != 0.0))
    {
        commands.push(vec![
            "af".into(),
            "remove".into(),
            format!("@{}", audio_equalizer::FILTER_LABEL),
        ]);
    }
    if let Some(filter) = filter {
        commands.push(vec!["af".into(), "add".into(), filter]);
    }
    Ok(commands)
}

pub(super) fn adjust_volume(delta: f64, boost: bool) -> Result<Vec<Vec<String>>, String> {
    if !delta.is_finite() {
        return Err("The volume adjustment must be finite".into());
    }
    Ok(volume_adjustment(delta, boost))
}

pub(super) fn volume_adjustment(delta: f64, boost: bool) -> Vec<Vec<String>> {
    // Native add clamps against the active range and uses the engine's current
    // volume. A zero adjustment also clamps when the user turns boost off.
    vec![
        set("volume-max", if boost { "200" } else { "100" }),
        vec!["add".into(), "volume".into(), delta.to_string()],
    ]
}
