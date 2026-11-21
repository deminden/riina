use super::{Command, cmd, flag, set};

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
