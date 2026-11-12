# Event and interaction rules

The sibling [events.rs](../events.rs) declares this part of the
[function evaluator layer](../README.md). Rules connect event fields, clocks,
captured state and emitted records through their own source bodies.

| File | Responsibility |
| --- | --- |
| [visibility_change_sample.rs](visibility_change_sample.rs) | Buffered visible/timestamp record and surrounding state changes |
| [interaction_timing_recorders.rs](interaction_timing_recorders.rs) | Keydown gap totals and keyup duration moments |
| [pointer_travel_distance.rs](pointer_travel_distance.rs), [touch_payload_record.rs](touch_payload_record.rs) | Pointer distance and touch record protocols |
| [submit_notification.rs](submit_notification.rs) | Notification and forward/reset proof; [helper guide](submit_notification/README.md) |
| [exchange_handshake.rs](exchange_handshake.rs), [exchange_teardown.rs](exchange_teardown.rs) | Exchange setup and cleanup roles |

Visibility sampling checks the owned record's `visible` and `timestamp`
expressions, including the event-target read and captured clock invocation.
A matching object key alone is not the record proof.

Timing recorders resolve every participant by symbol and retain arithmetic
order and literals. The keydown path stores press times and splits inter-key
gaps; the keyup path updates hold-duration moments, tracked-key counts and the
active flag. Keep their local temporaries distinct from captured state.

See [timing tests](../../../../tests/naming_interaction_timing_recorders.rs),
[visibility alias tests](../../../../tests/naming_visibility_method_alias.rs) and
[shared event tests](../../../../tests/naming_thread_shared_events.rs).
[Browser rules](../browser/README.md) own adjacent submit instrumentation teardown.
