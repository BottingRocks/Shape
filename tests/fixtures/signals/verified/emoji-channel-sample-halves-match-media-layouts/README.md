# Emoji channel sample halves match media layouts

The baseline pairs text-canvas sampling with media queries. Its direct slots include
native-function counting, DOM statistics, viewport dimensions, DOM marker flags and a named
cookie-value emission. This profile also owns the compressed projection proofs.

[slot-map.json](slot-map.json) records 10 layouts, each with 83 address-to-identity entries.
The baseline is
[emoji-channel-sample-halves-match-media-layout.json](schemas/emoji-channel-sample-halves-match-media-layout.json);
[builtin-probe-checksum-ui-event-is-trusted-layout.json](schemas/builtin-probe-checksum-ui-event-is-trusted-layout.json) is an independent alternate control.
Layout labels describe opening signal order, not a complete list of collected signals.

The [schemas guide](schemas/README.md) explains the trimmed grammar evidence: this profile
retains 9 signal records per layout, fewer than the slot map's identities.
The [proofs guide](proofs/README.md) covers the separate baseline projection answer key.

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
