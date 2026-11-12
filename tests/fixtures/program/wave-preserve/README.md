# Preserving independently established wave labels

Full cleaned JavaScript programs and a golden ledger preserve prior collector labels established by wiring even when registry position evidence is incomplete or conflicting.

[`wave-preservation-baseline.js`](wave-preservation-baseline.js), its alternate and heldout sources, and [`wave-preservation-reordered.js`](wave-preservation-reordered.js) pair with [`expected.json`](expected.json). The JSON records input identities and expected preserved function-ID/name pairs.

Consumed by:

- [naming_wave_preserve.rs](../../../naming_wave_preserve.rs)

The consumer also uses the parent `focused-threads.js` control. Corroborating wiring and registry positions form one claim; conflicting independent position evidence must not overwrite wiring-established labels. These are preservation controls, not proof that every registry is position-complete.

Keep each source and its golden names/IDs unchanged. Preserve reordered and wiring-only cases independently so new registry inference cannot silently replace prior semantic results.

Read these as static source inputs; do not execute recovered JavaScript.
Next: [related fixture guide](../wave-position/README.md) and the [program fixture overview](../README.md).
