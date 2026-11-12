# Observable and serialization rules

The sibling [observables.rs](../observables.rs) declares this part of
[function naming](../README.md). The folder covers stream protocols as well as
signal report, snapshot and serializer wiring.

| File or guide | Responsibility |
| --- | --- |
| [observable_factory.rs](observable_factory.rs) | Owned returned class and complete method protocols |
| [joined_observables](joined_observables/README.md) | Join proofs and constructor epochs |
| [subscription_operators](subscription_operators/README.md) | Scan/last, callback and pipe adapters; exposure readiness |
| [captured_handle_termination](captured_handle_termination/README.md) | Guarded opaque handle termination and completion |
| [serializer.rs](serializer.rs) | Configuration, extension registration and returned snapshot serializer |
| [snapshot.rs](snapshot.rs), [payload_finalizer.rs](payload_finalizer.rs) | Separate snapshot and payload finalization rules |

The factory proof establishes its own class and returned immutable alias, then
checks complete method bodies. Captured constructors and operators used by that
class retain their scoped identities without acquiring native purposes merely
from the class proof.

The serializer rule links omission keys, the seed alias, configuration and
extensions to the returned function. Its snapshot field reads must occur at that
function's own call-frame level. Numeric writers, payload finalization and
snapshot-to-bytes roles are separate proofs, not consequences of that match.

See [shared observable tests](../../../../tests/naming_thread_shared_observables.rs),
[serialization tests](../../../../tests/naming_signal_serialization_family.rs) and
[operator epoch tests](observable_operator_epochs_tests.rs). The
[naming guide](../../README.md) explains report and rewrite boundaries.
