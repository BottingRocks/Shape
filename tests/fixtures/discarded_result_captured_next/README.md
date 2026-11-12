# Discarded captured next result

[owning-scope.js](owning-scope.js) retains a complete JavaScript owner with tuple
capture setup, scheduling calls and a registered worker callback. The callback
calls the captured observer's next method and discards the returned value.

[The integration target](../../naming_discarded_result_captured_next.rs) expects
callCapturedNext for that exact body. Replacing the call with a return exercises
the separate forwardValueToCapturedObserver role; these outcomes are intentionally
distinct. Alpha-renaming and an ordered immutable alias remain positive.

Negative mutations change receiver, argument, method, function shape or capture
origin. Earlier registration and stores deferred into opaque schedulers must
refuse because those schedulers may invoke callbacks synchronously.

Keep the entire owner, exact byte length, registration sites and mutation anchors.
Tests deliberately distinguish a sibling arrow from the target declaration.
Do not execute this recovered source or reduce it to a context-free callback.

Read next: [return-forwarding controls](../../naming_captured_observer_forwarding.rs)
and [the fixture collection](../).
