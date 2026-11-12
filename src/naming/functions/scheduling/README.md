# Scheduling and queue rules

The sibling [scheduling.rs](../scheduling.rs) declares these
[function evaluators](../README.md). Timers, queue operations and bounded
collection protocols share this capability folder, but retain separate proofs.

| File | Responsibility |
| --- | --- |
| [bounded_timeout.rs](bounded_timeout.rs) | Captured provider timeout with a bounded delay |
| [elapsed_timer.rs](elapsed_timer.rs), [elapsed_hours.rs](elapsed_hours.rs) | Elapsed clock roles |
| [task_queue.rs](task_queue.rs) | Queue protocol recognition |
| [interval_extrema.rs](interval_extrema.rs) | Interval extrema handling |
| [work_batch_protocols.rs](work_batch_protocols.rs) | Closed collection mapping/batch protocols |
| [sine_prng_refill.rs](sine_prng_refill.rs) | Sine-based PRNG refill |

The timeout rule distinguishes data from callables. Scheduled value, provider
and handle are captured plain data bindings; the delay producer must be an
independently ordered callable. The claim names the provider's `setTimeout`
protocol without establishing that receiver as a browser-native timer.

Work-batch protocols are capture-free and describe exact owned operations on
formal receivers. Their names do not establish those receivers' native types.
Check [stages.rs](../../stages.rs) when changing registration or precedence.

See [timeout tests](../../../../tests/naming_bounded_timeout.rs),
[queue tests](../../../../tests/naming_queue_dispatch_protocol.rs) and
[callback variant tests](../../../../tests/naming_shared_callback_variants.rs).
