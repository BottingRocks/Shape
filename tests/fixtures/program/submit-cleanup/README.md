# Submit instrumentation cleanup lifecycles

This family preserves browser submit instrumentation initialization, publication, registry methods, readers, and paired teardown protocols.

[`direct-submit-parent.js`](direct-submit-parent.js) and [`captured-submit-parent.js`](captured-submit-parent.js) retain full parent context. The before/after `.map.json` reports record naming outcomes and edits. [`closed-lifecycle.js`](closed-lifecycle.js) and [`class-positive.js`](class-positive.js) supply closed ordinary/prototype and class registry controls.

Consumed by:

- [submit_instrumentation_cleanup_tests.rs](../../../../src/naming/functions/browser/submit_instrumentation_cleanup_tests.rs)

[`deferred-uninitialized-reader.js`](deferred-uninitialized-reader.js) and [`class-register-overwrites-clear.js`](class-register-overwrites-clear.js) are refusal cases. Tests distinguish identity from initialization/publication, check both cleanup roles, preserve ordered producers and flag resets, and reject early readers or reachable method clobbers.

Keep full source, report spans, and independent lifecycle controls unchanged. Local helper or borrowed-array paths are accepted only under their bounded closed protocol; do not infer safety from a familiar registry method name.

Read these as static source inputs; do not execute recovered JavaScript.
Next: [related fixture guide](../README.md) and the [program fixture overview](../README.md).
