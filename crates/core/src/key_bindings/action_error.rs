use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputActionError {
    InvalidAction,
    TooManyArguments,
    ArgumentTooLarge,
    UnfinishedEscapeOrQuote,
    UnsupportedInputSection,
    NoMpvCommand,
    ExternalProcess,
    EmptyCommand,
    TooManyCommands,
}

impl fmt::Display for InputActionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidAction => "Key binding action is empty, oversized, or contains a NUL byte",
            Self::TooManyArguments => "Key binding action exceeds the 64 argument limit",
            Self::ArgumentTooLarge => "Key binding argument exceeds the 4 KiB limit",
            Self::UnfinishedEscapeOrQuote => "Key binding action has an unfinished escape or quote",
            Self::UnsupportedInputSection => "Key binding uses an unsupported input section",
            Self::NoMpvCommand => "Key binding action contains no mpv command",
            Self::ExternalProcess => "Key binding actions cannot launch external processes",
            Self::EmptyCommand => "Key binding action contains an empty command",
            Self::TooManyCommands => "Key binding action exceeds the 32 command limit",
        })
    }
}

impl std::error::Error for InputActionError {}
