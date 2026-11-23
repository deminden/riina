//! Bind playlist context requests to the current player and observed row order.
use crate::{
    PlayerWindow,
    platform::playlist_menu::{self, Entry, Target},
    plugin_host::{PlaylistMenuContribution, PluginHostHandle},
};
use riina_core::plugin_menu::PluginMenuItem;
use riina_ui::{
    PlayerCatalogState, PlayerMenuActions, PlayerMusicModeState, PlayerSidebarState,
    PlaylistInteraction, PlaylistMenuOrigin,
};
use slint::{ComponentHandle, LogicalPosition, Model, SharedString};

enum Action {
    Player(i32),
    AddFiles,
    AddUrl,
    Plugin { identifier: String, id: String },
}

#[derive(PartialEq, Eq)]
enum Surface {
    Sidebars { leading: i32, trailing: i32 },
    MusicMode,
}

struct Row {
    index: usize,
    network: bool,
}

struct Request {
    generation: i32,
    token: SharedString,
    position: LogicalPosition,
    playlist: Vec<SharedString>,
    row: Option<Row>,
    surface: Surface,
    origin: PlaylistMenuOrigin,
}

pub(super) fn install(app: &PlayerWindow, host: Option<&PluginHostHandle>) {
    let Some(host) = host.filter(|_| playlist_menu::SUPPORTED).cloned() else {
        return;
    };
    let weak = app.as_weak();
    app.global::<PlaylistInteraction>()
        .on_request_playlist_menu(move |origin, token, x, y| {
            let Some(app) = weak.upgrade() else {
                return false;
            };
            let interaction = app.global::<PlaylistInteraction>();
            let generation = interaction.get_playlist_menu_generation().wrapping_add(1);
            interaction.set_playlist_menu_generation(generation);
            if !host.has_active_sessions() {
                return false;
            }
            let position = LogicalPosition::new(x, y);
            let Some((request, indices)) = Request::new(&app, generation, origin, token, position)
            else {
                return false;
            };
            let callback = weak.clone();
            let target = host.clone();
            host.build_playlist_menu(request.id(), indices, move |contributions| {
                let _ = callback.upgrade_in_event_loop(move |app| {
                    request.show(&app, &target, contributions);
                });
            })
            .is_ok()
        });
}

impl Request {
    fn new(
        app: &PlayerWindow,
        generation: i32,
        origin: PlaylistMenuOrigin,
        token: SharedString,
        position: LogicalPosition,
    ) -> Option<(Self, Vec<usize>)> {
        let surface = Surface::visible(app, origin)?;
        let rows = app.global::<PlayerCatalogState>().get_playlist();
        let playlist = rows.iter().map(|row| row.token).collect::<Vec<_>>();
        let (row, indices) = if token.is_empty() {
            (None, Vec::new())
        } else {
            let index = playlist.iter().position(|row| row == &token)?;
            let network = rows.row_data(index)?.network;
            (
                Some(Row { index, network }),
                selected_indices(app, &playlist, index),
            )
        };
        Some((
            Self {
                generation,
                token,
                position,
                playlist,
                row,
                surface,
                origin,
            },
            indices,
        ))
    }

    fn id(&self) -> u64 {
        u64::from(self.generation as u32)
    }

    fn current(&self, app: &PlayerWindow) -> bool {
        self.generation
            == app
                .global::<PlaylistInteraction>()
                .get_playlist_menu_generation()
            && Surface::visible(app, self.origin).as_ref() == Some(&self.surface)
            && self.playlist.iter().cloned().eq(app
                .global::<PlayerCatalogState>()
                .get_playlist()
                .iter()
                .map(|row| row.token))
    }

    fn show(
        self,
        app: &PlayerWindow,
        host: &PluginHostHandle,
        contributions: Vec<PlaylistMenuContribution>,
    ) {
        if !self.current(app) {
            return;
        }
        let entries = self.entries(contributions);
        let result = playlist_menu::popup(app.window(), self.position, entries);
        // The popup's native loop can deliver queued player events while it is open.
        if !self.current(app) {
            return;
        }
        let actions = app.global::<PlayerMenuActions>();
        match result {
            Ok(Some(Action::Player(action))) => {
                actions.invoke_playlist_action(action, self.token);
            }
            Ok(Some(Action::AddFiles)) => actions.invoke_browse_files(4),
            Ok(Some(Action::AddUrl)) => actions.invoke_open_playlist_url_dialog(),
            Ok(Some(Action::Plugin { identifier, id })) => {
                if let Err(error) = host.activate_playlist_menu_item(self.id(), identifier, id) {
                    crate::diagnostics::report(app, error);
                }
            }
            Ok(None) => {}
            Err(error) => crate::diagnostics::report(app, error),
        }
    }

    fn entries(&self, contributions: Vec<PlaylistMenuContribution>) -> Vec<Entry<Action>> {
        let mut entries = self
            .row
            .as_ref()
            .map_or_else(Vec::new, |row| row.entries(self.playlist.len()));
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
    fn entries(&self, count: usize) -> Vec<Entry<Action>> {
        let player = |title, action, enabled| item(title, Action::Player(action), enabled);
        let mut entries = vec![
            player("Play", 12, true),
            player("Play in New Window", 9, true),
            player("Remove", 1, true),
            Entry::Separator,
            player("Cut", 7, true),
            player("Copy", 6, true),
            player("Paste", 8, true),
            Entry::Separator,
            player("Move Up", 2, self.index > 0),
            player("Move Down", 3, self.index + 1 < count),
        ];
        if self.network {
            entries.extend([
                Entry::Separator,
                player("Open in Browser", 10, true),
                player("Copy URL", 11, true),
            ]);
        }
        entries
    }
}

fn item(title: &str, action: Action, enabled: bool) -> Entry<Action> {
    Entry::Item {
        title: title.into(),
        enabled,
        checked: false,
        target: Target::Action(action),
    }
}

fn separate(entries: &mut Vec<Entry<Action>>) {
    if matches!(entries.last(), Some(Entry::Item { .. })) {
        entries.push(Entry::Separator);
    }
}

fn selected_indices(app: &PlayerWindow, playlist: &[SharedString], clicked: usize) -> Vec<usize> {
    let selected = app
        .global::<PlaylistInteraction>()
        .get_selected_playlist_tokens();
    let selected = selected.iter().collect::<std::collections::BTreeSet<_>>();
    if !selected.contains(&playlist[clicked]) {
        return vec![clicked];
    }
    playlist
        .iter()
        .enumerate()
        .filter_map(|(index, token)| selected.contains(token).then_some(index))
        .collect()
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
        enabled: item.enabled,
        checked: item.selected,
        target,
    }
}
