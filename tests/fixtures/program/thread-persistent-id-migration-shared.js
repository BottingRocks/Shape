function main(options = {}) {
    const logs = [];
    let settings = { persistentId: { key: options.key || "account", resetKey: "expired" }, transactionToken: "token-A" };
    const entries = new Map([["f", options.legacy || "AAAAAABkAQAA"], ["expired", "delete-me"]]);
    if (options.existing) entries.set(settings.persistentId.key, "retained");
    let store = {
        getItem(key) { logs.push(["get", key]); return entries.has(key) ? entries.get(key) : null; },
        setItem(key, value) { logs.push(["set", key, value]); entries.set(key, value); },
        removeItem(key) { logs.push(["remove", key]); entries.delete(key); }
    };
    let ready = false, leaseKey = null, leaseValue = null, tokenState = null;
    let entropy = () => { logs.push(["entropy"]); return "salt-A"; };
    let consumeToken = value => { logs.push(["consume", value]); return value.toUpperCase(); };
    let notify = () => { logs.push(["notify", ready]); };
    let observers = [];
    let Stream = function(install) {
        logs.push(["construct"]);
        this.tag = "stream-A";
        install({ next(value) { logs.push(["forward", value]); } });
    };
    let modules = [{
        init(stream) {
            logs.push(["init", stream.tag]);
            observers[observers.length - 1]("message-A");
            if (options.throwModule) throw new Error("module-A");
        },
        afterReady() { logs.push(["afterReady"]); }
    }];
    let defer = !!options.defer, pending = [];
    let report = error => { logs.push(["report", error.message]); };
    function $THREAD_1_1234() {
        let key = settings.persistentId.key;
        try {
            if (store != null) {
                if (store.getItem(key) == null) {
                    let legacy = store.getItem("f");
                    if (legacy != null && typeof legacy === "string") {
                        let alphabet = {};
                        let cursor = 0;
                        while (cursor < "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_=".length) {
                            alphabet["ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_="[cursor]] = cursor;
                            ++cursor;
                        }
                        let bytes = [];
                        let offset = 0;
                        while (offset < legacy.length) {
                            if (alphabet[legacy[offset + 2]] === 64 && alphabet[legacy[offset + 3]] !== 64) {
                                throw new Error("The string to be decoded is not correctly padded.");
                            } else {
                                bytes.push(alphabet[legacy[offset]] << 2 | alphabet[legacy[offset + 1]] >> 4);
                                if (alphabet[legacy[offset + 2]] !== 64 && offset + 2 < legacy.length) {
                                    bytes.push((alphabet[legacy[offset + 1]] & 15) << 4 | alphabet[legacy[offset + 2]] >> 2);
                                }
                                if (alphabet[legacy[offset + 3]] !== 64 && offset + 3 < legacy.length) {
                                    bytes.push((alphabet[legacy[offset + 2]] & 3) << 6 | alphabet[legacy[offset + 3]]);
                                }
                                offset += 4;
                                continue;
                            }
                        }
                        if (bytes[5] >= 90 && bytes[5] <= 190 && bytes[6] === 1 && bytes[7] === 0 && bytes[8] === 0) {
                            store.removeItem("f");
                            try {
                                ready = false;
                                if (store) {
                                    if (!store.getItem(key)) { store.setItem(key, legacy); }
                                    ready = true;
                                }
                            } catch (migrationError) {}
                        }
                    }
                }
            }
        } catch (decodeError) {}
        let currentKey = settings.persistentId.key;
        let token = settings.transactionToken;
        try {
            ready = false;
            if (store) {
                if (typeof settings.persistentId.resetKey !== "undefined") {
                    store.removeItem(settings.persistentId.resetKey);
                }
                if (!store.getItem(currentKey)) { store.setItem(currentKey, token); }
                ready = true;
            }
        } catch (initError) {}
        try {
            if (store) {
                leaseKey = "l_" + settings.persistentId.key;
                leaseValue = "" + Date.now() + entropy();
                if (!store.getItem(leaseKey)) { store.setItem(leaseKey, leaseValue); }
            }
        } catch (leaseError) {}
        tokenState = consumeToken(settings.transactionToken);
        notify();
        try {
            modules.forEach(item => {
                item.init(new Stream(observer => {
                    let captured = observer;
                    observers.push(value => captured.next(value));
                }));
                if (item.afterReady != null) {
                    if (typeof setTimeout !== "function") {
                        item.afterReady.call(void 0);
                    } else if (defer) {
                        setTimeout(item.afterReady, 0);
                    } else if (pending) {
                        pending.push(item.afterReady);
                    }
                }
            });
        } catch (moduleError) {
            let error = {};
            error.message = "3.1:" + moduleError;
            report(error);
        }
    }
    $THREAD_1_1234();
    return { logs, entries, ready, leaseKey, leaseValue, tokenState, pending, observers };
}
