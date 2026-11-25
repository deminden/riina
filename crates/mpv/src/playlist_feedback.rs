//! Track playlist edits through native replies so the UI can report their outcome.

use riina_core::{Command, PlaylistChange, PlaylistFeedback};
use std::{collections::HashMap, num::NonZeroUsize};

#[derive(Clone, Copy)]
pub(super) enum Edit {
    Add,
    Clear,
}

impl Edit {
    pub(super) fn for_command(command: &Command) -> Option<Self> {
        match command {
            Command::AppendFiles(_)
            | Command::InsertPlaylistFiles { .. }
            | Command::InsertPlaylistFilesBefore { .. } => Some(Self::Add),
            Command::OpenMediaLink(link) if link.append => Some(Self::Add),
            Command::ClearPlaylist => Some(Self::Clear),
            _ => None,
        }
    }

    pub(super) fn tracks(self, verb: &str) -> bool {
        match self {
            Self::Add => verb == "loadfile",
            Self::Clear => verb == "playlist-clear",
        }
    }

    fn change(self, successful: NonZeroUsize) -> PlaylistChange {
        match self {
            Self::Add => PlaylistChange::Added(successful),
            Self::Clear => PlaylistChange::Cleared,
        }
    }
}

struct Batch {
    edit: Edit,
    generation: u64,
    outstanding: usize,
    successful: usize,
}

#[derive(Default)]
pub(super) struct PendingEdits(HashMap<u64, Batch>);

impl PendingEdits {
    pub(super) fn track(&mut self, id: u64, edit: Edit, generation: u64) {
        self.0
            .entry(id)
            .or_insert(Batch {
                edit,
                generation,
                outstanding: 0,
                successful: 0,
            })
            .outstanding += 1;
    }

    pub(super) fn reply(&mut self, id: u64, success: bool) -> Option<PlaylistFeedback> {
        let batch = self.0.get_mut(&id)?;
        batch.outstanding -= 1;
        batch.successful += usize::from(success);
        if batch.outstanding > 0 {
            return None;
        }
        let batch = self.0.remove(&id)?;
        Some(PlaylistFeedback {
            sequence: id,
            generation: batch.generation,
            change: batch.edit.change(NonZeroUsize::new(batch.successful)?),
        })
    }

    pub(super) fn clear(&mut self) {
        self.0.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_successful_entries_count_once_when_the_whole_edit_settles() {
        let mut edits = PendingEdits::default();
        for _ in 0..3 {
            edits.track(10, Edit::Add, 4);
        }
        edits.track(13, Edit::Clear, 4);
        assert_eq!(edits.reply(10, true), None);
        assert_eq!(edits.reply(13, false), None);
        assert_eq!(edits.reply(10, false), None);
        assert_eq!(
            edits.reply(10, true),
            Some(PlaylistFeedback {
                sequence: 10,
                generation: 4,
                change: PlaylistChange::Added(NonZeroUsize::new(2).unwrap()),
            })
        );
        assert_eq!(edits.reply(10, true), None);
    }
}
