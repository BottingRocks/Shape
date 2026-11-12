# Ring buffer constructor and consumer layouts

JavaScript excerpts retain ring-buffer classes together with constructor aliases, instance creation, writes, and snapshot consumers.

[`inline-constructor.js`](inline-constructor.js) and the three split-constructor sources cover equivalent capacity setup. [`residual-origin-baseline.js`](residual-origin-baseline.js) and its alternate split the zero/non-number predicate; [`residual-origins.json`](residual-origins.json) records exact excerpts and fixture identities.

Consumed by:

- [ring_buffer_tests.rs](../../../../src/naming/classes/ring_buffer_tests.rs)

Positive checks require complete `put`, `toArray`, and `clear` behavior with stable constructor transport. Independent negatives change capacity, snapshot order, receivers, alias writes, escape, consumption order, and donor ownership.

Keep constructor, carrier, and consumers together. The test supplies bootstrap context for excerpts; preserve exact source bytes and metadata rather than formatting or executing these fragments.

Read these as static source inputs; do not execute recovered JavaScript.
Next: [related fixture guide](../ring-buffer-constructor-epochs/README.md) and the [program fixture overview](../README.md).
