(function (iina, native, binaryNotFound, runtimeError) {
    const pending = new Map();
    let lastId = 0;
    const failure = (message, code) => Object.assign(new Error(message), { code });
    const request = (start, stdoutHook, stderrHook) => new Promise((resolve, reject) => {
        const id = lastId = (lastId + 1) >>> 0;
        pending.set(id, { resolve, reject, stdoutHook, stderrHook });
        try {
            start(id);
        } catch (error) {
            pending.delete(id);
            reject(failure(String(error), runtimeError));
        }
    });
    iina.utils = {
        ERROR_BINARY_NOT_FOUND: binaryNotFound,
        ERROR_RUNTIME: runtimeError,
        fileInPath: native.fileInPath,
        resolvePath: path => native.resolvePath(path) ?? null,
        keychainWrite: native.keychainWrite,
        keychainRead: native.keychainRead,
        preferredLocalizations: native.languages,
        open: native.open,
        ask: title => native.ask(title) != null,
        prompt: title => native.prompt(title) ?? null,
        chooseFile(title, options = {}) {
            return request(id => native.chooseFile(id, title, JSON.stringify({
                chooseDir: options?.chooseDir === true,
                allowedFileTypes: options?.allowedFileTypes ?? []
            })));
        },
        exec(file, args, cwd, stdoutHook, stderrHook) {
            if (!native.canExecute) return null;
            if (!Array.isArray(args) || args.some(arg => typeof arg !== "string"))
                throw new TypeError("Helper arguments must be strings");
            const hooks = (typeof stdoutHook === "function" ? 1 : 0)
                | (typeof stderrHook === "function" ? 2 : 0);
            return request(id =>
                native.exec(id, file, args, typeof cwd === "string" ? cwd : null, hooks),
                stdoutHook, stderrHook);
        }
    };
    return (id, stage, value, code) => {
        const entry = pending.get(id);
        if (!entry) return;
        if (stage === "stdout") { entry.stdoutHook(value); return; }
        if (stage === "stderr") { entry.stderrHook(value); return; }
        pending.delete(id);
        if (stage === "done") entry.resolve(value);
        else entry.reject(code === binaryNotFound ? code : failure(value, code));
    };
})
