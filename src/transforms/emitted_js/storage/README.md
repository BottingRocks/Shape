# Declarations, slots and storage

Passes over declarations and the cells behind them: VM `$KEY_n` slots, private
`$SSA_`/`$CARRY_` cells and their stores. `clean_function` runs the slot
passes: `remove_redundant_initializers` (`01-`), `remove_unread_key_stores`
(`11c3-`, `11c4-`, `11l-`), `clean_local_vars` (`11l2-`, `13b-`) and
`place_key_declarations` (`13b-`); `finish_program` runs `remove_unused_keys`
(dump `13d-`). The cell passes (`dead_locals`, `declaration_lifetimes`,
`fresh_values/`, `last_use_storage/`, `private_array_reads`) run only in the
thread cleanup driver (`cleanup_thread_impl` in
[`pipeline/thread.rs`](../pipeline/thread.rs): `trace program --thread-cleanup`,
`cleanup_thread`, `shape trace cleanup-thread`). `unread_declarations.rs` is
the standalone public pass and is in neither order.

## Files

| File | What it does |
| --- | --- |
| [`redundant_initializers.rs`](redundant_initializers.rs) | `let $KEY_a, $KEY_b; $KEY_a = void 0; $KEY_b = void 0;` → `let $KEY_a, $KEY_b;`, for the consecutive run directly after the `let`. Stage `01-redundant-initializers.js`; public |
| [`dead_key.rs`](dead_key.rs) | Drops statement stores to a local `$KEY_` with no resolved read; a non-pure RHS stays as `(rhs);`. Stages `11c3-dead-key-stores.js`, `11c4-`, `11l-` |
| [`local_vars.rs`](local_vars.rs) | `var $KEY_n = 9;` nothing reads → its stores go; `$KEY_4 = f(); var $KEY_4; use($KEY_4);` → `let $KEY_4 = f(); use($KEY_4);`. Stages `11l2-local-vars.js`, `13b-` |
| [`key_declarations.rs`](key_declarations.rs) | Declares each `$KEY_` once, as a `let` in the innermost block holding every use, before the first using statement (or as that store: `let x = v;`); never inside a loop the uses share. Stage `13b-tidy-expressions.js` |
| [`unused_keys.rs`](unused_keys.rs) | Whole program: removes a `$KEY_` slot with no read anywhere, with its quiet initializer and statement stores. `finish_program`, before dump `13d-positional-names.js` |
| [`dead_locals.rs`](dead_locals.rs) | Removes thread-local stores and values nothing observes, including dead copy cycles; a throwing RHS stays. Every thread phase except `reprint` |
| [`declaration_lifetimes.rs`](declaration_lifetimes.rs) | `tighten`: `let x; work(); x = v;` → `work(); let x = v;`, and branch channels move next to their join. `tighten_allocations`: inert fresh literals move to first use. Thread `late_propagation`, `literal_relocation` |
| [`private_array_reads.rs`](private_array_reads.rs) | `let $SSA_key = $ARRAY_a[i]; use($SSA_key, $SSA_key)` → `use($ARRAY_a[i], $ARRAY_a[i])` while the cell is unchanged, for a never-reassigned, non-escaping dense-literal or `slice()` copy; `.length` snapshots too. Thread `storage_sweep`, `late_propagation` |
| [`unread_declarations.rs`](unread_declarations.rs) | The standalone pass `cleanup`/`cleanup_with_options`/`cleanup_emitted_javascript[_with_options]`: unread declarators with pure, non-throwing initializers go. Also holds the public `EmittedCleanupOptions`, `EmittedCleanupReport`, `EmittedCleanupResult`, `CleanedEmittedJavaScript`, `RemovedRange`, `StatementMapStatus` and `EmittedCleanupError` re-exported from the module root |
| [`fresh_values/`](fresh_values/README.md) | `inline_fresh_values`: single-use allocations and inert computations move to their use. Thread only |
| [`last_use_storage/`](last_use_storage/README.md) | `recycle_last_inputs`, `recycle_ended_cells`: a later value reuses a cell whose old value has died. Thread only |
| [`mod.rs`](mod.rs) | Module declarations |

## Contracts

- Deleting a store never deletes an effect: `dead_key` and `local_vars` keep a
  RHS that can run code as a statement; `dead_locals` keeps throwing RHS
  evaluations. `unused_keys` removes a slot only when its initializer and every
  store run no code and each store is a statement of its own; any other write
  keeps the slot.
- `unused_keys` runs on the linked program, where a child thread's reads resolve
  to its parent's slot, so "no read" means no read anywhere. It deliberately
  leaves `$THREAD_` functions the deleted stores named: a thread nothing calls
  is for the tracer to explain, not for cleanup to hide.
- `local_vars` assumes no getters (a property read of a literal's name cannot
  throw); a missing global or a property read of a maybe-null value counts as
  throwing.
- `key_declarations` relies on each `$KEY_` being one binding per call of its
  function: the `let` never lands inside a loop the uses share, and goes first
  in its block when a nested function using it could run before the store.
- `redundant_initializers` needs only `let x;` producing `undefined`: no
  liveness or dynamic-scope analysis; the target is resolved by symbol.
- `unread_declarations` returns the input unchanged on `with`/`eval`, keeps
  destructuring, parameter-aliased `var`s and program-level declarations, and
  reports whether statement maps are preserved, invalidated or regenerated.
- Thread passes never rewrite a protected (`shared_slots`) name. An empty
  `$THREAD_` child marker makes `declaration_lifetimes`, `private_array_reads`
  and `last_use_storage` decline, and limits `fresh_values` to private
  temporaries, unless the tracer's checked-capture contract
  (`cleanup_emitted_thread`) is in force.
- Order in the thread driver: storage reuse (`last_use_storage`) runs after
  expression and descriptor reconstruction and after snapshots have claimed
  their storage; reusing cells earlier hides producer provenance.

[Emitted JavaScript cleanup](../README.md) ·
[pipeline table](../../../../docs/transforms/emitted-program-cleanup.md#pipeline) ·
[declaration cleanup reference](../../../../docs/transforms/emitted-js-cleanup.md)
