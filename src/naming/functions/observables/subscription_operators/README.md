# Subscription operator support

The sibling [subscription_operators.rs](../subscription_operators.rs) owns the
scan, last, callback-subscription and two-stage pipe proofs under the
[observable rules](../README.md).

| File | Responsibility |
| --- | --- |
| [readiness.rs](readiness.rs) | Callable-copy exposures and executed initialization checks |
| [scan_last.rs](scan_last.rs) | Test-only source-class scan/last controls |

Reducers, predicates and source methods retain opaque purposes. The parent
checks complete factory/operator/executor bodies, state producers and consumers,
error forwarding, return and cleanup. Constructor identity is proved separately;
there are distinct native and ordinary subscriber-class paths.

Callable aliases may be copied before captured setup, but every eventual read
must satisfy readiness, and each copy must itself have been stored before it is
read. Constructor object copies keep the stricter read-at-copy check in the
parent's `closed_constructor` logic. Do not use this callable adapter to relax
shared constructor resolution.

Parent inline tests exercise capture ordering and operational mutations.
[Integration tests](../../../../../tests/naming_subscription_operators.rs) and
[epoch tests](../observable_operator_epochs_tests.rs) cover the surrounding
factory and publication obligations.
