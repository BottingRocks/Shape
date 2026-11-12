# Wave names from registry positions

Three full cleaned JavaScript sources retain independent ordered registries and the transport/consumer paths used to name collectors.

[`wave-position-baseline.js`](wave-position-baseline.js), [`wave-position-alternate.js`](wave-position-alternate.js), and [`wave-position-heldout.js`](wave-position-heldout.js) pair with [`expected.json`](expected.json). The JSON pins input identities and role records with function IDs, slots, prior semantic labels, and anonymous status.

Consumed by:

- [naming_wave_position.rs](../../../naming_wave_position.rs)

Positive controls follow registry position even when source order or local spelling changes. Negative and conflict controls reject donor bodies, mutated shared bindings, early consumption, wrong ABI, capture collisions, and unsupported transport syntax. Scoped edits must preserve strings, comments, and shadows.

Keep the sources and golden slot/identity records immutable. Do not sort registries by declaration order or regenerate function IDs after formatting; positions are evidence carried through actual consumption.

Read these as static source inputs; do not execute recovered JavaScript.
Next: [related fixture guide](../wave-preserve/README.md) and the [program fixture overview](../README.md).
