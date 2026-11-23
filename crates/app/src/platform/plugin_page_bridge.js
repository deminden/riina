(() => {
    const key = "riinaPluginDocument";
    const tag = new URL(location.href).searchParams.get("riina_document");
    let documentId = tag;
    try {
        if (tag !== null) sessionStorage.setItem(key, tag);
        else documentId = sessionStorage.getItem(key);
    } catch (_) {}
    const listeners = new Map();
    window.iina = {
        onMessage(name, callback) { listeners.set(name, callback); },
        postMessage(name, data) {
            if (documentId !== null) {
                window.ipc.postMessage(JSON.stringify({ document: documentId, name, data }));
            }
        },
        _emit(name, data) { listeners.get(name)?.(data === undefined ? undefined : JSON.parse(data)); },
        _matchesDocument(tag) { return documentId === tag; }
    };
})();
