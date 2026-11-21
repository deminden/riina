iina.sidebar = __riinaSidebar;
    (() => {
        const sidebar = iina.sidebar;
        const postMessage = sidebar.postMessage;
        const loadFile = sidebar.loadFile;
        const onMessage = sidebar.onMessage;
        const listeners = new Map();
        sidebar.postMessage = (name, data) => postMessage(name, JSON.stringify(data));
        sidebar.loadFile = path => { const result = loadFile(path); listeners.clear(); return result; };
        sidebar.onMessage = (name, callback) => {
            if (typeof callback !== 'function') throw new TypeError('callback must be a function');
            onMessage(name, callback);
            const callbacks = listeners.get(name) || [];
            callbacks.push(callback);
            listeners.set(name, callbacks);
        };
        sidebar.__deliverMessage = (name, data) => {
            for (const callback of listeners.get(name) || []) callback(data);
        };
    })();