(function (iina, native, high) {
    const listeners = [new Map(), new Map(), new Map(), new Map(), new Map()];
    const low = 100;
    const register = (kind) => (key, callback, priority = low) => {
        if (callback != null && typeof callback !== "function") {
            throw new TypeError("An input listener must be a function");
        }
        const code = native.register(kind, key, typeof priority === "number" ? priority | 0 : low, callback != null);
        if (callback == null) listeners[kind].delete(code);
        else listeners[kind].set(code, callback);
    };
    iina.input = {
        PRIORITY_LOW: low,
        PRIORITY_HIGH: high,
        MOUSE: "*mouse",
        RIGHT_MOUSE: "*rightMouse",
        OTHER_MOUSE: "*otherMouse",
        normalizeKeyCode: native.normalize,
        getAllKeyBindings: native.bindings,
        onKeyDown: register(0),
        onKeyUp: register(1),
        onMouseDown: register(2),
        onMouseUp: register(3),
        onMouseDrag: register(4)
    };
    return (kind, code, event) => Boolean(listeners[kind].get(code)?.(event));
})
