(() => {
            const nativeRequest = __riinaHttp.__request;
            const nativeDownload = __riinaHttp.__download;
            const nativeXmlrpc = __riinaHttp.__xmlrpc;
            const xmlrpcAllowed = __riinaHttp.__xmlrpcAllowed;
            const pending = new Map();
            let nextId = 0;
            const xmlrpcTag = '__riinaXmlrpcType';
            const encodeXmlrpcArgs = args => {
                if (!Array.isArray(args)) throw new TypeError('XML-RPC arguments must be an array');
                const path = new WeakSet();
                const encode = (value, depth) => {
                    if (depth > 65) throw new RangeError('XML-RPC value nesting exceeds 64 levels');
                    if (value === null || typeof value !== 'object') return value;
                    if (value instanceof Date) {
                        return { [xmlrpcTag]: 'date', value: value.toISOString() };
                    }
                    if (value instanceof ArrayBuffer || ArrayBuffer.isView(value)) {
                        const bytes = value instanceof ArrayBuffer
                            ? new Uint8Array(value)
                            : new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
                        return { [xmlrpcTag]: 'binary', value: Array.from(bytes) };
                    }
                    if (path.has(value)) throw new TypeError('XML-RPC arguments contain a cycle');
                    path.add(value);
                    try {
                        if (Array.isArray(value)) return value.map(item => encode(item, depth + 1));
                        const members = Object.fromEntries(Object.entries(value).map(
                            ([key, item]) => [key, encode(item, depth + 1)]
                        ));
                        // A plugin struct can use any field name, including the internal tag.
                        return Object.hasOwn(members, xmlrpcTag)
                            ? { [xmlrpcTag]: 'struct', value: members }
                            : members;
                    } finally {
                        path.delete(value);
                    }
                };
                return JSON.stringify(encode(args, 0));
            };
            const hydrateMembers = members => Object.fromEntries(Object.entries(members).map(
                ([key, value]) => [key, hydrateXmlrpc(value)]
            ));
            const hydrateXmlrpc = value => {
                if (Array.isArray(value)) return value.map(hydrateXmlrpc);
                if (value === null || typeof value !== 'object') return value;
                switch (value[xmlrpcTag]) {
                    case 'binary': return new Uint8Array(value.value);
                    case 'date': return new Date(value.value);
                    case 'struct': return hydrateMembers(value.value);
                    default: return hydrateMembers(value);
                }
            };
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
                                const json = encodeXmlrpcArgs(args);
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
            globalThis.__riinaDeliverXmlrpc = (id, json) => {
                const callbacks = pending.get(id);
                if (!callbacks) return;
                pending.delete(id);
                try {
                    const reply = JSON.parse(json);
                    switch (reply.state) {
                        case 'value': callbacks.resolve(hydrateXmlrpc(reply.data)); break;
                        case 'fault': callbacks.reject(); break;
                        case 'failed': callbacks.reject(reply.data); break;
                        default: throw new Error('Invalid XML-RPC reply');
                    }
                } catch (error) {
                    callbacks.reject(error);
                }
            };
            globalThis.__riinaHttp = undefined;
        })();
