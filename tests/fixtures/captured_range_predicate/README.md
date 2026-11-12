# Captured range predicates

Whole JavaScript parents and JSON naming reports preserve the context needed to
prove two inclusive captured ranges plus the exact literal fallback.

[The integration target](../../naming_captured_range_predicate.rs) accepts
[initialized-bounds-parent.js](initialized-bounds-parent.js), including its
uninitialized-bound epoch, and refuses
[multi-epoch-constructor.js](multi-epoch-constructor.js). That negative is not an
interchangeable positive merely because it contains a similar predicate.

[The evaluator's unit controls](../../../src/naming/functions/observables/captured_range_predicate.rs)
also read [direct-call-parent.js](direct-call-parent.js),
[parent-outcome-control.json](parent-outcome-control.json) and
[parent-report-after-extension.json](parent-report-after-extension.json).
[parent-report-before-extension.json](parent-report-before-extension.json)
retains the earlier comparison report. The
[conditional-form tests](../../../src/naming/functions/observables/captured_range_predicate_conditional_tests.rs)
use [conditional-bounds.js](conditional-bounds.js) for initialized branch forms.

Preserve source bytes and JSON identities, evidence spans and edits. Tests vary
bounds, writers, exposure order and operators in memory; never regenerate the
saved reports or execute the captured JavaScript for a documentation change.
Read next: [observable rules](../../../src/naming/functions/observables/README.md)
and [scan/last ordering](../scan_last_capture_order/README.md).
