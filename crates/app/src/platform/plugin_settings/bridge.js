(() => {
    let request = 0;
    const callbacks = new Map();
    const post = message => window.ipc.postMessage(JSON.stringify(message));
    const preferences = {
        get(key, callback) {
            if (typeof callback !== "function") throw new TypeError("Expected a callback");
            if (typeof key !== "string" || key.length > 4096 || callbacks.size >= 128) return;
            request = (request + 1) >>> 0;
            callbacks.set(request, callback);
            post({ kind: "get", id: request, key });
        },
        set(key, value) { post({ kind: "set", key, value }); },
    };
    window.iina = {
        preferences,
        log(message) { post({ kind: "log", message: String(message) }); },
        _receivePreference(id, value) {
            const callback = callbacks.get(id);
            callbacks.delete(id);
            if (callback) callback(value);
        },
    };

    function bindInput(input) {
        const radio = input.type === "radio";
        const checkbox = input.type === "checkbox";
        const key = radio ? input.name : input.dataset.prefKey;
        if (!key) return;
        let edited = false;
        preferences.get(key, value => {
            if (edited || value === null) return;
            if (radio) input.checked = input.value === value;
            else if (checkbox) input.checked = Boolean(value);
            else input.value = value;
        });
        input.addEventListener("change", () => {
            edited = true;
            if (radio && !input.checked) return;
            let value = checkbox ? input.checked : input.value;
            if (!checkbox && !radio) {
                if (input.dataset.type === "int") value = Number.parseInt(value, 10);
                if (input.dataset.type === "float") value = Number.parseFloat(value);
                if (typeof value === "number" && !Number.isFinite(value)) return;
            }
            preferences.set(key, value);
        });
    }

    document.addEventListener("DOMContentLoaded", () => {
        const inputs = new Set(document.querySelectorAll("input[data-pref-key]"));
        for (const input of inputs) {
            if (input.type !== "radio" || !input.name) continue;
            for (const option of document.getElementsByName(input.name)) {
                if (option instanceof HTMLInputElement && option.type === "radio") inputs.add(option);
            }
        }
        for (const input of inputs) bindInput(input);
    }, { once: true });
})();
