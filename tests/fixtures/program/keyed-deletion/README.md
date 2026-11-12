# Keyed deletion state frames

Full cleaned JavaScript sources retain storage-event captures, live seed and writer factories, registration, and the callback that emits a keyed deletion frame.

[`keyed-deletion-alternate.js`](keyed-deletion-alternate.js) is the primary mutation input. [`keyed-deletion-baseline.js`](keyed-deletion-baseline.js) and [`keyed-deletion-heldout.js`](keyed-deletion-heldout.js) are independent negative controls whose unsupported probe topologies must remain unclaimed.

Consumed by:

- [naming_keyed_deletion_state_frame.rs](../../../naming_keyed_deletion_state_frame.rs)

The claimed role is `emitKeyedDeletionStateFrame`. Controls check deletion versus old value, removal/error/auxiliary flag positions, original coercions, header and value serialization order, observer identity, and unresolved probe limits. Unsupported probe topologies remain unclaimed.

Preserve each full scope and exact callback slice. Alpha variants may change spelling, but early calls and hoisted helpers must not borrow initialization that happens later.

Read these as static source inputs; do not execute recovered JavaScript.
Next: [related fixture guide](../storage-removal-layouts/README.md) and the [program fixture overview](../README.md).
