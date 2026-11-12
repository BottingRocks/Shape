# Schemas for console stack access flags match media layouts

These 10 JSON files supply seed lookup and selected first-emission grammar
evidence for the [profile slot map](../slot-map.json). They contain top-level
`seed` and `signals` keys; `signals` is keyed by slot address, not canonical name.

[console-stack-access-flags-match-media-layout.json](console-stack-access-flags-match-media-layout.json) is the baseline.
It retains 9 records: `DocumentReferrerHistory`, `UiEventIsTrusted`, `NavigatorPlugins`,
`WorkerRealmProfile`, `OuterArgsGlobalProcess`, `DeviceMemory`, `LocationUrlComponents`,
`FeaturePolicy`, `ShuffledDateString`.
The 82-entry slot map covers more identities; absence here means no retained grammar
for that slot, not evidence that the signal is absent from the source.

For example, baseline `wave:11:4` is `DocumentReferrerHistory`; its first
`emissions[0].grammar.items` operations are `raw_prefix → byte → string → number`.
[current-script-async-policy-state-emoji-channel-sample-halves-layout.json](current-script-async-policy-state-emoji-channel-sample-halves-layout.json) keeps an alternate seed's
addressing, ordering, expressions and source spans independently.

[Catalog tests](../../../../../../src/signals/analysis/catalog_tests.rs) use the exact
seed to locate external source under `SHAPE_FROZEN_ROOT`. They compare selected
bound field names to keywords in resolved expressions, conditions and values;
opaque items or incompatible item counts can leave fields unchecked. The files
are trimmed evidence, not complete schemas or runnable JavaScript.

Keep raw prefixes, choice/repeat structure, masks, write/value spans and literal
source slices exact. Do not regenerate or repair golden inputs to fit a result.
Read the [profile guide](../README.md), [field orders](../../../field-orders.json)
and [catalog registry](../../../../../../src/signals/catalog/mod.rs) when tracing a mismatch.
