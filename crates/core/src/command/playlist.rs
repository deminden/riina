use super::{Command, cmd};

impl Command {
    pub(super) fn playlist_args(&self) -> Result<Vec<String>, String> {
        let args = match self {
            Self::PlaylistNext => cmd(["playlist-next", "weak"]),
            Self::PlaylistPrevious => cmd(["playlist-prev", "weak"]),
            Self::SelectPlaylistEntry(index) => {
                // Setting playlist-pos does not reload an already selected entry.
                vec!["playlist-play-index".into(), index.to_string()]
            }
            Self::RemovePlaylistEntry(index) => vec!["playlist-remove".into(), index.to_string()],
            Self::MovePlaylistEntry { from, to } => {
                vec!["playlist-move".into(), from.to_string(), to.to_string()]
            }
            Self::ClearPlaylist => cmd(["playlist-clear"]),
            _ => return Err("This intent requires a command plan".into()),
        };
        Ok(args)
    }
}
