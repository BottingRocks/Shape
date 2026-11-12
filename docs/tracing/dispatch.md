# Dispatch preparation and invocation

Dispatch turns source-backed facts into bounded execution. This reference covers
the complete-dispatch driver and the mechanics it shares with prefix tracing.
Start with the [module guide](../../src/dispatch/README.md) for file ownership or
[trace CLI](trace-cli.md) for commands.

Complete dispatch uses an immutable image and a caller-declared terminal selector.
Prefix tracing supports proven image patches and structured control flow; a
prefix walk can also complete and become eligible for emission. Neither result
establishes whole-capture coverage. Source-free artifacts support prefix mode.

Read by task:

- [Prepare and invoke](#the-public-api)
- [Fetch and decode](#shared-step-mechanics-steprs)
- [Prove complete dispatch](#complete-dispatch-preparation-preparers--proofrs)
- [Interpret completion](#complete-dispatch-execution-executers)
- [Construct a thread](#thread-construction-threadrs)

Prefix-specific details live in [preparation](prefix-preparation.md),
[execution](prefix-execution.md), [diagnostics](prefix-diagnostics.md), and
[kernel artifacts](../deob/kernel-artifacts.md).

## The public API

This is the contract you build against: the entry points, the call shape, and the
items you reach for. Everything named below is re-exported from
[`mod.rs`](../../src/dispatch/mod.rs) — reach it through `shape::…`, not through the private
submodules.

### Entry points

| You have | Call | You get |
| --- | --- | --- |
| A `PipelineReport` and a terminal selector | `prepare_dispatch_from_report(source, &report, start, completion, limits)` | `DispatchPreparation` |
| A `PipelineReport`, no terminal selector | `prepare_prefix_from_report(source, &report, limits)` | `PrefixPreparation` |
| A saved report for JavaScript dispatch | `prepare_prefix_from_saved_report(&report, limits)` | `PrefixPreparation` |
| Source proofs already run | `KernelArtifact::compile_from_raw(source, &report, limits)` | `KernelArtifact` (source-free) |
| Only artifact JSON | `KernelArtifact::from_json(&json)` then `prepare_with_limits(limits)` | `PrefixPreparation` |

### The intended call shape: prepare → bind → invoke

Preparation is the expensive, one-time step. Binding an entry is cheap and
reusable, so the API separates them.

These are independent function-body fragments; each assumes the named caller
inputs and a compatible `Result` return type. Complete dispatch can reuse its
proofs at a different validated coordinate:

```rust
let prepared = prepare_dispatch_from_report(
    source, &pipeline_report, start, completion, DispatchLimits::default(),
)?;
let first = prepared.execute(TraceContext::new(), TraceLimits::default());
let invocation = prepared.with_entry(DispatchStart { pc, state })?;
let next = invocation.execute(context, TraceLimits::default());
```

Prefix tracing binds the descriptor and explicit captures before invocation:

```rust
let prepared = prepare_prefix_from_report(
    source, &pipeline_report, PrefixLimits::default(),
)?;
let thread = prepared.thread(entry, descriptor, inputs)?;
let result = thread.invoke(arguments)?;
```

For source-free tracing, load and validate the artifact, then bind through the
preparation carrying the requested limits:

```rust
let artifact = KernelArtifact::from_json(&artifact_json)?;
let prepared = artifact.prepare_with_limits(limits)?;
let thread = prepared.thread(entry, descriptor, inputs)?;
let result = thread.invoke(arguments)?;
```

`invoke`/`build_context` accept owned values or references, and the argument
iterator is bounded at `max_invocation_arguments + 1`, so an infinite iterator is
safe to pass.

### The pieces you'll want

| Need | Public items |
| --- | --- |
| Prepare | `prepare_dispatch_from_report`, `prepare_prefix_from_report`, `prepare_prefix_from_saved_report`, `KernelArtifact` |
| Bind an entry | `DispatchStart`, `DispatchPreparation::{with_entry, thread}`, `PrefixPreparation::thread`, `DispatchEntryError` |
| Describe a thread | `ThreadDescriptor`, `ThreadInputs`, `CapturePolicy`, `ThreadInvocation`, `PrefixThreadInvocation`, `ThreadStartupError` |
| Invoke | `DispatchInvocation::{execute, execute_with_debug}`, `PrefixThreadInvocation::{invoke, invoke_with_debug, build_context}`, `TraceContext`, `TraceLimits`, `TraceDebugOptions` |
| Declare completion | `CompletionContract`, `CompletionValue` |
| Read results | `DispatchTraceReport`, `PrefixTraceReport`, `DispatchExecutionRecord`, `PrefixExecutionRecord`, `PrefixStepEffect`, `PrefixBranch`, `PrefixBranchKind`, `ImagePatchRecord` |
| Classify a stop | `DispatchStopReason`, `PrefixStopReason`, `DispatchTraceReport::is_complete_completion`, `PrefixTraceReport::stop_detail` |
| Bound the work | `DispatchLimits`, `PrefixLimits` |
| Handle failure | `DispatchPrepareError`, `PrefixPrepareError`, `KernelArtifactError`, `KernelArtifactCompileError` |
| Artifact metadata | `ARTIFACT_FORMAT`, `COMPILER_PROVENANCE`, `MAX_ARTIFACT_BYTES`, `DescriptorRecord`, `EntryFacts` |
| Reusable artifact | `KernelArtifact::{from_json, to_json, prepare, prepare_with_limits, instantiate, entry_facts, descriptor_records}` |
| Cost accounting | `PrefixRetraces`, `PrefixTraceReport::{wasted_steps, image_digest, image_divergence, cell_writes, cell_reads}` |

### Choices baked into the API

Each of these is a deliberate ergonomics choice — a thing you would otherwise
have to work around.

* **Preparation is immutable and reusable.** `prepare_*` returns a value whose
  fields are private and have no mutating accessor. You read validated state
  through getters, and you cannot accidentally swap the proof behind a running
  invocation.
* **Binding is separate from preparation.** `with_entry`/`thread` validate a
  coordinate and return an invocation that reuses the prepared image, table,
  operand descriptors, recipes and completion contract. Continuing from a new
  coordinate costs one validation, not a reparse.
* **The completion policy travels with the preparation.** `with_entry` accepts a
  new entry but *not* a new completion contract: a caller cannot silently loosen
  the completion rule a run is judged by.
* **One report type, one completeness predicate.** `is_complete_completion` is
  the single place "is this a real, finished run?" is decided; you never
  re-derive it from the stop reason.
* **Diagnostics are first-class.** `stop_detail()` returns the last diagnostic
  message for a prefix stop; the full `TraceReport` carries the ordered
  diagnostics.
* **Errors are typed and pre-commit.** `DispatchPrepareError`,
  `PrefixPrepareError`, `ThreadStartupError` and `DispatchEntryError` are returned
  before any handler runs; a rejected request never leaves partial state.
* **The artifact is a first-class runtime input.** `KernelArtifact` compiles once
  and loads without Oxc; `entry_facts`/`descriptor_records` carry the source entry
  and every descriptor row (including typed-invalid rows), so a report-only
  consumer never re-parses `report.output`.
* **Instrumentation is not a second code path.** The `*_with_debug` variants take
  a `TraceDebugOptions`; the plain variants are exactly
  `*_with_debug(…, TraceDebugOptions::disabled())`.

## Shared step mechanics (`step.rs`)

A dispatch step reads an opcode from the image, resolves the current
`state -> opcode -> cell` coordinate, decodes the selected handler's operands, and
applies its PC/state effect. `step.rs` owns that mechanics once, shared by both
drivers.

### `VisitKey`: a position is PC + state + call stack

```rust
pub(super) type VisitKey = (usize, usize, Vec<CallFrame>);
```

The VM call stack is part of the position on purpose. Calling the same
subroutine from two different sites legitimately revisits its `(pc, state)`;
that is progress, not a loop. This is why a shared subroutine can be traced once
as a function and called many times (`step.rs` test
`the_call_stack_distinguishes_repeated_positions`).

### The prepared selector index

`SelectorIndex` maps a `u32` selector to the first position in a prepared handler
list. It is built once during preparation and reused for every step.

* `Dense(Vec<Option<usize>>)` when the selectors are dense enough — a direct
  array index.
* `Sparse(BTreeMap<u32, usize>)` when the selector set is sparse — otherwise a
  lone `u32::MAX` selector would demand a multi-gigabyte vector.
* The dense/sparse choice is bounded by `DENSE_INDEX_MAX_SLOTS` (`1 << 16`) and
  `DENSE_INDEX_GROWTH` (`4`): a dense table may be at most four times the handler
  count and at most 65 536 slots.
* A duplicate selector keeps its **first** position, preserving the previous
  `iter().find(..)` behavior for duplicate selectors.

### Fetch: policy-free vs. policy-applying

`fetch_cell` does the raw lookup (opcode byte, table row, cell) with no revisit
or next-state-bounds checks. `fetch_step` wraps it for the bounded drivers: it
records the `VisitKey` **first** (so a repeated coordinate is reported before any
image/table lookup), then bounds-checks the cell's `next_state`.

### Completion coordinates

`completion_coordinates` and `COMPLETION_RECIPE_IDENTITY` ("dispatch-completion")
give the terminal selector step one shared definition: the operand entry and the
successor PC both advance past the opcode byte, and the record carries no
operands. The complete-dispatch driver builds the record from these coordinates.

### Step faults, mapped per driver

`fetch_step`, `decode_step`, `checked_pc_increment`, `find_operand_handler` and
`find_template` report faults as an internal `StepFault`
(`TruncatedImage`, `MissingState`, `MissingOpcode`, `InvalidNextState`,
`PcOverflow`, `TruncatedOperands`, `UnsupportedOperands`, `NextPcOverflow`,
`MissingOperandHandler`). Each driver maps a fault to its **own** stop type,
diagnostic code and message: `dispatch_fault` in `execute.rs` and `prefix_fault`
in `prefix/execute/`. The two mappings deliberately produce different
observable output (for example the `dispatch-*` vs `prefix-*` diagnostic code
families) even though the underlying mechanics are one implementation.

## Complete dispatch: preparation (`prepare.rs` + `proof.rs`)

Reach for this when the kernel completes through a caller-declared sentinel (see
[driver selection](../../src/dispatch/README.md#choose-the-execution-path)). Preparation
is where the "proven program" is assembled and where required facts are proved or the request is rejected. Execution also
checks bounds defensively; those checks do not replace preparation.

`prepare_dispatch_from_report(source, report, start, completion, limits)` is the
only entry point. It re-derives every fact from the standardized report; nothing
is manufactured to make a fixture pass. The value it returns,
`DispatchPreparation`, has **private fields and no mutating accessor**, so a proof
cannot be silently swapped for a fabricated program after preparation. Callers
read validated state only through getters.

Preparation checks bounds as it builds the validated representation:

1. **Runtime facts.** `runtime_config::from_report` extracts the typed fact set.
2. **Exactly one dispatch table.** The dispatch image must be exactly one
   `Extracted` `DispatchTable` fact. There is no spelling fallback and no
   synthesized table.
3. **Table shape and bounds.** `parse_dispatch_value` builds
   `Vec<Vec<DispatchCell>>` from the literal `state -> opcode -> [next_state,
   selector]` value. Every cell is validated: nonempty rows, no empty state row,
   cell count ≤ `max_table_cells`, `next_state` inside the proven state count, and
   any `next_state`/`selector` ≤ `max_integer`. The entry state must be inside the
   table.
4. **Prove the read, bind the table.** `proof::prove_dispatch_read` finds exactly
   one nested dispatch read and resolves the method that owns it. The table
   fact's recorded span must resolve to that exact `SymbolId`
   (`require_binding_at_span`), the table binding must be immutable
   (`prove_immutable_binding`), and the fact's display binding must equal the
   proven read's table name.
5. **The image.** Either an `Extracted` `JsBytecodeImage` fact (bytes from a
   proven base64 decoder call, whose binding is joined to the proven read) or a
   direct bounded numeric-literal array (`prove_direct_bytecode`, which also
   rejects reassignment, member writes, updates, `delete`, and escapes of the
   image or any binding aliased from it). The image must be nonempty, ≤
   `max_image_bytes`, and contain the entry PC.
6. **Exactly one operation table.** `single_operation_table` requires exactly one
   proven table; `validate_operation_table_declaration` re-checks it by its
   code-generation-stable declaration **ordinal** (not by name), rejecting
   shadowed or decoy declarations.
7. **Handler recipes + operand descriptors.**
   `handler_templates_from_report` and
   `js_operand_separation_from_report_with_handoff_and_templates` reconstruct the
   recipes and operand descriptors, joined through the JS operand binding handoff
   from the proven read (`OperandEntryCoordinate::AfterOpcode`).
8. **One source identity.** The runtime config, template report and operand report
   must all agree with `report.input_hash`, or preparation fails with
   `SourceJoin`.
9. **Merge lifted templates.** `merge_lifted_templates` replaces any handler
   template whose operand separation is `Lifted` with that lifted template, so
   the walk uses the freshest proven recipe.
10. **Prepared indexes.** `template_index` and `operand_index` are built once
    over the handler indices.
11. **Table identity agreement.** `templates.table`, `operands.table` and the
    proven operation table must be identical.
12. **Completion contract.** `validate_completion` checks that the terminal
    selector collides with no operation-table handler, is present in the proven
    table, that handler identities are unique, and that every non-sentinel
    selector has a handler.
13. **All handlers lifted.** `require_lifted_handlers` demands that every
    non-sentinel selector has an operand handler whose status is `Lifted`;
    otherwise the run could reach a handler it cannot faithfully execute.
14. **Selector join + completion span.** `proof::prove_selector_join` proves the
    one consumer function and returns the real source span of the proven
    dispatch consumer, used as the completion record's coordinate. It is never a
    fabricated origin.
15. **Constant string tables.** `constant_string_tables` registers every
    extracted, unmutated string table so the tracer can resolve `table[index]`
    with a concrete index to a string literal.

### What the proofs in `proof.rs` establish

* **`prove_dispatch_read`** — exactly one nested dispatch read
  `table[this.S][bytecode[this.P++]]`, and a method installed under exactly one
  named property (`Object.defineProperty(target, "name", { value: fn })` or
  `target.name = fn`). The method body must be exactly the three-statement
  template `var record = table[this.S][bytecode[this.P++]]; this.S = record[0];
  return record[1];` (with an optional single transparent block wrapper). A decoy
  read placed in a spread-out target — assignment, update, destructuring, `for-of`
  — is still a candidate and makes the program ambiguous, so preparation rejects
  it rather than silently accepting the apparently valid method.
* **Prototype-alias resolution** — `var alias = Ctor.prototype` is admitted only
  when the alias is declared exactly once and never mutated; a chain that
  revisits an alias is a cycle and is left unresolved instead of looping.
* **Image immutability** — `tracked_aliases` + `MutationScanner` reject
  reassignment, member/index writes, updates, return escapes, call-argument
  escapes, unsupported destructuring, and use as a constructor/template/call
  callee. This is what makes the committed image trustworthy.
* **`recover_operation_table`** — when the input is already deobfuscated output
  and the FIND/RENAME provenance is gone, re-derive the same
  code-generation-stable identity only when the source contains exactly one
  consumed table with exactly the expected shape.
* **`prove_selector_join`** — the selector produced by the proven dispatch method
  invoked on a context is the selector indexing the proven operation table,
  called with that same context. The accepted consumer is intentionally tiny:
  one unconditional `for (;;)` body with exactly `selector = context.next()`, one
  selector-sentinel return guard, and one `operations[selector](context)` call.
  This prevents a matching pair hidden under an arbitrary branch or after an early
  return from becoming execution authority.
* **`prove_consumer_context_origin`** — every call of the selected consumer
  receives an instance of the constructor whose prototype owns the installed
  dispatch method. It is a deliberately small origin analysis: direct
  `new C(...)`, transparent local bindings, and one bounded chain of function
  parameters. An object with a method of the right spelling is never enough.

## Complete dispatch: execution (`execute.rs`)

`DispatchPreparation::execute` (and `execute_with_debug`) validate the prepared
entry once, then run. `DispatchPreparation::with_entry` binds a **different**
validated coordinate to the same immutable preparation and returns a
`DispatchInvocation`, so a caller can continue from a supplied coordinate without
reparsing or rebuilding any proof-backed artifact. The completion contract always
remains the one validated by the preparation.

One `TraceSession` owns the context and arena for the whole run, so prior
committed state and events are never replayed. The registered constant string
tables are attached to the session up front.

The run loop:

1. **Defense in depth.** It recomputes the table cell count and re-checks the
   image/table bounds even though preparation already validated them. A violation
   is a driver-level budget halt (`DispatchTableOverflow`,
   `DispatchPrepareBudget`), never a silent success.
2. **Fetch.** `step::fetch_step` records the `VisitKey` and fetches the cell. A
   fault becomes a typed stop.
3. **Completion before operand lookup.** If the cell selector equals the
   completion selector, the driver derives the completion coordinates, finalizes
   the stack (`finalize_stack`) or `undefined` (`finalize_undefined`), and commits
   the single terminal record. A `StackIndexOutOfRange`/`ValueGraphUnavailable`
   becomes a budget halt; an unsupported completion becomes an unsupported halt;
   an already-stopped session is classified by `existing_stop_reason`.
4. **Handler step.** Otherwise it finds the operand handler and template through
   the prepared indexes, decodes operands, sets the VM cursor, and executes the
   handler through the session.
   * `Continued`/`Returned` — a jump, call or return continues at the handler's
     assigned PC/state (taken from `take_control_transfer`); otherwise the VM
     falls through. A committed record is pushed and the cursor advances.
   * `Thrown` — committed: its effects and terminal record are retained and
     reported with the failed coordinates.
   * `Partial` — the tracer already recorded the unsupported diagnostic and
     rolled back the attempt, so **no record is committed** and the preceding
     prefix is intact.
   * `BudgetStopped` — a budget halt.
5. **Step budget.** When the loop exhausts `max_dispatch_steps` without a
   completion, the driver records `DispatchStepBudget`.

`DispatchTraceReport::is_complete_completion` is true **only** for a full,
selector-terminated run whose completion value was committed
(`CompletedSelector` + `TraceCompletion::Returned`). A truncated/budget/unsupported
halt is never reported as complete.

## Thread construction (`thread.rs`)

`DispatchPreparation::with_entry` binds a validated coordinate. `thread.rs`
supplies the missing **construction** step: a caller-supplied `ThreadDescriptor`
plus a `ThreadInputs` capture snapshot become the incoming `TraceContext` for that
coordinate.

**Provenance honesty:** the descriptor and inputs are *caller-supplied
parameters*, not facts derived from the analyzed source. They are never
advertised as source-proven, and they never replace the prepared dispatch proof
(image, table, operand descriptors, handler recipes, completion contract).

### The canonical construction order

`build_context_from_spec` builds the context in the same order the canonical
`vmCaptureThread`/`vmBuildThreadInvocation` helpers do:

1. set the receiver (`this`; defaults to `undefined`, a plain call),
2. apply the explicit incoming stack snapshot (`None` is a hole, `Some(Undefined)`
   is an explicit value),
3. write the captured-memory snapshot (the per-invocation clone),
4. write the optional aggregate `arguments` object into `arguments_slot`,
5. initialize `initial_slots` to `undefined` (**replacing** any captured cell),
6. write positional invocation arguments, then `undefined` for the missing ones.

Positional arguments follow the canonical `Math.min` mapping: only the first
`argument_slots.len()` values reach the slots, and extra arguments within the
documented bound are accepted and ignored. Every `argument_slots` target must
**already own a cell** — from capture/result/aggregate/initialization — because
the canonical `writeSlot` updates an existing cell rather than creating one; a
missing target is rejected (`MissingArgumentSlot`) instead of silently allocated.

### Bounds and rejection

`validate_descriptor` rejects, before any context is built:

* a `CapturePolicy::LiveShared` request — this API captures values, not live
  shared closure cells (`UnsupportedSharedCapture`),
* slot lists or an incoming stack over the configured bounds,
* an `arguments_slot`/`result_slot` selected without its value, or a value
  supplied without its slot,
* duplicate captured slots, a captured slot with no explicit value, or an
  incoming value that names a non-captured slot,
* a positional target with no existing cell,
* any caller-supplied `Computed` node (it can only belong to a foreign arena),
* an `arguments`/self value that is not an opaque `Object`/`External`.

`collect_invocation_arguments` stops the argument iterator at `limit + 1`, so an
oversized or infinite iterator never runs away; `ThreadInputs::max_invocation_arguments`
overrides the bound, otherwise it derives from `limits.max_stack_length` (4096 by
default). A rejected request leaves no partial state.

**Scope honesty:** this is an invocation snapshot, not live shared
closure-cell alias support. Captured values are copied into a fresh context per
invocation; requests that purport to model a live shared cell are rejected. The
snapshot API does not model live shared parent memory.

## Contracts to preserve

* **A step is one proven coordinate.** A dispatch step reads an opcode, resolves
  the `state -> opcode -> cell`, decodes that handler's operands, and applies its
  PC/state effect. Only the terminal selector satisfying the declared completion
  contract can mark complete dispatch complete.
* **Different completion rules.** Complete dispatch requires a caller-declared
  terminal selector and completion value. Prefix tracing completes when every
  explored path returns or continues a loop. Do not infer one driver's completion
  from the other's.
* **Inputs are not facts.** `ThreadDescriptor` and `ThreadInputs` are caller
  inputs, not source-proven capture facts. The snapshot API does not model live
  shared parent memory.
* **Proof, not spelling.** Every join is by `SymbolId` or a code-generation-stable
  declaration ordinal. An ambiguous program is rejected, never resolved to the
  first match.
* **No execution of input.** JavaScript and WebAssembly are never run; embedded
  WASM is lifted into recipes. The emitter renders trace facts; it is never an
  input to tracing.
* **Honest stops.** Truncation, address-out-of-bounds, an unsupported reached
  handler, a repeated `(pc, state)`, a partial recipe, or the budget all stop
  before commit and are reported as the partial results they are.

## Testing and fixtures

Complete dispatch is exercised by `execute.rs`'s own tests plus the shared step
tests. Prefix tracing's behavior contract is covered by `tests/vm_branches.rs`
(if/else merges, nested `if`s, loops, loop variables, fake branches by test and by
context, try/catch, try/finally, try variables, throws, out-of-line `then`
blocks), `tests/prefix.rs` (patch-then-consume, per-invocation image isolation,
truncation/address/budget no-commit, emitted-JavaScript refusal on a budget
prefix, run-loop/declaration/reference near-miss rejections), and
`tests/fresh_stack_proof.rs`. The tracked JavaScript VM fixtures
(`tests/fixtures/javascript-vm-prefix.raw.js`, `...-write-selector.raw.js`,
`...-branches.raw.js`) preserve the topology without baking in a selector
constant. The real-capture oracle test is ignored by default and requires
`SHAPE_PREFIX_SEED`.
