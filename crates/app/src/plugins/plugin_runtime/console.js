(function (api, sink) {
    const stringify = JSON.stringify;
    const tag = Object.prototype.toString;
    const asString = String;

    function format(value) {
        if (value === null) return "<null>";
        if (value === undefined) return "<undefined>";
        if (typeof value !== "object" && typeof value !== "function") return asString(value);
        try {
            if (tag.call(value) === "[object Date]") return asString(value);
            return stringify(value, null, 2) ?? "<undefined>";
        } catch (_) {
            return "<unserializable object>";
        }
    }

    function formatArguments(values) {
        if (values.length === 1) return format(values[0]);
        let message = "";
        for (const value of values) {
            const object = value !== null && (typeof value === "object" || typeof value === "function");
            message += format(value) + (object ? "\n" : " ");
        }
        return message;
    }

    api.console = {
        log: (...values) => sink.log(formatArguments(values)),
        warn: value => sink.warn(format(value)),
        error: value => sink.error(format(value)),
    };
    globalThis.console = {
        ...api.console,
        info: (...values) => sink.info(formatArguments(values)),
    };
})
