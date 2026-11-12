# Snapshot before an early store

[snapshot_then_early_store.js](snapshot_then_early_store.js) is a static emitted
JavaScript regression input for value forwarding. It preserves a source-text
comparison result, its slot snapshot, later byte encoding and a fresh-array store
back into the same slot.

The module-local test never_moves_a_store_between_a_snapshot_and_its_reads in
[forwarding tests](../../../src/transforms/emitted_js/ssa/forward_values/tests.rs)
passes this source through the forwarding transform. It rejects the unsafe
combination of moving the array store before the byte write while making that
write read the overwritten slot. Safe output may retain a snapshot or retain the
later store; the test checks their relationship rather than one golden spelling.

Keep snapshot reads, slot writes, byte expression and statement order intact.
Generated SSA names and outside-effect markers are meaningful analysis input.
This file is not a runnable reference implementation; do not execute it or
normalize away the sequence that reproduces the regression.

Read next: [forwarding engine guide](../../../src/transforms/emitted_js/ssa/forward_values/README.md)
and [the fixture collection](../).
