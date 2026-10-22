//! Menu models for plugin scopes; window registration stays with the player session.
use crate::{PlayerWindow, PluginMenuRow};
use slint::Model;
use std::collections::BTreeMap;

// Slint 1.18.1 converts this private title sentinel in dynamic menu models to
// an actual MenuSeparator on every backend.
const SLINT_MENU_SEPARATOR_TITLE: &str = "\u{e001}⸺";
const PLUGIN_MENU_GROUP_SEPARATOR_TOKEN: &str = "\u{1f}group-separator";

fn plugin_menu_rows_for_plugin(
    identifier: &str,
    global: bool,
    items: &[riina_core::plugin_menu::PluginMenuItem],
) -> Vec<PluginMenuRow> {
    let scope = if global { "global" } else { "player" };
    fn append(
        identifier: &str,
        scope: &str,
        items: &[riina_core::plugin_menu::PluginMenuItem],
        ancestors_enabled: bool,
        parents: &mut Vec<String>,
        rows: &mut Vec<PluginMenuRow>,
    ) {
        for item in items {
            if item.separator {
                rows.push(PluginMenuRow {
                    token: format!("{identifier}\u{1f}{scope}\u{1f}").into(),
                    label: SLINT_MENU_SEPARATOR_TITLE.into(),
                    enabled: false,
                    selected: false,
                    shortcut: Default::default(),
                    section_header: false,
                });
                continue;
            }
            parents.push(item.title.clone());
            rows.push(PluginMenuRow {
                token: format!("{identifier}\u{1f}{scope}\u{1f}{}", item.id).into(),
                label: parents.join(" › ").into(),
                enabled: ancestors_enabled && item.enabled && item.items.is_empty(),
                selected: item.selected,
                shortcut: item
                    .key_binding
                    .as_deref()
                    .and_then(crate::key_binding_preferences::shortcut_keys)
                    .unwrap_or_default(),
                // A submenu container is navigational in IINA; only its leaves
                // represent actions. The breadcrumb fallback must not invoke it.
                section_header: !item.items.is_empty(),
            });
            append(
                identifier,
                scope,
                &item.items,
                ancestors_enabled && item.enabled,
                parents,
                rows,
            );
            parents.pop();
        }
    }
    let mut rows = Vec::new();
    append(identifier, scope, items, true, &mut Vec::new(), &mut rows);
    rows
}

pub(super) fn replace_plugin_menu_rows(
    app: &PlayerWindow,
    identifier: &str,
    plugin_name: &str,
    global: bool,
    items: &[riina_core::plugin_menu::PluginMenuItem],
) {
    let current_rows = (0..app.get_plugin_menu_rows().row_count())
        .filter_map(|index| app.get_plugin_menu_rows().row_data(index))
        .collect();
    app.set_plugin_menu_rows(slint::ModelRc::new(slint::VecModel::from(
        updated_plugin_menu_rows(current_rows, identifier, plugin_name, global, items),
    )));
    let order = (0..app.get_plugin_menu_order().row_count())
        .filter_map(|index| app.get_plugin_menu_order().row_data(index))
        .map(|identifier| identifier.to_string())
        .collect::<Vec<_>>();
    reorder_plugin_menu_rows(app, &order);
}

fn updated_plugin_menu_rows(
    current_rows: Vec<PluginMenuRow>,
    identifier: &str,
    plugin_name: &str,
    global: bool,
    items: &[riina_core::plugin_menu::PluginMenuItem],
) -> Vec<PluginMenuRow> {
    let mut groups = BTreeMap::<String, Vec<PluginMenuRow>>::new();
    for row in current_rows {
        if row.token.as_str() == PLUGIN_MENU_GROUP_SEPARATOR_TOKEN {
            continue;
        }
        if let Some((plugin, _)) = row.token.as_str().split_once('\u{1f}') {
            groups.entry(plugin.to_owned()).or_default().push(row);
        }
    }

    let mut global_rows = Vec::new();
    let mut player_rows = Vec::new();
    if let Some(old_rows) = groups.remove(identifier) {
        for row in old_rows {
            if row.token.as_str().split('\u{1f}').nth(1) == Some("header") {
                continue;
            }
            match row
                .token
                .as_str()
                .split_once('\u{1f}')
                .and_then(|(_, rest)| rest.split_once('\u{1f}'))
                .map(|(scope, _)| scope)
            {
                Some("global") if !global => global_rows.push(row),
                Some("player") if global => player_rows.push(row),
                _ => {}
            }
        }
    }
    let new_rows = plugin_menu_rows_for_plugin(identifier, global, items);
    if global {
        global_rows.extend(new_rows);
    } else {
        player_rows.extend(new_rows);
    }

    if !global_rows.is_empty() || !player_rows.is_empty() {
        let mut plugin_rows = vec![PluginMenuRow {
            token: format!("{identifier}\u{1f}header\u{1f}").into(),
            label: plugin_name.into(),
            enabled: false,
            selected: false,
            shortcut: Default::default(),
            section_header: true,
        }];
        plugin_rows.extend(global_rows);
        plugin_rows.extend(player_rows);
        groups.insert(identifier.to_owned(), plugin_rows);
    }
    flatten_plugin_menu_groups(groups)
}

pub(super) fn reorder_plugin_menu_rows(app: &PlayerWindow, order: &[String]) {
    let mut groups = BTreeMap::<String, Vec<PluginMenuRow>>::new();
    for row in (0..app.get_plugin_menu_rows().row_count())
        .filter_map(|index| app.get_plugin_menu_rows().row_data(index))
    {
        if row.token.as_str() == PLUGIN_MENU_GROUP_SEPARATOR_TOKEN {
            continue;
        }
        if let Some((identifier, _)) = row.token.as_str().split_once('\u{1f}') {
            groups.entry(identifier.to_owned()).or_default().push(row);
        }
    }
    app.set_plugin_menu_rows(slint::ModelRc::new(slint::VecModel::from(
        flatten_plugin_menu_groups_in_order(groups, order),
    )));
}

pub(super) fn remove_plugin_menu_rows(app: &PlayerWindow, identifier: &str) {
    let mut groups = BTreeMap::<String, Vec<PluginMenuRow>>::new();
    for row in (0..app.get_plugin_menu_rows().row_count())
        .filter_map(|index| app.get_plugin_menu_rows().row_data(index))
    {
        if row.token.as_str() == PLUGIN_MENU_GROUP_SEPARATOR_TOKEN {
            continue;
        }
        if let Some((plugin, _)) = row.token.as_str().split_once('\u{1f}')
            && plugin != identifier
        {
            groups.entry(plugin.to_owned()).or_default().push(row);
        }
    }
    app.set_plugin_menu_rows(slint::ModelRc::new(slint::VecModel::from(
        flatten_plugin_menu_groups(groups),
    )));
    let order = (0..app.get_plugin_menu_order().row_count())
        .filter_map(|index| app.get_plugin_menu_order().row_data(index))
        .map(|identifier| identifier.to_string())
        .collect::<Vec<_>>();
    reorder_plugin_menu_rows(app, &order);
}

fn flatten_plugin_menu_groups(groups: BTreeMap<String, Vec<PluginMenuRow>>) -> Vec<PluginMenuRow> {
    flatten_plugin_menu_groups_in_order(groups, &[])
}

fn flatten_plugin_menu_groups_in_order(
    mut groups: BTreeMap<String, Vec<PluginMenuRow>>,
    order: &[String],
) -> Vec<PluginMenuRow> {
    let mut rows = Vec::new();
    let mut fallback = groups.keys().cloned().collect::<Vec<_>>();
    fallback.sort_by(|left, right| {
        let left_title = groups
            .get(left)
            .and_then(|rows| rows.first())
            .map(|row| row.label.as_str())
            .unwrap_or(left);
        let right_title = groups
            .get(right)
            .and_then(|rows| rows.first())
            .map(|row| row.label.as_str())
            .unwrap_or(right);
        left_title.cmp(right_title).then_with(|| left.cmp(right))
    });
    let identifiers = order
        .iter()
        .filter(|identifier| groups.contains_key(identifier.as_str()))
        .cloned()
        .chain(
            fallback
                .into_iter()
                .filter(|identifier| !order.contains(identifier)),
        )
        .collect::<Vec<_>>();
    for identifier in identifiers {
        let Some(group) = groups.remove(&identifier) else {
            continue;
        };
        if group.is_empty() {
            continue;
        }
        if !rows.is_empty() {
            rows.push(PluginMenuRow {
                token: PLUGIN_MENU_GROUP_SEPARATOR_TOKEN.into(),
                label: SLINT_MENU_SEPARATOR_TITLE.into(),
                enabled: false,
                selected: false,
                shortcut: Default::default(),
                section_header: false,
            });
        }
        rows.extend(group);
    }
    rows
}

#[cfg(test)]
mod plugin_menu_row_tests {
    use super::{PluginMenuRow, flatten_plugin_menu_groups_in_order, updated_plugin_menu_rows};
    use riina_core::plugin_menu::PluginMenuItem;
    use std::collections::BTreeMap;

    fn menu_item(id: &str, title: &str) -> PluginMenuItem {
        PluginMenuItem {
            id: id.into(),
            title: title.into(),
            enabled: true,
            selected: false,
            separator: false,
            key_binding: None,
            items: Vec::new(),
        }
    }

    #[test]
    fn global_and_player_items_share_one_display_name_heading() {
        let rows = updated_plugin_menu_rows(
            Vec::<PluginMenuRow>::new(),
            "org.example.test",
            "Example Plugin",
            true,
            &[menu_item("global-action", "Global action")],
        );
        let rows = updated_plugin_menu_rows(
            rows,
            "org.example.test",
            "Example Plugin",
            false,
            &[menu_item("player-action", "Player action")],
        );

        assert_eq!(
            rows.iter()
                .map(|row| row.label.as_str())
                .collect::<Vec<_>>(),
            ["Example Plugin", "Global action", "Player action"]
        );
        assert!(rows[0].section_header);
        assert!(rows[1].token.as_str().contains("\u{1f}global\u{1f}"));
        assert!(rows[2].token.as_str().contains("\u{1f}player\u{1f}"));
    }

    #[test]
    fn replacing_one_scope_preserves_the_other_scope_and_single_heading() {
        let mut global_parent = menu_item("global-tools", "Global tools");
        global_parent
            .items
            .push(menu_item("global-child", "Run global tool"));
        let rows = updated_plugin_menu_rows(
            Vec::<PluginMenuRow>::new(),
            "org.example.test",
            "Example Plugin",
            true,
            &[menu_item("global-action", "Global action"), global_parent],
        );
        let rows = updated_plugin_menu_rows(
            rows,
            "org.example.test",
            "Example Plugin",
            false,
            &[menu_item("old-player-action", "Old player action")],
        );
        let rows = updated_plugin_menu_rows(
            rows,
            "org.example.test",
            "Example Plugin",
            false,
            &[menu_item("new-player-action", "New player action")],
        );

        assert_eq!(
            rows.iter()
                .filter(|row| row.token.as_str().split('\u{1f}').nth(1) == Some("header"))
                .count(),
            1
        );
        assert_eq!(rows.len(), 5);
        assert!(rows.iter().any(|row| row.label == "Global action"));
        assert!(
            rows.iter()
                .any(|row| row.label == "Global tools" && row.section_header)
        );
        assert!(
            rows.iter()
                .any(|row| row.label == "Global tools › Run global tool")
        );
        assert!(rows.iter().any(|row| row.label == "New player action"));
        assert!(!rows.iter().any(|row| row.label == "Old player action"));
    }

    #[test]
    fn submenu_containers_are_not_actions_but_nested_leaves_remain_routable() {
        let mut parent = menu_item("parent", "Tools");
        parent.items.push(menu_item("child", "Run tool"));
        let rows = updated_plugin_menu_rows(
            Vec::<PluginMenuRow>::new(),
            "org.example.test",
            "Example Plugin",
            false,
            &[parent],
        );

        let container = rows
            .iter()
            .find(|row| row.label == "Tools")
            .expect("submenu breadcrumb is visible");
        assert!(container.section_header);
        assert!(!container.enabled);

        let leaf = rows
            .iter()
            .find(|row| row.label == "Tools › Run tool")
            .expect("nested action is visible");
        assert!(!leaf.section_header);
        assert!(leaf.token.as_str().ends_with("\u{1f}child"));
    }

    #[test]
    fn disabled_plugin_menu_ancestors_disable_descendants_without_disabling_siblings() {
        let mut parent = menu_item("parent", "Tools");
        parent.enabled = false;
        let mut nested = menu_item("nested", "More tools");
        nested.items.push(menu_item("child", "Run tool"));
        parent.items.push(nested);
        let rows = updated_plugin_menu_rows(
            Vec::new(),
            "org.example.test",
            "Example Plugin",
            false,
            &[parent, menu_item("sibling", "Available action")],
        );

        assert!(
            !rows
                .iter()
                .find(|row| row.token.ends_with("\u{1f}child"))
                .unwrap()
                .enabled
        );
        assert!(
            rows.iter()
                .find(|row| row.token.ends_with("\u{1f}sibling"))
                .unwrap()
                .enabled
        );
    }

    #[test]
    fn plugin_menu_order_follows_persisted_order_before_identifier_fallback() {
        let row = |identifier: &str| PluginMenuRow {
            token: format!("{identifier}\u{1f}header\u{1f}").into(),
            label: identifier.into(),
            enabled: false,
            selected: false,
            shortcut: Default::default(),
            section_header: true,
        };
        let groups = BTreeMap::from([
            ("a.plugin".into(), vec![row("a.plugin")]),
            ("b.plugin".into(), vec![row("b.plugin")]),
            ("c.plugin".into(), vec![row("c.plugin")]),
        ]);
        let rows =
            flatten_plugin_menu_groups_in_order(groups, &["c.plugin".into(), "a.plugin".into()]);
        let headers = rows
            .iter()
            .filter(|row| row.section_header)
            .map(|row| row.token.split('\u{1f}').next().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(headers, ["c.plugin", "a.plugin", "b.plugin"]);
    }

    #[test]
    fn plugin_menu_default_order_matches_the_name_sorted_preferences_list() {
        let row = |identifier: &str, name: &str| PluginMenuRow {
            token: format!("{identifier}\u{1f}header\u{1f}").into(),
            label: name.into(),
            enabled: false,
            selected: false,
            shortcut: Default::default(),
            section_header: true,
        };
        let groups = BTreeMap::from([
            ("a.plugin".into(), vec![row("a.plugin", "Zulu")]),
            ("z.plugin".into(), vec![row("z.plugin", "Alpha")]),
        ]);
        let rows = flatten_plugin_menu_groups_in_order(groups, &[]);
        let headers = rows
            .iter()
            .filter(|row| row.section_header)
            .map(|row| row.token.split('\u{1f}').next().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(headers, ["z.plugin", "a.plugin"]);
    }
}
