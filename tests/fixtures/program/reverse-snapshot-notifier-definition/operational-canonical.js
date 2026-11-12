"use strict";
const assert = require("node:assert/strict");
function exerciseRegistry() {
    class $THREAD_901_43 {
        constructor() { this.callbacks = []; }
        register(listener) { this.callbacks.push(listener); return this; }
        unregister(listener) {
            let slot = 0;
            for (; slot < this.callbacks.length; slot++) {
                if (this.callbacks[slot] === listener) { this.callbacks[slot] = void 0; }
            }
            return this;
        }
        clear() { this.callbacks = []; return this; }
        notify(message) {
            let copy = Array.prototype.slice.call(this.callbacks);
            let position = copy.length - 1;
            let previous = null;
            for (; position >= 0; position--) {
                if (copy[position] != null) {
                    try { previous = copy[position].call(void 0, message, previous); } catch (failure) {}
                }
            }
            return previous;
        }
    }
    const Registry = $THREAD_901_43;
    const registry = new Registry();
    const trace = [];
    function oldest(message, previous) {
        assert.equal(this, undefined);
        trace.push(["oldest", previous]);
        return previous + message.increment;
    }
    function broken(message, previous) {
        assert.equal(this, undefined);
        trace.push(["broken", previous]);
        throw new Error(message.reason);
    }
    function hole() { throw new Error("unregistered slot was delivered"); }
    function later(message, previous) {
        trace.push(["later", previous]);
        return message.increment * 2;
    }
    function newest(message, previous) {
        assert.equal(this, undefined);
        trace.push(["newest", previous]);
        registry.unregister(oldest).clear().register(later);
        return 10;
    }
    assert.equal(registry.register(oldest).register(broken).register(hole).register(newest), registry);
    assert.equal(registry.unregister(hole), registry);
    assert.equal(registry.notify({increment: 7, reason: "expected"}), 17);
    assert.deepEqual(trace, [["newest", null], ["broken", 10], ["oldest", 10]]);
    assert.equal(registry.notify({increment: 6}), 12);
    assert.deepEqual(trace[3], ["later", null]);
    assert.equal(registry.clear(), registry);
    assert.equal(registry.notify({increment: 1}), null);
    registry.register(oldest).register(oldest).unregister(oldest);
    assert.equal(registry.notify({increment: 1}), null);
}
exerciseRegistry();
