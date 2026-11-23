(function (iina, native) {
    const handlers = { server: null, connected: null, connection: null, message: null };
    const pending = new Map();
    let nextRequest = 0;
    let created = false;

    const setHandler = (key, callback) => {
        if (handlers[key] !== null || callback == null) {
            handlers[key] = null;
        } else if (typeof callback === "function") {
            handlers[key] = callback;
        } else {
            throw new TypeError("WebSocket callback must be a function");
        }
    };

    const rejectPending = (error) => {
        for (const operation of pending.values()) operation.reject(error);
        pending.clear();
    };

    iina.ws = {
        createServer(options) {
            const port = options?.port;
            const valid = Number.isInteger(port) && port >= 0 && port <= 65535;
            const replaced = native.create(valid ? port : null);
            created = valid;
            if (replaced) {
                rejectPending("WebSocket server was replaced");
                Promise.resolve().then(() => handlers.server?.("cancelled"));
            }
            if (!valid) throw new RangeError("ws.createServer requires a port from 0 to 65535");
        },
        startServer() { native.start(); },
        onStateUpdate(callback) { setHandler("server", callback); },
        onNewConnection(callback) { setHandler("connected", callback); },
        onConnectionStateUpdate(callback) { setHandler("connection", callback); },
        onMessage(callback) { setHandler("message", callback); },
        sendText(connection, text) {
            return new Promise((resolve, reject) => {
                if (!created) {
                    reject("server does not exist");
                    return;
                }
                if (pending.size >= 32) throw new Error("WebSocket send limit reached");
                do { nextRequest = (nextRequest + 1) >>> 0; } while (pending.has(nextRequest));
                const immediate = native.send(nextRequest, String(connection), String(text));
                if (immediate != null) {
                    resolve(immediate);
                } else {
                    pending.set(nextRequest, { resolve, reject });
                }
            });
        }
    };

    return {
        server(...args) {
            if (args[0] === "failed") rejectPending(args[1]);
            handlers.server?.(...args);
        },
        connected(...args) { handlers.connected?.(...args); },
        connection(...args) { handlers.connection?.(...args); },
        message(id, bytes) {
            handlers.message?.(id, {
                text: () => native.text(bytes),
                data: () => new Uint8Array(bytes.slice(0))
            });
        },
        sent(id, succeeded, value) {
            const operation = pending.get(id);
            pending.delete(id);
            if (succeeded) operation?.resolve(value);
            else operation?.reject(value);
        }
    };
})
