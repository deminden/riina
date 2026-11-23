use super::{Command, CommandError, cmd};
use super::{CommandPlan, flag, set};
use crate::{Playback, PlaylistEntry, hdr_preferences, playlist, valid_path};

impl Command {
    pub(super) fn playlist_args(&self) -> Result<Vec<String>, CommandError> {
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
            _ => {
                return Err(CommandError::InvalidState {
                    reason: "This intent requires a command plan",
                });
            }
        };
        Ok(args)
    }
}

impl Command {
    pub(super) fn plan_playlist_id(
        &self,
        state: &Playback,
        id: i64,
    ) -> Result<CommandPlan, CommandError> {
        // Stable native IDs survive inserts/reorders while a UI intent is queued.
        let index = state
            .playlist
            .iter()
            .position(|entry| entry.id == id)
            .ok_or(CommandError::Unavailable {
                what: "The playlist entry is no longer available",
            })?;
        let command = match self {
            Self::SelectPlaylistId(_) => Self::SelectPlaylistEntry(index),
            Self::RemovePlaylistId(_) => Self::RemovePlaylistEntry(index),
            Self::MovePlaylistId { direction: -1, .. } if index > 0 => Self::MovePlaylistEntry {
                from: index,
                to: index - 1,
            },
            Self::MovePlaylistId { direction: 1, .. } if index + 1 < state.playlist.len() => {
                Self::MovePlaylistEntry {
                    from: index,
                    to: index + 2,
                }
            }
            Self::MovePlaylistIdBefore { before_id, .. } if *before_id != id => {
                let to = state
                    .playlist
                    .iter()
                    .position(|entry| entry.id == *before_id)
                    .ok_or(CommandError::Unavailable {
                        what: "The playlist move destination is no longer available",
                    })?;
                Self::MovePlaylistEntry { from: index, to }
            }
            Self::MovePlaylistIdBefore { .. } => {
                return Err(CommandError::InvalidArgument {
                    what: "The playlist entry cannot move before itself",
                });
            }
            Self::MovePlaylistId { .. } => {
                return Err(CommandError::InvalidState {
                    reason: "The playlist entry cannot move in that direction",
                });
            }
            _ => unreachable!(),
        };
        command.plan(state)
    }
}

pub(super) fn remove_ids(state: &Playback, ids: &[i64]) -> Result<Vec<Vec<String>>, CommandError> {
    let mut indices = Vec::with_capacity(ids.len());
    for id in ids {
        let Some(index) = state.playlist.iter().position(|entry| entry.id == *id) else {
            return Err(CommandError::Unavailable {
                what: "A selected playlist entry is no longer available",
            });
        };
        indices.push(index);
    }
    indices.sort_unstable();
    if indices.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(CommandError::InvalidArgument {
            what: "A playlist entry was selected more than once",
        });
    }
    // IINA removes ascending index sets while accounting for each prior
    // removal; stable IDs are resolved before any native row can shift.
    Ok(indices
        .into_iter()
        .enumerate()
        .map(|(removed, index)| vec!["playlist-remove".into(), (index - removed).to_string()])
        .collect())
}

pub(super) fn move_entries(
    state: &Playback,
    command: &Command,
) -> Result<Vec<Vec<String>>, CommandError> {
    match command {
        Command::QueuePlaylistIdsNext(ids) => queue_next(state, ids),
        Command::MovePlaylistIdsBefore { ids, before_id } => move_before(state, ids, *before_id),
        _ => unreachable!(),
    }
}

fn queue_next(state: &Playback, ids: &[i64]) -> Result<Vec<Vec<String>>, CommandError> {
    let current = state.playlist.iter().position(|entry| entry.current);
    let mut selected = ids.to_vec();
    if let Some(current) = current
        && let Some(position) = selected
            .iter()
            .position(|id| *id == state.playlist[current].id)
    {
        selected.remove(position);
        if selected.is_empty() {
            return Ok(Vec::new());
        }
    }
    move_selection(state, &selected, current.map_or(0, |index| index + 1))
}

fn move_before(
    state: &Playback,
    ids: &[i64],
    before_id: Option<i64>,
) -> Result<Vec<Vec<String>>, CommandError> {
    let slot = match before_id {
        Some(id) => state
            .playlist
            .iter()
            .position(|entry| entry.id == id)
            .ok_or(CommandError::Unavailable {
                what: "The playlist move destination is no longer available",
            })?,
        None => state.playlist.len(),
    };
    move_selection(state, ids, slot)
}

fn move_selection(
    state: &Playback,
    ids: &[i64],
    slot: usize,
) -> Result<Vec<Vec<String>>, CommandError> {
    let entries = state
        .playlist
        .iter()
        .map(|entry| entry.id)
        .collect::<Vec<_>>();
    playlist::reorder_selection(&entries, ids, slot)
        .map(|moves| {
            moves
                .into_iter()
                .map(|(from, to)| vec!["playlist-move".into(), from.to_string(), to.to_string()])
                .collect()
        })
        .ok_or(CommandError::Unavailable {
            what: "A selected playlist entry is no longer available",
        })
}

pub(super) fn open_files(
    state: &Playback,
    command: &Command,
    paths: &[String],
) -> Result<Vec<Vec<String>>, CommandError> {
    if paths.is_empty() || paths.iter().any(|path| !valid_path(path)) {
        return Err(CommandError::InvalidArgument {
            what: "No valid files were supplied",
        });
    }
    if let Command::OpenFilesWithSubtitles { subtitles, .. } = command
        && (subtitles.is_empty()
            || subtitles.len() > 64
            || subtitles.iter().any(|path| !valid_path(path)))
    {
        return Err(CommandError::InvalidArgument {
            what: "Invalid dropped subtitle files were supplied",
        });
    }
    let mut commands: Vec<Vec<String>> = paths
        .iter()
        .map(|path| vec!["loadfile".into(), path.clone(), "append".into()])
        .collect();
    if matches!(
        command,
        Command::OpenFiles(_) | Command::OpenPlaylist(_) | Command::OpenFilesWithSubtitles { .. }
    ) {
        // PlayerCore.openURLs opens the first playable item, then appends
        // the remainder. Validate everything before replacing old media.
        commands[0][2] = "replace".into();
        commands.insert(0, set("pause", flag(state.pause_when_open)));
        // HDR overrides target the current source's primaries. Clear
        // them before replacement so the next source inherits auto/ICC.
        let hdr_reset = hdr_preferences::output_commands(false, None)?;
        commands.splice(0..0, hdr_reset);
    }
    Ok(commands)
}

pub(super) fn insert_files(
    state: &Playback,
    paths: &[String],
    index: usize,
) -> Result<Vec<Vec<String>>, CommandError> {
    if paths.is_empty()
        || paths.iter().any(|path| !valid_path(path))
        || index > state.playlist.len()
    {
        return Err(CommandError::InvalidArgument {
            what: "Playlist items or insertion position are invalid",
        });
    }
    let first_new = state.playlist.len();
    let mut commands = Vec::with_capacity(paths.len() * 2);
    for (offset, path) in paths.iter().enumerate() {
        commands.push(vec!["loadfile".into(), path.clone(), "append".into()]);
        let appended_index = first_new + offset;
        let destination = index + offset;
        if appended_index != destination {
            commands.push(vec![
                "playlist-move".into(),
                appended_index.to_string(),
                destination.to_string(),
            ]);
        }
    }
    Ok(commands)
}

pub(super) fn insert_files_before(
    state: &Playback,
    paths: &[String],
    before_id: Option<i64>,
) -> Result<Vec<Vec<String>>, CommandError> {
    let index = match before_id {
        Some(id) => state
            .playlist
            .iter()
            .position(|entry| entry.id == id)
            .ok_or(CommandError::Unavailable {
                what: "The playlist drop position is no longer available",
            })?,
        None => state.playlist.len(),
    };
    insert_files(state, paths, index)
}

pub(super) fn insert_files_around_current(
    state: &Playback,
    before: &[String],
    after: &[String],
) -> Result<Vec<Vec<String>>, CommandError> {
    if before.is_empty() && after.is_empty()
        || before.iter().chain(after).any(|path| !valid_path(path))
    {
        return Err(CommandError::InvalidArgument {
            what: "No valid folder media files were supplied",
        });
    }
    let current_index = state
        .playlist
        .iter()
        .position(|entry| entry.current)
        .ok_or(CommandError::Unavailable {
            what: "The current playlist item is no longer available",
        })?;
    let first_new = state.playlist.len();
    let mut commands = Vec::with_capacity(before.len() * 2 + after.len());
    for (offset, path) in before.iter().enumerate() {
        commands.push(vec!["loadfile".into(), path.clone(), "append".into()]);
        let appended_index = first_new + offset;
        let destination = current_index + offset;
        if appended_index != destination {
            commands.push(vec![
                "playlist-move".into(),
                appended_index.to_string(),
                destination.to_string(),
            ]);
        }
    }
    commands.extend(
        after
            .iter()
            .map(|path| vec!["loadfile".into(), path.clone(), "append".into()]),
    );
    Ok(commands)
}

impl Command {
    pub(super) fn plan_playlist_navigation(
        &self,
        state: &Playback,
    ) -> Result<CommandPlan, CommandError> {
        let mut resume_on_load = false;
        let commands = match self {
            Self::SelectPlaylistEntry(index) => {
                if *index >= state.playlist.len() {
                    return Err(CommandError::Unavailable {
                        what: "The playlist entry is no longer available",
                    });
                }
                resume_on_load = true;
                vec![set("pause", "yes"), self.mpv_args()?]
            }
            Self::PlaylistPrevious if state.playlist.first().is_some_and(|entry| entry.current) => {
                vec![cmd(["seek", "0", "absolute"])]
            }
            Self::PlaylistNext | Self::PlaylistPrevious => {
                resume_on_load = true;
                vec![set("pause", "yes"), self.mpv_args()?]
            }
            Self::RemovePlaylistEntry(index) if *index >= state.playlist.len() => {
                return Err(CommandError::Unavailable {
                    what: "The playlist entry is no longer available",
                });
            }
            Self::MovePlaylistEntry { from, to }
                if *from >= state.playlist.len() || *to > state.playlist.len() =>
            {
                return Err(CommandError::Unavailable {
                    what: "The playlist entry is no longer available",
                });
            }
            _ => vec![self.mpv_args()?],
        };
        Ok(CommandPlan {
            commands,
            resume_on_load,
        })
    }
}

pub(super) fn reorder(
    entries: &[PlaylistEntry],
    sort: playlist::PlaylistSort,
) -> Result<Vec<Vec<String>>, CommandError> {
    let desired = playlist::sorted_ids(entries, sort);
    let mut current = entries.iter().map(|entry| entry.id).collect::<Vec<_>>();
    let mut commands = Vec::new();
    for (target, id) in desired.into_iter().enumerate() {
        let from = current
            .iter()
            .position(|candidate| *candidate == id)
            .ok_or(CommandError::InvalidState {
                reason: "The playlist changed before sorting",
            })?;
        if from == target {
            continue;
        }
        // mpv's destination is an insertion position before removal; increment
        // downward moves to leave the item at its requested final index.
        let destination = if target > from { target + 1 } else { target };
        commands.push(vec![
            "playlist-move".into(),
            from.to_string(),
            destination.to_string(),
        ]);
        let moved = current.remove(from);
        current.insert(target, moved);
    }
    Ok(commands)
}
