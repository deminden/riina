//! Present the actions for the playlist selection captured when the menu opened.
use super::{catalog::playlist_row_targets, playlist_actions::playlist_network_url};
use crate::{
    PlayerWindow,
    platform::playlist_menu::{self, Entry, Target},
    plugin_host::{PlaylistMenuContribution, PluginHostHandle},
};
use riina_core::plugin_menu::PluginMenuItem;
use riina_ui::{
    PlayerCatalogState, PlayerMenuActions, PlayerMusicModeState, PlayerSidebarActions,
    PlayerSidebarState, PlaylistFooterMenu, PlaylistInteraction, PlaylistMenuOrigin,
    PlaylistSubtitleAction,
};
use slint::{ComponentHandle, LogicalPosition, Model, SharedString};
use std::sync::Arc;

#[derive(Clone)]
enum Action {
    Player(i32),
    Footer(FooterAction),
    Subtitle(PlaylistSubtitleAction),
    Plugin { identifier: String, id: String },
}

#[derive(Clone, Copy)]
enum FooterAction {
    AddFiles,
    AddFolders,
    AddUrl,
    Sort(i32),
}

impl FooterAction {
    fn activate(self, app: &PlayerWindow) {
        let menu = app.global::<PlayerMenuActions>();
        match self {
            Self::AddFiles => menu.invoke_browse_files(4),
            Self::AddFolders => menu.invoke_browse_files(5),
            Self::AddUrl => menu.invoke_open_playlist_url_dialog(),
            Self::Sort(index) => app
                .global::<PlayerSidebarActions>()
                .invoke_sort_playlist(index),
        }
    }
}

fn additions() -> impl Iterator<Item = (&'static str, FooterAction)> {
    [
        ("Add File…", FooterAction::AddFiles),
        ("Add Folder…", FooterAction::AddFolders),
        ("Add URL…", FooterAction::AddUrl),
    ]
    .into_iter()
    .filter(|(_, action)| {
        !playlist_menu::MIXED_PLAYLIST_PICKER || !matches!(action, FooterAction::AddFolders)
    })
}

fn footer_entries(menu: PlaylistFooterMenu) -> Vec<Entry<FooterAction>> {
    match menu {
        PlaylistFooterMenu::Add => additions()
            .map(|(title, action)| item(title, action, true))
            .collect(),
        PlaylistFooterMenu::Sort => [
            ("Filename Ascending", 0),
            ("Filename Descending", 1),
            ("File Path Ascending", 2),
            ("File Path Descending", 3),
        ]
        .into_iter()
        .map(|(title, index)| item(title, FooterAction::Sort(index), true))
        .collect(),
    }
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

struct MenuOwner {
    generation: i32,
    surface: Surface,
    origin: PlaylistMenuOrigin,
}

impl MenuOwner {
    fn new(app: &PlayerWindow, generation: i32, origin: PlaylistMenuOrigin) -> Option<Self> {
        Some(Self {
            generation,
            surface: Surface::visible(app, origin)?,
            origin,
        })
    }

    fn current(&self, app: &PlayerWindow) -> bool {
        self.generation
            == app
                .global::<PlaylistInteraction>()
                .get_playlist_menu_generation()
            && Surface::visible(app, self.origin).as_ref() == Some(&self.surface)
            && !app
                .global::<riina_ui::PlayerDialogState>()
                .get_dialog_open()
            && !app.global::<riina_ui::PlayerCropState>().get_crop_editing()
    }
}

struct Request {
    owner: MenuOwner,
    token: SharedString,
    position: LogicalPosition,
    playlist: Vec<(SharedString, SharedString)>,
    selection: Vec<usize>,
    row: Option<Row>,
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
    install_footer(app);
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

fn install_footer(app: &PlayerWindow) {
    let weak = app.as_weak();
    app.global::<PlaylistInteraction>()
        .on_request_playlist_footer_menu(move |origin, menu, x, y| {
            let Some(app) = weak.upgrade() else { return };
            let generation = next_generation(&app);
            let Some(request) = MenuOwner::new(&app, generation, origin) else {
                return;
            };
            playlist_menu::dismiss(&app);
            app.global::<riina_ui::PlayerControlActions>()
                .invoke_activity();
            let _ = weak.upgrade_in_event_loop(move |app| {
                if !request.current(&app) {
                    return;
                }
                let owner = app.as_weak();
                playlist_menu::popup(
                    &app,
                    LogicalPosition::new(x, y),
                    "Playlist menu",
                    footer_entries(menu),
                    move |action| {
                        if let Some(app) = owner.upgrade()
                            && request.current(&app)
                        {
                            action.activate(&app);
                        }
                    },
                );
            });
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
        let owner = MenuOwner::new(app, generation, origin)?;
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
            owner,
            token,
            position,
            playlist,
            selection,
            row,
        })
    }

    fn id(&self) -> u64 {
        u64::from(self.owner.generation as u32)
    }

    fn current(&self, app: &PlayerWindow) -> bool {
        let rows = app.global::<PlayerCatalogState>().get_playlist();
        self.owner.current(app)
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
        playlist_menu::popup(
            app,
            self.position,
            "Playlist menu",
            entries,
            move |action| {
                if let Some(app) = owner.upgrade() {
                    self.activate(&app, host.as_ref(), action);
                }
            },
        );
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
            Action::Footer(action) => action.activate(app),
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
        if self.owner.origin == PlaylistMenuOrigin::Sidebar && self.row.is_some() {
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
        let has_queued_items = app
            .global::<PlayerCatalogState>()
            .get_playlist()
            .iter()
            .any(|row| !row.playing);
        entries
            .extend(additions().map(|(title, action)| item(title, Action::Footer(action), true)));
        entries.push(item("Clear Playlist", Action::Player(4), has_queued_items));
        entries
    }
}

impl Surface {
    fn visible(app: &PlayerWindow, origin: PlaylistMenuOrigin) -> Option<Self> {
        let music = app.global::<PlayerMusicModeState>();
        let state = app.global::<PlayerSidebarState>();
        match origin {
            PlaylistMenuOrigin::MusicMode => (music.get_music_mode()
                && state.get_visible_catalog_pane() == 4)
                .then_some(Self::MusicMode),
            PlaylistMenuOrigin::Sidebar if !music.get_music_mode() => {
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

fn item<A>(title: &str, action: A, enabled: bool) -> Entry<A> {
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

    fn titles<A>(entries: Vec<Entry<A>>) -> Vec<String> {
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
