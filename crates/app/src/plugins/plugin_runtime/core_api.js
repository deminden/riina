(() => {
    const core = iina.core;
    const readStatus = core.__readStatus;
    const readChapters = core.__readChapters;
    const readRecentDocuments = core.__readRecentDocuments;
    const readHistory = core.__readHistory;
    const readVersion = core.__readVersion;
    const readWindowProperty = core.__readWindowProperty;
    const writeWindowProperty = core.__writeWindowProperty;
    const readCoreProperty = core.__readCoreProperty;
    const writeCoreProperty = core.__writeCoreProperty;
    const loadTrack = core.__loadTrack;
    core.status = new Proxy(Object.create(null), {
        get(_target, property) {
            if (typeof property !== 'string') return undefined;
            const serialized = readStatus(property);
            return serialized == null ? undefined : JSON.parse(serialized);
        },
        set() { return false; }
    });
    core.getChapters = () => JSON.parse(readChapters());
    core.getRecentDocuments = () => JSON.parse(readRecentDocuments());
    core.getHistory = () => JSON.parse(readHistory()).map(entry => ({
        ...entry,
        date: new Date(entry.date)
    }));
    core.getVersion = () => JSON.parse(readVersion());
    core.window = new Proxy(Object.create(null), {
        get(_target, property) {
            if (typeof property !== 'string') return undefined;
            const serialized = readWindowProperty(property);
            return serialized == null ? undefined : JSON.parse(serialized);
        },
        set(_target, property, value) {
            if (typeof property !== 'string') return false;
            return writeWindowProperty(property, JSON.stringify(value));
        }
    });
    for (const namespace of ['audio', 'subtitle', 'video']) {
        const api = new Proxy(Object.create(null), {
            get(_target, property) {
                if (property === 'loadTrack') return url => loadTrack(namespace, url);
                if (typeof property !== 'string') return undefined;
                const serialized = readCoreProperty(`${namespace}.${property}`);
                return serialized == null ? undefined : JSON.parse(serialized);
            },
            set(_target, property, value) {
                if (typeof property === 'string') writeCoreProperty(`${namespace}.${property}`, JSON.stringify(value));
                return true;
            }
        });
        core[namespace] = api;
    }
    delete core.__readStatus;
    delete core.__readChapters;
    delete core.__readRecentDocuments;
    delete core.__readHistory;
    delete core.__readVersion;
    delete core.__readWindowProperty;
    delete core.__writeWindowProperty;
    delete core.__readCoreProperty;
    delete core.__writeCoreProperty;
    delete core.__loadTrack;
})();
