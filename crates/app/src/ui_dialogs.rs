//! Dialog intent is shared; each backend owns native presentation and modality.
use crate::{NativeVideoHost, PlayerWindow, WelcomeWindow, media_open};
use std::path::PathBuf;

mod tracks;
pub(crate) use tracks::TrackFile;
use tracks::TrackRequest;

pub(crate) struct SaveFile {
    pub(crate) title: &'static str,
    pub(crate) name: String,
    pub(crate) filter: &'static str,
    pub(crate) extension: String,
}

pub(crate) fn open(
    app: &PlayerWindow,
    host: Option<slint::Weak<NativeVideoHost>>,
    purpose: Purpose,
    sender: riina_mpv::CommandSender,
    welcome: Option<slint::Weak<WelcomeWindow>>,
) {
    crate::platform::open_dialog(app, host, purpose, sender, welcome);
}

pub(crate) enum Purpose {
    Media,
    MediaAlternative,
    Playlist,
    PlaylistFolders,
    ExternalTrack(TrackRequest),
}

impl Purpose {
    pub(crate) fn picker(&self) -> OpenFiles {
        match self {
            Self::Media | Self::MediaAlternative => OpenFiles::media("Open File"),
            Self::Playlist => OpenFiles::media("Add to Playlist"),
            Self::PlaylistFolders => OpenFiles::folders("Add Folders to Playlist", true),
            Self::ExternalTrack(track) => track.picker(),
        }
    }
}

pub(crate) struct OpenFiles {
    pub(crate) title: &'static str,
    pub(crate) mode: SelectionMode,
}

pub(crate) enum SelectionMode {
    Media,
    Folders {
        multiple: bool,
    },
    Track {
        kind: TrackFile,
        directory: Option<PathBuf>,
    },
}

impl OpenFiles {
    fn media(title: &'static str) -> Self {
        Self {
            title,
            mode: SelectionMode::Media,
        }
    }

    pub(crate) fn folders(title: &'static str, multiple: bool) -> Self {
        Self {
            title,
            mode: SelectionMode::Folders { multiple },
        }
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn allows_multiple(&self) -> bool {
        matches!(
            self.mode,
            SelectionMode::Media | SelectionMode::Folders { multiple: true }
        )
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
            remember_selected_paths(app, &paths);
            media_open::file_dialog_selection(
                app,
                paths,
                matches!(purpose, Purpose::MediaAlternative),
                sender,
            );
            Ok(())
        }
        Purpose::Playlist | Purpose::PlaylistFolders => {
            crate::player::playlist_edits::add_files(app, &sender, paths)
        }
        Purpose::ExternalTrack(track) => track.attach(app, paths, &sender),
    }
}

pub(crate) fn remember_selected_paths(app: &PlayerWindow, paths: &[String]) {
    let rows = paths
        .iter()
        .map(|path| slint::SharedString::from(path.as_str()))
        .collect::<Vec<_>>();
    app.invoke_remember_recent_paths(slint::ModelRc::new(slint::VecModel::from(rows)));
}
