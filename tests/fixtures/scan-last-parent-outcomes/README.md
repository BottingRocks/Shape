# Scan and last outcome baselines

[parent-outcomes.json](parent-outcomes.json) is a JSON array of four complete
naming outcomes: the scan and last factories and their operator functions.
It belongs to the unchanged
[captured-order parent](../scan_last_capture_order/captured-order-parent.js).

The scan_last_preserves_exact_role_and_cleanup_metadata unit test in
[scan_last.rs](../../../src/naming/functions/observables/subscription_operators/scan_last.rs)
analyzes that parent and compares each outcome as a complete JSON value. The
baseline covers role and evaluator, declaration/reference identity, evidence
spans, explanation, confidence and edit counts rather than only a proposed name.

Keep the JSON and its corresponding source bytes immutable. Reformatting the
source would invalidate span-based identities even if its behavior looked
unchanged. This is an expected-output baseline, not an independent JavaScript
positive or a report to regenerate when a documentation file is added.

Read next: [capture-order guide](../scan_last_capture_order/README.md) for mutation
controls and [subscription operator guide](../../../src/naming/functions/observables/subscription_operators/README.md)
for the proof boundaries.
