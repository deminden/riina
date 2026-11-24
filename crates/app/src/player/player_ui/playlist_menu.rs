//! Present the actions for the playlist selection captured when the menu opened.
use super::{catalog::playlist_row_targets, playlist_actions::playlist_network_url};
use crate::{
    PlayerWindow,
    platform::playlist_menu::{self, Entry, Target},
    plugin_host::{PlaylistMenuContribution, PluginHostHandle},
};
use riina_core::plugin_menu::PluginMenuItem;
use riina_ui::{
    PlayerCatalogState, PlayerMenuActions, PlayerMusicModeState, PlayerSidebarState,
    PlaylistInteraction, PlaylistMenuOrigin, PlaylistSubtitleAction,
};
use slint::{ComponentHandle, LogicalPosition, Model, SharedString};
use std::sync::Arc;

#[derive(Clone)]
enum Action {
    Player(i32),
    AddFiles,
    AddUrl,
    Subtitle(PlaylistSubtitleAction),
    Plugin { identifier: String, id: String },
}

#[derive(PartialEq, Eq)]
enum Surface {
    Sidebars { leading: i32, trailing: i32 },
    MusicMode,
}

struct Row {
    index: usize,
    local_files: usize,
    urls: usize,
    open_in_browser: bool,
}

struct Request {
    generation: i32,
    token: SharedString,
    position: LogicalPosition,
    playlist: Vec<(SharedString, SharedString)>,
    selection: Vec<usize>,
    row: Option<Row>,
    surface: Surface,
    origin: PlaylistMenuOrigin,
}

pub(super) fn dismiss(app: &PlayerWindow) {
    next_generation(app);
    playlist_menu::dismiss(app);
}

fn next_generation(app: &PlayerWindow) -> i32 {
    let interaction = app.global::<PlaylistInteraction>();
    let generation = interaction.get_playlist_menu_generation().wrapping_add(1);
    interaction.set_playlist_menu_generation(generation);
    generation
}

pub(super) fn install(app: &PlayerWindow, host: Option<&PluginHostHandle>) {
    let host = host.cloned();
    let weak = app.as_weak();
    app.global::<PlaylistInteraction>()
        .on_request_playlist_menu(move |origin, token, x, y| {
            let Some(app) = weak.upgrade() else {
                return;
            };
            let generation = next_generation(&app);
            playlist_menu::dismiss(&app);
            let position = LogicalPosition::new(x, y);
            let Some(request) = Request::new(&app, generation, origin, token, position) else {
                return;
            };
            let request = Arc::new(request);
            if let Some(host) = host.as_ref().filter(|host| host.has_active_sessions()) {
                let callback = weak.clone();
                let target = host.clone();
                let menu = request.clone();
                if host
                    .build_playlist_menu(
                        request.id(),
                        request.selection.clone(),
                        move |contributions| {
                            let _ = callback.upgrade_in_event_loop(move |app| {
                                menu.show(&app, Some(target), contributions);
                            });
                        },
                    )
                    .is_ok()
                {
                    return;
                }
            }
            // Native popup tracking can dispatch window events before it returns.
            let _ = weak.upgrade_in_event_loop(move |app| request.show(&app, None, Vec::new()));
        });
}

impl Request {
    fn new(
        app: &PlayerWindow,
        generation: i32,
        origin: PlaylistMenuOrigin,
        token: SharedString,
        position: LogicalPosition,
    ) -> Option<Self> {
        let surface = Surface::visible(app, origin)?;
        let rows = app.global::<PlayerCatalogState>().get_playlist();
        let playlist = rows.iter().map(|row| (row.token, row.path)).collect();
        let (row, selection) = if token.is_empty() {
            (None, Vec::new())
        } else {
            let (index, selection) = playlist_row_targets(app, &rows, &token)?;
            let row = Row::new(
                index,
                selection.iter().filter_map(|&index| rows.row_data(index)),
            );
            (Some(row), selection)
        };
        Some(Self {
            generation,
            token,
            position,
            playlist,
            selection,
            row,
            surface,
            origin,
        })
    }

    fn id(&self) -> u64 {
        u64::from(self.generation as u32)
    }

    fn current(&self, app: &PlayerWindow) -> bool {
        let rows = app.global::<PlayerCatalogState>().get_playlist();
        self.generation
            == app
                .global::<PlaylistInteraction>()
                .get_playlist_menu_generation()
            && Surface::visible(app, self.origin).as_ref() == Some(&self.surface)
            && self
                .playlist
                .iter()
                .cloned()
                .eq(rows.iter().map(|row| (row.token, row.path)))
            && (self.row.is_none()
                || playlist_row_targets(app, &rows, &self.token)
                    .is_some_and(|(_, selection)| selection == self.selection))
    }

    fn show(
        self: Arc<Self>,
        app: &PlayerWindow,
        host: Option<PluginHostHandle>,
        contributions: Vec<PlaylistMenuContribution>,
    ) {
        if !self.current(app) {
            return;
        }
        let entries = self.entries(app, contributions);
        let owner = app.as_weak();
        playlist_menu::popup(app, self.position, entries, move |action| {
            if let Some(app) = owner.upgrade() {
                self.activate(&app, host.as_ref(), action);
            }
        });
    }

    fn activate(&self, app: &PlayerWindow, host: Option<&PluginHostHandle>, action: Action) {
        // Window events and playlist edits may arrive while the menu is visible.
        if !self.current(app) {
            return;
        }
        let actions = app.global::<PlayerMenuActions>();
        match action {
            Action::Player(action) => {
                actions.invoke_playlist_action(action, self.token.clone());
            }
            Action::AddFiles => actions.invoke_browse_files(4),
            Action::AddUrl => actions.invoke_open_playlist_url_dialog(),
            Action::Subtitle(action) => app
                .global::<PlaylistInteraction>()
                .invoke_subtitle_action(action, self.token.clone()),
            Action::Plugin { identifier, id } => {
                if let Some(host) = host
                    && let Err(error) = host.activate_playlist_menu_item(self.id(), identifier, id)
                {
                    crate::diagnostics::report(app, error);
                }
            }
        }
    }

    fn entries(
        &self,
        app: &PlayerWindow,
        contributions: Vec<PlaylistMenuContribution>,
    ) -> Vec<Entry<Action>> {
        let interaction = app.global::<PlaylistInteraction>();
        let mut entries = self
            .row
            .as_ref()
            .map_or_else(Vec::new, |row| row.entries(self.playlist.len()));
        if self.origin == PlaylistMenuOrigin::Sidebar && self.row.is_some() {
            let subtitles = interaction.invoke_subtitle_targets(self.token.clone());
            if subtitles.single || subtitles.matched > 0 {
                separate(&mut entries);
                entries.push(item(
                    &format!("Subtitles ({} loaded)", subtitles.matched),
                    Action::Player(0),
                    false,
                ));
            }
            if subtitles.single {
                entries.push(item(
                    "Add Subtitle…",
                    Action::Subtitle(PlaylistSubtitleAction::Add),
                    true,
                ));
            }
            if subtitles.matched > 0 {
                entries.push(item(
                    "Wrong Subtitle",
                    Action::Subtitle(PlaylistSubtitleAction::Clear),
                    true,
                ));
            }
        }
        if let Some(row) = &self.row {
            row.append_external_actions(
                &mut entries,
                interaction.get_reveal_action_label().as_str(),
            );
        }
        if !contributions.is_empty() {
            separate(&mut entries);
            entries.push(item("Plugin", Action::Player(0), false));
            entries.extend(contributions.into_iter().flat_map(|group| {
                group
                    .items
                    .into_iter()
                    .map(move |item| plugin_item(&group.identifier, item))
            }));
        }
        separate(&mut entries);
        entries.extend([
            item("Add File…", Action::AddFiles, true),
            item("Add URL…", Action::AddUrl, true),
            item("Clear Playlist", Action::Player(4), self.playlist.len() > 1),
        ]);
        entries
    }
}

impl Surface {
    fn visible(app: &PlayerWindow, origin: PlaylistMenuOrigin) -> Option<Self> {
        let music = app.global::<PlayerMusicModeState>();
        match origin {
            PlaylistMenuOrigin::MusicMode => (music.get_music_mode()
                && music.get_music_mode_show_playlist())
            .then_some(Self::MusicMode),
            PlaylistMenuOrigin::Sidebar if !music.get_music_mode() => {
                let state = app.global::<PlayerSidebarState>();
                let leading = state.get_leading_sidebar();
                let trailing = state.get_sidebar();
                (leading == 4 || trailing == 4).then_some(Self::Sidebars { leading, trailing })
            }
            PlaylistMenuOrigin::Sidebar => None,
        }
    }
}

impl Row {
    fn new(index: usize, rows: impl Iterator<Item = crate::CatalogRow>) -> Self {
        let mut targets = Self {
            index,
            local_files: 0,
            urls: 0,
            open_in_browser: false,
        };
        for row in rows {
            if let Some(url) = playlist_network_url(&row) {
                targets.urls += 1;
                targets.open_in_browser |= crate::platform::is_http_url(url);
            } else if crate::file_actions::local_media_path(row.path.as_str()).is_some() {
                targets.local_files += 1;
            }
        }
        targets
    }

    fn entries(&self, count: usize) -> Vec<Entry<Action>> {
        vec![
            player("Play", 12, true),
            player("Play Next", 13, count > 1),
            player("Play in New Window", 9, true),
            player("Remove", 1, true),
            Entry::Separator,
            player("Cut", 7, true),
            player("Copy", 6, true),
            player("Paste", 8, true),
            Entry::Separator,
            player("Move Up", 2, self.index > 0),
            player("Move Down", 3, self.index + 1 < count),
        ]
    }

    fn append_external_actions(&self, entries: &mut Vec<Entry<Action>>, reveal_label: &str) {
        if self.urls > 0 {
            separate(entries);
            if self.open_in_browser {
                entries.push(player("Open in Browser", 10, true));
            }
            let label = if self.urls == 1 {
                "Copy URL"
            } else {
                "Copy URLs"
            };
            entries.push(player(label, 11, true));
        }
        if self.local_files > 0 {
            separate(entries);
            let trash = crate::platform::system_trash_name();
            let trash_label = if self.local_files == 1 {
                format!("Move to {trash}")
            } else {
                format!("Move Selected Files to {trash}")
            };
            entries.extend([
                player(&trash_label, 14, true),
                player(reveal_label, 15, true),
            ]);
        }
    }
}

fn player(title: &str, action: i32, enabled: bool) -> Entry<Action> {
    item(title, Action::Player(action), enabled)
}

fn item(title: &str, action: Action, enabled: bool) -> Entry<Action> {
    Entry::Item {
        title: title.into(),
        shortcut: String::new(),
        enabled,
        checkable: false,
        checked: false,
        target: Target::Action(action),
    }
}

fn separate(entries: &mut Vec<Entry<Action>>) {
    if matches!(entries.last(), Some(Entry::Item { .. })) {
        entries.push(Entry::Separator);
    }
}

fn plugin_item(identifier: &str, item: PluginMenuItem) -> Entry<Action> {
    if item.separator {
        return Entry::Separator;
    }
    let target = if item.items.is_empty() {
        Target::Action(Action::Plugin {
            identifier: identifier.into(),
            id: item.id,
        })
    } else {
        Target::Submenu(
            item.items
                .into_iter()
                .map(|child| plugin_item(identifier, child))
                .collect(),
        )
    };
    Entry::Item {
        title: item.title,
        shortcut: String::new(),
        enabled: item.enabled,
        checkable: item.selected,
        checked: item.selected,
        target,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slint::{ModelRc, VecModel};

    fn titles(entries: Vec<Entry<Action>>) -> Vec<String> {
        entries
            .into_iter()
            .filter_map(|entry| match entry {
                Entry::Item { title, .. } => Some(title),
                Entry::Separator => None,
            })
            .collect()
    }

    #[test]
    fn a_mixed_selection_offers_each_action_and_rejects_stale_targets()
    -> Result<(), slint::PlatformError> {
        i_slint_backend_testing::init_no_event_loop();
        let app = PlayerWindow::new()?;
        app.global::<PlayerSidebarState>().set_leading_sidebar(4);
        let catalog = app.global::<PlayerCatalogState>();
        catalog.set_playlist(ModelRc::new(VecModel::from(
            [
                ("1", "one.avi"),
                ("2", "https://example.test/video"),
                ("3", "two.avi"),
                ("4", "rtsp://example.test/live"),
            ]
            .into_iter()
            .map(|(token, path)| crate::CatalogRow {
                token: token.into(),
                path: path.into(),
                ..Default::default()
            })
            .collect::<Vec<_>>(),
        )));
        let interaction = app.global::<PlaylistInteraction>();
        interaction.set_selected_playlist_tokens(ModelRc::new(VecModel::from(
            ["4", "2", "1", "3"].map(SharedString::from).to_vec(),
        )));
        let make_request = |token: &str| {
            Request::new(
                &app,
                0,
                PlaylistMenuOrigin::Sidebar,
                token.into(),
                LogicalPosition::default(),
            )
            .unwrap()
        };
        let mixed = make_request("1");
        let expected_trash = format!(
            "Move Selected Files to {}",
            crate::platform::system_trash_name()
        );
        assert_eq!(mixed.selection, [0, 1, 2, 3]);
        assert!(mixed.current(&app));
        let offered = titles(mixed.entries(&app, Vec::new()));
        let external = offered
            .iter()
            .position(|item| item == "Open in Browser")
            .unwrap();
        assert_eq!(
            offered[external..external + 4],
            [
                "Open in Browser",
                "Copy URLs",
                &expected_trash,
                "Show in File Manager"
            ]
        );
        interaction.set_selected_playlist_tokens(ModelRc::default());
        assert!(!mixed.current(&app));
        let stream = make_request("4");
        let offered = titles(stream.entries(&app, Vec::new()));
        assert!(offered.iter().any(|title| title == "Copy URL"));
        assert!(!offered.iter().any(|title| title == "Open in Browser"));

        let mut last = catalog.get_playlist().row_data(3).unwrap();
        last.path = "rtsp://example.test/new".into();
        catalog.get_playlist().set_row_data(3, last);
        assert!(!stream.current(&app));
        Ok(())
    }
}
