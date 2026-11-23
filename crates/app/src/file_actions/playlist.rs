use crate::{PlayerWindow, background, diagnostics, platform};
use riina_core::{Command, PlaylistFileReference};
use riina_mpv::CommandSender;
use slint::ComponentHandle;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub(crate) struct PlaylistFile {
    entry: PlaylistFileReference,
    path: PathBuf,
}

impl PlaylistFile {
    pub(crate) fn new(id: i64, filename: String, path: PathBuf) -> Self {
        Self {
            entry: PlaylistFileReference { id, filename },
            path,
        }
    }
}

pub(crate) fn local_media_path(reference: &str) -> Option<PathBuf> {
    if reference.is_empty() || reference.contains('\0') {
        return None;
    }
    if reference
        .split_once(':')
        .is_some_and(|(scheme, _)| scheme.eq_ignore_ascii_case("file"))
    {
        return riina_core::playlist::clipboard_references(reference)
            .into_iter()
            .next()
            .map(PathBuf::from);
    }
    (!reference.contains("://")).then(|| PathBuf::from(reference))
}

pub(crate) fn reveal_playlist_files(app: &PlayerWindow, files: Vec<PlaylistFile>) {
    let paths = group_paths(files).into_keys().collect::<Vec<_>>();
    let weak = app.as_weak();
    if let Err(error) = background::spawn_named("riina-reveal-playlist", move || {
        let paths = paths
            .into_iter()
            .filter(|path| path.is_file())
            .collect::<Vec<_>>();
        let result = if paths.is_empty() {
            Err("The selected files are no longer available".to_owned())
        } else {
            platform::reveal_files(&paths)
                .map_err(|error| format!("Could not show selected files: {error}"))
        };
        if let Err(error) = result {
            let _ = background::to_ui(weak, move |app| diagnostics::report(&app, error));
        }
    }) {
        diagnostics::report(app, error.to_string());
    }
}

pub(crate) fn trash_playlist_files(
    app: &PlayerWindow,
    files: Vec<PlaylistFile>,
    player: CommandSender,
) {
    let weak = app.as_weak();
    let trash = platform::system_trash_name();
    diagnostics::report(
        app,
        diagnostics::UserMessage::info(format!("Moving files to the {trash}…")),
    );
    if let Err(error) = background::spawn_named("riina-file-trash", move || {
        let outcome = move_files(files, |path| {
            if !path.is_file() {
                return Err("The file is no longer available".to_owned());
            }
            trash::delete(path).map_err(|error| error.to_string())
        });
        let _ = background::to_ui(weak, move |app| outcome.finish(&app, &player, trash));
    }) {
        diagnostics::report(
            app,
            format!("Could not start the file Trash operation: {error}"),
        );
    }
}

fn group_paths(files: Vec<PlaylistFile>) -> BTreeMap<PathBuf, Vec<PlaylistFileReference>> {
    let mut groups = BTreeMap::<_, Vec<_>>::new();
    for file in files {
        groups.entry(file.path).or_default().push(file.entry);
    }
    groups
}

#[derive(Default)]
struct TrashOutcome {
    moved: usize,
    removed_entries: Vec<PlaylistFileReference>,
    failed: usize,
    first_error: Option<String>,
}

impl TrashOutcome {
    fn finish(self, app: &PlayerWindow, player: &CommandSender, trash: &str) {
        if !self.removed_entries.is_empty()
            && let Err(error) =
                player.send(Command::RemoveTrashedPlaylistEntries(self.removed_entries))
        {
            diagnostics::report(
                app,
                format!(
                    "Files were moved to the {trash}, but the playlist could not be updated: {error}"
                ),
            );
            return;
        }
        if let Some(error) = self.first_error {
            diagnostics::report(
                app,
                format!(
                    "Moved {} files to the {trash}. Could not move {} files. {error}",
                    self.moved, self.failed
                ),
            );
        } else if self.moved > 0 {
            diagnostics::report(
                app,
                diagnostics::UserMessage::info(format!(
                    "Moved {} files to the {trash}",
                    self.moved
                )),
            );
        }
    }
}

fn move_files(
    files: Vec<PlaylistFile>,
    mut move_one: impl FnMut(&Path) -> Result<(), String>,
) -> TrashOutcome {
    let mut outcome = TrashOutcome::default();
    for (path, mut entries) in group_paths(files) {
        match move_one(&path) {
            Ok(()) => {
                outcome.moved += 1;
                outcome.removed_entries.append(&mut entries);
            }
            Err(error) => {
                outcome.failed += 1;
                outcome
                    .first_error
                    .get_or_insert_with(|| format!("{}: {error}", path.display()));
            }
        }
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trash_successes_keep_all_selected_identities_and_leave_failed_files() {
        let file = |id, name: &str| PlaylistFile::new(id, name.into(), name.into());
        let mut visited = Vec::new();
        let outcome = move_files(
            vec![
                file(7, "clip.mkv"),
                file(8, "clip.mkv"),
                file(9, "missing.mkv"),
            ],
            |path| {
                visited.push(path.to_owned());
                if path == Path::new("clip.mkv") {
                    Ok(())
                } else {
                    Err("File unavailable".into())
                }
            },
        );
        assert_eq!(
            visited,
            [PathBuf::from("clip.mkv"), PathBuf::from("missing.mkv")]
        );
        assert_eq!(
            outcome
                .removed_entries
                .iter()
                .map(|entry| entry.id)
                .collect::<Vec<_>>(),
            [7, 8]
        );
        assert_eq!(outcome.moved, 1);
        assert_eq!(outcome.failed, 1);
    }
}
