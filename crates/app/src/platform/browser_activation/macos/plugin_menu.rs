//! AppKit's Plugin menu while no player owns the menu bar.
use super::{LaunchTarget, idle_menu::item, plugin_shortcuts::Shortcuts};
use crate::plugin_host::{GlobalHandle, GlobalMenu};
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, rc::Retained, runtime::ProtocolObject, sel,
};
use objc2_app_kit::{NSControlStateValueOn, NSMenu, NSMenuItem};
use objc2_foundation::NSString;
use riina_core::plugin_menu::PluginMenuItem;
use std::cell::{Cell, RefCell};

const VISIBLE_ITEMS: usize = 5;
const FIELD_SEPARATOR: char = '\u{1f}';

#[derive(Default)]
pub(super) struct IdlePlugins {
    shown: RefCell<Snapshot>,
    tracking: Cell<bool>,
}

#[derive(Default, PartialEq, Eq)]
struct Snapshot {
    menus: Vec<GlobalMenu>,
    consoles: Vec<(u64, String)>,
}

pub(super) fn new(main: MainThreadMarker, target: &LaunchTarget) -> Retained<NSMenu> {
    let menu = submenu(main, &target.ivars().menu_labels.plugin_title);
    build(None, &menu, target, &Snapshot::default());
    menu.setDelegate(Some(ProtocolObject::from_ref(target)));
    menu
}

impl IdlePlugins {
    pub(super) fn set_tracking(&self, tracking: bool) {
        self.tracking.set(tracking);
    }

    pub(super) fn refresh(&self, bar: &NSMenu, menu: &NSMenu, target: &LaunchTarget, force: bool) {
        // AppKit is enumerating these items while the menu is open. The next
        // menu update will read any contributions that arrived during tracking.
        if self.tracking.get() {
            return;
        }
        let next = if let Some(host) = host(target) {
            Snapshot {
                menus: host.menus(),
                consoles: host
                    .ui
                    .developer
                    .list(0)
                    .map_or_else(Vec::new, |(_, rows)| rows),
            }
        } else {
            Snapshot::default()
        };
        if force || *self.shown.borrow() != next {
            self.shown.replace(next);
            build(Some(bar), menu, target, &self.shown.borrow());
        }
    }
}

pub(super) fn activate(target: &LaunchTarget, sender: Option<&NSMenuItem>) {
    if let (Some(host), Some(token)) = (host(target), represented(sender))
        && let Some((package, rest)) = token.split_once(FIELD_SEPARATOR)
        && let Some((generation, id)) = rest.split_once(FIELD_SEPARATOR)
        && let Ok(generation) = generation.parse()
    {
        target.ivars().default_action.take();
        if let Err(error) = host.activate_idle_menu(package.into(), generation, id.into()) {
            crate::diagnostics::warn!("Could not run the plugin menu action: {error}");
        }
    }
}

pub(super) fn open_console(target: &LaunchTarget, sender: Option<&NSMenuItem>) {
    if let Some(id) = represented(sender).and_then(|token| token.parse().ok()) {
        let launch = target.ivars();
        if launch.manager.is_quitting() {
            return;
        }
        launch.default_action.take();
        let manager = launch.manager.clone();
        // A standalone tool can change the key window and its menu bar.
        let _ = slint::spawn_local(async move {
            if !manager.is_quitting()
                && let Err(error) = manager.services().plugins.open_developer_tool(id)
            {
                crate::diagnostics::warn!("Could not open the plugin console: {error}");
            }
        });
    }
}

pub(super) fn reload(target: &LaunchTarget) {
    if let Some(host) = host(target)
        && let Err(error) = host.reload_all()
    {
        crate::diagnostics::warn!("Could not reload plugins: {error}");
    }
}

fn host(target: &LaunchTarget) -> Option<GlobalHandle> {
    let manager = &target.ivars().manager;
    (!manager.is_quitting())
        .then(|| manager.services().plugins.global_handle())
        .flatten()
}

fn build(bar: Option<&NSMenu>, menu: &NSMenu, target: &LaunchTarget, snapshot: &Snapshot) {
    let main = target.mtm();
    let labels = &target.ivars().menu_labels;
    menu.removeAllItems();
    menu.addItem(&item(
        main,
        &labels.manage_plugins,
        Some(sel!(riinaIdleManagePlugins:)),
        "",
        Some(target),
    ));
    let sidebar = item(main, &labels.show_plugin_panel, None, "", None);
    sidebar.setEnabled(false);
    menu.addItem(&sidebar);
    menu.addItem(&NSMenuItem::separatorItem(main));

    let mut shortcuts = Shortcuts::new(bar.filter(|_| !snapshot.menus.is_empty()), menu);
    for group in &snapshot.menus {
        let title = item(main, &group.title, None, "", None);
        title.setEnabled(false);
        menu.addItem(&title);
        let (visible, overflow) = group.items.split_at(group.items.len().min(VISIBLE_ITEMS));
        append(menu, target, group, visible, true, &mut shortcuts);
        if !overflow.is_empty() {
            let more = submenu(main, &labels.more);
            append(&more, target, group, overflow, true, &mut shortcuts);
            add_submenu(menu, main, &more, true);
        }
        menu.addItem(&NSMenuItem::separatorItem(main));
    }

    let developer = submenu(main, &labels.developer_tool);
    for (id, title) in &snapshot.consoles {
        let row = item(
            main,
            title,
            Some(sel!(riinaIdlePluginDeveloper:)),
            "",
            Some(target),
        );
        set_token(&row, &id.to_string());
        developer.addItem(&row);
    }
    add_submenu(menu, main, &developer, !snapshot.consoles.is_empty());
    menu.addItem(&item(
        main,
        &labels.reload_plugins,
        Some(sel!(riinaIdleReloadPlugins:)),
        "",
        Some(target),
    ));
}

fn append(
    menu: &NSMenu,
    target: &LaunchTarget,
    group: &GlobalMenu,
    entries: &[PluginMenuItem],
    ancestors_enabled: bool,
    shortcuts: &mut Shortcuts,
) {
    let main = target.mtm();
    for entry in entries {
        if entry.separator {
            menu.addItem(&NSMenuItem::separatorItem(main));
            continue;
        }
        let enabled = ancestors_enabled && entry.enabled;
        let row = if entry.items.is_empty() {
            let row = item(
                main,
                &entry.title,
                Some(sel!(riinaIdlePluginAction:)),
                "",
                Some(target),
            );
            set_token(
                &row,
                &format!(
                    "{}{FIELD_SEPARATOR}{}{FIELD_SEPARATOR}{}",
                    group.package, group.generation, entry.id
                ),
            );
            if enabled && let Some(binding) = entry.key_binding.as_deref() {
                shortcuts.assign(&row, binding);
            }
            row
        } else {
            let children = submenu(main, &entry.title);
            append(&children, target, group, &entry.items, enabled, shortcuts);
            let row = item(main, &entry.title, None, "", None);
            row.setSubmenu(Some(&children));
            row
        };
        row.setEnabled(enabled);
        if entry.selected {
            row.setState(NSControlStateValueOn);
        }
        menu.addItem(&row);
    }
}

fn submenu(main: MainThreadMarker, title: &str) -> Retained<NSMenu> {
    let menu = NSMenu::initWithTitle(NSMenu::alloc(main), &NSString::from_str(title));
    menu.setAutoenablesItems(false);
    menu
}

fn add_submenu(parent: &NSMenu, main: MainThreadMarker, menu: &NSMenu, enabled: bool) {
    let row = item(main, &menu.title().to_string(), None, "", None);
    row.setSubmenu(Some(menu));
    row.setEnabled(enabled);
    parent.addItem(&row);
}

fn set_token(item: &NSMenuItem, token: &str) {
    // SAFETY: the immutable NSString is retained by AppKit and read by our selector.
    unsafe { item.setRepresentedObject(Some(&NSString::from_str(token))) };
}

fn represented(sender: Option<&NSMenuItem>) -> Option<String> {
    sender?
        .representedObject()?
        .downcast::<NSString>()
        .ok()
        .map(|token| token.to_string())
}
