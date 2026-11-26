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
        return riina_core::playlist::clipboard_reference(reference).map(PathBuf::from);
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
    remove_playlist_files(app, files, player, FileRemoval::Trash);
}

#[derive(Clone, Copy)]
pub(super) enum FileRemoval {
    Trash,
    Permanent,
}

impl FileRemoval {
    fn label(self) -> String {
        match self {
            Self::Trash => format!("move to the {}", platform::system_trash_name()),
            Self::Permanent => "permanently delete".to_owned(),
        }
    }

    fn remove(self, path: &Path) -> Result<(), String> {
        if !path.is_file() {
            return Err("The file is no longer available".to_owned());
        }
        match self {
            Self::Trash => trash::delete(path).map_err(|error| error.to_string()),
            Self::Permanent => std::fs::remove_file(path).map_err(|error| error.to_string()),
        }
    }
}

pub(super) fn remove_playlist_files(
    app: &PlayerWindow,
    files: Vec<PlaylistFile>,
    player: CommandSender,
    removal: FileRemoval,
) {
    let weak = app.as_weak();
    let pending = match removal {
        FileRemoval::Trash => format!("Moving files to the {}…", platform::system_trash_name()),
        FileRemoval::Permanent => "Permanently deleting file…".into(),
    };
    diagnostics::report(app, diagnostics::UserMessage::info(pending));
    if let Err(error) = background::spawn_named("riina-file-removal", move || {
        let outcome = remove_files(files, |path| removal.remove(path));
        let _ = background::to_ui(weak, move |app| outcome.finish(&app, &player, removal));
    }) {
        diagnostics::report(
            app,
            format!("Could not {} selected files: {error}", removal.label()),
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
struct RemovalOutcome {
    removed: usize,
    removed_entries: Vec<PlaylistFileReference>,
    failed: usize,
    first_error: Option<String>,
}

impl RemovalOutcome {
    fn finish(self, app: &PlayerWindow, player: &CommandSender, removal: FileRemoval) {
        let success = match removal {
            FileRemoval::Trash => format!(
                "Moved {} files to the {}",
                self.removed,
                platform::system_trash_name()
            ),
            FileRemoval::Permanent => "Permanently deleted the current file".to_owned(),
        };
        if !self.removed_entries.is_empty()
            && let Err(error) =
                player.send(Command::RemoveDeletedPlaylistEntries(self.removed_entries))
        {
            diagnostics::report(
                app,
                format!("{success}. Could not update the playlist: {error}"),
            );
            return;
        }
        if let Some(error) = self.first_error {
            let failure = format!(
                "Could not {} {} files. {error}",
                removal.label(),
                self.failed
            );
            let message = if self.removed == 0 {
                failure
            } else {
                format!("{success}. {failure}")
            };
            diagnostics::report(app, message);
        } else if self.removed > 0 {
            diagnostics::report(app, diagnostics::UserMessage::info(success));
        }
    }
}

fn remove_files(
    files: Vec<PlaylistFile>,
    mut remove_one: impl FnMut(&Path) -> Result<(), String>,
) -> RemovalOutcome {
    let mut outcome = RemovalOutcome::default();
    for (path, mut entries) in group_paths(files) {
        match remove_one(&path) {
            Ok(()) => {
                outcome.removed += 1;
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
    fn removal_successes_keep_all_selected_identities_and_leave_failed_files() {
        let file = |id, name: &str| PlaylistFile::new(id, name.into(), name.into());
        let mut visited = Vec::new();
        let outcome = remove_files(
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
        assert_eq!(outcome.removed, 1);
        assert_eq!(outcome.failed, 1);
    }

    #[test]
    fn permanent_removal_unlinks_only_an_existing_file() {
        let directory = riina_test_support::TempDir::new("playlist-permanent-removal").unwrap();
        let media = directory.path().join("Café.mkv");
        std::fs::write(&media, b"test fixture").unwrap();
        let outcome = remove_files(
            vec![PlaylistFile::new(7, "Café.mkv".into(), media.clone())],
            |path| FileRemoval::Permanent.remove(path),
        );
        assert!(!media.exists());
        assert_eq!(outcome.removed_entries[0].id, 7);
        assert!(FileRemoval::Permanent.remove(&media).is_err());
        assert!(FileRemoval::Permanent.remove(directory.path()).is_err());
        assert!(directory.path().is_dir());
    }
}
