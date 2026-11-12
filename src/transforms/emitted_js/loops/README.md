# Loop reconstruction, counters and labels

These passes turn the VM's labeled `for (;;)` regions back into `while`,
`do…while`, `for` and `for…in` loops, lift and merge counters, and drop
labels no jump needs. Most run in `clean_function` (stages `11d2`, `11d3`,
`11e3`, `11h4`, `13b`); `copied_counters.rs` runs in `finish_program` (dump
`13e-`); `index_counters.rs` only in the `--thread-cleanup` driver.
`structured_loops.rs` runs in both.

## Files

| File | What it does |
| --- | --- |
| [`mod.rs`](mod.rs) | Declares the modules. |
| [`structured_loops.rs`](structured_loops.rs), [`structured_loops/`](structured_loops/README.md) | `reconstruct_loops`: `vm_loop_N: for (;;) { lets; if (t) { … continue; } else { … break; } }` → `while (t)` or a counted `for`, with the exit continuation after the loop. Stage `11d2-reconstructed-loops`; end of thread `expression_sweep`. |
| [`loop_counters.rs`](loop_counters.rs) | `lift_loop_counters`: `while (c < n) { …; c = Number(c) + 1; }` → `for (; c < n; c = Number(c) + 1) { … }`. Stages `11d3` and `11h4`. |
| [`loop_tests.rs`](loop_tests.rs) | `split_loop_tests`: `while (A, c) S` → `for (;;) { A; if (!c) break; S }`; `a, b;` → two statements; `c ? (a, b) : d;` → `if`. Stage `11e3-split-loop-tests`. |
| [`loop_counter_copies.rs`](loop_counter_copies.rs) | `merge_loop_counter_copies`: reads of a top-of-body copy `c = j;` become `j`, and the copy goes. Stage `13b`, before the final tidy. |
| [`native_for_in.rs`](native_for_in.rs) | `native_for_in_loops`: `let K = vmForInKeys(O); for (let i = 0; i < K["length"]; i++) { if (K[i] in O) BODY }` → `for (const i in O) BODY`. Stage `13b`. |
| [`loop_labels.rs`](loop_labels.rs) | `remove_needless_loop_labels`: drops labels and terminal `continue`s that jump where an unlabeled jump would; first, a `continue outer` from the (possibly labeled) inner loop ending the outer body becomes that inner loop's `break`, moving a trailing `throw`/`return` into the inner loop's one `break` or, for a tested inner loop with no `break`, into its failed test. Stage `13b`, after the final tidy. |
| [`copied_counters.rs`](copied_counters.rs) | `step_copied_counters`: `for (;; j = Number(c) + 1) { c = j; if (!(j < n)) break; … }` → `for (; j < n; j++)`. `finish_program`, after `fold_bit_accumulators`. |
| [`index_counters.rs`](index_counters.rs) | `recover_index_counters`: repeated left-associated `+1` index snapshots → one private counter. End of thread `late_propagation`. |

## Contracts

- `reconstruct_loops` never invents names and never lifts a captured
  per-iteration binding. Counter initializers named in `protected` (stage
  `11d2` passes every `$KEY_` name in the function) stay put. `with` or a
  reference to `eval` stops the pass.
- A counter update moves into a `for` header only when no `continue` can skip
  it and it reads no body-local binding (`structured_loops.rs`,
  `loop_counters.rs`). `loop_counters.rs` lifts the update past trailing
  stores only when the update cannot throw; it assumes standard built-ins
  and declines under `with`/`eval`.
- `loop_counter_copies.rs` and `copied_counters.rs` decline when a read of the
  copy sits in a nested function: it could run after a later step, where copy
  and counter differ.
- `native_for_in.rs` requires `K` read only as `K["length"]` and `K[i]`, `i`
  only inside `K[i]`, and `O` built from names the loop never assigns.
- `index_counters.rs` keeps each increment at the original index evaluation
  and does not reassociate floating-point sums; numeric category, full
  reference coverage and capture proofs are required.
- Order: `loop_tests.rs` leaves `for (;;) { if (!c) break; … }`, which the
  final tidy (`tidy_final_expressions`) writes back as `while (c)`;
  `loop_counter_copies.rs` runs before that tidy so it can rebuild the counted
  loop; `loop_labels.rs` runs after it, once restructured jumps are known.

[Emitted JavaScript cleanup](../README.md) ·
[pipeline table](../../../../docs/transforms/emitted-program-cleanup.md#pipeline)
