# Seeded signal frame nonce layouts

Two full cleaned JavaScript programs retain seed factories, number writers, observer consumers, and the reverse serialization loop for `emitSeededSignalFrame`.

[`separate-nonce.js`](separate-nonce.js) is the nonce-reuse witness selected by the test constant `REUSED`; [`reused-nonce.js`](reused-nonce.js) is selected by `SEPARATE` for the counterpart route. Read the consumer bindings rather than inferring semantics from filenames.

Consumed by:

- [naming_seeded_signal_frame_variants.rs](../../../naming_seeded_signal_frame_variants.rs)

Both routes are positive. Independent negatives change nonce-header byte order, reseeding, length and reverse-loop dataflow, writer identity, output tuple order, and completion flow. Factory proof requires exactly four owned warmups, not nested calls or a foreign returned helper.

Keep complete capture scopes and unique mutation anchors intact. Generated names and literal salts are not identity gates; the actual factory behavior and capture lifetime are.

Read these as static source inputs; do not execute recovered JavaScript.
Next: [related fixture guide](../audio-result/README.md) and the [program fixture overview](../README.md).
