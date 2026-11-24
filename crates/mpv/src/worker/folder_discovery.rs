//! A single-file open can discover siblings only for the entry it created.
use riina_core::{Command, file_selection, media_reference};

#[derive(Default)]
pub(super) struct FolderDiscovery {
    reply: Option<u64>,
    entry: Option<i64>,
}

impl FolderDiscovery {
    pub(super) fn submitted(&mut self, reply: u64, command: &Command) {
        *self = Self::default();
        let path = match command {
            Command::Load(path) | Command::LoadAtPosition { path, .. } => path,
            Command::OpenFiles(paths) | Command::OpenFilesWithSubtitles { paths, .. }
                if paths.len() == 1 =>
            {
                &paths[0]
            }
            _ => return,
        };
        if !media_reference::is_network_url_candidate(path)
            && !file_selection::is_playlist_file(path)
        {
            self.reply = Some(reply);
        }
    }

    pub(super) fn awaits(&self, reply: u64) -> bool {
        self.reply == Some(reply)
    }

    pub(super) fn replied(&mut self, entry: Option<i64>) {
        self.reply = None;
        self.entry = entry;
    }

    pub(super) fn take_for(&mut self, active: Option<i64>) -> bool {
        if self.entry.is_none() || self.entry != active {
            return false;
        }
        self.entry = None;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_discovery_follows_only_the_latest_single_file_open() {
        let mut open = FolderDiscovery::default();
        open.submitted(1, &Command::Load("first.mkv".into()));
        open.submitted(2, &Command::Load("second.mkv".into()));
        assert!(!open.awaits(1));
        assert!(open.awaits(2));
        open.replied(Some(15));
        assert!(!open.take_for(Some(14)));
        assert!(open.take_for(Some(15)));
        assert!(!open.take_for(Some(15)));

        for command in [
            Command::OpenFiles(vec!["a.avi".into(), "b.mkv".into()]),
            Command::OpenPlaylist(vec!["a.avi".into()]),
            Command::OpenFiles(vec!["list.m3u8".into()]),
            Command::Load("https://example.invalid/video.mkv".into()),
            Command::Stop,
        ] {
            open.submitted(3, &command);
            assert!(!open.awaits(3), "{command:?}");
        }
    }
}
