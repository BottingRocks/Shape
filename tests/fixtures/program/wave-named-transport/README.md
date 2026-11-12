# Named transport and guarded wave origins

This family covers parameter-only transport helpers and guarded writes that populate wave registries before their consuming calls.

[`parameter-helper.js`](parameter-helper.js) is a compact structural fixture for parameter binding transport, including anonymous arrows and hook controls. Baseline, guarded, parameter-layout, alternate, and heldout full sources retain actual helper calls. [`guarded-origin.js`](guarded-origin.js), [`branched-registry.js`](branched-registry.js), and [`dormant-registry.js`](dormant-registry.js) separate guarded, branch-local, and unconsumed origins.

Consumed by:

- [naming_wave_named_transport.rs](../../../naming_wave_named_transport.rs)
- [naming_wave_guarded_registry.rs](../../../naming_wave_guarded_registry.rs)

Positive routes preserve all transported origins and hook distinctions. Early/dormant helper calls, wrong argument provenance, four-argument hook consumption, or uninitialized registries cannot donate wave labels. The dormant source is an absence control, not a populated registry witness.

Preserve helper arguments, branch correlation, and consumption order. The small helper is fixture text, not a generator utility; no execution of recovered programs is needed.

Read these as static source inputs; do not execute recovered JavaScript.
Next: [related fixture guide](../wave-position/README.md) and the [program fixture overview](../README.md).
