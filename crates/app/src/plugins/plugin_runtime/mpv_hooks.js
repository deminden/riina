(function (mpv, register, log) {
    const callbacks = new Map();
    const AsyncFunction = (async function () {}).constructor;

    mpv.addHook = function (name, priority, callback) {
        if (typeof callback !== "function") {
            throw new TypeError("mpv.addHook requires a callback");
        }
        const async = callback.constructor === AsyncFunction;
        const index = register(name, priority);
        callbacks.set(index, { callback, async });
    };

    return function deliver(index, next) {
        const hook = callbacks.get(index);
        if (hook === undefined) {
            next();
            return;
        }
        try {
            const result = hook.callback.call(undefined, next);
            if (hook.async) {
                result.catch((error) => {
                    next();
                    log(`Plugin mpv hook failed: ${error}`);
                });
            } else {
                next();
            }
        } catch (error) {
            next();
            throw error;
        }
    };
})
