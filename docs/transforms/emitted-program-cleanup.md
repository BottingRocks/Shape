# Emitted program cleanup

This reference explains the post-emission JavaScript cleanup: the
per-function and whole-program stage order, local binding and control-flow
rewrites, reporting, and source coordinates.
The [module guide](../../src/transforms/emitted_js/README.md) maps the code and
entry points. [Declaration cleanup](emitted-js-cleanup.md) and
[property-write cleanup](emitted-property-writes.md) own their independent
contracts; [helper and tuple cleanup](emitted-helper-cleanup.md) covers the
helper stages.

## Measuring cleanup performance

Build once with `cargo build --release`, then time the executable directly.
`trace program --no-cleanup` measures preparation, tracing, per-function
emission and linking without the per-function and whole-program cleanup
stages. It produces different
JavaScript and is not a substitute for benchmarking the complete pipeline.

To compare an optimization against a saved baseline binary, use:

```sh
time ./target/release/shape trace program capture.raw.js --out /dev/null
```

The runner alternates binary order, reports complete-process wall times, and
requires identical output bytes and trace/cleanup counts on every run. Run it
without concurrent builds or benchmarks. Use `--dump-stages DIR` separately
to locate slow stages and compare intermediate output; writing stage dumps
adds I/O and is excluded from the benchmark command.

A timing difference is meaningful only when both binaries emit identical
output bytes and report the same trace and cleanup counts. Local timings are
not portable limits: capture size, flags, and hardware change the result.

Name freshness in branch folding and helper sharing uses a per-pass sorted
index of literal name occurrences, preserving substring matches in strings,
comments, and longer names. Alias and carry passes index nested function/block
regions instead of searching every region for each reference. The lexical
parser depth guard scans whole identifier tokens without accumulating their
bytes into a temporary string. These optimizations preserve pass order,
bounded iteration counts, binding proofs, and output validation.

## Reading this reference

Start with [Pipeline](#pipeline) when comparing stage dumps. The pass catalog
below describes the exact accepted forms and their decline conditions. A
shorter output is evidence of a rewrite, not a new execution proof.
Readability stages have explicit assumptions, especially for property writes,
discarded argument reads, tuple unpacking, and the
[late whole-program passes](#late-whole-program-passes).

## Why the emitted code needs cleanup

The emitter is optimized for fidelity, not for human reading. A correct trace
comes out with several kinds of representation tax:

- **Names carry function identity.** A temporary may be called
  `$SSA_2_46953_entry_9965`, and that identity is repeated at every read.
- **Every slot write is explicit.** Array/object stores are emitted as
  `Object["defineProperty"](...)` with a four-field descriptor.
- **Arrays are built defensively.** A fresh array is created with `void 0`
  placeholders and then filled by adjacent assignments.
- **SSA and carries are explicit.** Copies, two-arm joins, loop relays, and
  dead temporaries remain as separate statements.
- **Branches are materialized.** A value selected by an `if` is often written
  into a carry binding in both arms and read immediately afterwards.
- **VM helpers repeat.** Linked functions can contain identical `vm_sub_*`
  bodies, wrappers, and fresh-array tuples.

These are not algorithm bugs. They are the cost of representing control flow,
slot identity, and VM calls directly. Strict cleanup proves the replacement preserves the relevant observations.
Readability-oriented stages additionally rely on their documented runtime
assumptions. The goal is fewer bytes,
statements, calls, allocations, and names without changing evaluation order,
exceptions, `this`, or the objects a program can observe.

## Design rules

1. **Exact source edits.** Passes compute non-overlapping byte spans, splice
   source text, reparse the result, and fail rather than emit overlapping or
   malformed edits. RHS and literal text is copied verbatim, so escapes, lone
   surrogates, `-0`, parentheses, and comments are not reconstructed from
   cooked values.
2. **Resolved identity, not names.** A candidate is tied to an Oxc `SymbolId`.
   Same-spelled bindings in another scope are different symbols. Generated
   prefixes narrow the search; they do not authorize a rewrite.
3. **No execution.** The evaluator for `vmDecodeString` is Rust arithmetic over
   proven bytes. Everything else is an AST rewrite. Captured code is never run.
4. **Fail closed.** A whole pass returns the input unchanged (or reports an
   `Unsupported`/`Unknown` status) when dynamic scope, an unproven binding, an
   ambiguous receiver, or an unsafe edit makes the proof impossible. A few
   stages print a skip diagnostic and keep their input where the
   caller is written to tolerate it.
5. **Comments are source, not metadata.** Passes never synthesize or rewrite
   comment text. Whole-line deletion happens only when the rest of the line is
   whitespace, and several deletion passes decline when the removed span
   contains a comment. Identifier-only passes leave comments untouched;
   comment preservation by a deletion pass is a per-pass property pinned by its
   tests, not a global guarantee.
6. **Bounded fixpoints.** Repeated chain cleanup is capped (usually four
   rounds) and stops when a pass reports zero changes. No pass reasons across
   unrelated branches or unbounded loop backedges.
7. **Maps are explicit.** Expression-local property rewrites preserve the
   caller's statement/node maps. Array folding remaps statement indexes.
   Declaration removal either invalidates the old map or regenerates it.

## Pipeline

Post-emission order in `shape trace thread`/`corpus --cleanup` is:

```text
emit -> vm-string fold -> strict property writes -> array constructions
```

`shape trace program` runs that sequence on each emitted function
(`functions/<function>/00-emitted.js` … `03-array-constructions.js`), replaces
the generated `this` placeholder (`04-receiver-this.js`), and, with
`--thread-cleanup`, runs the per-thread driver (`05-thread-cleaned.js`). Each
function then goes through `clean_function` on its own tracing worker, is
taken back out (`06-thread-pipeline.js`) and has its arrays laid out. The
cleaned functions are linked (`00-linked.js`), and `finish_program` names and
tidies the whole program. There is no single pipeline over the linked
program.

[`pipeline/stages.rs`](../../src/transforms/emitted_js/pipeline/stages.rs)
owns both orders. [`program.rs`](../../cli/trace/commands/program.rs)
only traces, links, and calls them; `--no-cleanup` skips both. The
`--thread-cleanup` driver is `cleanup_thread_impl` in
[`pipeline/thread.rs`](../../src/transforms/emitted_js/pipeline/thread.rs);
its seven phases are in [thread cleanup](thread-cleanup.md).

Paths below are relative to `src/transforms/emitted_js/`. "×N" is a bounded
repeat that stops early when a round changes nothing. Stage numbers keep their
historical prefixes, so there are gaps (`05`, `10`, `11b6`, `11h2`, …): a
number names a dump, not a position.

### Per-function stages (`clean_function`)

Each function runs as one unit: its own code, with the slots it shares with
other threads declared as opaque outside values, so only its private `$KEY_`
slots are plain variables to the passes. Dumps go to
`DIR/functions/<function>/pipeline/`.

**`ssa_and_carries` (`01`–`09`)**

| Dump | Pass | File |
| --- | --- | --- |
| `01-redundant-initializers.js` | `remove_redundant_initializers` | `storage/redundant_initializers.rs` |
| `02-adjacent-ssa.js` | `inline_adjacent_ssa` ×4 | `ssa/adjacent_ssa/` |
| `03-literal-branch-joins.js` | `collapse_literal_branch_joins` | `branches/literal_branch_join.rs` |
| `04-literal-ssa.js` | `inline_literal_ssa` | `ssa/literal_ssa.rs` |
| `06-readable-property-writes.js` | `simplify_readable_program_property_writes` | `literals/property_writes/program.rs` |
| `07-carry-joins.js` | `inline_single_use_carry_joins` | `carries/carry_join.rs` |
| `07b-carry-coalesced.js` | `coalesce_carry_copies` ×4 | `carries/carry_coalesce.rs` |
| `07b2-carry-self-assignments.js` | `remove_carry_self_assignments` | `carries/carry_self_assign.rs` |
| `07c-dead-carries.js` | `remove_dead_carries` ×4 | `carries/dead_carry.rs` |
| `07d-branch-values.js` | `fold_branch_values` ×4 | `branches/branch_value.rs` |
| `08-alias-ssa.js` | `inline_alias_ssa` | `ssa/alias_ssa.rs` |
| `09-adjacent-after-alias.js` | `inline_adjacent_ssa` ×4 | `ssa/adjacent_ssa/` |

`01` and `03` print a skip diagnostic and keep their input on error; `02`
stops repeating on one.

**`vm_helpers` (`11`–`11b5`)**

| Dump | Pass | File |
| --- | --- | --- |
| `11-vm-sub-parameters.js` | `name_vm_sub_parameters` | `helpers/vm_sub_parameters.rs` |
| `11b-shared-vm-helpers.js` | `share_vm_sub_helpers` | `helpers/vm_sub_dedupe.rs` |
| `11b1-simplified-vm-subs.js` | `simplify_vm_sub_bodies` | `helpers/vm_sub_simplify.rs` |
| `11b2-inline-vm-wrappers.js` | `inline_vm_sub_wrappers` ×4 | `helpers/inline_vm_sub.rs` |
| `11b3-scalar-tuples.js` | `scalarize_literal_tuples` | `helpers/tuple_scalarize.rs` |
| `11b4-ssa-after-tuples.js` | `inline_alias_ssa`, then `inline_adjacent_ssa` ×4 | `ssa/alias_ssa.rs`, `ssa/adjacent_ssa/` |
| `11b5-unpacked-tuples.js` | `unpack_fresh_helper_tuples` | `helpers/tuple_unpack.rs` |

**`dead_code_and_loops` (`11c`–`11h`)**

| Dump | Pass | File |
| --- | --- | --- |
| `11c-dead-ssa.js` | `remove_dead_ssa` | `ssa/dead_ssa.rs` |
| `11c3-dead-key-stores.js` | `remove_unread_key_stores` | `storage/dead_key.rs` |
| `11c4-dead-after-keys.js` | ×4: `remove_dead_ssa`, `remove_noop_local_reads`, `remove_unread_key_stores`, `remove_dead_carries` | `ssa/dead_ssa.rs`, `storage/dead_key.rs`, `carries/dead_carry.rs` |
| `11c5-argument-reads.js` | `remove_discarded_argument_reads` | `ssa/dead_ssa.rs` |
| `11d-empty-else.js` | `remove_carry_self_assignments`, `remove_empty_else_blocks` | `carries/carry_self_assign.rs`, `branches/empty_else.rs` |
| `11d2-reconstructed-loops.js` | `reconstruct_loops` (every `$KEY_` name protected) | `loops/structured_loops.rs` |
| `11d3-lifted-loop-counters.js` | `lift_loop_counters` | `loops/loop_counters.rs` |
| `11e2-fold-literal-fills.js` | `fold_literal_fills` | `literals/fold_literal_fills.rs` |
| `11e3-split-loop-tests.js` | `split_loop_tests` | `loops/loop_tests.rs` |
| `11f-single-use-literals.js` | `inline_single_use_literals` | `literals/inline_single_use_literals.rs` |
| `11g-loop-catch-names.js` | `name_loop_and_catch_bindings` | `names/name_loop_and_catch_bindings.rs` |
| `11h-host-globals.js` | `unwrap_host_globals` | `program/unwrap_host_globals.rs` |

**`helper_expansion` (`11h0`–`11h5`)**

| Dump | Pass | File |
| --- | --- | --- |
| `11h0-resugared-helpers.js` | `forward_values`, `resugar_expressions` | `ssa/forward_values/`, `expressions/tidy_expressions/resugar.rs` |
| `11h1-expanded-helpers.js` | ×12: `hoist_helper_calls`, `inline_small_helpers`, `inline_tuple_helpers`; then the string fold | `helpers/inline_small_helpers/`, `helpers/inline_tuple_helpers.rs` |
| `11h3-expanded-literal-fills.js` | `fold_literal_fills`, `nest_literals` | `literals/fold_literal_fills.rs`, `literals/nest_literals.rs` |
| `11h4-lifted-loop-counters.js` | `lift_loop_counters`, `name_loop_and_catch_bindings` | `loops/loop_counters.rs`, `names/name_loop_and_catch_bindings.rs` |
| `11h5-tidied.js` | `tidy_expressions` | `expressions/tidy_expressions/` |

The string fold is the caller's `fold_strings` (`fold_program_strings` in
`program.rs`): VM string-decoder calls whose arguments became literals.

**`forwarding` (`11i`–`11p`)**

| Dump | Pass | File |
| --- | --- | --- |
| `11i-forwarded-values.js` | `forward_values` | `ssa/forward_values/` |
| `11i2-resugared.js` | `resugar_expressions` | `expressions/tidy_expressions/resugar.rs` |
| `11j-late-branch-values.js` | `fold_branch_values` | `branches/branch_value.rs` |
| `11k-forwarded-branch-values.js` | `forward_values` | `ssa/forward_values/` |
| `11k3-forwarded-again.js` | forward loop ×8: `resugar_expressions`, `fold_branch_values`, `fold_multi_branch_values`, `fold_plain_branch_assignments`, `hoist_branch_tails`, `flatten_exits`, `fold_literal_fills`, `forward_values_once` | `branches/branch_value.rs`, `branches/branch_tails.rs`, `branches/flatten_exits.rs`, `literals/fold_literal_fills.rs`, `ssa/forward_values/` |
| `11l-dead-after-forwarding.js` | `remove_dead_ssa`, `remove_dead_carries`, `remove_unread_key_stores`, `remove_discarded_argument_reads` | `ssa/dead_ssa.rs`, `carries/dead_carry.rs`, `storage/dead_key.rs` |
| `11l2-local-vars.js` | `clean_local_vars` | `storage/local_vars.rs` |
| `11m-merged-derived-values.js` | `forward_values_and_property_reads`, then `merge_derived_values` ×24 | `ssa/forward_values/`, `ssa/merge_derived/` |
| `11o-split-single-reads.js` | ×6: `split_single_reads`, `forward_values_and_property_reads`; then `merge_derived_values` ×24 | `ssa/merge_derived/`, `ssa/forward_values/` |
| `11p0-remaining-helpers.js` | `inline_small_helpers` ×4 (once), then ×4: `hoist_remaining_helper_calls`, `inline_remaining_helpers` | `helpers/inline_small_helpers/` |
| `11p1-forwarded-remaining-helpers.js` | when `11p0` expanded anything: `nest_literals`, `tidy_expressions`, `forward_values_and_property_reads`; then `merge_derived_values` ×24 | `literals/nest_literals.rs`, `expressions/tidy_expressions/`, `ssa/forward_values/`, `ssa/merge_derived/` |
| `11p-late-helpers.js` | result of the `11p0`/`11p1` loop | |

`11k3` and `11l` form an outer loop (×3) that repeats while `11l` removes
anything, so each dump holds the last round. `11p0`/`11p1` repeat (×4) while
helpers still expand.

**`final_tidy` (`13b`)**

| Dump | Pass | File |
| --- | --- | --- |
| `13b-tidy-expressions.js` | the string fold, `clean_local_vars`, `nest_literals`, `resugar_expressions`, `merge_loop_counter_copies`, `optional_chains`, `tidy_final_expressions`, `hoist_branch_tails` (and the tidy again if it moved anything), `native_for_in_loops`, `place_key_declarations`, `remove_needless_loop_labels`, `drop_trailing_returns` | `storage/local_vars.rs`, `literals/nest_literals.rs`, `expressions/tidy_expressions/`, `loops/loop_counter_copies.rs`, `expressions/optional_chains.rs`, `branches/branch_tails.rs`, `loops/native_for_in.rs`, `storage/key_declarations.rs`, `loops/loop_labels.rs`, `expressions/trailing_returns.rs` |

### Whole-program stages (`finish_program`)

These run once on the linked program, where a child thread's reads resolve to
the slots its parent declares. Dumps go to `DIR/`; several passes share one
dump, named after the last.

| Dump | Pass | File |
| --- | --- | --- |
| `00-linked.js` | linking, no cleanup yet | `cli/trace/commands/program.rs` |
| `12-short-generated-names.js` | `shorten_generated_names` | `names/short_generated_names.rs` |
| `13c-role-names.js` | `name_generated_roles` | `names/role_names.rs` |
| `13c2-classes.js` | `rebuild_classes` | `program/classes/` |
| `13c3-arrows.js` | `arrow_callbacks` | `program/arrows.rs` |
| `13c4-parameters.js` | `thread_parameters` | `program/thread_parameters.rs` |
| `13d-positional-names.js` | `replace_vm_values` (with the VM fingerprint fold), `inline_constant_slots`, `remove_unused_keys`, `name_bindings_by_position` | `program/vm_values.rs`, `program/vm_fingerprint.rs`, `expressions/constant_slots.rs`, `storage/unused_keys.rs`, `names/positional_names.rs` |
| `13e-dotted-properties.js` | `name_prototype_methods`, `inline_path_temporaries`, `fold_array_fills`, `fold_bit_accumulators`, `step_copied_counters`, `flip_negated_comparisons`, `tidy_empty_starts`, `inline_adjacent_literals`, `join_let_runs`, `dotted_properties` | `expressions/prototype_methods.rs`, `expressions/path_temporaries.rs`, `literals/array_fills.rs`, `expressions/bit_accumulators.rs`, `loops/copied_counters.rs`, `expressions/negated_comparisons.rs`, `expressions/empty_starts.rs`, `literals/adjacent_literals.rs`, `expressions/let_runs.rs`, `program/dotted_properties.rs` |

`13e-dotted-properties.js` is the program written to `--out` (or stdout);
with `--thread-cleanup` the output is that program reprinted once more.
`dotted_properties` stays last because earlier passes recognise code by its
bracketed spelling. The [late whole-program passes](#late-whole-program-passes)
below state what each of `13d`–`13e` assumes.

Both orders are deliberately order-sensitive. Coalescing a carry exposes the
copy immediately above it, and scalarizing a tuple exposes adjacent SSA
copies; those stages are repeated or followed by a second adjacent SSA/alias
pass. The standalone declaration pass is *not* part of either order.

## Local pass catalog

### Fresh-array folding (`array_initializers.rs`)

Recognizes one declaration followed immediately by dense assignments:

```js
let $ARRAY = [void 0, void 0, void 0];
$ARRAY[0] = 1;
$ARRAY[1] = 2;
$ARRAY[2] = 3;
```

The declaration must have one simple binding; every array element must be
exactly `void 0`; the writes must be consecutive `=` statements to
`$ARRAY[0]`, `$ARRAY[1]`, ...; all placeholders must be filled; and every RHS
must be a literal, `void 0`, or unary `+`/`-`/`~` over a numeric literal whose
source text can be copied. The receiver must have no other writes, no nested
closure capture, and no read before the last rewritten write. Holes, spreads,
extra or missing indexes, effectful RHSs, and any earlier observation decline.

The output is one literal `[1, 2, 3]`; the write statements are deleted. The
source is reparsed, and the adapter remaps statement indexes so entries on
removed writes point at the declaration and later entries shift by the number
of removed statements.

**Reduces:** N write statements, N index expressions, and the placeholder
allocation pattern, while preserving evaluation order and literal text.

### Redundant initializer removal (`redundant_initializers.rs`)

Recognizes a function-body `let` with one or more plain identifier declarators
and no initializers, immediately followed by a consecutive run of
`name = void 0;` statements that resolve to those declarations:

```js
let $KEY_a, $KEY_b;
$KEY_a = void 0;
$KEY_b = void 0;
// -> let $KEY_a, $KEY_b;
```

Only the contiguous run directly after the declaration is considered. An
intervening call, branch, or unrelated statement stops the pass; an assignment
inside an `if` is never reached. `void call()` does not match because the
initializer must be `void <numeric 0>`. Removing the stores is safe because
`let x;` already produces `undefined`; consequently no liveness or dynamic-scope
analysis is needed. A line is deleted whole only when the remainder is
whitespace, so trailing comments survive.

**Reduces:** the large linked-program slot-initialization prelude. It reports
`removed_assignments`.

### Adjacent SSA inlining (`adjacent_ssa/`)

Matches two adjacent statements in one function body. The first must be
`let $SSA_n = <initializer>;` with one simple binding. The second must hand that
temporary to the next operation in a position that is evaluated first:

- `return`/`throw` argument, or the right operand after an already-declared
  lexical left operand;
- `if` test;
- assignment of the identifier to a resolved `$KEY_`/`$CARRY_` target;
- a single-variable declaration whose first evaluated read is the temporary;
- an argument of a call to an immutable function declaration.

The temporary must have exactly one resolved reference and that reference must
be a read. The rewrite deletes the declaration and substitutes the initializer
at that one read. Substitution is parenthesized; when the read is a direct call
callee the replacement is `(0, value)` so the helper keeps bare-call `this`
instead of accidentally becoming a method call. Later call arguments are
accepted only when moving a plain lexical read across previously initialized
plain lexical reads cannot invoke a getter or mutate a binding. `with` or
`eval` disables the pass.

**Reduces:** one temporary statement per handoff, with no duplication or
movement across another statement. The caller repeats it up to four times.

### Literal branch joins (`literal_branch_join.rs`)

Matches an uninitialized `let $SSA_n;` (or `$CARRY_`) followed by an `if` whose
two arms assign only a numeric/string/boolean/null literal, and whose test is a
resolved identifier:

```js
let $SSA_1;
if (flag) { $SSA_1 = 1; } else { $SSA_1 = 2; }
// -> const $SSA_1 = flag ? 1 : 2;
```

Both arms must be single plain assignments to the same resolved symbol; exactly
two writes are required. The replacement is a `const`, which is safe because
the arms cannot read the old binding and adjacency leaves no window in which a
read could observe the pre-initialized `undefined`. The pass declines comments
in the replaced region and does not rewrite arbitrary branch bodies.

**Reduces:** the mutable declarator, the `if`, and its two assignment
statements.

### Literal SSA inlining (`literal_ssa.rs`)

Matches `let|const $SSA_n = <primitive literal>;` and replaces every resolved
read with that literal text, deleting the declaration. Accepted literals are
numeric, string, boolean, null, bigint, and unary `+`/`-` over a numeric
literal; regex literals are excluded. The binding must have at least one
reference, no writes, no read before the declaration completes, no read in a
different enclosing function, and no shorthand-object read. Captures are
retained because a closure can run before a `let` initializes. A declaration
containing `//` or `/*` is skipped, and `with`/`eval` disables the pass.

**Reduces:** the named constant and its statement. Literal text is copied from
the source; parentheses are added only where precedence needs them.

### Alias SSA inlining (`alias_ssa.rs`)

Matches `let|const $SSA_n = <plain identifier>;` or the same shape for
`$CARRY_`. The source binding must be declared before the copy; the alias must
have at least one reference, none a write; and every read must be after the
declaration, in the same enclosing function, not a shorthand property value,
and must re-resolve to the same source symbol under its own scope. The
declaration is deleted and each read is replaced by the source binding's name.

A write to the source after the copy normally declines. Two snapshot proofs
allow it: a loop-carried copy whose reads all stay in that loop and whose writes
are before the copy, after the last read, or inside the read's own RHS; and a
straight-line copy whose reads stay in the copy's block/loop and have no write
between the copy and the last read. Captured reads, read-before-init, comments,
and overlapping proposals decline. `with`/`eval` returns the input unchanged.

**Reduces:** generated copies of parameters, slots, and loop snapshots without
changing the value observed at each read.

### Literal fills (`fold_literal_fills.rs`)

Program mode builds a plain object and then assigns every property. When the
literal's own values are all `void 0`, the following `$X["k"] = value;` stores
can move into the literal. Replacing a `void 0` with a later value cannot move an
effect, and the store order is preserved (replaced keys keep literal order,
appended keys come last), so evaluation order is unchanged. A `$X["k"] = void 0`
store whose key is already `void 0` is simply dropped.

It declines when a store value reads the object, when folding would reorder
values, when a statement sits between the literal and a store, for a `__proto__`
key (assignment runs the inherited setter), a non-string key, a spread or
non-`void 0` literal value, a comment in the region, or dynamic scope.

An empty `[]`/`{}` (or a pre-sized `[void 0, …]`) moves down to its first
dense index or new-key store and takes the run of stores there. A literal
with real values (`[a, b]`, `{ ["k"]: a }` of plain data properties, no
`__proto__`) is extended only by the stores right after it, so its own values
still run first; a store to a key it already has stays a store. The fold also
runs in the forward loop, where forwarding exposes new adjacent stores.

**Reduces:** the construction-then-fill shape to one literal.

### Single-use array/object literals (`inline_single_use_literals.rs`)

Matches `let|const $ARRAY_n = <array literal>;` or `$OBJECT_n = <object
literal>;` whose binding has exactly one resolved reference. The reference must
be a read after the declaration, in the same function, and not a `delete` or a
shorthand property. The initializer must be movable: no spread, pure
elements/values, and static or primitive-literal computed keys. Because the
value is read once, building it at the read is unobservable; a read inside a
loop the declaration is not in declines (it would allocate once per iteration).
Comments and `with`/`eval` decline, and literal source text is copied verbatim.

**Reduces:** the named temporary for a literal that is used once, placing the
allocation at its use.

### Single-use carry joins (`carry_join.rs`)

Matches three adjacent statements: an uninitialized `let $CARRY_x;`, a two-arm
`if` whose arms each contain exactly one plain assignment to that resolved
symbol, and a following statement whose expression is exactly that same
identifier (an `if` test, `return`/`throw` argument, or single `let`
initializer). Globally the binding must have exactly two writes and one read;
the read must be the direct next use. Comments in the replaced region decline,
and `with`/`eval` disables the pass.

The declarator and `if` are deleted and the read becomes
`((test) ? (yes) : (no))`. The test is evaluated first and exactly one arm is
evaluated, matching the original order. Because the carry has no other read,
the deleted writes were unobservable.

**Reduces:** the carry declarator and branch-join statements, replacing three
statements with one expression.

### Carry coalescing (`carry_coalesce.rs`)

Two forms are recognized. In the uninitialized form, writes go to an inner
`$CARRY_inner`, then `$CARRY_outer = $CARRY_inner;` copies it out. In the
initialized-relay form, the inner carry starts as the outer carry and is copied
back later (typically after a loop). Both require the outer binding to be
declared first and the inner copy to be after the inner declaration. Every
inner reference must be in the copy's function, every write must be a plain
assignment before the copy, and the only inner reads must be the copy source
(or reads nested in an RHS that moves with the write). The initialized form
also checks that the initializer is the outer carry, both bindings share a
block, no `try` overlaps the region, no jump exits it, and the outer binding is
visible at every moved reference.

The inner declaration and copy are deleted and every inner write is renamed to
the outer carry. The initialized form renames all inner references. Comments in
the deleted declaration/copy decline. The driver repeats coalescing up to four
times because one join can expose the next.

**Reduces:** intermediate branch-local carries and their copy statements.

### Carry self-assignments (`carry_self_assign.rs`)

Matches an expression statement `$CARRY_x = $CARRY_x;` where both sides resolve
to the same symbol. The symbol must be declared before the statement, so the
removal cannot delete a TDZ throw. The RHS is a resolved local read, not a call,
so dropping it has no effect. Comments decline and `with`/`eval` disables the
pass. Whole-line deletion is used when the statement stands alone.

**Reduces:** relays exposed by coalescing. It reports a count and is run again
after dead-slot/empty-branch cleanup.

### Equivalent carries (`equivalent_carry.rs`)

Matches two or more `$CARRY_` bindings with byte-identical literal/null
initializers or no initializers, followed by lockstep adjacent stores of the
same value:

```js
$CARRY_a = value;
$CARRY_b = value;
```

Every left write must have a paired right write whose target is the other
carry and whose RHS matches: literals are byte-identical; identifiers must
resolve to the same non-merged symbol. Every right-hand reference must resolve
to the left binding under its own scope and must not be a shorthand property.
The second declarator and every paired second store are deleted, and reads of
the right carry are renamed to the left. Comments and overlaps decline.

**Reduces:** duplicate carries and one duplicate store per pair. Local reads of
the same initialized value cannot have different effects.

Only the `--thread-cleanup` driver
([`pipeline/thread.rs`](../../src/transforms/emitted_js/pipeline/thread.rs))
runs this pass; it is not a `clean_function` stage.

### Dead carries (`dead_carry.rs`)

Finds generated `let $CARRY_...` bindings with no resolved non-write references.
Every store must be a plain statement assignment to that symbol after its
declaration; `return $CARRY = 1` does not qualify. For each store, a pure,
non-throwing RHS deletes the statement; a resolved identifier declared earlier
in the same function also deletes; any other RHS is preserved as `(rhs);` at
the same point. The declaration is kept as `(init);` when its initializer has
effects, and surviving bindings are re-declared. Bare `$CARRY_` read
statements after a declaration are deleted separately and not counted.

Selection runs to a fixpoint within one parse: a store `$CARRY_b = $CARRY_a`
deleted with `$CARRY_b` no longer counts as a read of `$CARRY_a`, so a whole
relay chain is removed by one call. A store kept as `(rhs);` still reads its
source. Comments in the declaration and dynamic scope decline.
`removed_carries` counts bindings, not stores. The driver repeats up to four
times at `07c`, once more per round of the `11c4` loop, which catches carries
left unread by the helper and tuple stages, and once per round of `11l`.

**Reduces:** unread carry bindings and stores while preserving every effectful
initializer and RHS evaluation.

### Branch value folding (`branch_value.rs`)

Matches an uninitialized `let $CARRY_x;` and a two-arm `if` whose arms are
assignment-only, recursively allowing an assignment-only nested diamond. All
writes inside the `if` must resolve to the carry and the leaf count must match.
The result is one immutable binding:

```js
const $MERGE_x = (test) ? (yes) : (no);
```

The generated `$MERGE_` name must not already occur anywhere in the source. Any
read at or before the replaced span declines because the old code could observe
the pre-initialization `undefined`/TDZ window; the replacement has no such
window. Comments and overlapping rewrites decline. Test-first, one-arm
evaluation order is preserved.

**Reduces:** a mutable join carry plus an `if` tree, replacing it with a single
`const` conditional. The driver repeats up to four times.

`fold_multi_branch_values` does the same for a tree that sets several carries
in every leaf, in the same order and nothing else: one `let` per carry, each
with its own conditional. Every nested test must be repeatable; a top test
that is not is evaluated once into a fresh `$SSA_t…` temporary. In the
forward loop, `fold_plain_branch_assignments` also rewrites a store-only
diamond into a carry written elsewhere too as `x = t ? a : b;` (the
per-thread form also accepts arm prefixes as comma expressions; the program
loop does not).

### Exit flattening and arm tails (`flatten_exits.rs`)

Restructures statement lists inside the forward loop; every rewrite of an
`if` replaces it as one edit:

- **Else after exit.** `if (t) { …; return x; } else { REST }` -> the `if`,
  then `REST` (the arm that does not always leave is spliced after it), when
  the spliced bindings are generated names and no function or class moves.
- **Bare blocks.** A `{ … }` in a list whose own bindings are generated names
  is just its statements.
- **Declaration sinking.** Each bare `let $X` of a declaration whose first
  reference is a plain store statement later in the list is declared there;
  every reference must be in the declaring function (a closure could meet the
  TDZ).
- **Identical tails.** A carry every leaf of a complete tree sets to the same
  settled value (literal, stable global or built-in member, `this`, an
  unwritten binding declared before, fixed `arguments[n]`), followed only by
  settled stores, is set once after the tree. Every write is one of those
  stores; no other reference is inside the tree or in another function; no
  `try` surrounds it.
- **Test first.** `if (t) { B; $J = x; } else { $J = t; }` -> `$J = t;
  if ($J) { B; $J = x; }` (and the negated form) when `B` ends storing `$J`
  with no `break`/`continue`, `$J` is read nowhere in the `if` or by a
  closure, and no `try` surrounds it.
- **Return sinking.** `if (…) { …; $J = a; } else { $J = b; } return $J;` ->
  each leaf returns its value, when every leaf ends storing `$J` or leaves
  and `$J` has no other read.
- **Copy retargeting.** `if (…) { …; $J = a; } … K = $J;` (or `let K = $J;`)
  stores into `K` in each leaf instead; a `let K` is declared before the
  tree, which must not name `K`.

**Reduces:** join carries the VM's branches leave behind, `$MERGE_`
temporaries, and the nesting of continuations inside arms.

### Dead SSA and discarded reads (`dead_ssa.rs`)

This file contains four independent passes:

- **Unread `$SSA` declarations.** A single-binding `let $SSA_n = init;` with no
  resolved references is deleted when the initializer is pure. If the
  initializer can have effects, it is kept as an expression statement (wrapped
  in parentheses when it begins `{`, `function`, or `class`) only when that is
  strictly shorter than the original. An identifier initializer is also
  removable when the immediately preceding statement declared or plain-assigned
  the same resolved symbol.
- **Unread tuple slots.** In `let [a, b, c] = init;` an unread generated slot
  becomes a hole. The pattern keeps its length (`[a, , ,]`), so the iterator is
  stepped exactly as before for any `init`. When `init` calls a fresh-tuple
  helper (a never-reassigned `$VMHELPER_`/`vm_sub_` whose linear body returns a
  dense array literal), trailing holes are dropped and a fully unread tuple
  becomes `init;`; this relies on the standard Array iterator, as tuple
  unpacking does. Defaults, nested patterns, and rest elements decline.
- **No-op generated reads.** A bare `$KEY_n;` or `$CARRY_n;` expression
  statement is deleted when a declaration of the same resolved symbol has
  already executed earlier in that statement list. A read before initialization
  is kept.
- **Discarded `arguments` reads.** A statement `(arguments[<non-negative
  integer>]);` is deleted only in a function whose body never exposes the
  `arguments` object: no `arguments` parameter, every `arguments` token followed
  by `[digits]`, and no `delete`. Arrow functions are not eligible for
  themselves. This assumes ordinary built-ins with no numeric prototype getter.

All four decline on `with`/`eval`. The pure/unread and no-op passes are run in
a bounded loop with dead-key removal (`11c4`); the no-op pass has no dump of
its own.

**Reduces:** temporary declarations, bare slot reads, and unobservable argument
reads after the useful parts of a slot/SSA chain disappear.

### Forwarded values (`forward_values/`)

Runs late (from `11h0`, after the literal-fill and helper stages expose
adjacency, and again in the forward loop), and removes a generated `let|const $SSA_n|$CARRY_n = init;` that is
never written, by the rules below:

- **Stable value.** `init` is a primitive literal or unshadowed
  `undefined`/`NaN`/`Infinity`; an identifier of a never-written local declared
  once and before the copy that is not a `$KEY_`/`$CARRY_` slot (a parameter
  only if its function never mentions `arguments`); or `x[k]` where `x` holds a
  fresh-tuple helper result that is only ever read at in-range literal indexes.
  The value replaces every read in the same function after the declaration.
- **Next door.** Any `init` with exactly one read, in the next statement that
  does anything. Statements in between may only store literals/identifiers
  into locals that `init` does not read and no closure touches. Everything the
  read's statement evaluates before the read must be stable: no call, getter,
  coercing operator (only `===`, `!==`, `!`, `typeof`, `void` qualify), slot or
  global read; a member assignment target is evaluated without a read. The read
  must run exactly once there: not in a branch arm, a short-circuit operand, an
  optional chain, a loop, `try`, or a closure.
- **Duplicate slots.** In `let [a, b] = H(…)` where `H` returns one identifier
  in every slot, `b` becomes a hole and its reads read `a`.
- **Pass-through slots.** A slot `H` fills with an unwritten parameter is the
  call's argument; a stable argument replaces the slot's reads, and a pure
  shuffle call loses its statement when every slot goes.
- **Slot snapshots.** `let $SSA_n = $KEY_k;` reads the slot directly when no
  store (including a `var` redeclaration) lies between the copy and its last
  read, no read is in a loop the copy is not in, and — when another function
  can store into the slot — nothing in between can run code or every store is
  settled and the range does not `await`/`yield`. A store whose own block ends
  in `return`, `throw` or `continue` before a read it could reach does not
  count: `return`/`throw` leave the activation, and `continue` re-runs the
  target loop, whose next iteration re-executes a copy inside it. A store in
  another function does not qualify (it returns to its caller, where the read
  still runs), and a `try` finalizer runs after the jump. A settled store
  writes a binding owned by the function that runs it (or by the program):
  re-entry creates a fresh binding, and an enclosing activation is suspended
  on the stack.
- **Slot handoff.** A multi-use value whose first read is the next statement's
  store into a slot moves into that store; later reads read the slot under the
  snapshot checks.

- **Early slot.** A value stored by a later sibling statement into a `var`
  slot, or into a `let` carry declared before it, is written there where it
  is computed and every read reads the slot, when nothing reads or stores
  the slot in between (nothing runs code there either if another function
  sees the slot), and no enclosing `try` can observe the early write after
  a throw.
- **Internal slots and fixed arguments.** A `$KEY_` with one `var`
  declaration and initializer, never stored, outside loops and used only by
  its own function is an immutable local; in a parameterless function that
  only reads `arguments` at literal indexes, `arguments[n]` is stable.
- **Pure computations and decoys.** A pure computation over stable names
  read once moves anywhere it resolves; operators over typed primitives are
  not effects; a discarded pure expression statement (the VM's `x + 2;`,
  `$THREAD_n;`) is deleted and its reads ignored. Standard built-in globals
  and their named properties (`Object.defineProperty`,
  `Function.prototype.toString`) read as stable.
- **Store to load.** After `$KEY_k = v;` with a stable `v`, a later read of
  the slot in the same list, before any other store, reads `v` under the
  snapshot checks; the store goes once nothing reads it.
- **Branch-known booleans.** Inside the arms of `if (r)` / `r ? a : b`, a
  never-written boolean `r` reads as `true` in one arm and `false` in the
  other.
- **Fresh allocations.** An allocating initializer (`[]`, `{ k: v }`, a
  function) is never copied into several reads — that would split one
  object into several — but, read once, moves to its read anywhere in its
  function except into a loop it is not in.
- **Settled names.** A binding only its declaring function's own code writes
  cannot change while a closure runs (the declaring frame is suspended or
  finished; no `await`/`yield` in the file; a `let` must be that function's
  first statement), so it is stable inside closures. A next-door move may
  also cross reads of a local only its own function writes, that the moved
  value does not write and that is initialized first.

Decisions repeat in memory until settled, so a move may cross a later stable
declaration that goes, or a later move into the same statement; a final check
blocks any value whose assumption does not hold. Reading `.call`/`.apply`
off a plain name to call it counts as running no code (standard
`Function.prototype`).

Values compose: one forwarded into another forwarded initializer moves with
it, so a chain goes in one parse. A substitution whose names would resolve to
another binding at the read blocks that value. Non-trivial text is
parenthesized except in argument/initializer/return/test slots, and a member
callee becomes `(0, value)`. Comments, `with`, and `eval` decline.

`forward_values_and_property_reads` (`11m`, `11o`, `11p1`) also forwards an
initializer that only reads properties off a name with keys of known
primitive type: with no getters, it reads the same value wherever no code
runs in between. It runs after resugaring, whose folds need the plain names.

**Reduces:** copies, constants, and the handoff temporaries the stack
machine leaves, without moving anything past an effect or a changeable value.

### Expanded small helpers (`inline_small_helpers/`)

The VM routes ordinary operations through tiny shared helpers
(`$VMHELPER_64(x)` is `x`, `$VMHELPER_96(x)` is `x["length"]`,
`$VMHELPER_466(a, b, c, d)` is `a[b] = c ^ d()`). A never-reassigned
`$VMHELPER_`/`vm_sub_` helper with plain unwritten parameters, no
`this`/`arguments`/nested function/`await`/`yield`/comment, whose body is one
`return E` (expanded at any call) or up to three expression statements with
an optional `return` of a parameter or empty literal, is expanded at its
calls when its free names resolve the same there. A literal or
never-written non-slot local argument may be repeated; any other argument
must be used once, in argument order, with nothing in the body running
code first (reading `.call`/`.apply` counts as nothing). A call that is a
statement, a declaration initializer, an assignment's right side or a
return value may instead bind its computed arguments, in order, to fresh
`$SSA_x…` temporaries (never for a bare slot name) and expand into
statements followed by the original statement with the helper's value.
Statement sites are planned only from statement lists. Nested calls
compose in one parse; a helper whose references all expanded is deleted.

### Primitive types (`primitives.rs`)

Shared by forwarding and resugaring. Never-written bindings are typed from
their initializers in source order as number, string, boolean or nullish
(operator results, `Number`/`String`/`Boolean`/`parseInt`/`Math.*`
calls). A rewritten `let` keeps its initializer's type when every write
stores that type, and a plain parameter of a function only ever called
directly takes the type all its calls pass; these are assumed, then checked
together until no assumption drops. An expression is pure — no effect,
cannot throw — when it is made of literals, initialized names (`var`s,
plain parameters, a function's leading `let a, b;`, hoisted functions, and
declarations earlier in the same function), standard built-in globals and
their named properties, `this`, literals of pure parts, and operators that
cannot run user code on their operand types (`!`, `typeof`, `void`, `===`,
`== null`, and arithmetic or comparison over typed primitives).

### Resugared expressions (`tidy_expressions/resugar.rs`)

Folds the VM's two-read spellings into their one-read source forms:
`o[k].call(o, …)` (also through a `(0, o[k])` wrapper) -> `o[k](…)`,
`c ? c : b` -> `c || b`, `c ? b : c` -> `c && b`, `c == null ? b : c` ->
`c ?? b`, `c == null ? void 0 : c.p` -> `c?.p`, for plain names `c`/`o`, and
the same `||`/`&&` folds for a repeatable test (names, literals, `!`,
`typeof`, `void`, strict equality, `== null`) repeated in an arm, and
`$c ? a : a` -> `a` for a generated-local test. `??` treats the legacy
`document.all` as present.

### Role names (`role_names.rs`)

Stage `13c` renames every remaining `$SSA_`/`$CARRY_` binding by
`role_of`, keeping its suffix, so later readers can tell the cases apart:
`$LOOP_` (written in a loop it is declared outside of), `$JOIN_` (written in
branch arms), `$VAR_`, `$TUPLE_`, `$SNAP_` (a slot copy that must keep its
value), `$ONCE_` (read once, kept because moving it would reorder effects),
`$REUSED_` (read more than once), `$UNUSED_`. Renaming is by symbol and
declines on a clash; `role_of` is public for external analysis tools.
`13d` then names generated bindings by position (`$A`, `$B`, …;
`names/positional_names.rs`), so the role prefixes show only in the
`13c`–`13c4` dumps.

### Expanded tuple helpers (`inline_tuple_helpers.rs`)

Replaces `let|const [a, b] = H(x, y);` by the body of a never-reassigned
`$VMHELPER_`/`vm_sub_` helper of at most four `let`/`const`/expression
statements ending in `return [e0, …]`, with plain unwritten parameters and no
`this`, `arguments`, nested function or class, `await`, `yield`, or comment.
Its free names must resolve the same at the call. Non-stable arguments go to
fresh `$SSA_x…` temporaries first, in order; stable ones are substituted.
Locals are renamed fresh and each named slot becomes `let s = e;` (an unnamed
effectful slot stays as `(e);`). A helper whose every reference expanded is
deleted. Forwarding then removes the copies.

### Tidy expressions (`tidy_expressions/`)

The tidy parses once and drops `(0, e)` everywhere it means
nothing; `(0, name)(…)` becomes `name(…)` when the program has no `with`, and
`(0, o.m)(…)` becomes `o.m.call(void 0, …)`, the emitter's unbound-call form.
`delete (0, e)`, `typeof (0, name)` and a tag callee keep it. Bare `;`
statements in a statement list and empty `else {}` arms go, and
`if (t) {} else { X }` becomes `if (!t) { X }`. The program is reprinted with
tab-indented codegen, which emits only the parentheses precedence requires.
`tidy_expressions` runs before forwarding (`11h5`) and after late helper
expansion (`11p1`); `tidy_final_expressions` is part of each function's last
stage (`13b`).

### Dead key stores (`dead_key.rs`)

Matches direct assignment statements to generated `$KEY_` locals inside a
function body. The symbol must have no resolved non-write reference anywhere
and must not be a parameter. A pure, non-throwing RHS deletes the whole line; an
identifier resolving to an earlier declaration or a function-scoped
declaration is also removable. Any other RHS is preserved as `(rhs);` at the
same position. Top-level statements, comments, dynamic scope, and unresolved
targets decline. `removed_stores` counts stores, not bindings.

**Reduces:** writes to VM slots that no later JavaScript reads, without
discarding side effects.

### Empty `else` removal (`empty_else.rs`)

Matches `if (test) { ... } else { }` where both arms are blocks and the
alternate block is empty. It deletes exactly the ` else { }` span, keeping the
consequent and following code. Any comment inside the removed span keeps the
`else`. Non-block consequents and `else if` chains are not matched. This is
deliberately narrow because the pattern is the residue of deleting stores from
a two-arm carry branch.

**Reduces:** empty branch scaffolding left by carry cleanup.

### Loop reconstruction (`structured_loops.rs`)

Recovers a loop's condition, backedge updates, and exit continuation from the
VM's `vm_loop_N: for (;;) { … if (cond) { run; continue vm_loop_N } else
{ exit } }`. The run arm may be the `then` or the `else` arm; the exit arm may
be a trailing `break label`, an arm that cannot fall through, or nothing. The
declarations immediately before the tested `if` are lifted into the header
as `let x;` plus `x = (init)` assignments in the condition (or, for a counter
whose declaration is the statement above the loop, into the `for`
initializer). It declines when a lifted binding is captured, when a `break`
would change target, when the exit path could fall through, and when the
declaration form is not a plain generated `$SSA_`/`$CARRY_` binding. The
per-function stage protects every `$KEY_` slot name to keep a shared VM
register out of a header. This is the same pass the standalone thread cleanup
runs; `clean_function` runs it at `11d2` so the later value passes see real
loops.

**Reduces:** `for (;;)` scaffolding and the per-iteration declarations that
only feed the condition.

### Lifted loop counters (`loop_counters.rs`)

After reconstruction a counted loop carries its counter out of the body:

```js
let $CARRY_i = 0;
while ($CARRY_i < n) { use($CARRY_i); $CARRY_i = Number($CARRY_i) + 1; }
```

When every reference to the counter stays inside the loop in the same
function, its update is the last store to it, and the header can still resolve
everything the update reads, the update moves into the header and the loop
keeps its declaration where it was:

```js
let $CARRY_i = 0;
for (; $CARRY_i < n; $CARRY_i = Number($CARRY_i) + 1) { use($CARRY_i); }
```

The binding keeps its position, so its initializer keeps its evaluation point;
only the back-edge update moves, to the same point in the iteration. A
`continue` that would skip the update declines the rewrite (`for` runs the
update, `while` would not).

A loop usually stores its other carried values right after the counter, so the
update may also be lifted past trailing statements that are plain stores of a
call-free, member-free value to other bindings, provided they neither write the
counter nor read into the update. Reordering them past the update is invisible
only when the update cannot throw: a bound identifier or literal, or a counter
whose initializer is numeric and whose every store is `Number(counter)` and
arithmetic over that counter.

`11g` then names the counter like any other `for` counter. The pass runs twice
(`11d3` and `11h4`) because helper expansion in between turns an update like
`$VMHELPER_191($CARRY_i)` into `Number($CARRY_i) + 1`.

**Reduces:** loop-carried `$CARRY_` counters, which become ordinary `for`
counters. It declines a counter read after the loop (a real carried value), a
captured counter, an update reading a body-local binding, and a possible-throw
update that must be reordered.

### Loop and catch names (`name_loop_and_catch_bindings.rs`)

Gives the two loop positions the linker's generated names cannot express. A
`for` counter is a generated local written in the header (`init`/`update`) and
read there; it is renamed, with every resolved reference, to `i`, `j`, `k`, then
`i_2`, `j_2`, ... per function. A `catch` parameter becomes `e`, then `e_2`,
.... Only symbols whose current name starts with `$` are touched, and a
candidate name is skipped when any identifier in the declaring function's
subtree already uses it, so the rename cannot capture a reference or shadow a
binding. `with`/`eval` declines the pass, since name resolution is not then
statically known. Renaming is by symbol, so declaration and references move
together and evaluation order is untouched.

**Reduces:** opaque `$CARRY_…`/`$SSA_…` names in loop headers and `catch`
clauses; no statements are removed, but the bytes an author must scan.

### Host globals (`unwrap_host_globals.rs`)

The program closes over a global object emitted as the free name `host`, so
`host["document"]` is the global `document`. The pass rewrites a computed member
whose object is that free name and whose key is an identifier-shaped string
literal into the identifier: `host["Math"]["floor"]` becomes `Math["floor"]` and
`new host["TypeError"]()` becomes `new TypeError()`.

It is not a wholesale textual replacement. It leaves the member when it is the
direct callee or tag of a call, or an assignment, update, `delete`, or
`for`-`in`/`of` target: those positions read the receiver `host` or write through
it, and a bare name would change the receiver or the target. It leaves a key
that is not an identifier or is a reserved word, and it declines the whole
program when `host` is a local binding anywhere, because the free name is then
not the global object. The key is taken from its unescaped value, so
`host["\u0041"]` becomes `A`.

**Reduces:** the `host[...]` indirection on every global read, without resolving
names or moving an expression.

### Late whole-program passes

`finish_program` ends with passes that see every thread at once (`13d`,
`13e`). Each states its assumptions in its `//!` module doc; the common ones
are that built-ins and the host globals (`window`, `self`, `document`,
`navigator`) do not change, that property reads in the fingerprint probes run
no code, and that `.length` is a number. Paths are relative to
`src/transforms/emitted_js/`.

- **Constant slots** (`expressions/constant_slots.rs`, `inline_constant_slots`).
  A `$KEY_` slot written once, before any read in its function, with a
  global's value (`Array.prototype.push`, `window`, `$KEY_343.document`) is
  read as that global path, and the slot goes. A `RegExp` of literal source
  and flags without `g`/`y` becomes a regex literal. Assumes globals do not
  change and that a read in a function the thread creates runs after the
  setup store. Repeats until nothing changes.
- **Unused keys** (`storage/unused_keys.rs`, `remove_unused_keys`). A slot
  with no read anywhere in the program goes, including a write-only one, when
  its initializer and every store run no code and each store is a statement of
  its own. Threads the deleted stores named stay.
- **Prototype methods** (`expressions/prototype_methods.rs`,
  `name_prototype_methods`). `[].push`, `{}.hasOwnProperty`, `"".trim` ->
  `Array.prototype.push`, … . Only the throwaway receiver is rewritten.
  Assumes built-ins do not change; the constructor name must not be shadowed.
- **Path temporaries** (`expressions/path_temporaries.rs`,
  `inline_path_temporaries`). A variable holding a property path
  (`$KEY_9.Intl?.supportedValuesOf`) read only in the next statement is
  replaced by the path. Assumes property reads run no code and give the same
  value each time (the fingerprint probes read host objects only). Nothing in
  that statement that could store may run before the last read.
- **Array fills** (`literals/array_fills.rs`, `fold_array_fills`).
  `let q = [x, void 0]; q[1] = y;` -> `let q = [x, y];` for ascending
  whole-number stores right after the declaration, when the replaced
  placeholders and later elements run no code.
- **Bit accumulators** (`expressions/bit_accumulators.rs`,
  `fold_bit_accumulators`). A `let` built by `A = A | T;` steps and read once
  more becomes one `|` expression at that read. Start and steps are names,
  literals and operators whose names hold the same value at the read.
- **Copied counters** (`loops/copied_counters.rs`, `step_copied_counters`). A
  loop that steps its counter through the copy its body takes
  (`c = j; … j = Number(c) + 1`) steps the counter itself; a leading
  `if (!e) break;` becomes the test.
- **Negated comparisons** (`expressions/negated_comparisons.rs`,
  `flip_negated_comparisons`). `!(a === b)` -> `a !== b` always; `!(i < n)` ->
  `i >= n` only over numbers that are never NaN (numeric literals, bitwise
  results, counters, and `.length`, taken to be an array's or a string's).
  A negated `&&`/`||` group distributes the `!` (De Morgan) when every part
  negates and the result has at most one `!`.
- **Empty starts** (`expressions/empty_starts.rs`, `tidy_empty_starts`).
  `let x = void 0;` -> `let x;`; `let x; x = e;` -> `let x = e;` when nothing
  can read `x` while `e` runs.
- **Adjacent literals** (`literals/adjacent_literals.rs`,
  `inline_adjacent_literals`). An object or array literal read once by the
  next statement is built there, when only names, literals, `this` and
  property paths are evaluated before the read.
- **Let runs** (`expressions/let_runs.rs`, `join_let_runs`). Adjacent
  value-less `let`s of one scope join into one declaration; a comment breaks
  the run.
- **Dotted properties** (`program/dotted_properties.rs`, `dotted_properties`).
  `o["key"]` -> `o.key`, `{ ["key"]: v }` -> `{ key: v }`, `o["0"]` -> `o[0]`.
  Non-identifier names keep their brackets and `["__proto__"]` keys stay
  computed. Text only; last, because earlier passes match the bracketed
  spelling.

## Helper stages

The remaining stages rename, share, simplify, or inline linked helpers and
reduce fresh tuples. Their individual contracts are in
[helper and tuple cleanup](emitted-helper-cleanup.md). Property stages use the
[strict and linked-program property contract](emitted-property-writes.md).

## Status and reporting

The cleanup APIs use `StageStatus`:

| Status | Meaning |
| --- | --- |
| `Found` | at least one proven rewrite was applied |
| `Unknown` | the pass ran but found no applicable candidate (or was disabled by options) |
| `Unsupported` | whole-input proof failed, e.g. dynamic scope or missing authority; the input is unchanged |

The declaration report separately records eligible/removed statements,
retained/removed declarators, removed top-level function statement indexes, and
notes. The property outcome records `considered`, `changed`, skipped candidates
with typed reasons, prepopulated array/object counts, byte deltas, and notes. The
array outcome records the same byte/status shape plus compacted arrays and
removed statement indexes. Linked passes mostly return a result struct with a
count and/or the changed code; hard parse/edit failures are errors, while some
stages deliberately print a skip diagnostic and keep the previous code.

`trace thread` prints property/array and string-fold counts on stderr when the
relevant reports are present. `trace program` prints traced/stopped function
counts, each stopped function, the kernel globals the program reads, and with
`--thread-cleanup` the per-thread cleanup counts. `--dump-stages DIR`
writes every named stage (each function's `pipeline/01-…13b-` and the
program's `00-linked.js`, `12-…13e-`; see [Pipeline](#pipeline)) so a
regression can be bisected without guessing which pass changed a line.

## Source maps and coordinates

All transforms are byte-span edits over the original text. Edits are sorted and
checked for overlap; an overlap is an error, not a best-effort merge. The
declaration pass replaces a wholly deleted declaration with `;`, so later
statement indexes do not shift in the primary function. `RemovedRange` records
original input offsets, and the map regeneration path drops entries that pointed
at removed statements while preserving surviving parsed indexes.

Expression-local property writes keep the caller's statement/node maps because
no statement is removed or reordered. Array folding deletes whole write
statements, so its adapter remaps statement indexes to the declaration and
shifts later entries. Callers must not reuse a map after a structural edit; the
contract is “preserved”, “regenerated”, or “invalidated”, never “probably still
fine”.

## Extending a pass

Before adding a shape:

1. Write the exact emitted pattern down, including the surrounding statement
   position. Do not accept “looks like” variants.
2. Prove binding identity through the semantic graph and state which references
   are reads, writes, captures, or escapes.
3. Prove evaluation order. If the rewrite can move a call, getter, TDZ check, or
   `this` binding, decline unless the movement is separately justified.
4. Preserve source text for literals and RHSs; do not re-render values from Oxc
   cooked data.
5. Guard comments in every deleted span and reparse the output.
6. Define the status/counter outcome and the map behavior.
7. Add a positive test, a near-miss decline test, and a differential test when
   the rewrite changes runtime behavior.
