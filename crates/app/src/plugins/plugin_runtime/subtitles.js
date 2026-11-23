(function (iina) {
    const providers = new Map();
    const items = new WeakSet();
    const custom = "custom-implementation";
    let current = null;

    iina.subtitle = {
        CUSTOM_IMPLEMENTATION: custom,
        item(data, desc) {
            const item = { desc };
            Object.defineProperty(item, "data", { value: data, enumerable: true });
            items.add(item);
            return item;
        },
        registerProvider(id, provider) {
            if (typeof id !== "string" || id.length === 0 || id.length > 128) {
                throw new TypeError("A subtitle provider needs a valid ID");
            }
            if (!providers.has(id) && providers.size >= 32) {
                throw new RangeError("This plugin has reached its subtitle provider limit");
            }
            providers.set(id, provider);
        }
    };

    async function search(ticket, id, complete) {
        const request = { ticket, results: null, provider: providers.get(id) };
        current = request;
        try {
            const provider = request.provider;
            if (!provider || typeof provider.search !== "function" || typeof provider.download !== "function") {
                throw new Error("The plugin did not register a subtitle provider with search and download methods");
            }
            const result = await provider.search();
            if (current !== request) throw new Error("This subtitle search was replaced");
            if (result === custom) {
                complete(null, "null");
                return;
            }
            if (!Array.isArray(result) || result.length > 256) {
                throw new Error("Subtitle search must return up to 256 subtitle items");
            }
            const descriptions = result.map(item => {
                if (!items.has(item)) throw new Error("Use iina.subtitle.item to create subtitle results");
                if (item.desc == null && typeof provider.description === "function") {
                    item.desc = provider.description(item);
                }
                return {
                    name: String(item.desc?.name ?? "").slice(0, 512),
                    left: String(item.desc?.left ?? "").slice(0, 512),
                    right: String(item.desc?.right ?? "").slice(0, 512)
                };
            });
            request.results = result.slice();
            complete(null, JSON.stringify(descriptions));
        } catch (error) {
            complete(String(error), null);
        }
    }

    async function download(ticket, indices, complete) {
        const request = current;
        try {
            if (request?.ticket !== ticket || request.results === null) {
                throw new Error("The subtitle results have expired");
            }
            const paths = [];
            for (const index of indices) {
                const item = request.results[index];
                if (item === undefined) throw new Error("The subtitle result is unavailable");
                const result = await request.provider.download(item);
                if (current !== request) throw new Error("This subtitle search was replaced");
                if (!Array.isArray(result) || paths.length + result.length > 64) {
                    throw new Error("Subtitle download must return paths to up to 64 subtitle files");
                }
                for (const path of result) {
                    if (typeof path !== "string" || path.length > 8192) {
                        throw new TypeError("The subtitle provider returned an invalid file path");
                    }
                    paths.push(path);
                }
            }
            complete(null, JSON.stringify(paths));
        } catch (error) {
            complete(String(error), null);
        }
    }

    return { search, download };
})
