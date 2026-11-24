//! Subtitle associations live for the lifetime of a player’s playback session.
mod picker;

use super::catalog::selected_playlist_rows;
use crate::{NativeVideoHost, PlaybackStore, PlayerWindow, subtitle_discovery::Matches};
use riina_core::{Playback, PlaylistEntry, subtitle_controls::AutoLoadPreferences, sync::lock};
use riina_ui::{
    PlayerControlActions, PlayerMusicModeState, PlaylistInteraction, PlaylistSubtitleAction,
    PlaylistSubtitleTargets,
};
use slint::ComponentHandle;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[derive(Default, Clone)]
pub(super) struct Assignments(Arc<Mutex<Session>>);

#[derive(Default)]
struct Session {
    files: HashMap<String, FileMatches>,
    automatic: HashMap<PathBuf, AutomaticMatch>,
    preferences: Option<AutoLoadPreferences>,
    epoch: u64,
}

struct AutomaticMatch {
    anchor: i64,
    explicit_playlist: bool,
    subtitles: Vec<PathBuf>,
}

#[derive(Default)]
struct FileMatches {
    manual: Vec<ManualMatch>,
    suppressed: bool,
}

struct ManualMatch {
    path: String,
    added_at: u64,
}

impl Assignments {
    pub(super) fn reset(&self) {
        let mut session = lock(&self.0);
        session.files.clear();
        session.automatic.clear();
        session.epoch = session.epoch.wrapping_add(1);
    }

    pub(super) fn observe_preferences(&self, preferences: &AutoLoadPreferences) {
        let mut session = lock(&self.0);
        if session.preferences.as_ref() != Some(preferences) {
            session.automatic.clear();
            session.preferences = Some(preferences.clone());
        }
    }

    pub(super) fn manual(&self, filename: &str, generation: u64) -> Vec<String> {
        lock(&self.0)
            .files
            .get(filename)
            .map_or_else(Vec::new, |files| {
                files
                    .manual
                    .iter()
                    .filter(|file| file.added_at != generation)
                    .map(|file| file.path.clone())
                    .collect()
            })
    }

    pub(super) fn scan_epoch(&self, filename: &str, explicit: bool) -> Option<u64> {
        let session = lock(&self.0);
        (!explicit || !session.is_suppressed(filename)).then_some(session.epoch)
    }

    pub(super) fn record_automatic(
        &self,
        epoch: u64,
        state: &Playback,
        media: &PlaylistEntry,
        matches: Matches,
    ) -> Vec<String> {
        let mut session = lock(&self.0);
        if session.epoch != epoch {
            return Vec::new();
        }
        let live_ids: HashSet<_> = state.playlist.iter().map(|entry| entry.id).collect();
        session.automatic.retain(|_, entry| {
            entry.explicit_playlist == state.explicit_playlist && live_ids.contains(&entry.anchor)
        });
        session
            .automatic
            .extend(matches.into_iter().map(|(file, subtitles)| {
                (
                    file,
                    AutomaticMatch {
                        anchor: media.id,
                        explicit_playlist: state.explicit_playlist,
                        subtitles,
                    },
                )
            }));
        session
            .automatic_for(state, &media.filename)
            .unwrap_or_default()
    }

    pub(super) fn automatic_for(
        &self,
        state: &Playback,
        media: &PlaylistEntry,
    ) -> Option<Vec<String>> {
        let mut session = lock(&self.0);
        let paths = session.automatic_for(state, &media.filename)?;
        // Keep a folder scan attached to the selected entry as playback advances.
        let key = absolute_media_path(&media.filename)?;
        let anchor = session.automatic.get(&key)?.anchor;
        if !state.explicit_playlist && anchor != media.id {
            for cached in session
                .automatic
                .values_mut()
                .filter(|cached| cached.anchor == anchor)
            {
                cached.anchor = media.id;
            }
        }
        Some(paths)
    }

    fn add(&self, filename: &str, epoch: u64, generation: u64, paths: Vec<String>) {
        let mut session = lock(&self.0);
        if paths.is_empty() || session.epoch != epoch {
            return;
        }
        let files = session.files.entry(filename.into()).or_default();
        for path in paths {
            if !files.manual.iter().any(|file| file.path == path) {
                files.manual.push(ManualMatch {
                    path,
                    added_at: generation,
                });
            }
        }
    }

    fn clear(&self, state: &Playback, filename: &str) {
        let mut session = lock(&self.0);
        if session.count(state, filename) > 0 {
            session.files.insert(
                filename.into(),
                FileMatches {
                    suppressed: true,
                    ..FileMatches::default()
                },
            );
        }
    }
}

impl Session {
    fn is_suppressed(&self, filename: &str) -> bool {
        self.files
            .get(filename)
            .is_some_and(|files| files.suppressed)
    }

    fn automatic_paths(&self, state: &Playback, filename: &str) -> Option<&[PathBuf]> {
        let entry = self.automatic.get(&absolute_media_path(filename)?)?;
        (entry.explicit_playlist == state.explicit_playlist
            && state.playlist.iter().any(|media| media.id == entry.anchor))
        .then_some(entry.subtitles.as_slice())
    }

    fn automatic_for(&self, state: &Playback, filename: &str) -> Option<Vec<String>> {
        let cached = self.automatic_paths(state, filename)?;
        if self.is_suppressed(filename) {
            return Some(Vec::new());
        }
        let manual = self.files.get(filename).map(|files| &files.manual);
        Some(
            cached
                .iter()
                .filter_map(|path| path.to_str())
                .filter(|path| {
                    manual.is_none_or(|entries| {
                        !entries
                            .iter()
                            .any(|entry| entry.added_at != state.generation && entry.path == *path)
                    })
                })
                .map(str::to_owned)
                .collect(),
        )
    }

    fn count(&self, state: &Playback, filename: &str) -> usize {
        let manual = self
            .files
            .get(filename)
            .map(|files| files.manual.as_slice())
            .unwrap_or_default();
        let automatic = (!self.is_suppressed(filename))
            .then(|| self.automatic_paths(state, filename))
            .flatten()
            .unwrap_or_default();
        manual.len()
            + automatic
                .iter()
                .filter(|path| {
                    path.to_str().is_some()
                        && !manual.iter().any(|entry| Path::new(&entry.path) == **path)
                })
                .count()
    }
}

fn absolute_media_path(filename: &str) -> Option<PathBuf> {
    std::path::absolute(crate::file_actions::local_media_path(filename)?).ok()
}

pub(super) fn install(
    app: &PlayerWindow,
    assignments: &Assignments,
    playback: &PlaybackStore,
    host: Option<slint::Weak<NativeVideoHost>>,
) {
    let weak = app.as_weak();
    let counts = assignments.clone();
    let count_playback = playback.clone();
    let interaction = app.global::<PlaylistInteraction>();
    interaction.on_subtitle_targets(move |token| {
        weak.upgrade()
            .map_or_else(PlaylistSubtitleTargets::default, |app| {
                let rows = selected_playlist_rows(&app, &token);
                let state = lock(&count_playback).clone();
                let session = lock(&counts.0);
                let count: usize = rows
                    .iter()
                    .map(|row| session.count(&state, &row.path))
                    .sum();
                PlaylistSubtitleTargets {
                    single: rows.len() == 1,
                    matched: i32::try_from(count).unwrap_or(i32::MAX),
                }
            })
    });
    let weak = app.as_weak();
    let assignments = assignments.clone();
    let playback = playback.clone();
    interaction.on_subtitle_action(move |action, token| {
        let Some(app) = weak.upgrade() else { return };
        if app.global::<PlayerMusicModeState>().get_music_mode() {
            return;
        }
        app.global::<PlayerControlActions>().invoke_activity();
        let rows = selected_playlist_rows(&app, &token);
        match action {
            PlaylistSubtitleAction::Add => {
                if let [row] = rows.as_slice() {
                    picker::open(
                        &app,
                        row.clone(),
                        assignments.clone(),
                        playback.clone(),
                        host.clone(),
                    );
                }
            }
            PlaylistSubtitleAction::Clear => {
                let state = lock(&playback).clone();
                for row in rows {
                    assignments.clear(&state, &row.path);
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use riina_ui::PlayerCatalogState;
    use slint::{ModelRc, VecModel};

    #[test]
    fn playlist_selection_clears_both_matches_and_drops_stale_scan_results()
    -> Result<(), slint::PlatformError> {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new()?;
        let assignments = Assignments::default();
        let mut state = Playback {
            generation: 4,
            playlist: Arc::new(vec![
                PlaylistEntry {
                    id: 1,
                    filename: "one.mkv".into(),
                    title: None,
                    current: true,
                    playing: true,
                },
                PlaylistEntry {
                    id: 2,
                    filename: "two.mkv".into(),
                    title: None,
                    current: false,
                    playing: false,
                },
            ]),
            ..Default::default()
        };
        let playback = PlaybackStore::default();
        *lock(&playback) = Arc::new(state.clone());
        install(&app, &assignments, &playback, None);
        app.global::<PlayerCatalogState>()
            .set_playlist(ModelRc::new(VecModel::from(vec![
                crate::CatalogRow {
                    token: "1".into(),
                    path: "one.mkv".into(),
                    ..Default::default()
                },
                crate::CatalogRow {
                    token: "2".into(),
                    path: "two.mkv".into(),
                    ..Default::default()
                },
            ])));
        let first_scan = assignments.scan_epoch("one.mkv", false).unwrap();
        let second_scan = assignments.scan_epoch("two.mkv", false).unwrap();
        assignments.add(
            "two.mkv",
            second_scan,
            4,
            vec!["assigned.srt".into(), "assigned.srt".into()],
        );
        assert!(assignments.manual("two.mkv", 4).is_empty());
        assert_eq!(assignments.manual("two.mkv", 5), ["assigned.srt"]);
        let batch = || {
            [
                (
                    absolute_media_path("one.mkv").unwrap(),
                    vec![PathBuf::from("one.srt")],
                ),
                (
                    absolute_media_path("two.mkv").unwrap(),
                    vec![PathBuf::from("two.srt")],
                ),
            ]
            .into()
        };
        assert_eq!(
            assignments.record_automatic(first_scan, &state, &state.playlist[0], batch()),
            ["one.srt"]
        );
        let menu = app.global::<PlaylistInteraction>();
        assert_eq!(menu.invoke_subtitle_targets("2".into()).matched, 2);
        assert!(menu.invoke_subtitle_targets("2".into()).single);
        menu.set_selected_playlist_tokens(ModelRc::new(VecModel::from(vec![
            "1".into(),
            "2".into(),
        ])));
        let targets = menu.invoke_subtitle_targets("2".into());
        assert_eq!(targets.matched, 3);
        assert!(!targets.single);

        state.generation = 5;
        assert_eq!(
            assignments.automatic_for(&state, &state.playlist[1]),
            Some(vec!["two.srt".into()])
        );
        Arc::make_mut(&mut state.playlist).remove(0);
        assert_eq!(
            assignments.automatic_for(&state, &state.playlist[0]),
            Some(vec!["two.srt".into()])
        );
        // Playlist replacements assign new native IDs, even for the same filename.
        Arc::make_mut(&mut state.playlist)[0].id = 3;
        assert_eq!(assignments.automatic_for(&state, &state.playlist[0]), None);

        menu.invoke_subtitle_action(PlaylistSubtitleAction::Clear, "2".into());
        assert_eq!(menu.invoke_subtitle_targets("2".into()).matched, 0);
        assert!(assignments.manual("two.mkv", 5).is_empty());
        assert_eq!(assignments.scan_epoch("two.mkv", true), None);
        assert!(
            assignments
                .record_automatic(first_scan, &state, &state.playlist[0], batch())
                .is_empty()
        );

        assignments.reset();
        assignments.add("two.mkv", second_scan, 5, vec!["late.srt".into()]);
        assert_eq!(menu.invoke_subtitle_targets("2".into()).matched, 0);
        assert!(
            assignments
                .record_automatic(first_scan, &state, &state.playlist[0], batch())
                .is_empty()
        );
        assert!(assignments.scan_epoch("two.mkv", true).is_some());
        Ok(())
    }
}
