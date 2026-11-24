//! Playlist row projection and coalesced metadata refresh.

use crate::{
    CatalogRow, PlayerWindow,
    playlist_metadata::{durations, resume},
};
use riina_core::{Phase, Playback, sync::lock, time_label};
use riina_ui::PlayerMenuActions;
use slint::{ComponentHandle, Model};
use std::{
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

pub(in super::super) fn update(
    app: &PlayerWindow,
    state: &Playback,
    positions: Option<&resume::Positions>,
) {
    let show_metadata = app.get_show_playlist_metadata();
    let observed_duration = state.duration.filter(|seconds| {
        seconds.is_finite()
            && *seconds > 0.0
            && matches!(
                state.phase,
                Phase::Playing | Phase::Paused | Phase::Buffering | Phase::Seeking | Phase::Ended
            )
    });
    if let Some(seconds) = observed_duration
        && let Some(playing) = state.playlist.iter().find(|entry| entry.playing)
        && let Some(path) = durations::local_path(&playing.filename)
    {
        durations::remember(path, seconds);
    }
    let selected_playlist_tokens = app
        .global::<riina_ui::PlaylistInteraction>()
        .get_selected_playlist_tokens();
    let rows = state
        .playlist
        .iter()
        .map(|entry| {
            let token = entry.id.to_string();
            let file_label = entry.title.clone().unwrap_or_else(|| {
                if entry.filename.contains("://") {
                    entry.filename.clone()
                } else {
                    PathBuf::from(&entry.filename).file_name().map_or_else(
                        || entry.filename.clone(),
                        |name| name.to_string_lossy().into_owned(),
                    )
                }
            });
            let metadata = show_metadata
                .then(|| crate::playlist_metadata::lookup(&entry.filename))
                .flatten();
            let duration = observed_duration.filter(|_| entry.playing).or_else(|| {
                durations::local_path(&entry.filename).and_then(|path| durations::lookup(&path))
            });
            let position = positions.and_then(|positions| positions.get(&entry.filename).copied());
            CatalogRow {
                token: token.clone().into(),
                path: entry.filename.clone().into(),
                language: "".into(),
                label: metadata
                    .as_ref()
                    .map_or(file_label, |metadata| metadata.title.clone())
                    .into(),
                detail: metadata
                    .map_or_else(String::new, |metadata| metadata.artist)
                    .into(),
                selected: selected_playlist_tokens
                    .iter()
                    .any(|selected| selected.as_str() == token),
                playing: entry.current,
                duration_label: duration.map_or_else(String::new, duration_label).into(),
                duration_seconds: duration.unwrap_or_default() as f32,
                resume_fraction: resume_fraction(position, duration),
            }
        })
        .collect::<Vec<_>>();
    let catalog = app.global::<riina_ui::PlayerCatalogState>();
    let current = catalog.get_playlist();
    if current.row_count() == rows.len()
        && current
            .iter()
            .zip(&rows)
            .all(|(old, new)| old.token == new.token)
    {
        for (index, (old, new)) in current.iter().zip(rows).enumerate() {
            if old != new {
                current.set_row_data(index, new);
            }
        }
    } else {
        catalog.set_playlist(slint::ModelRc::new(slint::VecModel::from(rows)));
    }
    if !state.playlist.iter().any(|entry| {
        entry.id.to_string()
            == app
                .global::<riina_ui::PlaylistInteraction>()
                .get_selected_playlist_token()
                .as_str()
    }) {
        app.global::<riina_ui::PlaylistInteraction>()
            .set_selected_playlist_token("".into());
    }
    let selection = app
        .global::<riina_ui::PlaylistInteraction>()
        .get_selected_playlist_tokens();
    let retained = selection
        .iter()
        .filter(|token| {
            state
                .playlist
                .iter()
                .any(|entry| entry.id.to_string() == token.as_str())
        })
        .collect::<Vec<_>>();
    if retained != selection.iter().collect::<Vec<_>>() {
        app.global::<riina_ui::PlaylistInteraction>()
            .set_selected_playlist_tokens(slint::ModelRc::new(slint::VecModel::from(retained)));
    }
    refresh_duration_summary(app);
}

fn resume_fraction(position: Option<f64>, duration: Option<f64>) -> f32 {
    match (position, duration) {
        (Some(position), Some(duration))
            if position.is_finite() && duration.is_finite() && duration > 0.0 =>
        {
            (position / duration).clamp(0.0, 1.0) as f32
        }
        _ => 0.0,
    }
}

fn duration_label(seconds: f64) -> String {
    // IINA rounds half down at whole-second precision.
    time_label(Some((seconds - 0.5).ceil().max(0.0)))
}

pub(super) fn refresh_duration_summary(app: &PlayerWindow) {
    let catalog = app.global::<riina_ui::PlayerCatalogState>();
    let rows = catalog.get_playlist();
    let summary = if rows.row_count() == 0 {
        String::new()
    } else {
        duration_summary(rows.iter())
    };
    catalog.set_playlist_duration_summary(summary.into());
}

fn duration_summary(rows: impl Iterator<Item = CatalogRow>) -> String {
    let (mut total, mut selected) = (0.0, None);
    for row in rows {
        if row.duration_label.is_empty() {
            return String::new();
        }
        let duration = f64::from(row.duration_seconds);
        total += duration;
        if row.selected {
            *selected.get_or_insert(0.0) += duration;
        }
    }
    let total = duration_label(total);
    selected.map_or_else(
        || format!("{total} in total"),
        |selected| format!("{} of {total} selected", duration_label(selected)),
    )
}

pub(in super::super) struct MetadataRefresh {
    _timer: Rc<slint::Timer>,
    _subscription: crate::playlist_metadata::Subscription,
    resume: Rc<resume::Reader>,
}

impl MetadataRefresh {
    pub(in super::super) fn saved_positions(
        &self,
        playback: &Playback,
    ) -> Option<Arc<resume::Positions>> {
        self.resume.positions(playback)
    }
}

pub(in super::super) fn install_metadata_refresh(
    app: &PlayerWindow,
    playback: crate::PlaybackStore,
    preferences: &Path,
) -> MetadataRefresh {
    let timer = Rc::new(slint::Timer::default());
    let weak = app.as_weak();
    let demand = std::cell::RefCell::new(durations::Demand::default());
    let resume = Rc::new(resume::Reader::new(
        preferences.with_file_name("watch-later"),
    ));
    let reader = resume.clone();
    timer.start(
        slint::TimerMode::SingleShot,
        Duration::from_millis(300),
        move || {
            let Some(app) = weak.upgrade() else { return };
            let snapshot = lock(&playback).clone();
            reader.refresh(&app, &snapshot);
            let positions = reader.positions(&snapshot);
            update(&app, &snapshot, positions.as_deref());
            let paths = if app.get_prefetch_playlist_durations() {
                snapshot
                    .playlist
                    .iter()
                    .filter_map(|entry| durations::local_path(&entry.filename))
                    .collect()
            } else {
                Vec::new()
            };
            demand.borrow_mut().update(paths);
            app.global::<PlayerMenuActions>()
                .invoke_video_menu_changed();
        },
    );
    let weak_timer = Rc::downgrade(&timer);
    app.on_refresh_playlist_metadata(move || {
        if let Some(timer) = weak_timer.upgrade() {
            super::super::features::wake_timer(&timer);
        }
    });
    let weak_app = app.as_weak();
    let delivery = crate::background::CoalescedUiCallback::default();
    let subscription = crate::playlist_metadata::subscribe(move || {
        delivery.post(|| {
            let weak = weak_app.clone();
            move || {
                if let Some(app) = weak.upgrade()
                    && (app.get_show_playlist_metadata() || app.get_prefetch_playlist_durations())
                    && app
                        .global::<riina_ui::PlayerCatalogState>()
                        .get_playlist()
                        .row_count()
                        > 0
                {
                    app.invoke_refresh_playlist_metadata();
                }
            }
        });
    });
    MetadataRefresh {
        _timer: timer,
        _subscription: subscription,
        resume,
    }
}

#[cfg(test)]
mod metadata_tests {
    use super::*;

    #[test]
    fn playlist_duration_summary_requires_every_item_and_tracks_the_selection() {
        let row = |seconds: f32, selected| CatalogRow {
            duration_label: duration_label(f64::from(seconds)).into(),
            duration_seconds: seconds,
            selected,
            ..CatalogRow::default()
        };
        assert_eq!(
            duration_summary([row(5.5, true), row(3.0, false)].into_iter()),
            "0:05 of 0:08 selected"
        );
        assert_eq!(
            duration_summary([row(5.5, false), row(3.0, false)].into_iter()),
            "0:08 in total"
        );
        assert!(duration_summary([row(5.5, false), CatalogRow::default()].into_iter()).is_empty());
    }

    #[test]
    fn saved_progress_stays_with_the_media_across_row_moves_and_duration_changes()
    -> Result<(), slint::PlatformError> {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new()?;
        let directory = riina_test_support::TempDir::new("playlist-resume-rows").unwrap();
        let path = directory.path().join("resume.mkv");
        let filename = path.to_string_lossy().into_owned();
        let watched = riina_core::PlaylistEntry {
            id: 1,
            filename: filename.clone(),
            title: None,
            current: true,
            playing: true,
        };
        let unreadable = riina_core::PlaylistEntry {
            id: 2,
            filename: directory
                .path()
                .join("unknown.mkv")
                .to_string_lossy()
                .into_owned(),
            title: None,
            current: false,
            playing: false,
        };
        let positions =
            resume::Positions::from([(filename, 5.0), (unreadable.filename.clone(), 5.0)]);
        let mut state = Playback {
            phase: Phase::Paused,
            position: Some(2.0),
            duration: Some(10.0),
            playlist: vec![watched.clone(), unreadable.clone()].into(),
            ..Playback::default()
        };
        let rows = || {
            app.global::<riina_ui::PlayerCatalogState>()
                .get_playlist()
                .iter()
                .map(|row| (row.token.to_string(), row.resume_fraction))
                .collect::<Vec<_>>()
        };

        update(&app, &state, Some(&positions));
        assert_eq!(rows(), [("1".into(), 0.5), ("2".into(), 0.0)]);
        state.playlist = vec![unreadable, watched].into();
        state.duration = Some(3.0);
        update(&app, &state, Some(&positions));
        assert_eq!(rows(), [("2".into(), 0.0), ("1".into(), 1.0)]);
        update(&app, &state, None);
        assert_eq!(rows(), [("2".into(), 0.0), ("1".into(), 0.0)]);
        Ok(())
    }

    #[test]
    fn metadata_refresh_coalesces_changes_into_one_tick() -> Result<(), slint::PlatformError> {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new()?;
        let refresh = install_metadata_refresh(
            &app,
            crate::PlaybackStore::default(),
            Path::new("preferences.json"),
        );
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(300));
        assert!(!refresh._timer.running());

        let preferences = app.global::<riina_ui::PlayerPreferences>();
        preferences.set_playlist_show_metadata(false);
        i_slint_backend_testing::mock_elapsed_time(Duration::ZERO);
        assert!(!refresh._timer.running());

        preferences.set_playlist_show_metadata(true);
        preferences.set_playlist_show_metadata_in_music_mode(false);
        i_slint_backend_testing::mock_elapsed_time(Duration::ZERO);
        assert!(app.get_show_playlist_metadata());
        assert!(refresh._timer.running());
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(299));
        app.invoke_refresh_playlist_metadata();
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(1));
        assert!(!refresh._timer.running());
        Ok(())
    }
}
