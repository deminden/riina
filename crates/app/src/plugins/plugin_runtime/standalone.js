(function (iina, native) {
    const listeners = new Map();
    const changedDocument = (update, ...args) => {
        update(...args);
        listeners.clear();
    };
    const numberOrNull = value => typeof value === "number" && Number.isFinite(value) ? value : null;
    iina.standaloneWindow = {
        open: native.open,
        close: native.close,
        isOpen: native.isOpen,
        loadFile: path => changedDocument(native.loadFile, path),
        simpleMode: () => changedDocument(native.simpleMode),
        setStyle: native.setStyle,
        setContent: native.setContent,
        postMessage: (name, data) => native.postMessage(name, JSON.stringify(data ?? null)),
        onMessage(name, callback) {
            if (typeof name !== "string" || typeof callback !== "function") return;
            if (listeners.size < 128 || listeners.has(name)) listeners.set(name, callback);
        },
        setProperty(props) {
            if (props === null || typeof props !== "object") return;
            const values = {};
            if (typeof props.title === "string") values.title = props.title;
            for (const name of ["resizable", "fullSizeContentView", "hideTitleBar"]) {
                if (typeof props[name] === "boolean") values[name] = props[name];
            }
            native.setProperty(JSON.stringify(values));
        },
        setFrame(width, height, x, y) {
            native.setFrame(JSON.stringify({
                width: numberOrNull(width), height: numberOrNull(height),
                x: numberOrNull(x), y: numberOrNull(y)
            }));
        }
    };
    return (name, value) => listeners.get(name)?.(value);
})
