//! Playlist, chapter, track and audio-device models follow native observations.
use super::{PlayerContext, bind};
use crate::{
    CatalogRow, CatalogSelection, MenuCatalogRow, PlayerWindow, video_osd::ActionFeedback,
};
use riina_core::{Command, Phase, Playback, TrackKind, sync::lock, time_label};
use riina_ui::PlayerMenuActions;
use slint::{ComponentHandle, Model};
use std::collections::HashSet;
pub(super) mod playlist;

pub(super) fn install_catalog_selection(app: &PlayerWindow, context: &PlayerContext) {
    bind!(
        context,
        app.global::<PlayerMenuActions>(),
        on_select_item,
        |ctx, category, token| {
            let command = selection_command(category, token.as_str());
            let chapter_title = if let Ok(Command::SelectChapter(index)) = &command {
                lock(ctx.playback())
                    .chapters
                    .get(*index)
                    .and_then(|chapter| chapter.title.clone())
            } else {
                None
            };
            let Some(app) = ctx.app() else { return };
            app.global::<riina_ui::PlayerControlActions>()
                .invoke_activity();
            match (command, chapter_title) {
                (Ok(command), Some(title)) => {
                    ctx.send_with_osd(command, ActionFeedback::Chapter(title), false)
                }
                (Ok(command), None) => ctx.send(command),
                (Err(error), _) => ctx.report(error),
            }
        }
    );
}

fn selection_command(category: CatalogSelection, token: &str) -> Result<Command, String> {
    use CatalogSelection::{
        AudioTrack, Chapter, Playlist, PrimarySubtitle, SecondarySubtitle, Unknown, VideoTrack,
    };
    let track = |kind| parse_track_id(token).map(|id| Command::SelectTrack { kind, id });
    match category {
        AudioTrack => track(TrackKind::Audio),
        PrimarySubtitle => track(TrackKind::Sub),
        VideoTrack => track(TrackKind::Video),
        SecondarySubtitle => parse_track_id(token).map(Command::SelectSecondarySubtitle),
        Chapter => token.parse::<usize>().map(Command::SelectChapter),
        Playlist => token.parse::<i64>().map(Command::SelectPlaylistId),
        Unknown => return Err("Unknown selection category".into()),
    }
    .map_err(|error| error.to_string())
}

fn parse_track_id(token: &str) -> Result<Option<i64>, std::num::ParseIntError> {
    // Track IDs can exceed the exact numeric range of Slint's float properties.
    if token == "no" {
        Ok(None)
    } else {
        token.parse().map(Some)
    }
}

pub(super) fn install_playlist_selection(app: &PlayerWindow, selected: &[String]) {
    let current = app.global::<riina_ui::PlayerCatalogState>().get_playlist();
    let selected: HashSet<&str> = selected.iter().map(String::as_str).collect();
    for (index, mut row) in current.iter().enumerate() {
        let is_selected = selected.contains(row.token.as_str());
        if row.selected != is_selected {
            // Replacing the model also replaces the row that grabbed the pointer.
            row.selected = is_selected;
            current.set_row_data(index, row);
        }
    }
    playlist::refresh_duration_summary(app);
}

pub(super) fn selected_playlist_paths(app: &PlayerWindow, clicked_token: &str) -> Vec<String> {
    selected_playlist_rows(app, clicked_token)
        .into_iter()
        .map(|row| row.path.to_string())
        .collect()
}

pub(super) fn selected_playlist_rows(app: &PlayerWindow, clicked_token: &str) -> Vec<CatalogRow> {
    let rows = app.global::<riina_ui::PlayerCatalogState>().get_playlist();
    playlist_row_targets(app, &rows, clicked_token).map_or_else(Vec::new, |(_, indices)| {
        indices
            .into_iter()
            .filter_map(|index| rows.row_data(index))
            .collect()
    })
}

pub(super) fn playlist_row_targets(
    app: &PlayerWindow,
    rows: &slint::ModelRc<CatalogRow>,
    clicked_token: &str,
) -> Option<(usize, Vec<usize>)> {
    if clicked_token.is_empty() {
        return None;
    }
    let clicked = rows
        .iter()
        .position(|row| row.token.as_str() == clicked_token)?;
    let selected = app
        .global::<riina_ui::PlaylistInteraction>()
        .get_selected_playlist_tokens()
        .iter()
        .collect::<std::collections::BTreeSet<_>>();
    let clicked_token = slint::SharedString::from(clicked_token);
    let indices = if selected.contains(&clicked_token) {
        rows.iter()
            .enumerate()
            .filter_map(|(index, row)| selected.contains(&row.token).then_some(index))
            .collect()
    } else {
        vec![clicked]
    };
    Some((clicked, indices))
}

pub(super) fn selected_playlist_ids(
    app: &PlayerWindow,
    clicked_token: &str,
) -> Result<Vec<i64>, String> {
    let selected = app
        .global::<riina_ui::PlaylistInteraction>()
        .get_selected_playlist_tokens()
        .iter()
        .map(|token| token.to_string())
        .collect::<Vec<_>>();
    let selected = if selected.iter().any(|token| token == clicked_token) {
        selected
    } else {
        vec![clicked_token.to_owned()]
    };
    selected
        .iter()
        .map(|token| token.parse::<i64>().map_err(|error| error.to_string()))
        .collect()
}

pub(super) fn playlist_paste_index(app: &PlayerWindow, clicked_token: &str) -> usize {
    let selected = app
        .global::<riina_ui::PlaylistInteraction>()
        .get_selected_playlist_tokens()
        .iter()
        .collect::<HashSet<_>>();
    let rows = app.global::<riina_ui::PlayerCatalogState>().get_playlist();
    (0..rows.row_count())
        .find(|index| {
            rows.row_data(*index)
                .is_some_and(|row| selected.contains(&row.token))
        })
        .or_else(|| {
            (0..rows.row_count()).find(|index| {
                rows.row_data(*index)
                    .is_some_and(|row| row.token.as_str() == clicked_token)
            })
        })
        .unwrap_or(0)
}

fn audio_device_menu_rows(state: &Playback) -> Vec<MenuCatalogRow> {
    state
        .audio_devices
        .iter()
        .filter(|device| {
            device.name == "auto"
                || state.audio_output.as_deref().is_none_or(|driver| {
                    device
                        .name
                        .split_once('/')
                        .is_none_or(|(device_driver, _)| device_driver == driver)
                })
        })
        .map(|device| MenuCatalogRow {
            token: device.name.clone().into(),
            label: if device.description.trim().is_empty() {
                device.name.clone().into()
            } else {
                device.description.clone().into()
            },
            selected: state.audio_device.as_deref() == Some(device.name.as_str()),
        })
        .collect()
}

pub(super) fn should_fallback_removed_audio_device(
    previous: Option<&Playback>,
    state: &Playback,
) -> bool {
    let active = matches!(
        state.phase,
        Phase::Playing | Phase::Paused | Phase::Buffering | Phase::Ended
    );
    let selection_changed = previous.is_none_or(|previous| {
        previous.audio_device != state.audio_device || previous.audio_devices != state.audio_devices
    });
    active
        && selection_changed
        && !state.audio_devices.is_empty()
        && state.audio_device.as_deref().is_some_and(|selected| {
            selected != "auto"
                && !state
                    .audio_devices
                    .iter()
                    .any(|device| device.name == selected)
        })
}

pub(super) fn update_catalog(
    app: &PlayerWindow,
    state: &Playback,
    metadata: &playlist::MetadataSnapshot,
) {
    update_tracks(app, state);
    update_chapters(app, state);
    playlist::update(app, state, metadata);
    if app.get_prefetch_playlist_durations() {
        app.invoke_refresh_playlist_metadata();
    }
}

fn update_tracks(app: &PlayerWindow, state: &Playback) {
    app.set_downloaded_subtitle_save_available(
        crate::file_actions::downloaded_subtitle_path(state).is_some(),
    );
    app.global::<riina_ui::PlaybackAudioState>()
        .set_has_active_audio_track(
            state
                .tracks
                .iter()
                .any(|track| track.kind == TrackKind::Audio && track.is_primary_selected()),
        );
    app.global::<riina_ui::PlaybackAudioState>()
        .set_audio_device_menu_rows(slint::ModelRc::new(slint::VecModel::from(
            audio_device_menu_rows(state),
        )));
    let languages = crate::platform::language::LanguageNames::current();
    for kind in [TrackKind::Video, TrackKind::Audio, TrackKind::Sub] {
        let tracks: Vec<_> = state
            .tracks
            .iter()
            .filter(|track| track.kind == kind)
            .collect();
        let mut rows = vec![CatalogRow {
            token: "no".into(),
            label: "<None>".into(),
            detail: "".into(),
            path: "".into(),
            language: "".into(),
            selected: !tracks.iter().any(|track| track.is_primary_selected()),
            playing: false,
            ..CatalogRow::default()
        }];
        rows.extend(tracks.into_iter().map(|track| CatalogRow {
            token: track.id.to_string().into(),
            label: track.display_name().into(),
            detail: "".into(),
            path: "".into(),
            language: languages.display_name(track.lang.as_deref()).into(),
            selected: track.is_primary_selected(),
            playing: false,
            ..CatalogRow::default()
        }));
        let selected_index = rows.iter().position(|row| row.selected).unwrap_or(0) as i32;
        let model = slint::ModelRc::new(slint::VecModel::from(rows));
        match kind {
            TrackKind::Video => {
                let video = app.global::<riina_ui::PlaybackVideoState>();
                video.set_video_track_selected_index(selected_index);
                video.set_video_tracks(model);
            }
            TrackKind::Audio => {
                app.global::<riina_ui::PlaybackAudioState>()
                    .set_audio_track_selected_index(selected_index);
                app.global::<riina_ui::PlaybackAudioState>()
                    .set_audio_tracks(model);
            }
            TrackKind::Sub => {
                app.global::<riina_ui::PlaybackSubtitleState>()
                    .set_subtitle_track_selected_index(selected_index);
                app.global::<riina_ui::PlaybackSubtitleState>()
                    .set_subtitle_tracks(model);
            }
        }
    }
    let subtitle_tracks: Vec<_> = state
        .tracks
        .iter()
        .filter(|t| t.kind == TrackKind::Sub)
        .collect();
    let secondary_selected =
        |track: &&riina_core::Track| track.selected && track.main_selection == Some(1);
    let mut secondary_rows = vec![CatalogRow {
        token: "no".into(),
        label: "<None>".into(),
        detail: "".into(),
        path: "".into(),
        language: "".into(),
        selected: !subtitle_tracks.iter().any(secondary_selected),
        playing: false,
        ..CatalogRow::default()
    }];
    secondary_rows.extend(subtitle_tracks.into_iter().map(|track| CatalogRow {
        token: track.id.to_string().into(),
        label: track.display_name().into(),
        detail: "".into(),
        path: "".into(),
        language: languages.display_name(track.lang.as_deref()).into(),
        selected: track.selected && track.main_selection == Some(1),
        playing: false,
        ..CatalogRow::default()
    }));
    app.global::<riina_ui::PlaybackSubtitleState>()
        .set_secondary_subtitle_track_selected_index(
            secondary_rows
                .iter()
                .position(|row| row.selected)
                .unwrap_or(0) as i32,
        );
    app.global::<riina_ui::PlaybackSubtitleState>()
        .set_secondary_subtitle_tracks(slint::ModelRc::new(slint::VecModel::from(secondary_rows)));
}

fn update_chapters(app: &PlayerWindow, state: &Playback) {
    app.global::<riina_ui::PlayerCatalogState>()
        .set_chapters(slint::ModelRc::new(slint::VecModel::from(
            state
                .chapters
                .iter()
                .enumerate()
                .map(|(index, chapter)| CatalogRow {
                    token: index.to_string().into(),
                    detail: "".into(),
                    path: "".into(),
                    language: "".into(),
                    label: format!(
                        "{}  {}",
                        time_label(Some(chapter.time)),
                        chapter
                            .title
                            .clone()
                            .unwrap_or_else(|| format!("Chapter {}", index + 1))
                    )
                    .into(),
                    selected: state.position.is_some_and(|position| {
                        position >= chapter.time
                            && state
                                .chapters
                                .get(index + 1)
                                .is_none_or(|next| position < next.time)
                    }),
                    playing: false,
                    ..CatalogRow::default()
                })
                .collect::<Vec<_>>(),
        )));
    // IINA omits the first chapter boundary because it coincides with the bar origin.
    app.global::<riina_ui::PlaybackTimelineState>()
        .set_chapter_times(slint::ModelRc::new(slint::VecModel::from(
            state
                .chapters
                .iter()
                .skip(1)
                .map(|chapter| chapter.time as f32)
                .filter(|time| time.is_finite() && *time >= 0.0)
                .collect::<Vec<_>>(),
        )));
}

pub(super) fn current_chapter(state: &Playback) -> Option<usize> {
    state.position.and_then(|position| {
        state
            .chapters
            .iter()
            .rposition(|chapter| chapter.time <= position)
    })
}

pub(super) fn install_track_cycles(app: &PlayerWindow, context: &PlayerContext) {
    bind!(
        context,
        app.global::<PlayerMenuActions>(),
        on_cycle_audio_track,
        Command::CycleTrack(TrackKind::Audio)
    );
    bind!(
        context,
        app.global::<PlayerMenuActions>(),
        on_cycle_subtitle_track,
        Command::CycleTrack(TrackKind::Sub)
    );
    bind!(
        context,
        app.global::<PlayerMenuActions>(),
        on_cycle_video_track,
        Command::CycleTrack(TrackKind::Video)
    );
}

#[cfg(test)]
mod audio_device_menu_tests {
    use super::{audio_device_menu_rows, should_fallback_removed_audio_device};
    use riina_core::{Phase, Playback, audio_preferences::OutputDevice};

    #[test]
    fn device_menu_uses_native_ids_and_marks_the_observed_selection() {
        let state = Playback {
            audio_output: Some("wasapi".into()),
            audio_device: Some("wasapi/{speaker-id}".into()),
            audio_devices: vec![
                OutputDevice {
                    name: "auto".into(),
                    description: "Autoselect device".into(),
                },
                OutputDevice {
                    name: "wasapi/{speaker-id}".into(),
                    description: "Speakers (USB Audio)".into(),
                },
                OutputDevice {
                    name: "dsound/{speaker-id}".into(),
                    description: "Incompatible DirectSound device".into(),
                },
            ]
            .into(),
            ..Playback::default()
        };
        let rows = audio_device_menu_rows(&state);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].token.as_str(), "auto");
        assert!(!rows[0].selected);
        assert_eq!(rows[1].token.as_str(), "wasapi/{speaker-id}");
        assert_eq!(rows[1].label.as_str(), "Speakers (USB Audio)");
        assert!(rows[1].selected);
    }

    #[test]
    fn device_menu_uses_native_id_when_description_is_missing() {
        let state = Playback {
            audio_devices: vec![OutputDevice {
                name: "wasapi/{speaker-id}".into(),
                description: "  ".into(),
            }]
            .into(),
            ..Playback::default()
        };
        let rows = audio_device_menu_rows(&state);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label.as_str(), "wasapi/{speaker-id}");
    }

    #[test]
    fn removed_device_falls_back_once_only_for_active_playback() {
        let previous = Playback {
            phase: Phase::Playing,
            audio_device: Some("wasapi/{speaker-id}".into()),
            audio_devices: vec![OutputDevice {
                name: "wasapi/{speaker-id}".into(),
                description: "Speakers".into(),
            }]
            .into(),
            ..Playback::default()
        };
        let removed = Playback {
            phase: Phase::Playing,
            audio_device: previous.audio_device.clone(),
            audio_devices: vec![OutputDevice {
                name: "wasapi/{headphones-id}".into(),
                description: "Headphones".into(),
            }]
            .into(),
            ..Playback::default()
        };
        assert!(should_fallback_removed_audio_device(
            Some(&previous),
            &removed
        ));
        assert!(!should_fallback_removed_audio_device(
            Some(&removed),
            &removed
        ));

        let stopped = Playback {
            phase: Phase::Stopped,
            ..removed.clone()
        };
        assert!(!should_fallback_removed_audio_device(
            Some(&previous),
            &stopped
        ));
        let no_devices = Playback {
            audio_devices: Default::default(),
            ..removed
        };
        assert!(!should_fallback_removed_audio_device(
            Some(&previous),
            &no_devices
        ));
    }
}
