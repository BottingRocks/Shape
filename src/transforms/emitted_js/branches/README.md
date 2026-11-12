# Branch joins, exits and blocks

These passes fold branch joins (carries assigned in each arm) into single
values, flatten `else` after arms that leave, move shared arm tails out, and
drop empty blocks and statements. Some run in `clean_function` (stages `03`,
`07d`, `11d`, `11j`, the `11k3` forward loop and `13b`); the rest run only in
the `--thread-cleanup` driver (`cleanup_thread_impl`).

## Files

| File | What it does |
| --- | --- |
| [`mod.rs`](mod.rs) | Declares the modules. |
| [`literal_branch_join.rs`](literal_branch_join.rs) | `let $SSA_n; if (t) { $SSA_n = 1; } else { $SSA_n = "a"; }` → `const $SSA_n = t ? 1 : "a";` for literal arms and an identifier test. Stage `03-literal-branch-joins`; thread `expression_sweep`. |
| [`branch_value.rs`](branch_value.rs) | Assignment-only carry diamonds (nested included) → `const $MERGE_x = test ? a : b`. `fold_branch_values` runs at `07d` (×4), `11j` and in the `11k3` loop; `fold_multi_branch_values` (several carries per leaf) and `fold_plain_branch_assignments` (a diamond storing a carry written elsewhere) in the `11k3` loop; `fold_thread_branch_values` and `fold_branch_assignments` (keeps the carry name, accepts comma prefixes) in the thread driver. |
| [`branch_tails.rs`](branch_tails.rs) | `hoist_branch_tails`: a statement every arm of an `if`/`else if`/`else` chain ends with (same text, same bindings) moves after the chain. `11k3` loop and `13b`. |
| [`flatten_exits.rs`](flatten_exits.rs) | `if (t) { …; return x; } else { REST }` → `if (t) { …; return x; } REST`; splices bare blocks; sinks `let $X;` into its first plain store; sets a settled carry value once after an `if` tree. `11k3` loop. |
| [`empty_else.rs`](empty_else.rs) | Drops empty `else {}` arms left by carry cleanup. Stage `11d-empty-else`; thread `late_propagation` and `reprint`. |
| [`return_joins.rs`](return_joins.rs) | Removes private join storage read only by the next consumer (a return, expression statement or generated initializer), sinking it into each fallthrough branch. Thread `storage_sweep` and `late_propagation`. |
| [`branch_handoffs.rs`](branch_handoffs.rs) | Stores an immutable snapshot directly in its private copy-out cell when no branch can observe either cell's old value. Thread `branch_cells`. |
| [`conditional_values.rs`](conditional_values.rs) | `value ? next : value` → `value && next` for resolved lexical reads. Thread `expression_sweep`. |
| [`blocks.rs`](blocks.rs) | Flattens standalone statement-list blocks when every reference keeps its declaration; control-flow bodies and labeled blocks stay. Thread `storage_sweep` and `recycling`. |
| [`empty_statements.rs`](empty_statements.rs) | Removes cleanup placeholder `;` from statement lists, keeping real empty bodies and the barrier that stops a string from becoming a directive. Thread `reprint`. |

## Contracts

- A join folds only when no read sees the carry before the replaced span:
  the old code could observe its `undefined`/TDZ value. Every write must
  resolve to the carry, the arms must hold only the assignments (or, in the
  thread variants, comma-able prefixes), and the test runs first and once.
- `fold_multi_branch_values` tests each carry separately, so nested tests must
  be repeatable (unwritten names, literals, `!`, `typeof`, `void`, `===`,
  `== null`); a non-repeatable top test is saved in a fresh `$SSA_t…` first.
- Captured cells and function/class right-hand sides decline in the thread
  variants: their previous value or inferred `name` can be observed.
- `flatten_exits` moves the spliced arm's `let`/`const` to the enclosing
  block, so it declines for a non-`$` name, or a function or class declared
  in the arm. The settled-carry store needs no other reference to the carry
  and no `try` around the tree.
- `hoist_branch_tails` keeps the first arm's order, never reorders two
  dependent statements, and moves a throwing statement past local-only writes
  only when no `try` in the function can see it. Arms that jump after the
  statement keep it.
- `blocks.rs` blanks braces to spaces during its proof so spans stay put;
  directives and reference-to-declaration identity must survive.
- Regions holding comments decline in `branch_value.rs` and
  `literal_branch_join.rs`; `empty_else.rs` keeps comments and dangling-else
  barriers.
- `branch_value.rs`, `flatten_exits.rs`, `branch_tails.rs` and the
  forwarding pass feed each other: the `11k3` round in
  `pipeline/stages.rs` (`forwarded_round`) repeats them (×8) until none
  changes anything.

[Emitted JavaScript cleanup](../README.md) ·
[pipeline table](../../../../docs/transforms/emitted-program-cleanup.md#pipeline)
