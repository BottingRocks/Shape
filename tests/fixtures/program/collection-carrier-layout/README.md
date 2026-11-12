# Collection API carrier layouts

JavaScript factory excerpts preserve collection API installation, cleanup, captured state, and encoded-header children across carrier layouts.

[`immutable-alias-carrier.js`](immutable-alias-carrier.js), its [`alternate`](immutable-alias-carrier-alternate.js), and [`grouped-alias-carrier.js`](grouped-alias-carrier.js) exercise positive alias/grouping forms. [`encoded-capture-reuse.js`](encoded-capture-reuse.js) is retained as an unproven-child refusal control.

Consumed by:

- [naming_collection_carrier_layout.rs](../../../naming_collection_carrier_layout.rs)

Checks distinguish valid lifecycle and direct-return relationships from mutable aliases, wrong state, unrelated installed children, and sibling factories donating lifecycle evidence.

Preserve the child body, installed function identity, immutable capture aliases, and cleanup/restoration order. The test supplies opaque captures around these excerpts; do not execute them.

Read these as static source inputs; do not execute recovered JavaScript.
Next: [related fixture guide](../collection-mapper-variants/README.md) and the [program fixture overview](../README.md).
