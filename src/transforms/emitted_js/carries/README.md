# Loop and branch carries

Passes over `$CARRY_` relays (and, for the channel passes, private `$SSA_` and
local cells) that the tracer leaves at branch joins and loop copies. Four run in
`clean_function` during `ssa_and_carries` (`07-`–`07c-`), and
`remove_dead_carries` and `remove_carry_self_assignments` run again in
`dead_code_and_loops` (`11c4-`, `11d-`) and after forwarding (`11l-`). Equivalent
carries, join channels, carry lifetimes and stable channels run only in the
thread cleanup driver (`cleanup_thread_impl` in
[`pipeline/thread.rs`](../pipeline/thread.rs): `trace program --thread-cleanup`,
`cleanup_thread`, `shape trace cleanup-thread`).

## Files

| File | What it does |
| --- | --- |
| [`carry_join.rs`](carry_join.rs) | `let $CARRY_x; if (t) { $CARRY_x = a; } else { $CARRY_x = b; } return $CARRY_x;` → `return ((t) ? (a) : (b));` when the carry has two writes and one read, the next statement. Stage `07-carry-joins.js`; thread `expression_sweep` |
| [`carry_coalesce.rs`](carry_coalesce.rs) | Renames an inner carry's writes to the outer carry and drops `$CARRY_outer = $CARRY_inner;`, or an initialized relay copied back after a loop. Stage `07b-carry-coalesced.js` (×4); thread `expression_sweep` |
| [`carry_self_assign.rs`](carry_self_assign.rs) | Drops discarded `$SSA_`/`$CARRY_` self-stores and initialized reads: statement `$SSA_1 = $SSA_1;` → `;`, unused comma prefixes, for-header fields → `0`. Stages `07b2-`, `11d-`; thread `expression_sweep`, `branch_cells`, `recycling` |
| [`dead_carry.rs`](dead_carry.rs) | Removes `let $CARRY_*` bindings nothing reads; a store with an effectful RHS stays as `(rhs);`. Stages `07c-` (×4), `11c4-`, `11l-`; thread `expression_sweep`, `late_propagation` |
| [`equivalent_carry.rs`](equivalent_carry.rs) | Merges carries with identical initializers and lockstep stores of the same value: drops the right declarator and its paired stores, renames its reads. Thread `expression_sweep` only |
| [`join_channels.rs`](join_channels.rs) | `normalize_join_channels`: moves identical trailing expression statements of both `if` arms after the `if`; `let x; work(); x = v;` → `let x = v;` for a private undefined cell with one later unconditional store. Thread `expression_sweep`, `storage_sweep`, `late_propagation` |
| [`join_channels/`](join_channels/README.md) | The common-value factoring `normalize_join_channels` also applies |
| [`carry_lifetimes/`](carry_lifetimes/README.md) | `coalesce_local_handoffs` and `finish_private_channels`: a copy-out cell takes over its source's storage. Thread only |
| [`stable_channels/`](stable_channels/README.md) | `eliminate_stable_channels`: reads of argument and private-cell forwarding channels go back to their input. Thread `late_propagation` only |
| [`carry_semantics_tests.rs`](carry_semantics_tests.rs) | Node differential tests: nested branch-value diamonds, loop-snapshot coalescing, equivalent-carry merges |
| [`mod.rs`](mod.rs) | Module declarations |

## Contracts

- Every pass declines on `with` or a reference named `eval`, and on comments in
  the text it would delete or move.
- Deleting a store never deletes its RHS evaluation: `dead_carry` keeps an
  effectful RHS or initializer as `(rhs);`/`(init);` at the same point.
  A store must be a plain statement assignment after the declaration
  (`return $CARRY = 1` declines).
- `carry_join` evaluates the test first and exactly one arm; it needs exactly
  two writes and one read, globally.
- `carry_coalesce` needs the outer binding declared first, every inner
  reference in the copy's function and all inner writes before the copy. The
  initialized form also rejects an overlapping `try`, an exiting jump and an
  outer write in the moved region.
- `carry_self_assign` keeps const and TDZ errors (the binding must be a
  `let`/`var` initialized before the statement), concise-arrow results, a used
  comma result's GetValue, and an empty-statement barrier before a following
  string expression.
- `equivalent_carry` pairs literals byte-for-byte and identifiers by resolved
  symbol; renamed reads must re-resolve to the left binding and must not be
  shorthand properties.
- Order: coalescing exposes copies the self-assignment and dead-carry passes
  then remove, so `07b-` precedes `07b2-` and `07c-`. `finish_private_channels`
  runs only after snapshots have chosen storage, so a new channel cannot steal a
  longer-lived destination.
- The thread-only passes take the protected `$KEY_` set from
  `ThreadCleanupOptions::shared_slots`; a protected name is never rewritten.

`cargo test --lib carry_semantics_tests` needs Node on `PATH`.

[Emitted JavaScript cleanup](../README.md) ·
[pipeline table](../../../../docs/transforms/emitted-program-cleanup.md#pipeline) ·
[thread cleanup](../../../../docs/transforms/thread-cleanup.md)
