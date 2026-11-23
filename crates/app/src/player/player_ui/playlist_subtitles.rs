//! Subtitle associations live for the lifetime of a player’s playback session.
mod picker;

use super::catalog::selected_playlist_rows;
use crate::{NativeVideoHost, PlaybackStore, PlayerWindow};
use riina_core::sync::lock;
use riina_ui::{
    PlayerControlActions, PlayerMusicModeState, PlaylistInteraction, PlaylistSubtitleAction,
    PlaylistSubtitleTargets,
};
use slint::ComponentHandle;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[derive(Default, Clone)]
pub(super) struct Assignments(Arc<Mutex<Session>>);

#[derive(Default)]
struct Session {
    files: HashMap<String, FileMatches>,
    epoch: u64,
}

#[derive(Default)]
struct FileMatches {
    manual: Vec<ManualMatch>,
    automatic: Vec<String>,
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
        session.epoch = session.epoch.wrapping_add(1);
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

    pub(super) fn scan_epoch(&self, filename: &str) -> Option<u64> {
        let session = lock(&self.0);
        (!session
            .files
            .get(filename)
            .is_some_and(|file| file.suppressed))
        .then_some(session.epoch)
    }

    pub(super) fn record_automatic(
        &self,
        filename: &str,
        epoch: u64,
        generation: u64,
        paths: Vec<String>,
    ) -> Vec<String> {
        let mut session = lock(&self.0);
        if session.epoch != epoch || (paths.is_empty() && !session.files.contains_key(filename)) {
            return Vec::new();
        }
        let files = session.files.entry(filename.into()).or_default();
        if files.suppressed {
            return Vec::new();
        }
        files.automatic = paths;
        files
            .automatic
            .iter()
            .filter(|path| {
                !files
                    .manual
                    .iter()
                    .any(|file| file.path == **path && file.added_at != generation)
            })
            .cloned()
            .collect()
    }

    fn count(&self, filename: &str) -> usize {
        lock(&self.0).files.get(filename).map_or(0, |files| {
            files.manual.len()
                + files
                    .automatic
                    .iter()
                    .filter(|path| !files.manual.iter().any(|file| file.path == **path))
                    .count()
        })
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

    fn clear(&self, filename: &str) {
        if let Some(files) = lock(&self.0).files.get_mut(filename) {
            *files = FileMatches {
                suppressed: true,
                ..FileMatches::default()
            };
        }
    }
}

pub(super) fn install(
    app: &PlayerWindow,
    assignments: &Assignments,
    playback: &PlaybackStore,
    host: Option<slint::Weak<NativeVideoHost>>,
) {
    let weak = app.as_weak();
    let counts = assignments.clone();
    let interaction = app.global::<PlaylistInteraction>();
    interaction.on_subtitle_targets(move |token| {
        weak.upgrade()
            .map_or_else(PlaylistSubtitleTargets::default, |app| {
                let rows = selected_playlist_rows(&app, &token);
                let count: usize = rows.iter().map(|row| counts.count(&row.path)).sum();
                PlaylistSubtitleTargets {
                    single: rows.len() == 1,
                    matched: i32::try_from(count).unwrap_or(i32::MAX),
                    // Slint's fallback menu is clipped by the player window.
                    compact: app
                        .window()
                        .size()
                        .to_logical(app.window().scale_factor())
                        .height
                        < 620.0,
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
                for row in rows {
                    assignments.clear(&row.path);
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
        install(&app, &assignments, &PlaybackStore::default(), None);
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
        let first_scan = assignments.scan_epoch("one.mkv").unwrap();
        let second_scan = assignments.scan_epoch("two.mkv").unwrap();
        assignments.add(
            "two.mkv",
            second_scan,
            4,
            vec!["assigned.srt".into(), "assigned.srt".into()],
        );
        assert!(assignments.manual("two.mkv", 4).is_empty());
        assert_eq!(assignments.manual("two.mkv", 5), ["assigned.srt"]);
        assert_eq!(
            assignments.record_automatic("one.mkv", first_scan, 4, vec!["one.srt".into()]),
            ["one.srt"]
        );
        let menu = app.global::<PlaylistInteraction>();
        assert_eq!(menu.invoke_subtitle_targets("2".into()).matched, 1);
        assert!(menu.invoke_subtitle_targets("2".into()).single);
        menu.set_selected_playlist_tokens(ModelRc::new(VecModel::from(vec![
            "1".into(),
            "2".into(),
        ])));
        let targets = menu.invoke_subtitle_targets("2".into());
        assert_eq!(targets.matched, 2);
        assert!(!targets.single);

        menu.invoke_subtitle_action(PlaylistSubtitleAction::Clear, "2".into());
        assert_eq!(menu.invoke_subtitle_targets("2".into()).matched, 0);
        assert!(assignments.manual("two.mkv", 5).is_empty());
        assert_eq!(assignments.scan_epoch("one.mkv"), None);
        assert!(
            assignments
                .record_automatic("one.mkv", first_scan, 5, vec!["one.srt".into()])
                .is_empty()
        );

        assignments.reset();
        assignments.add("two.mkv", second_scan, 5, vec!["late.srt".into()]);
        assert_eq!(menu.invoke_subtitle_targets("2".into()).matched, 0);
        assert!(assignments.scan_epoch("one.mkv").is_some());
        Ok(())
    }
}
