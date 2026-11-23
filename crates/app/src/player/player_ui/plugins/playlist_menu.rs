//! Bind playlist context requests to the current player and observed row order.
use crate::{
    PlayerWindow,
    platform::playlist_menu::{self, Entry, Target},
    plugin_host::{PlaylistMenuContribution, PluginHostHandle},
};
use riina_core::plugin_menu::PluginMenuItem;
use riina_ui::{PlayerCatalogState, PlayerMenuActions, PlayerSidebarState, PlaylistInteraction};
use slint::{ComponentHandle, LogicalPosition, Model, SharedString};

enum Action {
    Player(i32),
    Plugin { identifier: String, id: String },
}

struct Request {
    generation: i32,
    token: SharedString,
    position: LogicalPosition,
    playlist: Vec<SharedString>,
    index: usize,
    sidebar: (i32, i32),
    network: bool,
}

pub(super) fn install(app: &PlayerWindow, host: Option<&PluginHostHandle>) {
    let Some(host) = host.filter(|_| playlist_menu::SUPPORTED).cloned() else {
        return;
    };
    let weak = app.as_weak();
    app.global::<PlaylistInteraction>()
        .on_request_playlist_menu(move |token, x, y| {
            let Some(app) = weak.upgrade() else {
                return false;
            };
            let interaction = app.global::<PlaylistInteraction>();
            let generation = interaction.get_playlist_menu_generation().wrapping_add(1);
            interaction.set_playlist_menu_generation(generation);
            if !host.has_active_sessions() {
                return false;
            }
            let Some((request, indices)) = Request::new(&app, generation, token, x, y) else {
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
        token: SharedString,
        x: f32,
        y: f32,
    ) -> Option<(Self, Vec<usize>)> {
        let rows = app.global::<PlayerCatalogState>().get_playlist();
        let playlist = rows.iter().map(|row| row.token).collect::<Vec<_>>();
        let index = playlist.iter().position(|row| row == &token)?;
        let network = rows.row_data(index)?.network;
        let indices = selected_indices(app, &playlist, index);
        Some((
            Self {
                generation,
                token,
                position: LogicalPosition::new(x, y),
                playlist,
                index,
                sidebar: sidebars(app),
                network,
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
            && self.sidebar == sidebars(app)
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
        match result {
            Ok(Some(Action::Player(action))) => {
                app.global::<PlayerMenuActions>()
                    .invoke_playlist_action(action, self.token);
            }
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
        let player = |title: &str, action, enabled| Entry::Item {
            title: title.into(),
            enabled,
            checked: false,
            target: Target::Action(Action::Player(action)),
        };
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
            player("Move Down", 3, self.index + 1 < self.playlist.len()),
        ];
        if self.network {
            entries.extend([
                Entry::Separator,
                player("Open in Browser", 10, true),
                player("Copy URL", 11, true),
            ]);
        }
        if !contributions.is_empty() {
            entries.extend([Entry::Separator, player("Plugin", 0, false)]);
            entries.extend(contributions.into_iter().flat_map(|group| {
                group
                    .items
                    .into_iter()
                    .map(move |item| plugin_item(&group.identifier, item))
            }));
        }
        entries
    }
}

fn sidebars(app: &PlayerWindow) -> (i32, i32) {
    let state = app.global::<PlayerSidebarState>();
    (state.get_leading_sidebar(), state.get_sidebar())
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
