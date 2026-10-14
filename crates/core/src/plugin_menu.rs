//! Bounded, serializable snapshots for plugin-contributed application menus.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MAX_PLUGIN_MENU_ITEMS: usize = 128;
pub const MAX_PLUGIN_MENU_DEPTH: usize = 8;
const MAX_TITLE_BYTES: usize = 256;
const MAX_SHORTCUT_BYTES: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginMenuItem {
    pub id: String,
    pub title: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub selected: bool,
    #[serde(default)]
    pub separator: bool,
    #[serde(default)]
    pub key_binding: Option<String>,
    #[serde(default)]
    pub items: Vec<PluginMenuItem>,
}

fn default_enabled() -> bool {
    true
}

impl PluginMenuItem {
    pub fn validate_all(items: &[Self]) -> Result<(), String> {
        let mut identifiers = BTreeSet::new();
        let mut count = 0usize;
        for item in items {
            validate_item(item, 1, &mut count, &mut identifiers)?;
        }
        Ok(())
    }
}

fn validate_item(
    item: &PluginMenuItem,
    depth: usize,
    count: &mut usize,
    identifiers: &mut BTreeSet<String>,
) -> Result<(), String> {
    *count = count.saturating_add(1);
    if *count > MAX_PLUGIN_MENU_ITEMS {
        return Err("Plugin menu exceeds the 128 item limit".into());
    }
    if depth > MAX_PLUGIN_MENU_DEPTH {
        return Err("Plugin menu nesting exceeds the 8 level limit".into());
    }
    if item.id.is_empty()
        || item.id.len() > 64
        || !item
            .id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        || !identifiers.insert(item.id.clone())
    {
        return Err("Plugin menu item identifier is invalid or duplicated".into());
    }
    if item.title.len() > MAX_TITLE_BYTES || item.title.chars().any(char::is_control) {
        return Err("Plugin menu title exceeds 256 bytes or contains control characters".into());
    }
    if item
        .key_binding
        .as_ref()
        .is_some_and(|key| key.len() > MAX_SHORTCUT_BYTES || key.chars().any(char::is_control))
    {
        return Err(
            "Plugin menu key binding exceeds 64 bytes or contains control characters".into(),
        );
    }
    if item.separator && (!item.items.is_empty() || !item.title.is_empty()) {
        return Err("Plugin menu separators cannot have a title or submenu".into());
    }
    for child in &item.items {
        validate_item(child, depth + 1, count, identifiers)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str) -> PluginMenuItem {
        PluginMenuItem {
            id: id.into(),
            title: "Action".into(),
            enabled: true,
            selected: false,
            separator: false,
            key_binding: None,
            items: Vec::new(),
        }
    }

    #[test]
    fn validates_nested_menu_items_and_rejects_duplicate_ids() {
        let mut parent = item("parent");
        parent.items.push(item("child"));
        assert!(PluginMenuItem::validate_all(&[parent.clone()]).is_ok());
        parent.items[0].id = "parent".into();
        assert!(PluginMenuItem::validate_all(&[parent]).is_err());
    }

    #[test]
    fn bounds_menu_size_depth_titles_shortcuts_and_separator_shape() {
        let too_many = (0..=MAX_PLUGIN_MENU_ITEMS)
            .map(|index| item(&format!("item-{index}")))
            .collect::<Vec<_>>();
        assert!(PluginMenuItem::validate_all(&too_many).is_err());

        let mut too_deep = item("root");
        let mut current = &mut too_deep;
        for depth in 1..MAX_PLUGIN_MENU_DEPTH {
            current.items.push(item(&format!("depth-{depth}")));
            current = current.items.last_mut().unwrap();
        }
        current.items.push(item("too-deep"));
        assert!(PluginMenuItem::validate_all(&[too_deep]).is_err());

        let mut invalid = item("invalid");
        invalid.title = "x".repeat(MAX_TITLE_BYTES + 1);
        assert!(PluginMenuItem::validate_all(&[invalid]).is_err());
        let mut separator = item("separator");
        separator.separator = true;
        assert!(PluginMenuItem::validate_all(&[separator]).is_err());
    }
}
