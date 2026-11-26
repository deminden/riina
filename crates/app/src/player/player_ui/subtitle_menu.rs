//! Choose loaded and nearby subtitles from the owning playback session.
use crate::{
    PlaybackStore, PlayerWindow,
    platform::playlist_menu::{self, Entry, Target},
    subtitle_discovery::{self, MenuFiles},
};
use riina_core::{
    Command, Phase, Playback, TrackKind, media_reference::same_native_path, sync::lock,
};
use riina_mpv::CommandSender;
use riina_ui::{
    PlayerControlActions, PlayerControlsState, PlayerCropState, PlayerDialogState,
    PlayerMenuActions, PlayerMusicModeState, PlayerSidebarState, SubtitleMenuSource,
};
use slint::{ComponentHandle, LogicalPosition};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

struct Selection {
    media: u64,
    open_request: i32,
    source: SubtitleMenuSource,
    position: LogicalPosition,
}

#[derive(Clone)]
enum Action {
    Select(Option<i64>),
    Load(String),
}

pub(super) fn install(app: &PlayerWindow, sender: &CommandSender, playback: &PlaybackStore) {
    let weak = app.as_weak();
    let playback = playback.clone();
    let sender = sender.clone();
    let busy = Arc::new(AtomicBool::new(false));
    app.global::<PlayerMenuActions>()
        .on_show_subtitle_menu(move |source, x, y| {
            let Some(app) = weak.upgrade().filter(|app| visible(app, source)) else {
                return;
            };
            let state = lock(&playback).clone();
            if !active(&state) || busy.swap(true, Ordering::AcqRel) {
                return;
            }
            let lease = ScanLease(busy.clone());
            let selection = Selection {
                media: state.generation,
                open_request: app.get_media_request_generation(),
                source,
                position: LogicalPosition::new(x, y),
            };
            super::playlist_menu::dismiss(&app);
            app.global::<PlayerControlActions>().invoke_activity();
            let path = state
                .playlist
                .iter()
                .find(|entry| {
                    state
                        .active_playlist_entry
                        .map_or(entry.current, |id| id == entry.id)
                })
                .and_then(|entry| crate::file_actions::local_media_path(&entry.filename));
            let preferences = crate::subtitle_auto_load_preferences::current_player(&app).ok();
            let weak = weak.clone();
            let playback = playback.clone();
            let sender = sender.clone();
            if let Err(error) = crate::background::spawn_named("riina-subtitle-menu", move || {
                let files = path.zip(preferences.as_ref()).map_or_else(
                    MenuFiles::default,
                    |(path, preferences)| {
                        subtitle_discovery::files_for_menu(&path, preferences, || {
                            let state = lock(&playback);
                            state.generation == selection.media && active(&state)
                        })
                    },
                );
                let _ = crate::background::to_ui(weak, move |app| {
                    drop(lease);
                    let state = lock(&playback).clone();
                    if selection.current(&app, &state)
                        && crate::subtitle_auto_load_preferences::current_player(&app).ok()
                            == preferences
                    {
                        selection.show(&app, &state, files, sender, playback);
                    }
                });
            }) {
                crate::diagnostics::report(
                    &app,
                    format!("Could not read nearby subtitles: {error}"),
                );
            }
        });
}

struct ScanLease(Arc<AtomicBool>);

impl Drop for ScanLease {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

fn visible(app: &PlayerWindow, source: SubtitleMenuSource) -> bool {
    if !app.get_loaded()
        || app.global::<PlayerMusicModeState>().get_music_mode()
        || app.global::<PlayerDialogState>().get_dialog_open()
        || app.global::<PlayerCropState>().get_crop_editing()
    {
        return false;
    }
    match source {
        SubtitleMenuSource::Sidebar => {
            let sidebars = app.global::<PlayerSidebarState>();
            sidebars.get_sidebar() == 2 || sidebars.get_leading_sidebar() == 2
        }
        SubtitleMenuSource::Controller => {
            use slint::Model;
            app.global::<PlayerControlsState>()
                .get_toolbar_visible_order()
                .iter()
                .any(|kind| kind == 5)
        }
    }
}

fn active(state: &Playback) -> bool {
    !matches!(state.phase, Phase::Loading | Phase::Stopped | Phase::Failed)
}

impl Selection {
    fn current(&self, app: &PlayerWindow, state: &Playback) -> bool {
        self.media == state.generation
            && self.open_request == app.get_media_request_generation()
            && active(state)
            && visible(app, self.source)
    }

    fn show(
        self,
        app: &PlayerWindow,
        state: &Playback,
        files: MenuFiles,
        sender: CommandSender,
        playback: PlaybackStore,
    ) {
        let entries = entries(state, &files, self.source);
        let weak = app.as_weak();
        playlist_menu::popup(
            app,
            self.position,
            "Subtitles menu",
            entries,
            move |action| {
                let Some(app) = weak.upgrade() else { return };
                let state = lock(&playback).clone();
                if self.current(&app, &state)
                    && let Some(command) = action.command(&state)
                    && let Err(error) = sender.send_for_media(self.media, command)
                {
                    crate::diagnostics::report(&app, error);
                }
            },
        );
    }
}

impl Action {
    fn command(self, state: &Playback) -> Option<Command> {
        match self {
            Self::Select(id)
                if id.is_none_or(|id| {
                    state
                        .tracks
                        .iter()
                        .any(|track| track.kind == TrackKind::Sub && track.id == id)
                }) =>
            {
                Some(Command::SelectTrack {
                    kind: TrackKind::Sub,
                    id,
                })
            }
            Self::Select(_) => None,
            Self::Load(path) => Some(Command::AddSubtitle(path)),
        }
    }
}

fn entries(state: &Playback, files: &MenuFiles, source: SubtitleMenuSource) -> Vec<Entry<Action>> {
    let subtitles = || {
        state
            .tracks
            .iter()
            .filter(|track| track.kind == TrackKind::Sub)
    };
    let mut entries = Vec::new();
    if source == SubtitleMenuSource::Controller {
        if subtitles().next().is_none() {
            entries.push(disabled("No subtitles loaded"));
        } else {
            let languages = crate::platform::language::LanguageNames::current();
            entries.push(item(
                "<None>".to_owned(),
                Action::Select(None),
                true,
                Some(!subtitles().any(riina_core::Track::is_primary_selected)),
            ));
            entries.extend(subtitles().map(|track| {
                item(
                    languages.track_title(track),
                    Action::Select(Some(track.id)),
                    true,
                    Some(track.is_primary_selected()),
                )
            }));
        }
        entries.push(Entry::Separator);
    }
    let external = |path: &Path| {
        let reference = path.to_str()?;
        let name = path.file_name()?.to_str()?;
        let checked = (source == SubtitleMenuSource::Sidebar).then(|| {
            subtitles().any(|track| {
                track
                    .external_filename
                    .as_deref()
                    .is_some_and(|existing| same_native_path(existing, reference))
            })
        });
        Some(item(
            name.to_owned(),
            Action::Load(reference.to_owned()),
            true,
            checked,
        ))
    };
    let start = entries.len();
    entries.extend(files.related.iter().filter_map(|path| external(path)));
    if entries.len() > start {
        entries.push(Entry::Separator);
    }
    entries.extend(files.all.iter().filter_map(|path| external(path)));
    if entries.len() == start {
        entries.push(disabled("No external subtitles found"));
    }
    entries
}

fn disabled(title: &str) -> Entry<Action> {
    item(title.to_owned(), Action::Select(None), false, None)
}

fn item(title: String, action: Action, enabled: bool, checked: Option<bool>) -> Entry<Action> {
    Entry::Item {
        title,
        shortcut: String::new(),
        enabled,
        checkable: checked.is_some(),
        checked: checked.unwrap_or_default(),
        target: Target::Action(action),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subtitle_menu_keeps_primary_selection_separate_from_loaded_files() {
        let track = |id, main_selection, external: Option<&str>| {
            serde_json::from_value(
                serde_json::json!({"id": id, "type": "sub", "title": format!("Track {id}"),
                "external-filename": external, "selected": true, "main-selection": main_selection}),
            )
            .unwrap()
        };
        let state = Playback {
            tracks: vec![track(2, 0, None), track(3, 1, Some("sub/episode.srt"))].into(),
            ..Playback::default()
        };
        let files = MenuFiles {
            related: vec!["sub/episode.srt".into()],
            all: vec!["sub/episode.srt".into(), "extra.ass".into()],
        };
        let sidebar = entries(&state, &files, SubtitleMenuSource::Sidebar);
        assert!(matches!(
            &sidebar[0],
            Entry::Item {
                checkable: true,
                checked: true,
                target: Target::Action(Action::Load(path)),
                ..
            } if path == "sub/episode.srt"
        ));
        assert!(matches!(sidebar[1], Entry::Separator));
        let controller = entries(&state, &files, SubtitleMenuSource::Controller);
        assert!(matches!(
            controller[0],
            Entry::Item {
                checkable: true,
                checked: false,
                target: Target::Action(Action::Select(None)),
                ..
            }
        ));
        assert!(matches!(
            controller[1],
            Entry::Item {
                checked: true,
                target: Target::Action(Action::Select(Some(2))),
                ..
            }
        ));
        assert!(matches!(
            controller[2],
            Entry::Item {
                checked: false,
                target: Target::Action(Action::Select(Some(3))),
                ..
            }
        ));
        assert!(matches!(
            controller[4],
            Entry::Item {
                checkable: false,
                ..
            }
        ));
        assert!(Action::Select(Some(9)).command(&state).is_none());
        assert_eq!(
            Action::Load("extra.ass".into()).command(&state),
            Some(Command::AddSubtitle("extra.ass".into()))
        );
    }
}
