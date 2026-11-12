# Last-use storage

Recycles a private generated cell once its old value dies: a later `$SSA_`/
`$CARRY_` value is stored into that cell instead of a new binding.
`let $SSA_old = read(); let $SSA_next = $SSA_old + 1; use($SSA_next, $SSA_next);`
→ `let $SSA_old = read(); $SSA_old = ($SSA_old+1); use($SSA_old, $SSA_old);`
(a `const` lender becomes `let`). Computations stay in
place and run once; no name is invented. Runs only in the thread cleanup driver
(`cleanup_thread_impl` in [`pipeline/thread.rs`](../../pipeline/thread.rs), used
by `trace program --thread-cleanup`): `recycle_last_inputs` in `branch_cells`
(a cell whose last use is the new value's RHS) and `recycle_ended_cells` in
`recycling` (also unrelated cells whose live range already ended).

## Files

| File | What it does |
| --- | --- |
| [`mod.rs`](mod.rs) | The two entry points (`ended` selects the mode), the `with`/`eval`/pending-child gate, and edit application |
| [`scan.rs`](scan.rs) | The scan for values whose storage another binding can reuse: statement lists, `for` headers, branch successors, grouped `let a, b;` declarators, ended candidates, and the edit that moves creation into the lender |
| [`successors.rs`](successors.rs) | Proof that no later use sees the reused storage: per-path availability (`SuccessorState`), truth-split conditions, loop entry and backedges (`LoopEdges`), copy-backs, invariant regions |
| [`declarations.rs`](declarations.rs) | Source declarations that may lend storage, later writes that are safe, recycled inputs |
| [`tests.rs`](tests.rs) | Positive and decline cases across `for` headers, loops, branches, `try`, copy-backs and ended live ranges |

## Contracts

- Only generated `$SSA_`/`$CARRY_` storage is recycled, never an application
  local, and only statement-list declarations lend storage: a `for`-header
  update runs after its body although it is textually earlier.
- Every old reference must finish before reuse, on every path reaching a read;
  branch-merged successors must initialize on each such path. Captured and
  shared (`shared_slots`) cells, and simultaneously live unequal versions, stay
  separate.
- Loop headers are proved against first entry and every backedge; `break`
  exits and `continue` edges keep separate states. Opaque regions with
  unmodelled jumps (nested-loop or `switch` breaks) decline.
- Logical `&&`/`||`, negation, sequences and conditionals keep separate truth
  states, so a copy on one edge does not prove the other.
- A write inside a structured region invalidates the other saved version,
  including on zero-iteration paths. A copy-back restores equality only for the
  value it copies.
- `let next;` keeps its undefined initialization when it borrows an ended
  cell (`input = void 0;`).
  Const-write errors, TDZ, captured iteration cells, and function/class name
  inference from an assignment target remain barriers.
- A `for`-header cell may borrow ended storage only when its initializer runs
  once per activation and every old observation precedes the parent loop's
  entry.
- Order: runs after snapshots and producer reconstruction have settled, so
  reuse does not hide array or descriptor provenance from earlier passes.
- An empty `$THREAD_` child marker makes the pass decline unless the tracer's
  checked-capture contract is in force.

[Emitted JavaScript cleanup](../../README.md) ·
[thread cleanup](../../../../../docs/transforms/thread-cleanup.md)
