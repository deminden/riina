use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandError {
    AudioEqualizer(crate::audio_equalizer::AudioEqualizerError),
    InvalidArgument { what: &'static str },
    Unavailable { what: &'static str },
    InvalidState { reason: &'static str },
    InputBinding(crate::key_bindings::InputActionError),
    Preference(crate::PreferenceError),
    Crop(crate::crop::CropError),
}

impl fmt::Display for CommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::InvalidArgument { what } | Self::Unavailable { what } => *what,
            Self::InvalidState { reason } => *reason,
            Self::InputBinding(error) => return error.fmt(formatter),
            Self::Crop(error) => return error.fmt(formatter),
            Self::AudioEqualizer(error) => return error.fmt(formatter),
            Self::Preference(error) => return error.fmt(formatter),
        };
        formatter.write_str(text)
    }
}

impl std::error::Error for CommandError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InputBinding(error) => Some(error),
            Self::Crop(error) => Some(error),
            Self::AudioEqualizer(error) => Some(error),
            Self::Preference(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Command, Playback};

    #[test]
    fn invalid_values_and_unavailable_state_keep_their_messages() {
        let invalid = Command::Volume(f64::NAN).mpv_args().unwrap_err();
        assert!(matches!(invalid, CommandError::InvalidArgument { .. }));
        assert_eq!(
            invalid.to_string(),
            "The requested value is out of range or non-finite"
        );

        let unavailable = Command::SelectChapter(0)
            .plan(&Playback::default())
            .err()
            .unwrap();
        assert!(matches!(unavailable, CommandError::Unavailable { .. }));
        assert_eq!(
            unavailable.to_string(),
            "The chapter is no longer available"
        );
    }
}
