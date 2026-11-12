# Cleanup drivers

This folder holds no passes of its own: it fixes the order the other families
run in. [`stages.rs`](stages.rs) is the stage order of `shape trace program`:
`clean_function` runs each function's stages (dumps `01-…13b-`) on its tracing
worker before linking, and `finish_program` the whole-program stages
(`12-…13e-`) after. [`thread.rs`](thread.rs) is the `--thread-cleanup` and
`shape trace cleanup-thread` driver. The command driver in
[`program.rs`](../../../../cli/trace/commands/program.rs) traces, links and calls
both.

## Files

| File | What it does |
| --- | --- |
| [`mod.rs`](mod.rs) | Declares `stages` and `thread` |
| [`stages.rs`](stages.rs) | `clean_function`, `finish_program`, the `stage()` helper (a pass result's code, or its error prefixed with a context), the `PassOutput` trait (takes the code out of `String`, `(String, usize)`, `(String, usize, usize)` and results with a `code` field), and the `Dump` callback that writes `--dump-stages` files |
| [`thread.rs`](thread.rs) | `cleanup_thread`, `cleanup_thread_with_options`, `cleanup_emitted_thread` and their shared `cleanup_thread_impl`; `ThreadCleanup` (code and counts) and `ThreadCleanupOptions` (shared slots) |

## `clean_function` phases

| Phase | Dumps | Covers |
| --- | --- | --- |
| `ssa_and_carries` | `01`–`09` | Redundant initializers, adjacent SSA ×4, literal branch joins, literal SSA, readable property writes (`06`), carry joins, coalescing ×4, self-assignments, dead carries ×4, branch values ×4, alias SSA, adjacent SSA again ×4 |
| `vm_helpers` | `11`–`11b5` | VM subroutine parameters, shared helpers, simplified bodies, wrapper inlining ×4, tuple scalarizing, SSA after tuples, fresh tuple unpacking |
| `dead_code_and_loops` | `11c`–`11h` | Dead SSA and unread `$KEY_` stores (then a ×4 dead-code loop with no-op reads and dead carries), argument reads, empty `else`, loop reconstruction, counter lifting, literal fills, early loop-test splitting, single-use literals, loop/catch names (`11g`), `host["Name"]` unwrapping |
| `helper_expansion` | `11h0`–`11h5` | Helper bodies forwarded and resugared; hoist/small/tuple expansion alternated ×12; string folding; literal fills and nesting; a second counter-lifting and loop-name round (`11h4`); `tidy_expressions` |
| `forwarding` | `11i`–`11p` | `forward_and_fold` (forwarding, resugaring, branch values; then ×3 of `forwarded_round` ×8 plus `dead_after_forwarding`), `merge_and_split` (local vars, derived-value merging, single-read splitting), `late_helpers` (late small helpers, then remaining helpers expanded exactly, forwarded and merged) |
| `final_tidy` | `13b` | String folding, local vars, literal nesting, resugaring, counter copies, optional chains, `tidy_final_expressions`, branch tails, native `for…in`, key declaration placement, loop labels, trailing returns |

`finish_program` then runs, in one sequence: short generated names (`12`),
role names (`13c`), classes (`13c2`), arrows (`13c3`), thread parameters
(`13c4`), VM values, constant slots, unused keys and positional names (`13d`),
then prototype methods, path temporaries, array fills, bit accumulators,
copied counters, negated comparisons, empty starts, adjacent literals, let runs
and dotted properties (`13e`).

## `cleanup_thread_impl` phases

The input must be exactly one function declaration. `vm_sub_*` and runtime
helpers are expanded first, then seven phases run in order:

| Phase | Covers |
| --- | --- |
| `expression_sweep` | Fixed point (×24): null flags, branch facts, join channels, point-literal keys, constants, dense arrays, carries, branch values, tuples, literal/snapshot/alias/adjacent SSA, descriptor literals and property writes (only once those relays settle), method calls, local values, dead locals, numeric coercions, slot forwarding; then structured loops |
| `storage_sweep` | Fixed point (×24): tail joins, local handoffs, private array reads, fresh values, adjacent SSA, dead locals; then numeric coercions, block flattening, argument snapshots |
| `late_propagation` | Fixed point (×24): declaration lifetimes, stable channels, nullish syntax, SSA inlining, handoffs, fresh values, tail joins over the flattened continuations; then index counters |
| `branch_cells` | Last-input recycling, branch handoffs, finished private channels (×24) |
| `literal_relocation` | Allocation tightening, dense arrays, fresh values (×4) |
| `recycling` | Ended-cell recycling, point literals, constants, branch facts (×24) |
| `reprint` | Empty statements and `else` blocks removed; the AST printed once and arrays compacted |

## Contracts

- Every repeat is bounded and stops early when a round changes nothing; the
  bound is the round count shown above or in the code.
- Stage numbers name dumps, not positions; gaps (`05`, `10`, …) are historical.
  A new stage gets a new dump name; existing names stay stable for comparisons.
- In `clean_function`, only redundant initializers, the first adjacent-SSA
  repeat and literal branch joins log and keep the input on error; every other
  stage error fails the function.
- `collect_key_names` protects every `$KEY_` name from being lifted into a loop
  header by `reconstruct_loops`.
- `dotted_properties` stays last in `finish_program`: earlier passes recognise
  code by its bracketed spelling.
- The thread driver keeps SSA structure until expression and descriptor
  reconstruction finish, and recycles storage only after producer
  reconstruction: earlier reuse hides the provenance those proofs need.
- `cleanup_thread` protects every `$KEY_` binding when the input holds an empty
  `$THREAD_` child marker. Only `cleanup_emitted_thread` (the program tracer,
  with validated facts) sets `checked_captures`.

[Emitted JavaScript cleanup](../README.md) ·
[pipeline table](../../../../docs/transforms/emitted-program-cleanup.md#pipeline) ·
[thread cleanup](../../../../docs/transforms/thread-cleanup.md)
