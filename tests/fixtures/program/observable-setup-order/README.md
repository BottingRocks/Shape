# Observable capture setup order

These full cleaned JavaScript parents preserve ordinary subscriber construction and constructor assignment epochs around observable factories.

[`ordinary-parent.js`](ordinary-parent.js) covers class-first ordinary factory layout, tuple receivers, map, and captured scalar-frame behavior. [`constructor-epoch-parent.js`](constructor-epoch-parent.js) supplies the guarded source-class setup and constructor timing context.

Consumed by:

- [observable_factory_layout_tests.rs](../../../../src/naming/functions/observables/observable_factory_layout_tests.rs)
- [observable_factory_source_class_tests.rs](../../../../src/naming/functions/observables/observable_factory_source_class_tests.rs)
- [observable_map_operator.rs](../../../../src/naming/functions/observables/observable_map_operator.rs)
- [seeded_signal_frame.rs](../../../../src/naming/functions/observables/seeded_signal_frame.rs)

Positive assertions require readiness at the actual factory call and complete owned APIs. Mutations independently change method contracts, call receivers, class publication, capture copies, and input writers; later initialization cannot authorize an earlier use.

Keep full parents and their assignment order unchanged. The sibling mixed-factory suite also consumes these as independent controls, so abbreviated bodies would lose relevant initialization and escape evidence.

Read these as static source inputs; do not execute recovered JavaScript.
Next: [related fixture guide](../observable-mixed-factories/README.md) and the [program fixture overview](../README.md).
