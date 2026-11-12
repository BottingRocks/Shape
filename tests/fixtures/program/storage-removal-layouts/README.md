# Storage removal frame layouts

JavaScript callbacks and full parents cover storage-removal frames with different result holders, seed carriers, aliases, and flag layouts.

[`fixed-result-callback.js`](fixed-result-callback.js), [`mixed-carrier-callback.js`](mixed-carrier-callback.js), and [`aliased-factory-callback.js`](aliased-factory-callback.js) retain distinct excerpt layouts. [`current-storage-callback.js`](current-storage-callback.js) and its alternate preserve full scopes for direct-frame lane checks.

Consumed by:

- [naming_storage_removal_layouts.rs](../../../naming_storage_removal_layouts.rs)

Controls keep low/high removal lanes and independent wire-order evidence separate. Negatives change tuple receivers, cursor/tag values, old-value flags, factory origins, observer identity, immutable aliases, and owned effects; nested factory donors cannot supply a live seed.

Preserve each lane and its own frame order. Do not combine the independent controls or infer the complete wire contract from a single matching callback shape.

Read these as static source inputs; do not execute recovered JavaScript.
Next: [related fixture guide](../keyed-deletion/README.md) and the [program fixture overview](../README.md).
