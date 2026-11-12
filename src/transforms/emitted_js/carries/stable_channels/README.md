# Stable channels

`eliminate_stable_channels` removes `$SSA_`/`$CARRY_` forwarding channels whose
every read sees a copy of the current input: an `arguments[n]` read, a local
binding, or an incoming `$KEY_` slot. Reads become the input and the channel
goes:

```js
let $CARRY_0; if (arguments[0]) { $CARRY_0 = arguments[1]; left(); }
else { right(); $CARRY_0 = arguments[1]; } return $CARRY_0;
// -> … return arguments[1];
```

Runs only in the thread cleanup driver (`cleanup_thread_impl` in
[`pipeline/thread.rs`](../../pipeline/thread.rs), used by
`trace program --thread-cleanup`), once per `late_propagation` round. It
returns the code, the argument channels removed and the alias channels removed.

## Files

| File | What it does |
| --- | --- |
| [`mod.rs`](mod.rs) | Entry point, `Input` (local, incoming slot, argument), the per-channel proof (`Channels::channel`, `declaration`, `branch_copy`) and edit assembly |
| [`facts.rs`](facts.rs) | Branch copies and stores, per-list facts (`Facts`), branch leaves of a copy-out, and `immutable_producer` for a delayed copy |
| [`flow.rs`](flow.rs) | The path walk (`Flow`): which values reach a read through branches, short-circuit operators, loops, `try`/`catch`/`finally` and abrupt exits |
| [`tests.rs`](tests.rs) | Argument and alias channels, loop-test refreshes, incoming-slot reads, const errors, copy-outs |
| [`exceptional_flow_tests.rs`](exceptional_flow_tests.rs) | Normal and abrupt control: aliases cross only with reaching equality; partial exception entries and `finally` overrides keep snapshots |

## Contracts

- A read is replaced only when every reaching normal path has copied the
  current input. A store to the input invalidates equality after its RHS is
  evaluated; a destructuring store, `var` redeclaration or a closure that
  writes the input also invalidate it.
- Loops: equality must hold on entry and every backedge. A loop test that
  re-copies the input before any channel read proves the exit value too.
- `try`: a finalizer must be safe on abrupt entries, so equality established
  only in the `try`'s normal suffix does not reach it.
- Only `let`/`const` `$SSA_`/`$CARRY_` channels that are not redeclared or
  protected are candidates. Retained declarators keep their const contract
  (assignment to a const still throws).
- A delayed copy keeps its destination write at the original point; only an
  inert producer whose lexical inputs cannot change may move there. No new slot
  or name is introduced.
- Argument reads rely on the readonly-`arguments` proof in `ssa/dead_ssa.rs` and
  `ssa/argument_snapshots.rs`; cell facts come from `facts/cell_facts.rs`.
- `with`/`eval` returns the input unchanged; an empty `$THREAD_` child marker
  declines unless the tracer's checked-capture contract is in force.

[Emitted JavaScript cleanup](../../README.md) ·
[thread cleanup](../../../../../docs/transforms/thread-cleanup.md)
