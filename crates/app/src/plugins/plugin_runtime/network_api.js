(() => {
            const nativeRequest = __riinaHttp.__request;
            const nativeDownload = __riinaHttp.__download;
            const nativeXmlrpc = __riinaHttp.__xmlrpc;
            const xmlrpcAllowed = __riinaHttp.__xmlrpcAllowed;
            const pending = new Map();
            let nextId = 0;
            const request = (method, url, options = {}) => new Promise((resolve, reject) => {
                nextId = (nextId + 1) >>> 0;
                const id = nextId;
                pending.set(id, { resolve, reject });
                try {
                    nativeRequest(id, method, String(url), JSON.stringify(options));
                } catch (error) {
                    pending.delete(id);
                    reject(error);
                }
            });
            iina.http = {
                get: (url, options) => request('GET', url, options),
                post: (url, options) => request('POST', url, options),
                put: (url, options) => request('PUT', url, options),
                patch: (url, options) => request('PATCH', url, options),
                delete: (url, options) => request('DELETE', url, options),
                download: (url, destination, options = {}) => new Promise((resolve, reject) => {
                    nextId = (nextId + 1) >>> 0;
                    const id = nextId;
                    pending.set(id, { resolve, reject });
                    try {
                        nativeDownload(id, String(url), String(destination), JSON.stringify(options));
                    } catch (error) {
                        pending.delete(id);
                        reject(error);
                    }
                }),
                xmlrpc: location => {
                    const endpoint = String(location);
                    if (!xmlrpcAllowed(endpoint)) throw new Error(`URL ${endpoint} is not allowed.`);
                    return {
                        call: (method, args = []) => new Promise((resolve, reject) => {
                            nextId = (nextId + 1) >>> 0;
                            const id = nextId;
                            pending.set(id, { resolve, reject });
                            try {
                                const json = JSON.stringify(args, function (key, value) {
                                    const original = this && this[key];
                                    if (original instanceof Date) return { __riinaXmlrpcDate: original.toISOString() };
                                    if (ArrayBuffer.isView(original)) return { __riinaXmlrpcBinary: Array.from(original) };
                                    return value;
                                });
                                nativeXmlrpc(id, endpoint, String(method), json);
                            } catch (error) {
                                pending.delete(id);
                                reject(error);
                            }
                        })
                    };
                }
            };
            globalThis.__riinaDeliverHttp = (id, response, error) => {
                const callbacks = pending.get(id);
                if (!callbacks) return;
                pending.delete(id);
                if (error !== null) callbacks.reject(new Error(error));
                else if (response.statusCode < 400 || response.statusCode >= 600) callbacks.resolve(response);
                else callbacks.reject(response);
            };
            globalThis.__riinaDeliverDownload = (id, error) => {
                const callbacks = pending.get(id);
                if (!callbacks) return;
                pending.delete(id);
                if (error !== null) callbacks.reject(new Error(error));
                else callbacks.resolve();
            };
            globalThis.__riinaDeliverXmlrpc = (id, response, error) => {
                const callbacks = pending.get(id);
                if (!callbacks) return;
                pending.delete(id);
                if (error !== null) callbacks.reject(new Error(error));
                else {
                    const hydrate = value => {
                        if (Array.isArray(value)) return value.map(hydrate);
                        if (value && typeof value === 'object') {
                            if (Array.isArray(value.__riinaXmlrpcBinary)) return new Uint8Array(value.__riinaXmlrpcBinary);
                            if (typeof value.__riinaXmlrpcDate === 'string') return new Date(value.__riinaXmlrpcDate);
                            for (const key of Object.keys(value)) value[key] = hydrate(value[key]);
                        }
                        return value;
                    };
                    callbacks.resolve(hydrate(response));
                }
            };
            globalThis.__riinaHttp = undefined;
        })();