# Joined streams and constructor epochs

[constructor-epoch-parent.js](constructor-epoch-parent.js) is a whole JavaScript
parent retaining joined-stream state, subscription cleanup and several stores
to a captured constructor. [native-control-maker.js](native-control-maker.js)
is a separate complete native observable maker used to construct epoch controls.

[Join unit tests](../../../src/naming/functions/observables/joined_observables.rs)
show why the parent's three constructor writes fail the older single-origin
guard. [Epoch tests](../../../src/naming/functions/observables/joined_observables/epochs.rs)
collect all three stores and verify factory proofs and publication readiness;
they combine the control maker with an existing join control to test accepted
exclusive branches and successive initialization epochs.

[Operator epoch tests](../../../src/naming/functions/observables/observable_operator_epochs_tests.rs)
also consume the parent. Keep these separate from the single-origin control:
one successful factory proof cannot justify an unrelated or later store.

Preserve source bytes, constructor writes, factory APIs and cleanup order.
Do not execute the recovered programs or flatten the enclosing scopes.
Read next: [join implementation guide](../../../src/naming/functions/observables/joined_observables/README.md)
and [operator epoch controls](../observable-operator-epochs/README.md).
