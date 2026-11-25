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
    MediaAlternative,
    Audio,
    Subtitle,
    Playlist,
    PlaylistFolders,
}

impl Purpose {
    pub(crate) fn allows_multiple(self) -> bool {
        matches!(
            self,
            Self::Media | Self::MediaAlternative | Self::Playlist | Self::PlaylistFolders
        )
    }

    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Media | Self::MediaAlternative => "Open File",
            Self::Audio => "Load External Audio",
            Self::Subtitle => "Load Subtitle",
            Self::Playlist => "Add to Playlist",
            Self::PlaylistFolders => "Add Folders to Playlist",
        }
    }

    pub(crate) fn command(self, paths: Vec<String>) -> Option<Command> {
        if paths.is_empty() {
            return None;
        }
        Some(match self {
            Self::Media => Command::OpenFiles(paths),
            Self::MediaAlternative => return None,
            Self::Audio => Command::AddAudio(paths[0].clone()),
            Self::Subtitle => Command::AddSubtitle(paths[0].clone()),
            Self::Playlist | Self::PlaylistFolders => Command::AppendFiles(paths),
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
        Purpose::Media | Purpose::MediaAlternative => {
            for path in &paths {
                app.invoke_remember_recent_path(path.clone().into());
            }
            media_open::file_dialog_selection(
                app,
                paths,
                matches!(purpose, Purpose::MediaAlternative),
                sender,
            );
            Ok(())
        }
        Purpose::Playlist | Purpose::PlaylistFolders => {
            media_open::selection(app, paths, true, sender);
            Ok(())
        }
        Purpose::Audio | Purpose::Subtitle => purpose
            .command(paths)
            .map_or(Ok(()), |command| sender.send(command).map_err(String::from)),
    }
}
