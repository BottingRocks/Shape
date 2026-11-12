# Mixed observable factory parents

Whole-parent JavaScript inputs cover observable construction with mixed factory captures, source-class inputs, and complete class behavior.

[`mixed-factory-parent.js`](mixed-factory-parent.js) retains the setup-order case; [`mixed-factory-before.map.json`](mixed-factory-before.map.json) is the earlier naming report with class/function outcomes and edits. [`source-class-parent.js`](source-class-parent.js) supplies source-class dependencies, while [`whole-class.js`](whole-class.js) keeps a compact complete class control.

Consumed by:

- [observable_factory_mixed_tests.rs](../../../../src/naming/functions/observables/observable_factory_mixed_tests.rs)
- [observable_factory_setup_order_tests.rs](../../../../src/naming/functions/observables/observable_factory_setup_order_tests.rs)
- [scan_last.rs](../../../../src/naming/functions/observables/subscription_operators/scan_last.rs)

Tests close whole-parent obligations before claiming factories or `Observable`, and retain scan/last operator controls. Alpha and neutral declaration variants pass; changed methods, cleanup, capture copies, publication, late writers, escape, and reentry refuse.

Keep the report and exact source bytes intact. A valid first guard is insufficient if another method or parent obligation fails; do not replace complete classes with method stubs.

Read these as static source inputs; do not execute recovered JavaScript.
Next: [related fixture guide](../observable-setup-order/README.md) and the [program fixture overview](../README.md).
