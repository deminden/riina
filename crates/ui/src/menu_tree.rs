//! Supply dynamic Plugin submenus to Slint's native and in-window menu presenters.
use slint_runtime::private_unstable_api::re_exports::{
    ItemTreeRc, Menu, MenuEntry, MenuFromItemTree as SlintMenu, MenuVTable, MenuVTable_static,
    Property, SharedString, SharedVector, vtable,
};
use std::{cell::RefCell, iter::Peekable, pin::Pin};

const PLUGIN_ROOT: &str = "\u{1f}riina-plugin-menu";
const ROW_PREFIX: &str = "\u{1f}riina-plugin-row:";
const TOP_LEVEL_LIMIT: usize = 5;

pub(crate) struct MenuFromItemTree {
    source: SlintMenu,
    plugin: RefCell<PluginMenu>,
    refresh: Pin<Box<Property<()>>>,
}

impl MenuFromItemTree {
    pub(crate) fn new(item_tree: ItemTreeRc) -> Self {
        Self::wrap(SlintMenu::new(item_tree))
    }

    pub(crate) fn new_with_condition_and_visible(
        item_tree: ItemTreeRc,
        condition: impl Fn() -> bool + 'static,
        visible: impl Fn() -> bool + 'static,
    ) -> Self {
        Self::wrap(SlintMenu::new_with_condition_and_visible(
            item_tree, condition, visible,
        ))
    }

    fn wrap(source: SlintMenu) -> Self {
        Self {
            source,
            plugin: RefCell::default(),
            refresh: Box::pin(Property::default()),
        }
    }

    fn update_plugin(&self, root: &MenuEntry) {
        let mut plugin = self.plugin.borrow_mut();
        if plugin.root.as_ref() == Some(&root.id) {
            return;
        }
        let mut rows = SharedVector::default();
        self.source.sub_menu(Some(root), &mut rows);
        *plugin = PluginMenu::new(root.id.clone(), rows);
    }
}

impl Menu for MenuFromItemTree {
    fn sub_menu(&self, parent: Option<&MenuEntry>, result: &mut SharedVector<MenuEntry>) {
        self.refresh.as_ref().get();
        let mut roots = SharedVector::default();
        self.source.sub_menu(None, &mut roots);
        let root_index = roots.iter().position(|entry| entry.title == PLUGIN_ROOT);
        if let Some(root) = root_index.and_then(|index| roots.get(index)) {
            self.update_plugin(root);
        } else {
            self.plugin.replace(PluginMenu::default());
        }
        *result = match parent {
            None => {
                if let Some(index) = root_index {
                    roots.make_mut_slice()[index].title = "Plugin".into();
                }
                roots
            }
            Some(parent) => self
                .plugin
                .borrow()
                .children
                .get(&parent.id)
                .cloned()
                .unwrap_or_else(|| {
                    let mut entries = SharedVector::default();
                    self.source.sub_menu(Some(parent), &mut entries);
                    entries
                }),
        };
    }

    fn activate(&self, entry: &MenuEntry) {
        let plugin = self.plugin.borrow();
        if plugin.children.contains_key(&entry.id) {
            return;
        }
        let restore_check = plugin.checked.contains(&entry.id);
        drop(plugin);
        self.source.activate(entry);
        if restore_check {
            // Muda toggles checked menu items itself. Restore the plugin's own state.
            self.refresh.mark_dirty();
        }
    }

    fn visible(&self) -> bool {
        self.source.visible()
    }
}

MenuVTable_static!(static ADAPTED_MENU for MenuFromItemTree);

#[derive(Default)]
struct PluginMenu {
    root: Option<SharedString>,
    children: std::collections::BTreeMap<SharedString, SharedVector<MenuEntry>>,
    checked: std::collections::BTreeSet<SharedString>,
}

impl PluginMenu {
    fn new(root: SharedString, rows: SharedVector<MenuEntry>) -> Self {
        let mut menu = Self {
            root: Some(root.clone()),
            ..Self::default()
        };
        let entries = menu.level(&mut with_overflow(rows).into_iter().peekable(), 0);
        menu.children.insert(root, entries);
        menu
    }

    fn level(
        &mut self,
        rows: &mut Peekable<impl Iterator<Item = MenuEntry>>,
        depth: u8,
    ) -> SharedVector<MenuEntry> {
        let mut entries = SharedVector::<MenuEntry>::default();
        while let Some(mut entry) = rows.next_if(|entry| row_depth(entry) == depth) {
            if let Some((_, kind, label)) = decode_title(&entry.title) {
                let label = SharedString::from(label);
                entry.is_separator = kind == "s";
                entry.checkable = entry.checked && kind == "a";
                if entry.checkable {
                    self.checked.insert(entry.id.clone());
                }
                if kind == "b" {
                    let children = self.level(rows, depth + 1);
                    entry.has_sub_menu = true;
                    self.children.insert(entry.id.clone(), children);
                }
                entry.title = label;
            }
            if !entry.is_separator
                || entries
                    .last()
                    .is_some_and(|previous| !previous.is_separator)
            {
                entries.push(entry);
            }
        }
        if entries.last().is_some_and(|entry| entry.is_separator) {
            entries.pop();
        }
        entries
    }
}

fn row_depth(entry: &MenuEntry) -> u8 {
    decode_title(&entry.title).map_or(0, |(depth, _, _)| depth)
}

fn decode_title(title: &str) -> Option<(u8, &str, &str)> {
    let (depth, payload) = title.strip_prefix(ROW_PREFIX)?.split_once(':')?;
    let (kind, label) = payload.split_once(':')?;
    Some((depth.parse().ok()?, kind, label))
}

fn with_overflow(rows: SharedVector<MenuEntry>) -> Vec<MenuEntry> {
    let mut result = Vec::with_capacity(rows.len());
    let mut count = 0;
    for mut row in rows {
        match decode_title(&row.title) {
            Some((0, "h", _)) | None => count = 0,
            Some((depth, kind, title)) => {
                if depth == 0 {
                    count += 1;
                    if count == TOP_LEVEL_LIMIT + 1 {
                        result.push(MenuEntry {
                            id: format!("more:{}", row.id).into(),
                            title: format!("{ROW_PREFIX}0:b:More…").into(),
                            enabled: true,
                            ..Default::default()
                        });
                    }
                }
                if count > TOP_LEVEL_LIMIT {
                    row.title = format!("{ROW_PREFIX}{}:{kind}:{title}", depth + 1).into();
                }
            }
        }
        result.push(row);
    }
    result
}
