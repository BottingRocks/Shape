# Baseline signal projection proofs

These gzip-compressed JSON documents preserve per-signal evidence for the
[baseline emoji-channel sample-halves layout](../schemas/emoji-channel-sample-halves-match-media-layout.json).
[EmojiChannelSampleHalves.json.gz](EmojiChannelSampleHalves.json.gz) records numeric writes and
the two repeated sample halves; [MatchMedia.json.gz](MatchMedia.json.gz) adds
conditional and repeated media-query fields. [CorsCookieValue.json.gz](CorsCookieValue.json.gz)
instead records a named-object `generic_value` emission.

Each proof includes `signal`, `label`, `address`, `descriptor`, source identity
metadata and `field_set`, alongside computation and source-audit evidence.
Field rows preserve emission order, repeat/choice paths, masks, static XORs,
byte alternatives and exact write/value spans. Some proofs also retain
`baseline_field_sets`; those records are not interchangeable with the main field set.

The [frozen-layout reader](../../../../../support/frozen_signal_layout.rs)
reads only `.json.gz` files and filters by label. The ignored
[program_signal_projection](../../../../../program_signal_projection.rs) target
compares flattened projected leaves and generic keys with these rows. It allows
the existing cookie-emitter contract gap; that unsupported projection is not
a passing complete layout. Byte-literal folding is accepted only when the
original source proves the same conditional pair, using the exact seed and
recorded source digest under `SHAPE_PROOF_SOURCE_ROOT`.

Treat decompressed evidence, source hashes, source spans and golden field sets as
immutable. Read/decompress as data only; never run recovered JavaScript or WASM.
Follow the [profile guide](../README.md) and [schemas guide](../schemas/README.md)
for the separate identity and trimmed-grammar controls.
