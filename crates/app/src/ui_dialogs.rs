//! Dialog intent is shared; each backend owns native presentation and modality.
use crate::{NativeVideoHost, PlayerWindow, WelcomeWindow, media_open};
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
    pub(crate) fn allows_multiple(self) -> bool {
        matches!(self, Self::Media | Self::MediaInNewPlayer | Self::Playlist)
    }

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

pub(crate) fn dispatch_selection(
    app: &PlayerWindow,
    purpose: Purpose,
    paths: Vec<String>,
    sender: riina_mpv::CommandSender,
) -> Result<(), String> {
    match purpose {
        Purpose::Media | Purpose::MediaInNewPlayer | Purpose::Playlist => {
            if matches!(purpose, Purpose::Media | Purpose::MediaInNewPlayer) {
                for path in &paths {
                    app.invoke_remember_recent_path(path.clone().into());
                }
            }
            if matches!(purpose, Purpose::MediaInNewPlayer) {
                media_open::selection_in_new_player(app, paths);
            } else {
                media_open::selection(app, paths, matches!(purpose, Purpose::Playlist), sender);
            }
            Ok(())
        }
        Purpose::Audio | Purpose::Subtitle => purpose
            .command(paths)
            .map_or(Ok(()), |command| sender.send(command).map_err(String::from)),
    }
}
