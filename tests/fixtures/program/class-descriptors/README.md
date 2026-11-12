# Subscription class descriptor layouts

This family exercises class-local subscription and observer proofs with descriptor initialization loops.

[`captured-range-class.js`](captured-range-class.js) is a full parent containing the exact subscription interval. [`observer-class.js`](observer-class.js) supplies a complete observer with a for-initializer layout. [`dead-complete-class.js`](dead-complete-class.js) is a negative lifecycle control.

Consumed by:

- [subscriptions_descriptor_tests.rs](../../../../src/naming/classes/subscriptions_descriptor_tests.rs)

The two positive layouts establish subscription and observer class roles. Negatives change descriptor keys, flags, getter identities, loop bounds and effects, publication, or eager uses; a dead completion method cannot supply an observer lifecycle.

Keep the full parent and its byte-addressed interval unchanged. Descriptor arrays, own getters, and installation receivers are proof inputs rather than incidental formatting.

Read these as static source inputs; do not execute recovered JavaScript.
Next: [related fixture guide](../observable-mixed-factories/README.md) and the [program fixture overview](../README.md).
