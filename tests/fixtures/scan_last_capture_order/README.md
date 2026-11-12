# Scan and last capture ordering

[captured-order-parent.js](captured-order-parent.js) retains the whole JavaScript
parent for scan and last operators, including native constructor creation,
captured stores, state transitions, error forwarding and subscription cleanup.

[Capture-order unit tests](../../../src/naming/functions/observables/subscription_operators.rs)
inspect the real scoped declarations and mutate only the intended own bodies.
They distinguish accepted ordered initialization from missing, foreign or early
constructor exposure; unrelated sibling evidence cannot supply readiness.
[Scan/last unit tests](../../../src/naming/functions/observables/subscription_operators/scan_last.rs)
also exercise production routing and compare complete stored outcomes from the
[paired JSON baseline](../scan-last-parent-outcomes/README.md).

Preserve exact source bytes and declaration locations: the baseline records
function identities, evidence spans and edit metadata. Keep state producers,
subscribers and cleanup receivers distinct when designing in-memory controls.
Do not execute the recovered program or regenerate its outcome JSON for docs.

Read next: [subscription operator guide](../../../src/naming/functions/observables/subscription_operators/README.md)
and [constructor epoch fixtures](../joined_stream_state_and_cleanup/README.md).
