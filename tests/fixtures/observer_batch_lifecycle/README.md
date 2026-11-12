# Observer batch lifecycle and reverse canvas rows

[observer-batch-baseline.js](observer-batch-baseline.js) and
[observer-batch-alternate.js](observer-batch-alternate.js) preserve JavaScript
batch lifecycle bodies with explicit opaque constructor/reset captures.
[reverse-canvas.js](reverse-canvas.js) separately preserves a canvas sampling
harness whose rows are encoded in reverse order.

[The integration target](../../naming_observer_batch_protocols.rs) expects the
linked lifecycle roles, including runObserverBatchLifecycle, in both batch forms.
It checks evidence as well as application. Alpha variants remain positive;
changed buffering, error forwarding, cleanup or capture availability must refuse.
The reverse canvas control checks dimensions, pixel lanes, row cursor and numeric
writer arguments, including every exposure through aliases.

These are analysis harnesses: helper spellings such as queueFactory do not
establish native implementations. Preserve capture setup and all body/order
relationships; do not execute the JavaScript or normalize these controls.

The target also retains the [canvas setup family](../canvas_text_sampling_setup/README.md).
Read next: [observable rules](../../../src/naming/functions/observables/README.md)
and [the fixture collection](../).
