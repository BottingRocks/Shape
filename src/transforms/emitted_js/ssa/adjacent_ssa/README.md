# Adjacent SSA inlining

Inlines a single-use generated temporary into the statement right after it:
`let $SSA_n = f(); return $SSA_n;` → `return f();`. `inline_adjacent_ssa`
runs per function at `02-adjacent-ssa.js`, `09-adjacent-after-alias.js`
(`ssa_and_carries`) and `11b4-ssa-after-tuples.js` (`vm_helpers`), each
repeated up to four times. `inline_thread_adjacent_ssa` is the
`--thread-cleanup` mode, called in `expression_sweep`, `storage_sweep`,
`late_propagation` and `literal_relocation`.

## Files

| File | What it does |
| --- | --- |
| [`mod.rs`](mod.rs) | Entry points and the two policies: plain, and thread mode (reaching-store facts from `LocalReads`, `cell_facts`, `cell_liveness`, `protected` slots); applies the edits and reparses |
| [`scan.rs`](scan.rs) | The scan over statement pairs and comma sequences: which store is inlined, its first use, scope fit, version end, single-read proof, and the `(0, value)` replacement |
| [`prefixes.rs`](prefixes.rs) | What the use evaluates before the read: harmless prefixes, `first_evaluated_identifier`, an argument after an immutable callee, numeric/primitive input proofs, and `prefix_read` |
| [`local_reads.rs`](local_reads.rs) | `LocalReads`: per statement list, which owned locals are initialized, private to their owner, or of a proved primitive/numeric category, by version. Shared with `alias_ssa`, `local_values`, `argument_snapshots`, `facts/cell_facts.rs` and passes in `carries/`, `branches/`, `loops/`, `literals/`, `storage/` |
| [`visit.rs`](visit.rs) | The walk that feeds the scan (comma sequences, functions, blocks, `with`) and helper visitors for inserted-scope resolution and direct-call callees |
| [`tests.rs`](tests.rs) | Handoffs to return/throw/slot/branch carry, callee and receiver forms, getters and TDZ, `eval` |
| [`once_consumer_tests.rs`](once_consumer_tests.rs) | Unconditional first-use consumers; loop shadowing; comma snapshots keep GetValue for calls, tags and `typeof` |
| [`point_prefix_tests.rs`](point_prefix_tests.rs) | A successful current store proving an ordered numeric prefix |
| [`branch_version_tests.rs`](branch_version_tests.rs) | Consuming one stored version at a branch's last use; readonly-argument prefixes; comments outside the value |

## Contracts

- The temporary has exactly one read and no writes; in thread mode, one read
  among the references that can observe that stored version, so a cell can
  give up one version and keep later ones.
- The next statement must evaluate the read first (return/throw, `if` test,
  lexical slot write, single-variable initializer, immutable helper argument),
  or after a prefix proved harmless: no throw, no user code, and every binding
  it reads is stable across the move.
- Later call arguments are accepted only when the moved value cannot invoke a
  getter or change what the crossed initialized local reads observe.
- Names in the moved initializer must resolve to the same bindings at the use
  (a new `for`-`let` environment can shadow them).
- A direct-call callee or `typeof` operand becomes `(0, value)`; computed
  assignment keys keep their coercion after the right-hand side.
- `with`/`eval` disable the pass. Edits that overlap or do not all apply are
  an error. The caller repeats it to a bounded fixpoint.
- `alias_ssa` exposes new adjacent pairs; that is why `09` and `11b4` follow it.

[Emitted JavaScript cleanup](../../README.md) ·
[Pipeline table](../../../../../docs/transforms/emitted-program-cleanup.md#pipeline) ·
[SSA folder](../README.md)
