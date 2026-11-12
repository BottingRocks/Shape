# Observable API definition epochs

[control.js](control.js) and [eager-decorator.js](eager-decorator.js) form a precise
JavaScript positive/negative pair. The second adds an eager decorator expression
to static of; the surrounding native observable maker and operator wiring remain
available for analysis.

[Operator epoch unit tests](../../../src/naming/functions/observables/observable_operator_epochs_tests.rs)
assert the exact single replacement between these sources. The undecorated
control retains the owned observable class proof and all five checked map and
callback-adapter roles. The eager decorator must refuse those claims because
class definition can activate the operator before its setup is ready.

These files test definition-time execution and exposure ordering, not merely
whether decorated syntax parses. Preserve the pair's exact relationship, native
API bodies, captured constructor stores and role identities. Do not execute the
captured JavaScript or remove the decorator to make the negative pass.

Read next: [constructor epoch fixtures](../joined_stream_state_and_cleanup/README.md)
and [observable naming rules](../../../src/naming/functions/observables/README.md).
