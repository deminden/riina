//! Track playlist edits through native replies so the UI can report their outcome.

use crate::ffi::{EndFile, end_reason};
use riina_core::{Command, PlaylistChange, PlaylistFeedback};
use std::{collections::HashMap, num::NonZeroUsize};

#[derive(Clone, Copy)]
pub(super) enum Edit {
    Add,
    Clear,
    OpenGroup,
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

    pub(super) fn for_open(command: &Command) -> Option<Self> {
        match command {
            Command::OpenFiles(paths) | Command::OpenPlaylist(paths) if paths.len() > 1 => {
                Some(Self::OpenGroup)
            }
            _ => None,
        }
    }

    pub(super) fn tracks(self, verb: &str) -> bool {
        match self {
            Self::Add | Self::OpenGroup => verb == "loadfile",
            Self::Clear => verb == "playlist-clear",
        }
    }

    fn change(self, successful: NonZeroUsize) -> PlaylistChange {
        match self {
            Self::Add | Self::OpenGroup => PlaylistChange::Added(successful),
            Self::Clear => PlaylistChange::Cleared,
        }
    }
}

pub(super) enum Reply {
    Entry(bool),
    Lead(Option<i64>),
}

struct Batch {
    edit: Edit,
    generation: u64,
    outstanding: usize,
    successful: usize,
}

struct GroupOpen {
    sequence: u64,
    lead: Option<i64>,
    count: Option<NonZeroUsize>,
}

#[derive(Default)]
pub(super) struct PendingEdits {
    batches: HashMap<u64, Batch>,
    opening: Option<GroupOpen>,
}

impl PendingEdits {
    pub(super) fn track(&mut self, id: u64, edit: Edit, generation: u64) {
        if matches!(edit, Edit::OpenGroup) && self.opening.is_none() {
            self.opening = Some(GroupOpen {
                sequence: id,
                lead: None,
                count: None,
            });
        }
        self.batches
            .entry(id)
            .or_insert(Batch {
                edit,
                generation,
                outstanding: 0,
                successful: 0,
            })
            .outstanding += 1;
    }

    pub(super) fn reply(&mut self, id: u64, reply: Reply) -> Option<PlaylistFeedback> {
        let success = match reply {
            Reply::Entry(success) => success,
            Reply::Lead(Some(entry)) => {
                let group = self.opening.as_mut().filter(|group| group.sequence == id)?;
                group.lead = Some(entry);
                true
            }
            Reply::Lead(None) => {
                if self
                    .opening
                    .as_ref()
                    .is_some_and(|group| group.sequence == id)
                {
                    self.retire_open();
                }
                return None;
            }
        };
        let batch = self.batches.get_mut(&id)?;
        batch.outstanding -= 1;
        batch.successful += usize::from(success);
        if batch.outstanding > 0 {
            return None;
        }
        let batch = self.batches.remove(&id)?;
        if matches!(batch.edit, Edit::OpenGroup) {
            if batch.successful > 1 {
                self.opening.as_mut()?.count = NonZeroUsize::new(batch.successful);
            } else {
                self.retire_open();
            }
            return None;
        }
        Some(PlaylistFeedback {
            sequence: id,
            generation: batch.generation,
            change: batch.edit.change(NonZeroUsize::new(batch.successful)?),
        })
    }

    pub(super) fn ready_open(
        &mut self,
        active: Option<i64>,
        generation: u64,
    ) -> Option<PlaylistFeedback> {
        let group = self.opening.as_ref()?;
        if group.lead.is_none() || group.lead != active {
            return None;
        }
        let count = group.count?;
        let sequence = group.sequence;
        self.opening = None;
        Some(PlaylistFeedback {
            sequence,
            generation,
            change: PlaylistChange::Added(count),
        })
    }

    pub(super) fn file_started(&mut self, active: Option<i64>) {
        if self
            .opening
            .as_ref()
            .is_some_and(|group| group.lead.is_some() && group.lead != active)
        {
            self.retire_open();
        }
    }

    pub(super) fn file_ended(&mut self, end: &EndFile) {
        if let Some(group) = self.opening.as_mut()
            && group.lead == Some(end.playlist_entry_id)
        {
            if end.reason == end_reason::REDIRECT && end.playlist_insert_num_entries > 0 {
                group.lead = Some(end.playlist_insert_id);
            } else {
                self.retire_open();
            }
        }
    }

    pub(super) fn retire_open(&mut self) {
        if let Some(group) = self.opening.take() {
            self.batches.remove(&group.sequence);
        }
    }

    pub(super) fn clear(&mut self) {
        self.batches.clear();
        self.opening = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grouped_open_follows_the_native_lead_and_drops_superseded_edits() {
        let mut edits = PendingEdits::default();
        for _ in 0..3 {
            edits.track(10, Edit::OpenGroup, 1);
        }
        assert_eq!(edits.reply(10, Reply::Entry(true)), None);
        assert_eq!(edits.reply(10, Reply::Lead(Some(21))), None);
        edits.file_ended(&EndFile {
            reason: end_reason::REDIRECT,
            error: 0,
            playlist_entry_id: 21,
            playlist_insert_id: 23,
            playlist_insert_num_entries: 1,
        });
        edits.file_started(Some(23));
        assert_eq!(edits.ready_open(Some(23), 3), None);
        assert_eq!(edits.reply(10, Reply::Entry(false)), None);
        assert_eq!(edits.ready_open(Some(21), 3), None);
        assert_eq!(
            edits.ready_open(Some(23), 3),
            Some(PlaylistFeedback {
                sequence: 10,
                generation: 3,
                change: PlaylistChange::Added(NonZeroUsize::new(2).unwrap()),
            })
        );
        assert_eq!(edits.ready_open(Some(23), 3), None);

        for _ in 0..2 {
            edits.track(14, Edit::OpenGroup, 3);
        }
        edits.retire_open();
        for _ in 0..2 {
            edits.track(17, Edit::OpenGroup, 3);
        }
        assert_eq!(edits.reply(14, Reply::Lead(Some(24))), None);
        assert_eq!(edits.reply(17, Reply::Lead(Some(26))), None);
        edits.file_started(Some(28));
        assert_eq!(edits.reply(17, Reply::Entry(true)), None);
        assert_eq!(edits.ready_open(Some(26), 4), None);
    }

    #[test]
    fn only_successful_entries_count_once_when_the_whole_edit_settles() {
        let mut edits = PendingEdits::default();
        for _ in 0..3 {
            edits.track(10, Edit::Add, 4);
        }
        edits.track(13, Edit::Clear, 4);
        assert_eq!(edits.reply(10, Reply::Entry(true)), None);
        assert_eq!(edits.reply(13, Reply::Entry(false)), None);
        assert_eq!(edits.reply(10, Reply::Entry(false)), None);
        assert_eq!(
            edits.reply(10, Reply::Entry(true)),
            Some(PlaylistFeedback {
                sequence: 10,
                generation: 4,
                change: PlaylistChange::Added(NonZeroUsize::new(2).unwrap()),
            })
        );
        assert_eq!(edits.reply(10, Reply::Entry(true)), None);
    }
}
