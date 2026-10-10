//! Capture content is distinct from the eventual file/clipboard destination.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Content {
    Video,
    #[default]
    Subtitles,
    Window,
}

impl Content {
    pub const fn native_value(self) -> &'static str {
        match self {
            Self::Video => "video",
            Self::Subtitles => "subtitles",
            Self::Window => "window",
        }
    }
}
