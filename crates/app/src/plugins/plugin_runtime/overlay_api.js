iina.overlay = __riinaOverlay;
    (() => {
        const overlay = iina.overlay;
        const postMessage = overlay.postMessage;
        const loadFile = overlay.loadFile;
        const simpleMode = overlay.simpleMode;
        const onMessage = overlay.onMessage;
        const listeners = new Map();
        overlay.postMessage = (name, data) => postMessage(name, JSON.stringify(data));
        overlay.loadFile = path => { const result = loadFile(path); listeners.clear(); return result; };
        overlay.simpleMode = () => { const result = simpleMode(); listeners.clear(); return result; };
        overlay.onMessage = (name, callback) => {
            if (typeof callback !== 'function') throw new TypeError('callback must be a function');
            onMessage(name, callback);
            const callbacks = listeners.get(name) || [];
            callbacks.push(callback);
            listeners.set(name, callbacks);
        };
        overlay.__deliverMessage = (name, data) => {
            for (const callback of listeners.get(name) || []) callback(data);
        };
    })();