//! Dialog intent is shared; each backend owns native presentation and modality.
use riina_core::Command;

pub(crate) use crate::platform::open_dialog as open;

#[derive(Clone, Copy)]
pub enum Purpose {
    Media,
    Audio,
    Subtitle,
    Playlist,
}

impl Purpose {
    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Media => "Open File",
            Self::Audio => "Load External Audio",
            Self::Subtitle => "Load Subtitle",
            Self::Playlist => "Add to Playlist",
        }
    }

    pub(crate) fn command(self, paths: Vec<String>) -> Option<Command> {
        if paths.is_empty() {
            return None;
        }
        Some(match self {
            Self::Media => Command::OpenFiles(paths),
            Self::Audio => Command::AddAudio(paths[0].clone()),
            Self::Subtitle => Command::AddSubtitle(paths[0].clone()),
            Self::Playlist => Command::AppendFiles(paths),
        })
    }
}
