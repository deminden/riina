//! Playlist, chapter, track and audio-device models follow native observations.
use crate::{CatalogRow, MenuCatalogRow, PlayerWindow};
use riina_core::{Phase, Playback, TrackKind, time_label};
use slint::{ComponentHandle, Model};
use std::{cell::Cell, path::PathBuf, rc::Rc, sync::Arc, time::Duration};

pub(super) fn install_playlist_selection(app: &PlayerWindow, selected: &[String]) {
    let current = app.get_playlist();
    let rows = (0..current.row_count())
        .filter_map(|index| current.row_data(index))
        .map(|mut row| {
            row.selected = selected
                .iter()
                .any(|token| token.as_str() == row.token.as_str());
            row
        })
        .collect::<Vec<_>>();
    app.set_playlist(slint::ModelRc::new(slint::VecModel::from(rows)));
}

pub(super) fn selected_playlist_paths(app: &PlayerWindow, clicked_token: &str) -> Vec<String> {
    let selected = app
        .get_selected_playlist_tokens()
        .iter()
        .map(|token| token.to_string())
        .collect::<Vec<_>>();
    let selected = if selected.iter().any(|token| token == clicked_token) {
        selected
    } else {
        vec![clicked_token.to_owned()]
    };
    let rows = app.get_playlist();
    (0..rows.row_count())
        .filter_map(|index| rows.row_data(index))
        .filter(|row| selected.iter().any(|token| token == row.token.as_str()))
        .map(|row| row.path.to_string())
        .collect()
}

pub(super) fn selected_playlist_ids(
    app: &PlayerWindow,
    clicked_token: &str,
) -> Result<Vec<i64>, String> {
    let selected = app
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
        .get_selected_playlist_tokens()
        .iter()
        .map(|token| token.to_string())
        .collect::<Vec<_>>();
    let rows = app.get_playlist();
    (0..rows.row_count())
        .find(|index| {
            rows.row_data(*index)
                .is_some_and(|row| selected.iter().any(|token| token == row.token.as_str()))
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

pub(super) fn update_catalog(app: &PlayerWindow, state: &Playback) {
    app.set_downloaded_subtitle_save_available(
        crate::file_actions::downloaded_subtitle_path(state).is_some(),
    );
    app.set_has_active_audio_track(
        state
            .tracks
            .iter()
            .any(|track| track.kind == TrackKind::Audio && track.is_primary_selected()),
    );
    app.set_audio_device_menu_rows(slint::ModelRc::new(slint::VecModel::from(
        audio_device_menu_rows(state),
    )));
    let languages = crate::language_names::LanguageNames::current();
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
            network: false,
        }];
        rows.extend(tracks.into_iter().map(|track| CatalogRow {
            token: track.id.to_string().into(),
            label: track.display_name().into(),
            detail: "".into(),
            path: "".into(),
            language: languages.display_name(track.lang.as_deref()).into(),
            selected: track.is_primary_selected(),
            playing: false,
            network: false,
        }));
        let selected_index = rows.iter().position(|row| row.selected).unwrap_or(0) as i32;
        let model = slint::ModelRc::new(slint::VecModel::from(rows));
        match kind {
            TrackKind::Video => {
                app.set_video_track_selected_index(selected_index);
                app.set_video_tracks(model);
            }
            TrackKind::Audio => {
                app.set_audio_track_selected_index(selected_index);
                app.set_audio_tracks(model);
            }
            TrackKind::Sub => {
                app.set_subtitle_track_selected_index(selected_index);
                app.set_subtitle_tracks(model);
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
        network: false,
    }];
    secondary_rows.extend(subtitle_tracks.into_iter().map(|track| CatalogRow {
        token: track.id.to_string().into(),
        label: track.display_name().into(),
        detail: "".into(),
        path: "".into(),
        language: languages.display_name(track.lang.as_deref()).into(),
        selected: track.selected && track.main_selection == Some(1),
        playing: false,
        network: false,
    }));
    app.set_secondary_subtitle_track_selected_index(
        secondary_rows
            .iter()
            .position(|row| row.selected)
            .unwrap_or(0) as i32,
    );
    app.set_secondary_subtitle_tracks(slint::ModelRc::new(slint::VecModel::from(secondary_rows)));
    app.set_chapters(slint::ModelRc::new(slint::VecModel::from(
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
                network: false,
            })
            .collect::<Vec<_>>(),
    )));
    // IINA omits the first chapter boundary because it coincides with the bar origin.
    app.set_chapter_times(slint::ModelRc::new(slint::VecModel::from(
        state
            .chapters
            .iter()
            .skip(1)
            .map(|chapter| chapter.time as f32)
            .filter(|time| time.is_finite() && *time >= 0.0)
            .collect::<Vec<_>>(),
    )));
    let selected_playlist_tokens = app.get_selected_playlist_tokens();
    app.set_playlist(slint::ModelRc::new(slint::VecModel::from(
        state
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
                let metadata = (app.get_playlist_show_metadata()
                    && (!app.get_playlist_show_metadata_in_music_mode() || app.get_music_mode()))
                .then(|| crate::playlist_metadata::lookup(&entry.filename))
                .flatten();
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
                    network: crate::platform::is_http_url(entry.filename.as_str()),
                }
            })
            .collect::<Vec<_>>(),
    )));
    if !state
        .playlist
        .iter()
        .any(|entry| entry.id.to_string() == app.get_selected_playlist_token().as_str())
    {
        app.set_selected_playlist_token("".into());
    }
    let selection = app.get_selected_playlist_tokens();
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
        app.set_selected_playlist_tokens(slint::ModelRc::new(slint::VecModel::from(retained)));
    }
    let retained_selection = app
        .get_selected_playlist_tokens()
        .iter()
        .map(|token| token.to_string())
        .collect::<Vec<_>>();
    install_playlist_selection(app, &retained_selection);
}

pub(super) fn install_playlist_metadata_refresh(
    app: &PlayerWindow,
    playback: Arc<std::sync::Mutex<Playback>>,
) -> Rc<slint::Timer> {
    let timer = Rc::new(slint::Timer::default());
    let weak = app.as_weak();
    let last_state = Rc::new(Cell::new(None::<(u64, bool, bool, bool)>));
    let observed = last_state.clone();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(300),
        move || {
            let Some(app) = weak.upgrade() else { return };
            let state = (
                crate::playlist_metadata::revision(),
                app.get_playlist_show_metadata(),
                app.get_playlist_show_metadata_in_music_mode(),
                app.get_music_mode(),
            );
            if observed.get() == Some(state) {
                return;
            }
            observed.set(Some(state));
            if let Ok(playback) = playback.lock() {
                update_catalog(&app, &playback.clone());
            }
        },
    );
    timer
}

pub(super) fn current_chapter(state: &Playback) -> Option<usize> {
    state.position.and_then(|position| {
        state
            .chapters
            .iter()
            .rposition(|chapter| chapter.time <= position)
    })
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
            ],
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
            }],
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
            }],
            ..Playback::default()
        };
        let removed = Playback {
            phase: Phase::Playing,
            audio_device: previous.audio_device.clone(),
            audio_devices: vec![OutputDevice {
                name: "wasapi/{headphones-id}".into(),
                description: "Headphones".into(),
            }],
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
            audio_devices: Vec::new(),
            ..removed
        };
        assert!(!should_fallback_removed_audio_device(
            Some(&previous),
            &no_devices
        ));
    }
}
