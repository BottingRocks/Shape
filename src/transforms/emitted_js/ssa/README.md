# SSA forwarding and folding

Passes that remove the generated temporaries the tracer materializes
(`$SSA_…`, `$CARRY_…` copies, `$KEY_n` slot snapshots) by substituting,
forwarding or merging their values. Some run in the per-function stages of
[`pipeline/stages.rs`](../pipeline/stages.rs) (`ssa_and_carries` `02`–`09`,
`vm_helpers` `11b4`, `dead_code_and_loops` `11c`–`11c5`, `helper_expansion`
`11h0`, `forwarding` `11i`–`11p1`); the rest run only under `--thread-cleanup`
(`cleanup_thread_impl` in [`pipeline/thread.rs`](../pipeline/thread.rs)).
No whole-program stage (`12-…13e-`) uses this folder.

## Files

| File | What it does |
| --- | --- |
| [`mod.rs`](mod.rs) | Declares the passes below |
| [`literal_ssa.rs`](literal_ssa.rs) | `let $SSA_n = 3;` → `3` at each read; primitive literals only, text copied so escapes and `-0` survive. Stage `04`; thread `expression_sweep`, `late_propagation` |
| [`alias_ssa.rs`](alias_ssa.rs) | `let $SSA_n = x;` → reads of `x` while `x` is stable at every read. Stages `08`, `11b4`; thread `expression_sweep`, `late_propagation` |
| [`dead_ssa.rs`](dead_ssa.rs) | Unread `$SSA_` declarations and tuple slots, bare `$KEY_n;`/`$CARRY_n;` reads, discarded `arguments[n];` reads. Stages `11c`, `11c4`, `11c5`, `11l`; thread `expression_sweep`, `late_propagation` |
| [`adjacent_ssa/`](adjacent_ssa/README.md) | A single-use temporary into the statement right after it. Stages `02`, `09`, `11b4`; thread mode in four phases |
| [`forward_values/`](forward_values/README.md) | The late forwarding engine: stable values, next-door moves, slot snapshots and stores. Stages `11h0`, `11i`, `11k`, `11k3`, `11m`, `11o`, `11p1` |
| [`merge_derived/`](merge_derived/README.md) | `let r = f(L);` → `L = f(L);` when `L`'s old value is dead; and the reverse split. Stages `11m`, `11o`, `11p1` |
| [`snapshot_ssa.rs`](snapshot_ssa.rs) | Moves a computed `$SSA_` value while the versions of its lexical inputs are unchanged; calls and control flow end a region. Thread `expression_sweep`, `late_propagation` |
| [`snapshot_copies.rs`](snapshot_copies.rs) | Shares duplicate snapshots of the same lexical input within an effect-free region. Thread `expression_sweep` |
| [`slot_forward.rs`](slot_forward.rs) | `let $SSA = v; $KEY = $SSA` → `$KEY = v`, later SSA reads read the slot while it is unchanged. Thread `expression_sweep` |
| [`local_values.rs`](local_values.rs) | Local value numbering: substitutes pure definitions at dominated, owned reads and fuses an identical pure consumer into its definition. Thread `expression_sweep`, `late_propagation` |
| [`argument_snapshots.rs`](argument_snapshots.rs) | Rematerializes numeric `arguments[n]` reads of an unchanged, unescaped arguments object. Thread `storage_sweep`; its `input` test is shared with `facts/`, `carries/` and `storage/` |
| [`constant_values.rs`](constant_values.rs) | Folds literal-only primitive computations (Oxc arithmetic, local no-getter/no-throw check). Thread `expression_sweep`, `recycling` |

## Contracts

- Candidates are selected by generated prefix, but every rewrite resolves
  symbols: each substituted name must re-resolve to the same binding at the
  read, reads stay in the declaring function, and shorthand properties and
  `delete` of the binding decline.
- A value is copied to several reads only when it cannot change between them
  (literal, never-written local, stable slot); an effectful value moves only
  to a single read that is the first thing its statement evaluates.
  Allocations are never copied.
- A direct-call callee becomes `(0, value)` to keep bare-call `this`.
- `with` or a reference to `eval` disables every pass that resolves bindings
  (all but `constant_values`, which folds literal-only operands). A comment in
  a removed declaration declines it; `adjacent_ssa` lets a comment inside the
  moved value travel with it.
- Overlapping edits are errors, not silent skips. Most passes also reparse
  their output; `local_values`, `constant_values` and `snapshot_copies` leave
  that to the next parse.
- Thread-mode passes take a `protected` set (`$KEY_` slots shared with a
  creator or pending children) and never treat those slots as private.
- `dead_ssa::readonly_arguments` is the shared test for a function whose
  `arguments` is only read at literal indexes; `forward_values`,
  `argument_snapshots`, `facts/cell_facts.rs`, `carries/stable_channels/`,
  `branches/flatten_exits.rs` and `storage/fresh_values/` rely on it. Changing
  it changes all of them.
- Ordering: `alias_ssa` exposes adjacent pairs, so `09` and `11b4` repeat
  `inline_adjacent_ssa` after it (bounded ×4). `forward_values` and
  `merge_derived` feed each other in `11m`–`11p1`; property-read forwarding
  runs before merging, which would change objects under copied reads.

[Emitted JavaScript cleanup](../README.md) ·
[Pipeline table](../../../../docs/transforms/emitted-program-cleanup.md#pipeline) ·
[Thread cleanup](../../../../docs/transforms/thread-cleanup.md)
