//! Dialog intent is shared; each backend owns native presentation and modality.
use crate::{NativeVideoHost, PlayerWindow, WelcomeWindow};
use riina_core::Command;

pub(crate) fn open(
    app: &PlayerWindow,
    host: Option<slint::Weak<NativeVideoHost>>,
    purpose: Purpose,
    sender: riina_mpv::CommandSender,
    welcome: Option<slint::Weak<WelcomeWindow>>,
) {
    crate::platform::open_dialog(app, host, purpose, sender, welcome);
}

#[derive(Clone, Copy)]
pub enum Purpose {
    Media,
    MediaInNewPlayer,
    Audio,
    Subtitle,
    Playlist,
}

impl Purpose {
    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Media => "Open File",
            Self::MediaInNewPlayer => "Open in New Window",
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
            Self::MediaInNewPlayer => return None,
            Self::Audio => Command::AddAudio(paths[0].clone()),
            Self::Subtitle => Command::AddSubtitle(paths[0].clone()),
            Self::Playlist => Command::AppendFiles(paths),
        })
    }
}
