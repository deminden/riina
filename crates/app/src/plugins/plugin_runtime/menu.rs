//! Plugin-owned menu objects and action dispatch.

use riina_core::plugin_menu::PluginMenuItem;
use rquickjs::Context;

pub(super) const PLUGIN_MENU_API: &str = r#"(() => {
    let nextId = 0;
    const roots = [];
    const menuItems = new WeakSet();
    let publishedItems = new WeakSet();
    let playlistBuilder = null;
    let playlistMenu = null;
    const snapshot = (item, reached) => ({
        id: item.id,
        title: item.title,
        enabled: item.enabled,
        selected: item.selected,
        separator: item.separator,
        keyBinding: item.keyBinding,
        items: item.items.map(child => {
            if (!menuItems.has(child)) throw new TypeError('expected a plugin menu item');
            reached.add(child);
            return snapshot(child, reached);
        })
    });
    const serialize = (items, reached = new WeakSet()) => JSON.stringify(items.map(item => {
        if (!menuItems.has(item)) throw new TypeError('expected a plugin menu item');
        reached.add(item);
        return snapshot(item, reached);
    }));
    const publish = () => {
        const reached = new WeakSet();
        iina.__menuUpdate(serialize(roots, reached));
        publishedItems = reached;
    };
    const changed = item => {
        if (publishedItems.has(item)) publish();
    };
    if (iina.playlist) {
        iina.playlist.registerMenuItemBuilder = builder => {
            if (typeof builder !== 'function') throw new TypeError('menu builder must be a function');
            playlistBuilder = builder;
            playlistMenu = null;
        };
    }
    const activate = (items, id) => {
        const find = branch => {
            for (const item of branch) {
                if (!item.enabled || item.separator) continue;
                if (item.id === id) return item;
                const child = find(item.items);
                if (child) return child;
            }
            return null;
        };
        const item = find(items);
        if (!item || !item.__action || item.items.length !== 0) return false;
        try { (0, item.__action)(item); return true; }
        catch (error) { iina.log(`Plugin menu action threw: ${String(error)}`); return false; }
    };
    const makeItem = (title, action, options, separator = false) => {
        if (action != null && typeof action !== 'function') throw new TypeError('action must be a function or null');
        const item = {
            _title: separator ? '' : String(title),
            _enabled: !separator && options?.enabled !== false,
            _selected: !separator && options?.selected === true,
            _keyBinding: options?.keyBinding == null ? null : String(options.keyBinding),
            _items: [],
            addSubMenuItem(child) {
                if (!menuItems.has(child) || this.separator) {
                    throw new TypeError('child must be a plugin menu item');
                }
                this._items.push(child);
                changed(this);
                return this;
            }
        };
        Object.defineProperties(item, {
            id: { value: `menu-${++nextId}`, writable: false, enumerable: false },
            separator: { value: separator, writable: false, enumerable: false },
            __action: { value: action, writable: false, enumerable: false },
            items: { get() { return this._items; }, enumerable: true },
            title: { get() { return this._title; }, set(value) { this._title = String(value); changed(this); } },
            enabled: { get() { return this._enabled; }, set(value) { this._enabled = Boolean(value); changed(this); } },
            selected: { get() { return this._selected; }, set(value) { this._selected = Boolean(value); changed(this); } },
            keyBinding: { get() { return this._keyBinding; }, set(value) { this._keyBinding = value == null ? null : String(value); changed(this); } }
        });
        menuItems.add(item);
        return item;
    };
    iina.menu = {
        item(title, action, options) { return makeItem(title, action, options); },
        separator() { return makeItem('', null, null, true); },
        addItem(item) {
            if (!menuItems.has(item)) throw new TypeError('item must be a plugin menu item');
            roots.push(item);
            publish();
        },
        items() { return roots.slice(); },
        removeAt(index) {
            if (!Number.isInteger(index) || index < 0 || index >= roots.length) return false;
            roots.splice(index, 1);
            publish();
            return true;
        },
        removeAllItems() {
            roots.length = 0;
            publish();
        },
        forceUpdate: publish,
        __activate(id) { return activate(roots, id); },
        __buildPlaylistMenu(request, indices) {
            playlistMenu = null;
            if (!playlistBuilder) return '[]';
            const items = playlistBuilder(indices);
            if (!Array.isArray(items)) throw new TypeError('menu builder must return an array');
            const json = serialize(items);
            playlistMenu = { request, items: items.slice() };
            return json;
        },
        __activatePlaylistItem(request, id) {
            if (playlistMenu?.request !== request) return false;
            const { items } = playlistMenu;
            playlistMenu = null;
            return activate(items, id);
        }
    };
})();"#;

pub(super) fn build_playlist_menu(
    context: &Context,
    request: u64,
    indices: &[usize],
) -> Result<Vec<PluginMenuItem>, String> {
    let script = format!(
        "iina.menu.__buildPlaylistMenu(\"{request}\", {})",
        serde_json::json!(indices)
    );
    let json = context.with(|ctx| {
        ctx.eval::<String, _>(script).map_err(|error| {
            if matches!(error, rquickjs::Error::Exception) {
                ctx.catch();
            }
            error.to_string()
        })
    })?;
    let items: Vec<PluginMenuItem> =
        serde_json::from_str(&json).map_err(|error| error.to_string())?;
    PluginMenuItem::validate_all(&items).map_err(|error| error.to_string())?;
    Ok(items)
}

#[cfg(test)]
mod tests {
    use crate::plugin_runtime::{HostEvent, PluginRuntimeSession, UiEffect};
    use std::time::Duration;

    #[test]
    fn plugin_menu_actions_respect_live_ancestors_and_submenu_containers() {
        let session = PluginRuntimeSession::start_entry(
            r#"
            const parent = iina.menu.item('Tools', () => iina.log('parent'), {enabled: false});
            const nested = iina.menu.item('Nested', null);
            const child = iina.menu.item('Run', () => iina.log('child'));
            parent.addSubMenuItem(nested);
            nested.addSubMenuItem(child);
            iina.menu.addItem(parent);
            iina.menu.addItem(iina.menu.item('Available', () => iina.log('sibling')));
            iina.event.on('iina.menu-update', () => { parent.enabled = true; });
            "#,
            "org.example.menu",
            &[],
            &[],
        )
        .unwrap();
        session.drain_events().unwrap();
        session.deliver_plugin_menu_action("menu-3".into()).unwrap();
        session.deliver_plugin_menu_action("menu-4".into()).unwrap();
        session.flush_events(Duration::from_secs(1)).unwrap();
        let logs = |events: Vec<HostEvent>| {
            events
                .into_iter()
                .filter_map(|event| {
                    if let HostEvent::Ui(UiEffect::Log(message)) = event {
                        Some(message)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(logs(session.drain_events().unwrap()), ["sibling"]);

        session
            .deliver_player_event("iina.menu-update".into(), serde_json::Value::Null)
            .unwrap();
        session.deliver_plugin_menu_action("menu-1".into()).unwrap();
        session.deliver_plugin_menu_action("menu-3".into()).unwrap();
        session.flush_events(Duration::from_secs(1)).unwrap();
        assert_eq!(logs(session.drain_events().unwrap()), ["child"]);
    }
}
