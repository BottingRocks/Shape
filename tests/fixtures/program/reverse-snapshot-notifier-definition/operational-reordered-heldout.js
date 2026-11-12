"use strict";
const verify = require("node:assert/strict");
function runIndependentScenario() {
    class $THREAD_902_17 {
        notify(value) {
            let frozen = Array.prototype.slice.call(this.callbacks);
            let offset = frozen.length - 1;
            let carried = null;
            for (; offset >= 0; offset--) {
                if (frozen[offset] != null) {
                    try { carried = frozen[offset].call(void 0, value, carried); } catch (ignored) {}
                }
            }
            return carried;
        }
        clear() { this.callbacks = []; return this; }
        constructor() { this.callbacks = []; }
        unregister(fn) {
            let cursor = 0;
            for (; cursor < this.callbacks.length; cursor++) {
                if (this.callbacks[cursor] === fn) { this.callbacks[cursor] = void 0; }
            }
            return this;
        }
        register(fn) { this.callbacks.push(fn); return this; }
    }
    const left = new $THREAD_902_17();
    const right = new $THREAD_902_17();
    const calls = [];
    const tail = function(value, carried) {
        verify.equal(this, undefined);
        calls.push("tail");
        return carried * value.factor;
    };
    const fail = function(value, carried) {
        calls.push("failure:" + carried);
        throw value.problem;
    };
    const late = function() { calls.push("late"); return 91; };
    const head = function(value, carried) {
        verify.equal(this, undefined);
        verify.equal(carried, null);
        calls.push("head");
        left.register(late).unregister(head);
        return value.start;
    };
    left.register(tail).register(fail).register(head);
    right.register(function(value, carried) { return value + (carried === null ? 1 : carried); });
    verify.equal(left.notify({start: 4, factor: 3, problem: "expected"}), 12);
    verify.deepEqual(calls, ["head", "failure:4", "tail"]);
    verify.equal(right.notify(8), 9);
    verify.equal(left.notify({factor: 2, problem: "again"}), 182);
    verify.deepEqual(calls.slice(3), ["late", "failure:91", "tail"]);
    right.clear();
    verify.equal(right.notify(8), null);
    left.clear();
    verify.equal(left.notify({}), null);
}
runIndependentScenario();
