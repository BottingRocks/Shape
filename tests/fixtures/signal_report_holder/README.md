# Signal report holder protocols

[actual.js](actual.js) contains a JavaScript snapshot body that selects options,
notifies last-chance callbacks and partitions results into named, positional and
numeric-keyed arrays. Receiver/method/value holders preserve the push operation
through branch joins. [baseline.js](baseline.js) is a separate report-builder control.

[The integration target](../../naming_signal_report_holder.rs) supplies explicit
lexical captures for the snapshot and checks getSignalSnapshot under
signals_signal_snapshot. The older control retains buildSignalReport under
signals_signal_report_builder; it must not be silently replaced by the snapshot.

Positive renamed and equivalent declaration forms retain exact receiver and
metadata relationships. Negative mutations change holder producers, field order,
notification/debug options, captures and effects. Rewrite edits are replayed to
check the returned source, not merely the claimed role.

Preserve both inputs, option-default distinctions, array destinations and mutation
anchors. These JavaScript bodies are static analysis data; do not run them.
Read next: [serialization envelope](../signal_serialization_family/README.md)
and [observable rules](../../../src/naming/functions/observables/README.md).
