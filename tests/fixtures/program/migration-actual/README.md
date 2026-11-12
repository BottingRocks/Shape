# Storage migration protocol definitions

Two full cleaned JavaScript sources retain complete persistent-identifier migration definitions within their original lexical surroundings.

[`storage-migration-baseline.js`](storage-migration-baseline.js) and [`storage-migration-alternate.js`](storage-migration-alternate.js) are independent protocol witnesses, each checked at its exact declaration range.

Consumed by:

- [naming_persistent_id_migration.rs](../../../naming_persistent_id_migration.rs)

The consumer distinguishes the owned migration protocol from stronger provider-readiness or algorithm claims. Related tests vary decode order, receivers, effects, opaque ports, collisions, and existing human names; changing an opaque port does not establish its implementation.

Preserve complete definitions and byte spans. Do not collapse these sources into the smaller migration examples in the parent folder or infer live initialization from a definition-only claim.

Read these as static source inputs; do not execute recovered JavaScript.
Next: [related fixture guide](../README.md) and the [program fixture overview](../README.md).
