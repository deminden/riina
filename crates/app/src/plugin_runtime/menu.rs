//! Plugin-owned menu objects and action dispatch.

pub(super) const PLUGIN_MENU_API: &str = r#"(() => {
    let nextId = 0;
    const actions = new Map();
    const roots = [];
    const publish = () => iina.__menuUpdate(JSON.stringify(roots.map(snapshot)));
    const snapshot = item => ({
        id: item.id,
        title: item.title,
        enabled: item.enabled,
        selected: item.selected,
        separator: item.separator,
        keyBinding: item.keyBinding,
        items: item.items.map(snapshot)
    });
    const dispose = item => {
        actions.delete(item.id);
        for (const child of item.items) dispose(child);
    };
    const register = item => {
        if (item.__action) actions.set(item.id, item.__action);
        for (const child of item.items) register(child);
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
                if (!child || typeof child.id !== 'string' || child.separator && child.items.length) {
                    throw new TypeError('child must be a plugin menu item');
                }
                this._items.push(child);
                register(child);
                publish();
                return this;
            }
        };
        Object.defineProperties(item, {
            id: { value: `menu-${++nextId}`, writable: false, enumerable: false },
            separator: { value: separator, writable: false, enumerable: false },
            __action: { value: action, writable: false, enumerable: false },
            items: { get() { return this._items; }, enumerable: true },
            title: { get() { return this._title; }, set(value) { this._title = String(value); publish(); } },
            enabled: { get() { return this._enabled; }, set(value) { this._enabled = Boolean(value); publish(); } },
            selected: { get() { return this._selected; }, set(value) { this._selected = Boolean(value); publish(); } },
            keyBinding: { get() { return this._keyBinding; }, set(value) { this._keyBinding = value == null ? null : String(value); publish(); } }
        });
        if (action) actions.set(item.id, action);
        return item;
    };
    iina.menu = {
        item(title, action, options) { return makeItem(title, action, options); },
        separator() { return makeItem('', null, null, true); },
        addItem(item) {
            if (!item || typeof item.id !== 'string') throw new TypeError('item must be a plugin menu item');
            roots.push(item);
            register(item);
            publish();
        },
        items() { return roots.slice(); },
        removeAt(index) {
            if (!Number.isInteger(index) || index < 0 || index >= roots.length) return false;
            dispose(roots[index]);
            roots.splice(index, 1);
            publish();
            return true;
        },
        removeAllItems() {
            for (const item of roots) dispose(item);
            roots.length = 0;
            publish();
        },
        forceUpdate: publish,
        __activate(id) {
            const action = actions.get(id);
            if (!action) return false;
            const find = items => {
                for (const item of items) {
                    if (!item.enabled || item.separator) continue;
                    if (item.id === id) return item;
                    const child = find(item.items);
                    if (child) return child;
                }
                return null;
            };
            const item = find(roots);
            if (!item || item.items.length !== 0) return false;
            try { action(item); return true; }
            catch (error) { iina.log(`Plugin menu action threw: ${String(error)}`); return false; }
        }
    };
})();"#;

#[cfg(test)]
mod tests {
    use crate::plugin_runtime::{HostEvent, PluginRuntimeSession};
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
                    if let HostEvent::Log(message) = event {
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
