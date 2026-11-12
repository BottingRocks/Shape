# Signal fixture answer keys

This directory holds static expected identities and field-order evidence for
signal analysis. Recovered programs live in an external frozen corpus; the
JSON here is test truth, never recognition input or executable JavaScript.

[field-orders.json](field-orders.json) records per-layout permutations for
`NamespacePropertyNameHashes` and `IframeRealmPropertyNameHashes`, plus the
`ConsoleMemberNativenessHashPair` order (`nonnative_first` or `other_first`).
Repeated permutation rows and storage-success labels preserve independent
observations and branch controls; do not deduplicate them.

The [verified profiles](verified/README.md) hold address-to-identity slot maps
and trimmed grammar schemas. The [emoji-channel sample-halves proofs](verified/emoji-channel-sample-halves-match-media-layouts/proofs/README.md)
also retain compressed baseline projection evidence.

The default [offline fixture test](../../canonical_signal_fixtures.rs) validates every
slot map, schema, compressed proof and field-order layout against the current catalog
without the external corpus.

[Catalog tests](../../../src/signals/analysis/catalog_tests.rs) check identity claims,
card field names, field order and close-pair separation. The ignored
[projection](../../program_signal_projection.rs) and [coverage](../../program_signal_coverage.rs)
targets share the [frozen-layout reader](../../support/frozen_signal_layout.rs).
These corpus checks require external input and can skip absent programs; ordinary
structural extraction cases are in [program_signal_extraction.rs](../../program_signal_extraction.rs).

Baseline and alternate layouts are independent positive identity references;
other slots become negative controls for any particular selected recognizer.
An expected identity is not proof of complete recognition or binding support.
Preserve seeds, slot keys, canonical identities, ordering, source slices and spans,
and decompressed proof evidence. Do not normalize fixtures or execute recovered JS/WASM.
For implementation boundaries and existing test invocations, read the
[signals source guide](../../../src/signals/README.md) and [verified guide](verified/README.md).
