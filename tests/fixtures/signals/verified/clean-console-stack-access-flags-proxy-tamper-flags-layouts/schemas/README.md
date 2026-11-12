# Schemas for clean console stack access flags proxy tamper flags layouts

These 10 JSON files supply seed lookup and selected first-emission grammar
evidence for the [profile slot map](../slot-map.json). They contain top-level
`seed` and `signals` keys; `signals` is keyed by slot address, not canonical name.

[clean-console-stack-access-flags-proxy-tamper-flags-layout.json](clean-console-stack-access-flags-proxy-tamper-flags-layout.json) is the baseline.
It retains 6 records: `OuterArgsGlobalProcess`, `DeviceMemory`, `LocationUrlComponents`,
`NavigatorPlugins`, `UiEventIsTrusted`, `ShuffledDateString`.
The 37-entry slot map covers more identities; absence here means no retained grammar
for that slot, not evidence that the signal is absent from the source.

For example, baseline `wave:4:4` is `ShuffledDateString`; its first
`emissions[0].grammar.items` operations are `raw_prefix → number → string → number`.
[storage-access-api-presence-user-agent-data-layout.json](storage-access-api-presence-user-agent-data-layout.json) keeps an alternate seed's
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
