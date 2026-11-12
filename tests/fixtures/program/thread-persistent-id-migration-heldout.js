// Independent operational environment: indexed records, accessor methods,
// asynchronous-style stream subscription and different keys/token/hooks.
function scenario(options = {}) {
    let trace = [], records = [["f", options.legacy || "AAAAAABkAQAA"], ["retired-key", "old"]];
    let configuration = { transactionToken: "beta:77", persistentId: { resetKey: "retired-key", key: "device-b" } };
    if (options.existing) records.push(["device-b", "keep-b"]);
    class RecordPort {
        find(key) { return records.findIndex(entry => entry[0] === key); }
        get getItem() {
            trace.push(["lookup-get"]);
            return key => { trace.push(["read", key]); const index = this.find(key); return index < 0 ? null : records[index][1]; };
        }
        setItem(key, value) {
            trace.push(["write", key, value]);
            const position = this.find(key);
            if (position < 0) records.push([key, value]); else records[position][1] = value;
        }
        removeItem(key) { trace.push(["delete", key]); const at = this.find(key); if (at >= 0) records.splice(at, 1); }
    }
    let backend = new RecordPort();
    let active = false, leaseName = "", leaseData = "", processed = null;
    let salt = () => { trace.push(["salt"]); return "salt-B"; };
    let transform = token => { trace.push(["transform", token]); return token.split(":").reverse().join("/"); };
    let onConfigured = () => trace.push(["configured", active]);
    let deliveries = [];
    let Bridge = class {
        constructor(register) {
            trace.push(["bridge"]);
            this.values = [];
            register({ next: value => { this.values.push(value); trace.push(["delivery", value]); } });
        }
        subscribe(value) { deliveries[deliveries.length - 1](value); }
    };
    let extensions = [{
        afterReady: function() { trace.push(["ready-b"]); },
        init(channel) {
            channel.subscribe("message-B");
            trace.push(["subscribed", channel.values.length]);
            if (options.throwModule) throw new Error("extension-B");
        }
    }];
    let deferred = !!options.defer, scheduled = [];
    let onError = payload => trace.push(["error-b", payload.message]);
    function $THREAD_2_991() {
        let originalKey = configuration.persistentId.key;
        try {
            if (backend != null) {
                if (backend.getItem(originalKey) == null) {
                    let previous = backend.getItem("f");
                    if (previous != null && typeof previous === "string") {
                        let table = {};
                        let n = 0;
                        while (n < "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_=".length) {
                            table["ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_="[n]] = n;
                            n++;
                        }
                        let decoded = [];
                        let group = 0;
                        while (group < previous.length) {
                            if (table[previous[group + 2]] === 64 && table[previous[group + 3]] !== 64) {
                                throw new Error("The string to be decoded is not correctly padded.");
                            } else {
                                decoded.push(table[previous[group]] << 2 | table[previous[group + 1]] >> 4);
                                if (table[previous[group + 2]] !== 64 && group + 2 < previous.length) {
                                    decoded.push((table[previous[group + 1]] & 15) << 4 | table[previous[group + 2]] >> 2);
                                }
                                if (table[previous[group + 3]] !== 64 && group + 3 < previous.length) {
                                    decoded.push((table[previous[group + 2]] & 3) << 6 | table[previous[group + 3]]);
                                }
                                group += 4;
                                continue;
                            }
                        }
                        if (decoded[5] >= 90 && decoded[5] <= 190 && decoded[6] === 1 && decoded[7] === 0 && decoded[8] === 0) {
                            backend.removeItem("f");
                            try {
                                active = false;
                                if (backend) {
                                    if (!backend.getItem(originalKey)) { backend.setItem(originalKey, previous); }
                                    active = true;
                                }
                            } catch (writeError) {}
                        }
                    }
                }
            }
        } catch (decodeError) {}
        let selectedKey = configuration.persistentId.key;
        let fallback = configuration.transactionToken;
        try {
            active = false;
            if (backend) {
                if (typeof configuration.persistentId.resetKey !== "undefined") { backend.removeItem(configuration.persistentId.resetKey); }
                if (!backend.getItem(selectedKey)) { backend.setItem(selectedKey, fallback); }
                active = true;
            }
        } catch (initialError) {}
        try {
            if (backend) {
                leaseName = "l_" + configuration.persistentId.key;
                leaseData = "" + Date.now() + salt();
                if (!backend.getItem(leaseName)) { backend.setItem(leaseName, leaseData); }
            }
        } catch (leaseError) {}
        processed = transform(configuration.transactionToken);
        onConfigured();
        try {
            extensions.forEach(extension => {
                extension.init(new Bridge(observer => {
                    let destination = observer;
                    deliveries.push(value => destination.next(value));
                }));
                if (extension.afterReady != null) {
                    if (typeof setTimeout !== "function") {
                        extension.afterReady.call(void 0);
                    } else if (deferred) {
                        setTimeout(extension.afterReady, 0);
                    } else if (scheduled) {
                        scheduled.push(extension.afterReady);
                    }
                }
            });
        } catch (extensionError) {
            let details = {};
            details.message = "3.1:" + extensionError;
            onError(details);
        }
    }
    $THREAD_2_991();
    return { trace, records, active, leaseName, leaseData, processed, scheduled, deliveries };
}
