//! Reuse the generated player's live menu and shortcuts in a bounded presenter.
use crate::{
    PlayerWindow,
    menu_tree::{MenuFromItemTree, capture_bar},
};
use slint_runtime::{
    ModelRc, PlatformError, SharedString, VecModel,
    private_unstable_api::re_exports::{Menu, MenuEntry, SharedVector},
};

pub struct PlayerMenuCatalog(MenuFromItemTree);

pub struct PlayerMenuItem {
    pub id: SharedString,
    pub title: SharedString,
    pub shortcut: String,
    pub separator: bool,
    pub enabled: bool,
    pub checkable: bool,
    pub checked: bool,
    pub children: Vec<Self>,
}

impl PlayerMenuCatalog {
    pub fn new_player() -> Result<(PlayerWindow, Self), PlatformError> {
        let (component, tree) = capture_bar(PlayerWindow::new)?;
        component.set_native_video(false);
        // The open overlay keeps one generation even while the shortcut menu
        // refreshes its own entries as focus and modifiers change.
        let source = Self(MenuFromItemTree::new(tree));
        let labels = source
            .entries(None)
            .iter()
            .map(|entry| entry.title.clone())
            .collect::<Vec<_>>();
        component.set_player_menu_labels(ModelRc::new(VecModel::from(labels)));
        Ok((component, source))
    }

    pub fn items(&self) -> Vec<PlayerMenuItem> {
        self.items_under(None)
    }

    pub fn items_in_family(&self, family: usize) -> Vec<PlayerMenuItem> {
        self.entries(None)
            .get(family)
            .map_or_else(Vec::new, |entry| self.items_under(Some(entry)))
    }

    pub fn activate(&self, id: SharedString) {
        self.0.activate(&MenuEntry {
            id,
            ..Default::default()
        });
    }

    fn entries(&self, parent: Option<&MenuEntry>) -> SharedVector<MenuEntry> {
        let mut entries = SharedVector::default();
        self.0.sub_menu(parent, &mut entries);
        entries
    }

    fn items_under(&self, parent: Option<&MenuEntry>) -> Vec<PlayerMenuItem> {
        self.entries(parent)
            .iter()
            .map(|entry| PlayerMenuItem {
                id: entry.id.clone(),
                title: entry.title.clone(),
                shortcut: entry.shortcut.to_parts().collect::<Vec<_>>().join("+"),
                separator: entry.is_separator,
                enabled: entry.enabled,
                checkable: entry.checkable,
                checked: entry.checked,
                children: if entry.has_sub_menu {
                    self.items_under(Some(entry))
                } else {
                    Vec::new()
                },
            })
            .collect()
    }
}
