//! AppKit projection of the shared playback menu catalog when there is no player.
use super::{LaunchTarget, idle_menu};
use objc2::{MainThreadMarker, MainThreadOnly, rc::Retained, runtime::AnyObject, sel};
use objc2_app_kit::{NSEventModifierFlags as Flags, NSMenu, NSMenuItem};
use objc2_foundation::NSString;
use riina_ui::{PlaybackMenuCatalog, PlaybackMenuRow};
use slint::{Keys, ModelRc, VecModel, platform::Key};
use std::cell::RefCell;

const SCREENSHOT_FOLDER: &str = "Go to Screenshot Folder";

pub(super) struct Menus {
    sections: [Retained<NSMenu>; 4],
    catalog: PlaybackMenuCatalog,
    rows: RefCell<Vec<PlaybackMenuRow>>,
}

impl Menus {
    pub(super) fn new(main: MainThreadMarker) -> Result<Self, slint::PlatformError> {
        let catalog = PlaybackMenuCatalog::new()?;
        Ok(Self {
            sections: PlaybackMenuCatalog::FAMILIES.map(|name| submenu(main, name)),
            catalog,
            rows: RefCell::default(),
        })
    }

    pub(super) fn sections(&self) -> &[Retained<NSMenu>] {
        &self.sections
    }

    pub(super) fn refresh(&self, target: &LaunchTarget, profile: &riina_core::Preferences) {
        self.catalog
            .set_options(super::super::playback_menu_options::from_profile(profile));
        let shortcuts = crate::key_binding_preferences::profile_menu_shortcuts(
            &profile.key_bindings.current_profile,
        )
        .unwrap_or_default();
        let rows = self
            .catalog
            .snapshot(ModelRc::new(VecModel::from(shortcuts)));
        if *self.rows.borrow() == rows {
            return;
        }
        for (menu, name) in self.sections.iter().zip(PlaybackMenuCatalog::FAMILIES) {
            menu.removeAllItems();
            if let Some(root) = rows.iter().find(|root| root.title == name) {
                append(menu, target, &root.children);
            }
        }
        self.rows.replace(rows);
    }
}

fn append(menu: &NSMenu, target: &LaunchTarget, rows: &[PlaybackMenuRow]) {
    let main = target.mtm();
    for row in rows {
        if row.separator {
            menu.addItem(&NSMenuItem::separatorItem(main));
            continue;
        }
        let action = match row.title.as_str() {
            SCREENSHOT_FOLDER => Some(sel!(riinaIdleScreenshotFolder:)),
            "Video Filters…" => Some(sel!(riinaIdleVideoFilters:)),
            "Audio Filters…" => Some(sel!(riinaIdleAudioFilters:)),
            _ => None,
        };
        let (key, modifiers) = equivalent(&row.shortcut);
        let action_target: Option<&AnyObject> = if action.is_some() { Some(target) } else { None };
        let item = idle_menu::item(main, &row.title, action, &key, action_target);
        item.setKeyEquivalentModifierMask(modifiers);
        if !row.children.is_empty() {
            let children = submenu(main, &row.title);
            append(&children, target, &row.children);
            item.setSubmenu(Some(&children));
        }
        item.setEnabled(action.is_some() || !row.children.is_empty());
        menu.addItem(&item);
    }
}

fn submenu(main: MainThreadMarker, title: &str) -> Retained<NSMenu> {
    let menu = NSMenu::initWithTitle(NSMenu::alloc(main), &NSString::from_str(title));
    menu.setAutoenablesItems(false);
    menu
}

fn equivalent(shortcut: &Keys) -> (String, Flags) {
    let mut modifiers = Flags::empty();
    let mut key = String::new();
    for part in shortcut.to_parts() {
        match part {
            "Meta" => modifiers |= Flags::Command,
            "Control" => modifiers |= Flags::Control,
            "Alt" => modifiers |= Flags::Option,
            "Shift" => modifiers |= Flags::Shift,
            "Shift?" | "Alt?" => {}
            value => {
                let replacement = [
                    (Key::Backspace, '\u{8}'),
                    (Key::Return, '\r'),
                    (Key::Tab, '\t'),
                    (Key::Escape, '\u{1b}'),
                    (Key::Delete, '\u{f728}'),
                ]
                .into_iter()
                .find_map(|(slint, appkit)| {
                    value
                        .chars()
                        .eq(std::iter::once(char::from(slint)))
                        .then_some(appkit)
                });
                if let Some(value) = replacement {
                    key.push(value);
                } else {
                    key.push_str(value);
                }
            }
        }
    }
    (key, modifiers)
}
