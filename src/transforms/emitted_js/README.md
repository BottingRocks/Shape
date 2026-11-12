# Emitted JavaScript cleanup

These passes simplify JavaScript produced by the tracer. They parse source with
Oxc, prove a narrow generated shape, apply checked byte-span edits, and reparse
the result. They do not run captured code. Generated prefixes select candidates;
each semantic rewrite still needs its own binding and evaluation-order proof.

The [program cleanup reference](../../../docs/transforms/emitted-program-cleanup.md) owns
stage order, scalar/carry/branch passes, reporting, and source coordinates.
[Helper and tuple cleanup](../../../docs/transforms/emitted-helper-cleanup.md) owns linked
helper rewrites. The separate [declaration](../../../docs/transforms/emitted-js-cleanup.md)
and [property-write](../../../docs/transforms/emitted-property-writes.md) references define
those public contracts.

## Entry points and command integration

| Path | Cleanup |
| --- | --- |
| `cleanup`, `cleanup_with_options` | Standalone removal of unread, pure, non-throwing declarations |
| `cleanup_emitted_javascript` | Declaration cleanup with regenerated provenance maps |
| `shape trace thread` | String folding, strict property writes, fresh-array folding |
| `shape trace corpus --cleanup` | The same post-emission sequence |
| `shape trace program` | Per-function cleanup, linking, then ordered program passes |
| `cleanup_thread`, `shape trace cleanup-thread` | Function-local helper expansion and SSA/snapshot cleanup; program workers use it with `--thread-cleanup` |

The [individual thread cleanup guide](../../../docs/transforms/thread-cleanup.md) describes
dependency versions, mutation barriers, and the independent replay checks.

The standalone declaration pass is not automatically part of the program
stages. [`pipeline/stages.rs`](pipeline/stages.rs) owns stage order and
bounded repeats: `clean_function` runs each function's stages (`01-…13b-`, in
six phases) on its walk thread before linking, and `finish_program` the whole
program's (`12-…13e-`) after; the driver in
[`program.rs`](../../../cli/trace/commands/program.rs) traces, links and calls
them. The `--thread-cleanup` driver is `cleanup_thread_impl` in
[`pipeline/thread.rs`](pipeline/thread.rs). Use `--dump-stages DIR` to locate the
first changed JavaScript stage; the [pipeline table](../../../docs/transforms/emitted-program-cleanup.md#pipeline)
maps every dump to a pass.

## Contracts to preserve

Preserve evaluation order, throws, call receivers, binding identity, and object
observability. A same-spelled identifier in another scope is a different symbol.
Dynamic scope, unsafe movement, unsupported source shapes, and overlapping edits
must decline or fail according to the caller's contract. Copy literal/RHS text
so escapes, lone surrogates, and signed zero survive unchanged.

Strict property rewrites require an exact descriptor and a proved fresh,
unobserved receiver. Program output also has a readability-oriented property
stage with explicit ordinary-receiver and primitive-key assumptions. Fresh tuple
unpacking assumes the standard Array iterator; discarded argument reads have
ordinary-built-in assumptions. The late whole-program passes take built-ins and
the host globals not to change (constant slots, prototype methods), property
reads in the fingerprint probes to run no code (path temporaries), and
`.length` to be a number (negated comparisons); each `//!` module doc states
its own. These are specific pass boundaries, not a blanket
claim that every readable rewrite works for arbitrary JavaScript.

Deleted bindings must leave effectful initializers and RHSs at the same point.
Inlining must preserve bare-call `this` and must not duplicate a call or getter.
Comments need per-pass handling. Repeated cleanup is bounded, usually to four
rounds, and map status must be preserved, regenerated, or explicitly invalidated.

## File map

`mod.rs` holds the public entry points and re-exports; every pass lives in a
folder by family, and each folder has its own `README.md` describing its files.

| Folder | What | Files |
| --- | --- | --- |
| [`pipeline/`](pipeline/README.md) | the cleanup drivers and the stage order | [`stages.rs`](pipeline/stages.rs), [`thread.rs`](pipeline/thread.rs) |
| [`support/`](support/README.md) | shared helpers: edits, owners, statement lists, syntax, purity, primitive types, indexes | [`edits.rs`](support/edits.rs), [`index.rs`](support/index.rs), [`lists.rs`](support/lists.rs), [`owners.rs`](support/owners.rs), [`primitives.rs`](support/primitives.rs), [`purity.rs`](support/purity.rs), [`syntax.rs`](support/syntax.rs) |
| [`facts/`](facts/README.md) | facts passes consult: cell values, liveness, branch facts | [`branch_facts.rs`](facts/branch_facts.rs), [`cell_facts.rs`](facts/cell_facts.rs), [`cell_liveness.rs`](facts/cell_liveness.rs) |
| [`ssa/`](ssa/README.md) | forwarding and folding generated values | [`alias_ssa.rs`](ssa/alias_ssa.rs), [`argument_snapshots.rs`](ssa/argument_snapshots.rs), [`constant_values.rs`](ssa/constant_values.rs), [`dead_ssa.rs`](ssa/dead_ssa.rs), [`literal_ssa.rs`](ssa/literal_ssa.rs), [`local_values.rs`](ssa/local_values.rs), [`slot_forward.rs`](ssa/slot_forward.rs), [`snapshot_copies.rs`](ssa/snapshot_copies.rs), [`snapshot_ssa.rs`](ssa/snapshot_ssa.rs), [`adjacent_ssa/`](ssa/adjacent_ssa/README.md), [`forward_values/`](ssa/forward_values/README.md), [`merge_derived/`](ssa/merge_derived/README.md) |
| [`carries/`](carries/README.md) | loop and branch carries | [`carry_coalesce.rs`](carries/carry_coalesce.rs), [`carry_join.rs`](carries/carry_join.rs), [`carry_self_assign.rs`](carries/carry_self_assign.rs), [`carry_semantics_tests.rs`](carries/carry_semantics_tests.rs), [`dead_carry.rs`](carries/dead_carry.rs), [`equivalent_carry.rs`](carries/equivalent_carry.rs), [`join_channels.rs`](carries/join_channels.rs), [`carry_lifetimes/`](carries/carry_lifetimes/README.md), [`stable_channels/`](carries/stable_channels/README.md) |
| [`branches/`](branches/README.md) | branch joins, exits and blocks | [`blocks.rs`](branches/blocks.rs), [`branch_handoffs.rs`](branches/branch_handoffs.rs), [`branch_tails.rs`](branches/branch_tails.rs), [`branch_value.rs`](branches/branch_value.rs), [`conditional_values.rs`](branches/conditional_values.rs), [`empty_else.rs`](branches/empty_else.rs), [`empty_statements.rs`](branches/empty_statements.rs), [`flatten_exits.rs`](branches/flatten_exits.rs), [`literal_branch_join.rs`](branches/literal_branch_join.rs), [`return_joins.rs`](branches/return_joins.rs) |
| [`loops/`](loops/README.md) | loop reconstruction, counters and labels | [`copied_counters.rs`](loops/copied_counters.rs), [`index_counters.rs`](loops/index_counters.rs), [`loop_counter_copies.rs`](loops/loop_counter_copies.rs), [`loop_counters.rs`](loops/loop_counters.rs), [`loop_labels.rs`](loops/loop_labels.rs), [`loop_tests.rs`](loops/loop_tests.rs), [`native_for_in.rs`](loops/native_for_in.rs), [`structured_loops.rs`](loops/structured_loops.rs) |
| [`helpers/`](helpers/README.md) | VM helper and tuple inlining | [`expand_vm_sub.rs`](helpers/expand_vm_sub.rs), [`inline_tuple_helpers.rs`](helpers/inline_tuple_helpers.rs), [`inline_vm_sub.rs`](helpers/inline_vm_sub.rs), [`runtime_helpers.rs`](helpers/runtime_helpers.rs), [`tuple_scalarize.rs`](helpers/tuple_scalarize.rs), [`tuple_unpack.rs`](helpers/tuple_unpack.rs), [`vm_sub_dedupe.rs`](helpers/vm_sub_dedupe.rs), [`vm_sub_parameters.rs`](helpers/vm_sub_parameters.rs), [`vm_sub_simplify.rs`](helpers/vm_sub_simplify.rs), [`inline_small_helpers/`](helpers/inline_small_helpers/README.md) |
| [`storage/`](storage/README.md) | declarations, slots and their storage | [`dead_key.rs`](storage/dead_key.rs), [`dead_locals.rs`](storage/dead_locals.rs), [`declaration_lifetimes.rs`](storage/declaration_lifetimes.rs), [`key_declarations.rs`](storage/key_declarations.rs), [`local_vars.rs`](storage/local_vars.rs), [`private_array_reads.rs`](storage/private_array_reads.rs), [`redundant_initializers.rs`](storage/redundant_initializers.rs), [`unread_declarations.rs`](storage/unread_declarations.rs), [`unused_keys.rs`](storage/unused_keys.rs), [`fresh_values/`](storage/fresh_values/README.md), [`last_use_storage/`](storage/last_use_storage/README.md) |
| [`literals/`](literals/README.md) | array and object literals, property writes | [`adjacent_literals.rs`](literals/adjacent_literals.rs), [`array_fills.rs`](literals/array_fills.rs), [`array_format.rs`](literals/array_format.rs), [`array_initializers.rs`](literals/array_initializers.rs), [`dense_arrays.rs`](literals/dense_arrays.rs), [`fold_literal_fills.rs`](literals/fold_literal_fills.rs), [`inline_single_use_literals.rs`](literals/inline_single_use_literals.rs), [`nest_literals.rs`](literals/nest_literals.rs), [`point_literals.rs`](literals/point_literals.rs), [`property_writes/`](literals/property_writes/README.md) |
| [`expressions/`](expressions/README.md) | expression tidying | [`bit_accumulators.rs`](expressions/bit_accumulators.rs), [`constant_slots.rs`](expressions/constant_slots.rs), [`discarded_values.rs`](expressions/discarded_values.rs), [`empty_starts.rs`](expressions/empty_starts.rs), [`let_runs.rs`](expressions/let_runs.rs), [`method_calls.rs`](expressions/method_calls.rs), [`negated_comparisons.rs`](expressions/negated_comparisons.rs), [`null_flags.rs`](expressions/null_flags.rs), [`nullish_syntax.rs`](expressions/nullish_syntax.rs), [`numeric_coercions.rs`](expressions/numeric_coercions.rs), [`optional_chains.rs`](expressions/optional_chains.rs), [`path_temporaries.rs`](expressions/path_temporaries.rs), [`prototype_methods.rs`](expressions/prototype_methods.rs), [`trailing_returns.rs`](expressions/trailing_returns.rs), [`tidy_expressions/`](expressions/tidy_expressions/README.md) |
| [`program/`](program/README.md) | whole-program passes: classes, arrows, parameters, VM values | [`arrows.rs`](program/arrows.rs), [`dotted_properties.rs`](program/dotted_properties.rs), [`thread_parameters.rs`](program/thread_parameters.rs), [`unwrap_host_globals.rs`](program/unwrap_host_globals.rs), [`vm_fingerprint.rs`](program/vm_fingerprint.rs), [`vm_values.rs`](program/vm_values.rs), [`classes/`](program/classes/README.md) |
| [`names/`](names/README.md) | naming generated bindings | [`name_loop_and_catch_bindings.rs`](names/name_loop_and_catch_bindings.rs), [`positional_names.rs`](names/positional_names.rs), [`role_names.rs`](names/role_names.rs), [`short_generated_names.rs`](names/short_generated_names.rs) |

## Checks and extension

```sh
cargo test --test emitted_js_cleanup --test emitted_property_writes   --test emitted_array_constructions
cargo test --lib carry_semantics_tests
```

The carry semantics tests and differential JavaScript cases need Node on `PATH`.
A new shape needs an exact positive case, a nearby decline, and a differential
check when runtime behavior changes. State the binding, order, comment, counter,
and map contract before extending the matcher. See the
[extension checklist](../../../docs/transforms/emitted-program-cleanup.md#extending-a-pass).

Private stored-type proofs converge over all initializers and stores, including
cyclic join/loop copies. Numeric and broader primitive categories stay separate;
string facts may permit harmless prefix evaluation, but cannot justify Number
rewrites. A result-type proof never makes its effectful producer removable.
Repeated inert-expression substitution uses output cost rather than a fixed
reference-count limit. See the latest batch notes in
[thread cleanup](../../../docs/transforms/thread-cleanup.md).

Private declaration activation also relocates unobserved inert literals before
their first construction use. Reconstruction runs late to retain the producer
provenance used by earlier snapshot and method cleanup. Dense-array elements
retain comma-expression parentheses. Finished private channels use path-sensitive
definite assignment and old-value liveness; they never invent storage names.
Computed successor storage also crosses nested statement blocks and follows
later explicit assignments/updates. Branch-merged successors must initialize
on every path reaching a read, and every old input read must precede reuse.
Once that equality is established, unrelated structured control flow may retain
it. Captured cells, for-header input cells, const-write errors and anonymous
function/class name inference remain barriers.

After reconstruction, ended private SSA/CARRY live ranges may lend their cells
to independent later producers. Every old reference must finish before reuse;
all surviving references must resolve to the same uncaptured cell in the same
iteration. Producer effects stay in place. This is real storage reuse with no
new names. It runs after array/descriptor reconstruction to retain provenance.
Grouped uninitialized joins and proven copy-back versions share the same path
checks. Bounded reads from private dense key lists also permit constant lengths
across loops; unknown indices, escapes and inherited-accessor risks decline.

Ended storage reuse also handles uninitialized joins, preserving their original
undefined initialization and grouped siblings. Structured writes invalidate the
other saved version. Exact point literals propagate computed property keys before
construction and other reads afterward; late constant/branch cleanup then removes
inert tails. Negative zero, getter order, unknown global effects, directive
boundaries and implicit block-function stores retain explicit guards.

Successor version proofs also solve loop headers against initial entry and all
backedges. Break exits and continue-to-update/test edges retain separate states;
failed tests preserve any still-observed old snapshot. Initialized copies may
share storage while equality holds or a later explicit copy-back restores it.
Opaque control regions with unmodeled loop jumps conservatively decline.

Boolean control edges retain separate availability states through logical
AND/OR, negation, sequences and conditional expressions. This proves continuation
copies without treating false-exit snapshots as overwritten. Nullish coalescing
retains its conservative nullish skip/evaluate merge.

Single initialized for-header cells can borrow finished generated storage late.
The header initializer remains once per activation, and the only newly entered
repeated region may be that for loop itself. Captured iteration cells, later
source observers, scope changes, implicit named-function stores and const/TDZ
errors retain their guards. Ordinary body reuse keeps its stricter iteration
rule; header cells do not become statement-list source candidates.

Discarded generated-cell values and exact mutable self-stores are removed only
where their result is unused. Used comma results keep GetValue; concise-arrow
returns, directive boundaries, getters, const/resource/name-binding errors and
unknown coercions stay protected. Dead for fields retain a syntactically valid
inert expression when the parser's span excludes surrounding parentheses.

`cell_liveness.rs` provides backward statement facts for adjacent forwarding of
one private stored version. Branches join and loop backedges reach a fixed point;
unknown control is opaque and exception owners decline. A final branch read can
therefore consume its producer without conflating other versions of that cell.
Readonly numeric arguments reads may commute under the existing arguments and
ordinary-prototype proofs. Computed-key coercion stays after the producer.

Ended header storage may cross a parent activation only when all old source
observations precede that parent's entry; dependent parent-loop inputs decline.
