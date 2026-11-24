//! Assign plugin equivalents without shadowing the native menu or input profile.
use crate::key_binding_preferences::{mappings_for, plugin_key_binding};
use objc2_app_kit::{self as kit, NSEventModifierFlags as Flags, NSMenu, NSMenuItem};
use objc2_foundation::NSString;
use std::collections::HashSet;

pub(super) struct Shortcuts {
    profile: HashSet<String>,
    used: HashSet<(String, usize)>,
}

impl Shortcuts {
    pub(super) fn new(bar: Option<&NSMenu>, plugins: &NSMenu) -> Self {
        let mut used = HashSet::new();
        let Some(bar) = bar else {
            return Self {
                profile: HashSet::new(),
                used,
            };
        };
        let settings = crate::platform::preferences_path()
            .ok()
            .and_then(|path| crate::preference_load::read_valid(&path).ok().flatten())
            .unwrap_or_default();
        let profile = mappings_for(&settings.key_bindings.current_profile)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|mapping| plugin_key_binding(&mapping.key))
            .collect();
        collect(bar, plugins, &mut used);
        Self { profile, used }
    }

    pub(super) fn assign(&mut self, item: &NSMenuItem, binding: &str) {
        let Some(binding) = plugin_key_binding(binding) else {
            return;
        };
        if !self.profile.contains(&binding)
            && let Some((key, flags)) = equivalent(&binding)
            && self.used.insert((key.clone(), flags.bits()))
        {
            item.setKeyEquivalent(&NSString::from_str(&key));
            item.setKeyEquivalentModifierMask(flags);
        }
    }
}

fn collect(menu: &NSMenu, plugins: &NSMenu, keys: &mut HashSet<(String, usize)>) {
    if std::ptr::eq(menu, plugins) {
        return;
    }
    for item in menu.itemArray().iter() {
        if item.isHidden() {
            continue;
        }
        let key = item.keyEquivalent().to_string();
        if !key.is_empty() {
            keys.insert((key, item.keyEquivalentModifierMask().bits()));
        }
        if let Some(child) = item.submenu() {
            collect(&child, plugins, keys);
        }
    }
}

fn equivalent(binding: &str) -> Option<(String, Flags)> {
    let mut parts = binding.split('+');
    let key = parts.next_back()?;
    let mut flags = Flags::empty();
    for part in parts {
        flags |= match part {
            "Ctrl" => Flags::Control,
            "Alt" => Flags::Option,
            "Shift" => Flags::Shift,
            "Meta" => Flags::Command,
            _ => return None,
        };
    }
    let key = if key.len() == 1 && key.as_bytes()[0].is_ascii_uppercase() {
        flags |= Flags::Shift;
        key.to_ascii_lowercase()
    } else {
        native_key(key)?.to_string()
    };
    Some((key, flags))
}

fn native_key(key: &str) -> Option<char> {
    let code = match key {
        "SPACE" => return Some(' '),
        "PLUS" => return Some('+'),
        "SHARP" => return Some('#'),
        "ENTER" => return Some('\r'),
        "TAB" => return Some('\t'),
        "ESC" => return Some('\u{1b}'),
        "BS" => return Some('\u{8}'),
        "UP" => kit::NSUpArrowFunctionKey,
        "DOWN" => kit::NSDownArrowFunctionKey,
        "LEFT" => kit::NSLeftArrowFunctionKey,
        "RIGHT" => kit::NSRightArrowFunctionKey,
        "HOME" => kit::NSHomeFunctionKey,
        "END" => kit::NSEndFunctionKey,
        "PGUP" => kit::NSPageUpFunctionKey,
        "PGDWN" => kit::NSPageDownFunctionKey,
        "DEL" => kit::NSDeleteFunctionKey,
        "INS" => kit::NSInsertFunctionKey,
        key if key.chars().count() == 1 => return key.chars().next(),
        key => {
            let function: u32 = key.strip_prefix('F')?.parse().ok()?;
            if !(1..=24).contains(&function) {
                return None;
            }
            kit::NSF1FunctionKey + function - 1
        }
    };
    char::from_u32(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_plugin_equivalents_preserve_modifiers_and_special_keys() {
        assert_eq!(
            equivalent("Meta+Q"),
            Some(("q".into(), Flags::Command | Flags::Shift))
        );
        assert_eq!(equivalent("Alt+PLUS"), Some(("+".into(), Flags::Option)));
        assert_eq!(
            equivalent("Ctrl+LEFT"),
            Some(("\u{f702}".into(), Flags::Control))
        );
        assert_eq!(equivalent("F24"), Some(("\u{f71b}".into(), Flags::empty())));
        assert_eq!(equivalent("F25"), None);
        assert_eq!(equivalent("Alt+MOUSE_BTN0"), None);
    }
}
