# Reverse audio encoding with a prefix

These short JavaScript function excerpts cover fractional random nonce values coerced into header bytes before reverse sample encoding.

[`fractional-hash-local.js`](fractional-hash-local.js) keeps a separate hash local; [`fractional-reseed.js`](fractional-reseed.js) reuses the nonce binding for the reseeded state. Both are positive controls for `emitSeededHashEncoding`.

Consumed by:

- [naming_audio_reverse_prefix.rs](../../../naming_audio_reverse_prefix.rs)

The test supplies a lexical carrier and normalizes participant names. Independent negatives break channel ownership, prefix and length writes, traversal bounds, observer tuple order, helper stability, or loop ownership; integer-coerced nonce spelling is also accepted.

Preserve the distinct local-state layouts and exact mutation anchors. These excerpts depend on captures supplied by the test and are not standalone development scripts.

Read these as static source inputs; do not execute recovered JavaScript.
Next: [related fixture guide](../audio-result/README.md) and the [program fixture overview](../README.md).
