(() => {
            const native = __riinaFile;
            iina.file = {
                exists: path => native.__exists(String(path)),
                showInFinder: path => native.__showInFinder(String(path)),
                read: (path, options = {}) => native.__read(String(path), JSON.stringify(options)),
                write: (path, content) => native.__write(String(path), String(content)),
                delete: path => native.__delete(String(path)),
                trash: path => native.__trash(String(path)),
                move: (source, destination) => native.__move(String(source), String(destination)),
                list: (path, options = {}) => JSON.parse(native.__list(String(path), JSON.stringify(options))),
                handle: (path, mode) => {
                    const id = native.__open(String(path), String(mode));
                    return {
                        offset: () => native.__offset(id),
                        seekTo: offset => native.__seek(id, Number(offset)),
                        seekToEnd: () => native.__seekEnd(id),
                        read: length => new Uint8Array(JSON.parse(native.__readHandle(id, Number(length)))),
                        readToEnd: () => new Uint8Array(JSON.parse(native.__readHandleEnd(id))),
                        write: data => {
                            if (typeof data === 'string') return native.__writeHandleText(id, data);
                            if (data && data.length > 4194304) throw new Error('Handle writes are limited to 4 MiB per call');
                            const bytes = Array.from(data);
                            if (bytes.length > 4194304) throw new Error('Handle writes are limited to 4 MiB per call');
                            return native.__writeHandle(id, JSON.stringify(bytes));
                        },
                        close: () => native.__close(id)
                    };
                }
            };
            globalThis.__riinaFile = undefined;
        })();