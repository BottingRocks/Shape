# Nested frame anchor property names hash incognito storage probe layouts

The baseline starts with a nested-frame property-name probe and a storage probe; its direct
slot records DOM element statistics. Alternate opening sequences retain independent address
assignments.

[slot-map.json](slot-map.json) records 10 layouts, each with 76 address-to-identity entries.
The baseline is
[nested-frame-anchor-property-names-hash-incognito-storage-probe-layout.json](schemas/nested-frame-anchor-property-names-hash-incognito-storage-probe-layout.json);
[browser-api-proxy-flags-host-environment-layout.json](schemas/browser-api-proxy-flags-host-environment-layout.json) is an independent alternate control.
Layout labels describe opening signal order, not a complete list of collected signals.

The [schemas guide](schemas/README.md) explains the trimmed grammar evidence: this profile
retains 9 signal records per layout, fewer than the slot map's identities.

The ignored corpus tests in [catalog_tests.rs](../../../../../src/signals/analysis/catalog_tests.rs)
validate these canonical identities against [catalog registry](../../../../../src/signals/catalog/mod.rs),
check card names and field orders, and reject wrong or ambiguous recognizer claims.
Other slots supply negative controls for a selected recognizer; a mapped identity
does not promise that its binder emits a supported report.

Preserve each layout independently: do not copy baseline addresses over alternate
addresses, rename identities, change seed values, or normalize source expressions
and spans. These are static answer keys; never execute the recovered programs.
Continue with the [verified profiles overview](../README.md) for corpus selection
and the [field-order evidence](../../field-orders.json) for ordering controls.
