//! Playlist row projection and coalesced metadata refresh.

use crate::{
    CatalogRow, PlayerWindow,
    playlist_metadata::{durations, resume, series},
};
use riina_core::{Phase, Playback, sync::lock, time_label};
use riina_ui::{PlayerMenuActions, PlaylistInteraction};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

#[derive(Default)]
pub(in super::super) struct MetadataSnapshot {
    positions: Option<Arc<resume::Positions>>,
    series: Option<Arc<series::Series>>,
}

pub(in super::super) fn update(app: &PlayerWindow, state: &Playback, metadata: &MetadataSnapshot) {
    let catalog = app.global::<riina_ui::PlayerCatalogState>();
    let current = catalog.get_playlist();
    let expanded: HashMap<_, _> = current
        .iter()
        .filter(|row| row.series_expanded)
        .map(|row| (row.token, row.series_prefix))
        .collect();
    let tags = if app.get_show_playlist_metadata() {
        crate::playlist_metadata::tags(state.playlist.iter().map(|entry| entry.filename.as_str()))
    } else {
        HashMap::new()
    };
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
        && let Some(source) = durations::Source::from_filename(&playing.filename)
    {
        durations::remember(source, seconds);
    }
    let interaction = app.global::<PlaylistInteraction>();
    let selected_token = interaction.get_selected_playlist_token();
    let selection = interaction.get_selected_playlist_tokens();
    let selected_playlist_tokens = selection.iter().collect::<HashSet<_>>();
    let rows = state
        .playlist
        .iter()
        .map(|entry| {
            let token = SharedString::from(entry.id.to_string());
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
            let tags = tags.get(entry.filename.as_str());
            let duration = observed_duration.filter(|_| entry.playing).or_else(|| {
                durations::Source::from_filename(&entry.filename)
                    .and_then(|source| durations::lookup(&source))
            });
            let position = metadata
                .positions
                .as_ref()
                .and_then(|positions| positions.get(&entry.filename).copied());
            let (prefix, episode) = metadata
                .series
                .as_ref()
                .filter(|_| tags.is_none())
                .and_then(|series| series.split_title(&entry.filename, &file_label))
                .unwrap_or_default();
            let series_expanded = !prefix.is_empty()
                && expanded
                    .get(&token)
                    .is_some_and(|label| label.as_str() == prefix);
            let selected = selected_playlist_tokens.contains(&token);
            CatalogRow {
                token,
                path: entry.filename.clone().into(),
                language: "".into(),
                series_expanded,
                series_prefix: prefix.into(),
                episode_label: episode.into(),
                label: tags
                    .map_or(file_label, |metadata| metadata.title.clone())
                    .into(),
                detail: tags
                    .map_or_else(String::new, |metadata| metadata.artist.clone())
                    .into(),
                selected,
                playing: entry.current,
                duration_label: duration.map_or_else(String::new, duration_label).into(),
                duration_seconds: duration.unwrap_or_default() as f32,
                resume_fraction: resume_fraction(position, duration),
            }
        })
        .collect::<Vec<_>>();
    let single_retained = rows.iter().any(|row| row.token == selected_token);
    let retained = {
        let live_selection = rows
            .iter()
            .filter(|row| row.selected)
            .map(|row| &row.token)
            .collect::<HashSet<_>>();
        selection
            .iter()
            .filter(|token| live_selection.contains(token))
            .collect::<Vec<_>>()
    };
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
    } else if !update_playlist_structure(&current, &rows) {
        catalog.set_playlist(slint::ModelRc::new(slint::VecModel::from(rows)));
    }
    if !single_retained && !selected_token.is_empty() {
        interaction.set_selected_playlist_token("".into());
    }
    if retained.len() != selection.row_count() {
        interaction.set_selected_playlist_tokens(ModelRc::new(VecModel::from(retained)));
    }
    refresh_duration_summary(app);
}

fn update_playlist_structure(current: &ModelRc<CatalogRow>, rows: &[CatalogRow]) -> bool {
    let Some(model) = current.as_any().downcast_ref::<VecModel<CatalogRow>>() else {
        return false;
    };
    let old_tokens = current.iter().map(|row| row.token).collect::<Vec<_>>();
    let old_ids = old_tokens.iter().collect::<HashSet<_>>();
    let new_ids = rows.iter().map(|row| &row.token).collect::<HashSet<_>>();
    let retained_old = old_tokens.iter().filter(|token| new_ids.contains(*token));
    let retained_new = rows
        .iter()
        .map(|row| &row.token)
        .filter(|token| old_ids.contains(*token));
    if !retained_old.eq(retained_new) {
        return false;
    }
    // Keep the repeater instances of surviving rows alive, including the row
    // that owns keyboard focus during Paste and folder discovery.
    for (index, token) in old_tokens.iter().enumerate().rev() {
        if !new_ids.contains(token) {
            model.remove(index);
        }
    }
    for (index, row) in rows.iter().enumerate() {
        if !old_ids.contains(&row.token) {
            model.insert(index, row.clone());
        } else if model.row_data(index).is_some_and(|old| old != *row) {
            model.set_row_data(index, row.clone());
        }
    }
    true
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
    sources: Rc<Sources>,
}

impl MetadataRefresh {
    pub(in super::super) fn snapshot(&self, playback: &Playback) -> MetadataSnapshot {
        self.sources.snapshot(playback)
    }
}

struct Sources {
    resume: resume::Reader,
    series: series::Cache,
}

impl Sources {
    fn snapshot(&self, playback: &Playback) -> MetadataSnapshot {
        MetadataSnapshot {
            positions: self.resume.positions(playback),
            series: if playback.explicit_playlist {
                None
            } else {
                lock(&self.series).clone()
            },
        }
    }
}

pub(in super::super) fn install_metadata_refresh(
    app: &PlayerWindow,
    playback: crate::PlaybackStore,
    preferences: &Path,
    series: series::Cache,
) -> MetadataRefresh {
    let timer = Rc::new(slint::Timer::default());
    let weak = app.as_weak();
    let demand = std::cell::RefCell::new(durations::Demand::default());
    let sources = Rc::new(Sources {
        resume: resume::Reader::new(preferences.with_file_name("watch-later")),
        series,
    });
    let reader = sources.clone();
    timer.start(
        slint::TimerMode::SingleShot,
        Duration::from_millis(300),
        move || {
            let Some(app) = weak.upgrade() else { return };
            let snapshot = lock(&playback).clone();
            reader.resume.refresh(&app, &snapshot);
            update(&app, &snapshot, &reader.snapshot(&snapshot));
            let sources = if app.get_prefetch_playlist_durations() {
                snapshot
                    .playlist
                    .iter()
                    .filter_map(|entry| durations::Source::from_filename(&entry.filename))
                    .collect()
            } else {
                Vec::new()
            };
            demand.borrow_mut().update(sources);
            app.global::<PlayerMenuActions>()
                .invoke_video_menu_changed();
        },
    );
    let weak = app.as_weak();
    app.global::<PlaylistInteraction>()
        .on_toggle_series_prefix(move |token| {
            if let Some(app) = weak.upgrade() {
                toggle_series_prefix(&app, &token);
            }
        });
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
        sources,
    }
}

fn toggle_series_prefix(app: &PlayerWindow, token: &str) {
    let rows = app.global::<riina_ui::PlayerCatalogState>().get_playlist();
    if let Some((index, mut row)) = rows
        .iter()
        .enumerate()
        .find(|(_, row)| row.token.as_str() == token && !row.series_prefix.is_empty())
    {
        row.series_expanded = !row.series_expanded;
        rows.set_row_data(index, row);
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
    fn a_played_stream_keeps_its_duration_after_advancing_to_another_row()
    -> Result<(), slint::PlatformError> {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new()?;
        let entries = [
            "https://media.example/clip.mkv?part=one",
            "https://media.example/clip.mkv?part=two",
        ]
        .into_iter()
        .enumerate()
        .map(|(index, filename)| riina_core::PlaylistEntry {
            id: i64::try_from(index).unwrap(),
            filename: filename.into(),
            title: None,
            current: index == 0,
            playing: index == 0,
        })
        .collect::<Vec<_>>();
        let mut state = Playback {
            phase: Phase::Playing,
            duration: Some(13.0),
            playlist: entries.into(),
            ..Playback::default()
        };
        let catalog = app.global::<riina_ui::PlayerCatalogState>();
        update(&app, &state, &MetadataSnapshot::default());
        assert_eq!(
            catalog.get_playlist().row_data(0).unwrap().duration_label,
            "0:13"
        );
        assert!(catalog.get_playlist_duration_summary().is_empty());

        let list = Arc::make_mut(&mut state.playlist);
        for row in list {
            row.current = !row.current;
            row.playing = !row.playing;
        }
        state.duration = Some(9.0);
        update(&app, &state, &MetadataSnapshot::default());
        assert_eq!(
            catalog.get_playlist().row_data(0).unwrap().duration_label,
            "0:13"
        );
        assert_eq!(catalog.get_playlist_duration_summary(), "0:22 in total");
        Ok(())
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

        let metadata = MetadataSnapshot {
            positions: Some(Arc::new(positions)),
            ..MetadataSnapshot::default()
        };
        update(&app, &state, &metadata);
        assert_eq!(rows(), [("1".into(), 0.5), ("2".into(), 0.0)]);
        state.playlist = vec![unreadable, watched].into();
        state.duration = Some(3.0);
        update(&app, &state, &metadata);
        assert_eq!(rows(), [("2".into(), 0.0), ("1".into(), 1.0)]);
        update(&app, &state, &MetadataSnapshot::default());
        assert_eq!(rows(), [("2".into(), 0.0), ("1".into(), 0.0)]);
        Ok(())
    }

    #[test]
    fn expanding_an_episode_preserves_its_full_title_and_survives_reordering()
    -> Result<(), slint::PlatformError> {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new()?;
        let paths = [
            "Show name - 1.avi",
            "Show name - 2.avi",
            "Show name - 3.avi",
        ]
        .map(str::to_owned);
        let entries = paths
            .iter()
            .enumerate()
            .map(|(index, filename)| riina_core::PlaylistEntry {
                id: i64::try_from(index).unwrap(),
                filename: filename.clone(),
                title: None,
                current: false,
                playing: false,
            })
            .collect::<Vec<_>>();
        let mut state = Playback {
            playlist: entries.into(),
            ..Playback::default()
        };
        let metadata = MetadataSnapshot {
            series: Some(Arc::new(series::Series::from_files(&paths))),
            ..MetadataSnapshot::default()
        };
        let row = |index| {
            app.global::<riina_ui::PlayerCatalogState>()
                .get_playlist()
                .row_data(index)
                .unwrap()
        };
        update(&app, &state, &metadata);
        assert_eq!(row(0).series_prefix.as_str(), "Show name - ");
        assert_eq!(row(0).label.as_str(), paths[0]);
        assert_eq!(row(0).episode_label.as_str(), "1.avi");
        assert!(!row(0).series_expanded);
        toggle_series_prefix(&app, "0");
        assert!(row(0).series_expanded);
        assert!(!row(1).series_expanded);
        Arc::make_mut(&mut state.playlist).swap(0, 2);
        update(&app, &state, &metadata);
        assert_eq!(row(2).token.as_str(), "0");
        assert!(row(2).series_expanded);
        Arc::make_mut(&mut state.playlist)[2].title = Some("Custom display title".into());
        update(&app, &state, &metadata);
        assert_eq!(row(2).label.as_str(), "Custom display title");
        assert!(row(2).series_prefix.is_empty());
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
            series::Cache::default(),
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
