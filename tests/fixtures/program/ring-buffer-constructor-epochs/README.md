# Ring buffer constructor assignment epochs

A full cleaned JavaScript parent preserves constructor captures, producer activation, publication, and consumer entry order beyond the compact ring-buffer excerpts.

[`constructor-epoch-parent.js`](constructor-epoch-parent.js) is the retained whole-program witness used for assignment-epoch and constructor-entry checks.

Consumed by:

- [ring_buffer_tests.rs](../../../../src/naming/classes/ring_buffer_tests.rs)

Tests pair direct model checks with production naming assertions. Supported hoisted consumers and local callable aliases retain proof; early aliases, transitive publication, changed producer destinations, capture reentry, and resource escapes must fail their relevant obligations.

Preserve the full parent and its assignment order. A constructor declaration alone is insufficient to establish the capture state at a public entry point; keep these controls separate from the sibling small-layout tests.

Read these as static source inputs; do not execute recovered JavaScript.
Next: [related fixture guide](../ring-buffer/README.md) and the [program fixture overview](../README.md).
