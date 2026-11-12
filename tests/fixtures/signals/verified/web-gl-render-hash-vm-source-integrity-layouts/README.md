# Web gl render hash vm source integrity layouts

The baseline pairs a WebGL context probe with vm source integrity. Direct collectors include DOM
statistics and emoji glyph rgba samples. Its schema subset omits worker-realm-profile and
environment-check grammar records found in larger profiles.

[slot-map.json](slot-map.json) records 10 layouts, each with 65 address-to-identity entries.
The baseline is
[web-gl-render-hash-vm-source-integrity-layout.json](schemas/web-gl-render-hash-vm-source-integrity-layout.json);
[console-stack-access-flags-emoji-channel-sample-halves-in-web-gl-render-hash-vm-source-integrity-layout.json](schemas/console-stack-access-flags-emoji-channel-sample-halves-in-web-gl-render-hash-vm-source-integrity-layout.json) is an independent alternate control.
Layout labels describe opening signal order, not a complete list of collected signals.

The [schemas guide](schemas/README.md) explains the trimmed grammar evidence: this profile
retains 7 signal records per layout, fewer than the slot map's identities.

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
