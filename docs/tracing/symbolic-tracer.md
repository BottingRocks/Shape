# Symbolic tracer

The symbolic tracer turns admitted VM handler recipes into a graph of values
and control flow, then emits JavaScript from eligible traces. It carries
explicit stack and memory state between handlers. Dispatch drivers supply the
instructions and decide how to walk branches, loops, and VM calls.

This guide starts with a small result you can reproduce, then explains the
state model, emission, and driver contracts. For whole-capture commands, use
the [trace CLI](trace-cli.md). For implementation locations, use the
[symbolic module guide](../../src/tracing/symbolic/README.md).

## Choose the tracing boundary

There are three useful entry points. Choose according to what is already
known about the input:

| You have | Start with | Responsibility |
| --- | --- | --- |
| Admitted recipes and decoded operands | [Direct recipe tracing](#direct-recipe-tracing) | Execute one handler or an explicit sequence |
| A source-proven JavaScript dispatch loop and completion selector | [Bounded JS dispatch](#bounded-js-dispatch) | Decode and walk the bounded complete-dispatch contract |
| A capture or compiled kernel artifact needing a thread walk | [Prefix tracing](#prefix-tracing-and-capture-coverage) | Walk supported control flow and report completion or the exact stopping boundary |

All three use the same symbolic execution model. A preparation proves how to
read instructions; a session records what those instructions do; the emitter
projects an eligible report into JavaScript. None of these stages invokes
Node, a browser, or a native JavaScript/WASM runtime.

A symbolic input can remain unknown in value while still having a valid
representation in the output. For example, an external argument `x` can
produce the expression `x + "!"` without knowing the caller's string. This is
different from an unsupported operation whose behavior the tracer cannot
represent safely.

## Read the synthetic fixture

The [symbolic tracer module](../../src/tracing/symbolic/README.md) exercise direct recipe
tracing. For runnable capture workflows, use the [trace CLI](trace-cli.md).

### What the fixture does

The [fixture](../../tests/fixtures/symbolic-trace.raw.js) contains two handlers.
The first appends `"!"` to its operand and stores the result on the VM stack.
The second initializes a memory slot, copies the stack value into it, and
returns the slot's value.

A direct recipe caller supplies the external operand `x` and slot number `4`.
The standalone fixture has no surrounding constructor proof; callers must supply
the operation table and context-role hand-off before extracting templates.
This does not automatically admit arbitrary arrays of functions.

Tracing these two handlers can emit the following function:

```js
function trace(x) {
  let $SSA_0 = x + "!";
  let $SSA_1 = 0;
  let $SSA_2 = $SSA_0;
  let $SSA_3 = $SSA_2;
  var $SLOT_4 = $SSA_3;
  $SLOT_4 = $SSA_3;
  return $SLOT_4;
}
```

### Why the output still has copies

The important result is that the computation survives the stack-to-memory
transfer and becomes a return value. The generated function does not need a
VM context object to replay those storage operations.

The copies and slot assignments preserve trace materialization. Default
emission does not promise dead-value elimination or SSA cleanup; optional
binding simplification and the separate emitted cleanup passes can reduce
them later. `handlers_traced=2` counts handler occurrences, while `events=5`
counts recorded observations. Neither count is the number of output lines.

## Direct recipe tracing

Use the direct API when another component already owns instruction decoding.
`handler_templates_from_report` supplies the recovered handler recipes;
`VMInstruction` supplies one occurrence's selected handler and operands.

### Handler definitions and instruction occurrences

A `HandlerIR` is the recovered program for a handler and can be reused many
times. A `VMInstruction` is one execution input: `handler_index` plus
`operands`. It is the preferred public name for `DecodedInstruction`, a type
alias with identical behavior and type identity.

The operand vector contains the source handler's parameters **after** its
context parameter. Recipe operand position `1` is therefore decoded operand
`0`. Dispatch PC and provenance belong to the dispatch record; they are not
additional fields on `VMInstruction`.

For the templates extracted by the example above, the execution portion is:

```rust
use shape::{trace_handlers, SymbolicValue, TraceContext, TraceLimits,
            VMInstruction};

let instructions = [
    VMInstruction::new(0, vec![SymbolicValue::External { name: "x".into() }]),
    VMInstruction::new(1, vec![SymbolicValue::Number(4.0)]),
];
let trace = trace_handlers(
    &templates,
    &instructions,
    TraceContext::new(),
    TraceLimits::default(),
);
```

This is a function-body fragment with an existing `templates` report. It
shows the execution boundary, not template discovery. The tracer does not
infer operand widths, decode a raw bytecode format, or assign semantic
meaning to the numeric handler key.

### What is shared and what commits

A sequence shares one arena, stack, and memory store. Temporary locals reset
for each handler, so a local from one occurrence cannot accidentally become
an input to the next.

Each handler is preflighted for unsupported fragments, including nested
`SemanticValue::Unknown` values. If execution then encounters an invalid
input, unsupported operation, or budget stop, the handler attempt is
transactional: earlier committed handlers remain, and the failed attempt's
state changes are rolled back.

In the direct sequence API, return and throw end the sequence. A dispatch
loop needs a continuing session because a handler's ordinary return can mean
“back to the dispatcher.” The distinction is covered under
[VM function returns](#branches-and-vm-function-returns).

## Context, limits, and reports

### Construct state deliberately

`TraceContext` distinguishes a stack hole, an absent memory slot, and a slot
initialized to `undefined`. Stack holes and explicit `undefined` values can
read alike in JavaScript while still recording different VM actions. A
missing memory cell is more consequential: a recipe write cannot create it.

Use `initialize_memory_slot` to create or replace a cell with initialized
`undefined`. Calling it on an existing value discards that value. Use
`set_memory_value` when the incoming cell should contain a specific value.
Externally supplied indices and lengths should go through the checked
`*_bounded` setters.

These distinctions matter at startup and at joins. Treating a missing cell as
an initialized one can turn a failing write into a successful operation;
treating initialization as a no-op can preserve a capture that the VM would
have overwritten.

### Choose the right input value

| Value | Meaning to the tracer |
| --- | --- |
| `Undefined`, `Null`, booleans, numbers, strings | Concrete inputs with their JavaScript distinctions preserved |
| `External { name }` | An opaque valid input whose name must be admitted by the emitter |
| `Object { identity }` | An existing object-like identity, preserved across references |
| `Computed { node }` | A value belonging to this trace's arena; valid internally, rejected as an externally supplied input |
| `Unknown { reason }` | Unsupported input that stops the trace with a diagnostic |

A computed node ID is local to its arena. Copying it from another report into
a new context or decoded operand would not copy the computation, so the API
rejects it. Within a session, computed values can move between stack and
memory while retaining the same node identity.

### Apply budgets at the right layer

`TraceLimits` defaults to 10,000 recipe steps, 50,000 constructed nodes, a
stack length of 4,096, and 4,096 allocated memory slots. These are execution
and state limits; dispatch drivers have their own instruction/walk budgets.
One decoded instruction can execute several recipe steps.

A limit hit is an explicit result. Increasing a budget may allow a longer
supported trace, but it cannot make an unsupported recipe or missing proof
valid. For a driver stop, inspect the driver's reason as well as the inner
symbolic report.

### Read completion before emission

`TraceReport` contains the arena, completion, diagnostics, committed handler
count, external inputs, and a final state summary. `final_state` reports stack
length and memory-slot count; it is not a serialized context to resume later.

| Completion | Meaning |
| --- | --- |
| `Completed` | The supplied work completed under this API's contract; direct sequence exhaustion does not prove a whole VM function returned |
| `Returned(value)` | A terminal return was recorded |
| `Thrown(value)` | A terminal JavaScript throw was represented, rather than an unsupported-analysis failure |
| `PartialUnsupported` | Execution stopped at an unsupported operation or input |
| `BudgetStopped` | A symbolic execution budget was exhausted |
| `PrefixStopped(code)` | The owning driver stopped before a terminal outcome |

The last three are ineligible for `emit_javascript`. A returned or thrown
report still has to pass arena validation and external-input checks. Calling
`report()` alone does not establish that a driver reached its own completion
contract.

### Nodes, statements, and events

The arena gives nodes, statements, and events stable IDs within the report,
with source, handler, and step provenance. Nodes hold values; statements
organize emitted computation and control flow; events retain ordered VM
observations such as reads, writes, and slot initialization.

An event need not become a line of JavaScript. A stack read can resolve to a
previously recorded value node, and a write can become a binding or shared-cell
assignment. To explain an observation, follow its event provenance and the
nodes it consumed or produced instead of looking for a matching output line.

## JavaScript emission

`emit_javascript` builds a function/body with Oxc `AstBuilder` and prints it
with Oxc codegen. It consumes the trace's established values and control flow;
it does not rerun source extraction or recover a missing instruction.

### External inputs and object identity

The example's external `x` must be explicitly allowed:

```rust
use shape::{emit_javascript, EmitOptions};

let emitted = emit_javascript(
    &trace,
    &EmitOptions {
        external_parameters: vec!["x".into()],
        ..EmitOptions::default()
    },
)?;
println!("{}", emitted.code);
```

This fragment continues the direct example in an error-returning function.
`external_parameters` is an allow-list, not a source of values to execute.
Invalid or colliding names are sanitized deterministically in one namespace
shared by the function, parameters, and generated locals.

Existing object identities become stable function parameters, normally named
`object_<identity>` with hygiene suffixes when needed. They never become fresh
`{}` literals, which would change identity. `EmittedJavaScript::object_parameters`
exposes the identity-to-parameter mapping to the caller.

`EmitOptions::context_parameter` remains for source compatibility and is
ignored. Stack and memory effects are already represented in the graph, so
the emitted function has no generic VM context parameter. Supported operations
can still need helpers, such as `vmForInKeys`.

### Materialization and provenance

Computed recipe values are materialized at their recipe step. Later transfers
reuse their immutable bindings, preserving the value that was observed even
if the VM cell subsequently changes. Shared-cell reads after escaping closure
calls need explicit proof before an earlier value can be reused.

The emission result contains `code`, `statement_map`, `node_map`, and
`object_parameters`. Keep those maps with the generated code when explaining
its origin. A later cleanup that removes or moves code must update provenance;
see the [emitted cleanup module](../../src/transforms/emitted_js/README.md).

### Binding simplification

`EmitOptions::simplify_bindings` is off by default. When enabled, an
unreassigned binding containing a constant, a safe copy, or a free name can
be substituted at its reads. A pure binding with no readers can be omitted.
The simplifier works on the graph and reaches nested blocks.

The default preserves statement-level structure for provenance and later
cleanup. Callers can enable simplification for completed walks. This option
does not bypass completion or arena checks.

### Validation before code generation

Public arenas can be constructed manually, so emission validates them
iteratively before building output. It checks references, operator kinds,
cycles, expression depth (256), and expanded DAG cost (100,000 nodes).
A small shared graph can still be too expensive to expand into an expression.

NaN, infinities, and negative zero use numeric AST expressions whose meaning
cannot be changed by a parameter named `NaN` or `Infinity`. Earlier parse and
recipe boundaries also enforce depth/work limits, including a lexical guard
against pathological nested unary expressions before Oxc recursion.

## Bounded JS dispatch

`prepare_dispatch_from_report` connects runtime-config facts, operand
separation, and handler-template proofs. Use it when the source satisfies the
complete-dispatch contract and the entry and completion policy are explicit.

### What preparation proves

Preparation needs a unique static nested lookup
`state -> bytecode[pc++] -> [next_state, selector]`, immutable image/table
bindings, a shape-checked callable operation initializer, and a symbol-checked
join from the returned selector to that operation table.

The admitted runner is narrow: an unconditional `for (;;)` with one
`context.next()` binding, one selector return guard, and one
`operations[selector](context)` call. Direct numeric bytecode arrays are
accepted when the consumer proves their declaration identity. Mutation,
ambiguity, aliasing, or shadowing that breaks the proof is rejected.

The prepared driver reads the opcode, decodes operands at the proven PC
coordinate, and advances by the lifted `pc_delta`. A handler's ordinary
return goes back to dispatch. The caller supplies `DispatchStart { pc, state }`
and a `CompletionContract::SelectorSentinel`; the requested selector and
result policy must agree with the source proof.

### The complete-dispatch fixture

The [tracked fixture](../../tests/fixtures/dispatch-trace.raw.js) dispatches
handlers 0 and 1 through nonidentity state changes, then reaches selector
`255`. Its completion policy returns stack slot 0. The first handler stores
`261`; the second adds `7`, producing the terminal result `268`.

The numeric selector is a fixture contract, not a universal VM sentinel.
Preparation does not guess an entry from constructor argument position or an
outer kernel call. Failed attempts do not commit new coordinates or erase
previously completed records.

### Reuse preparation at a known entry

With `prepared` from that fixture, a caller can bind its second handler's
entry and supply the state that should arrive there:

```rust
let invocation = prepared.with_entry(DispatchStart { pc: 3, state: 1 })?;
let mut incoming = TraceContext::new();
incoming.set_memory_value(0, SymbolicValue::Number(100.0))?;
let result = invocation.execute(incoming, TraceLimits::default());
```

This function-body fragment uses the same `SymbolicValue`, `TraceContext`,
and `TraceLimits` imports as above, plus `shape::DispatchStart`. It returns
`107`: the selected handler reads memory slot 0 and adds its decoded operand
`7`.

`with_entry` reuses the image, table, recipes, and source-validated completion
contract. It validates PC/state bounds, but cannot infer a missing caller
continuation or reconstruct state at an arbitrary mid-block coordinate. The
incoming context remains the caller's responsibility.

### Thread startup and context construction

Use `thread` when the incoming context should be constructed from a
`ThreadDescriptor`, capture values, and positional arguments. The preparation
remains reusable; each invocation builds its own context.

Continuing with the same fixture preparation:

```rust
use shape::{ThreadDescriptor, ThreadInputs};

let descriptor = ThreadDescriptor::new()
    .initial_slots([0])
    .argument_slots([0]);
let thread = prepared.thread(
    DispatchStart { pc: 3, state: 1 },
    descriptor,
    ThreadInputs::new(),
)?;
let result = thread.invoke(&[SymbolicValue::Number(100.0)])?;
```

This also returns `107`. Initialization creates slot 0 as `undefined`, then
the positional argument overwrites it with `100`. For a symbolic argument,
use `External { name: "arg0".into() }` and include `arg0` in the emitter's
allow-list; the addition remains in the output.

The root uses the same construction path: pass `prepared.start()` as the
entry. The descriptor and incoming values are **caller-supplied**, not facts
proved by dispatch preparation.

### Startup order and overlapping slots

The builder follows the canonical `vmCaptureThread` /
`vmBuildThreadInvocation` order:

1. Copy each `capturedSlots` value from the explicit incoming snapshot.
2. Capture the optional `resultSlot` self/function value.
3. Clone the captured memory for this invocation.
4. Write the optional `argumentsSlot` aggregate object.
5. Initialize `initialSlots`, replacing any earlier cells with `undefined`.
6. Write positional `argumentSlots`, using explicit `undefined` for missing
   arguments.

Order determines the result when roles overlap. A captured `5` in slot 0
becomes `undefined` if slot 0 is also initialized; a later positional `100`
replaces that value again. Initialization is not a “create only if absent”
operation.

Every positional target must already have a cell from an earlier stage.
Otherwise construction returns `MissingArgumentSlot`. Duplicate positional
slots are allowed and written left to right, so the last write wins.
The stack starts empty unless the caller supplies an incoming stack snapshot.

### Captures, arguments, and receiver values

Each captured slot requires exactly one explicit incoming value, including
`SymbolicValue::Undefined` when appropriate. Missing or extra captures,
duplicate captured-slot declarations, unknown values, and out-of-bound inputs
are rejected before a handler runs.

Only the first `argumentSlots.len()` positional arguments are written.
Extras are accepted and ignored within the invocation bound, but still
validated: a caller-supplied `Computed` is invalid even when an extra argument
or later write would discard it. `max_invocation_arguments` defaults to the
supplied `max_stack_length`; at most `bound + 1` iterator values are consumed
to detect overflow before unbounded collection.

The receiver defaults to `undefined`, matching a plain invocation. Handlers
read `receiverValue` as supplied; `receiverObject` yields `host` for
null/undefined, preserves an object, and otherwise represents `Object(value)`.
A manually built `TraceContext` has no receiver until `set_receiver`; reading
an absent receiver stops the handler.

The aggregate arguments object and self/function value must be opaque
object-like inputs (`Object` or `External`), not primitives. They are not
simulated arrays or functions. The startup API captures snapshots and rejects
`CapturePolicy::LiveShared`; the prefix/corpus shared-cell analysis is a
separate contract described in the [trace CLI](trace-cli.md#shared-cells).

## Branches and VM function returns

The direct sequence API is enough for a fixed list of instructions. A thread
walk also needs a driver to choose successors, reconcile paths, and decide
when a VM function has ended. `TraceSession` exposes those operations; it does
not choose the walk strategy itself.

### Handler returns and VM returns

An ordinary recipe return returns from a **handler**. `TraceSession::new`
treats it as terminal; `new_continuing` lets a dispatcher consume it and
continue. By contrast, `RecipeOperation::CompleteThread` models the VM
function's return, `vmPendingResult = value`.

`with_thread_control` enables the driver hand-off for program-value branches,
VM returns, and try entries. The driver consumes a branch with `take_branch`
or a VM return with `take_thread_return`; the latter follows emission of
`return value` for that path. Without thread control, these boundaries stop
with `dynamic-branch`, `unhandled-thread-return`, or `unhandled-try`.

A `RecipeOperation::JumpIf` whose test is already known can be followed as a
VM decision. A test that depends on program input needs path handling unless
predicate analysis can establish its outcome.

### Fork, walk, and join

For an undecided condition, the driver follows this lifecycle:

1. `fork` records an `If` and a declaration for possible merge variables,
   returning one `TracePath` per arm.
2. `switch_path` makes an arm current while the driver walks its successors.
3. At a common VM position, `joinable` checks whether the states can merge;
   `join` combines compatible arms.
4. If one arm ended, `continue_alone` closes the fork and continues the other
   path after the conditional.

Compatible arms need matching call/exception state, receiver, stack length,
and memory-slot sets. The driver must also account for its dispatch position
and image state. Equal concrete cell values stay concrete; differing values
become merge variables assigned by each arm.

For example, if one arm leaves slot 0 as `3` and the other as `5`, later
program arithmetic can use the merge variable. If the VM then needs that cell
as a concrete dispatch state or index, the driver may need to separate the
paths again rather than use a symbolic coordinate.

`join_tagged` remembers joins of different concrete values. A later
`dynamic-index` stop can expose the relevant join and cell through `take_split`;
`cells_differ` helps the driver decide whether those paths still need separate
walks. The emitter renders ordinary `if`/`else` structure, can put the
fall-through arm first for a negated test, and omits empty arms.

### Proving a branch outcome

Shape bytecode can contain fake branches whose unused successor leads into
invalid VM state. Before forking, the
[predicate analysis](../../src/tracing/predicate.rs) follows graph bindings and
tracks possible JavaScript types and concrete values.

Useful facts include object/array/VM-function truthiness, `typeof` of a known
type, strict inequality of disjoint types, and rejection of `== null` for
non-nullish values. Self-equality needs care when NaN is possible. String
addition and numeric arithmetic also propagate type information.

Call results usually remain undecided. The current predicate rules retain
result types for `Global` conversion nodes named `Number`, `String`, or
`Boolean`, and for `vmDecodeString`. They do not execute those calls to obtain
a concrete result. Outlined calls can also use facts from their traced bodies.

When the outcome is proved, `PendingBranch::decided` and a `BranchDecided`
event with `BranchEvidence::Test` record the selected successor.

A driver can also reject a path because it reaches a bad VM state. It rolls
back to its checkpoint and records the surviving decision with
`record_fake_branch`, which uses `BranchEvidence::Context`. Keep these reasons
distinct when diagnosing a walk: a test proof and a context-based rejection
are different evidence. `TraceArena::describe` renders a compact expression
with bindings inlined for that inspection.

### Try/catch paths

A handler that pushes a try frame exposes it through `take_try_entry`.
`fork_try` opens body and catch paths; the catch starts with the state that
`vmHandleException` establishes. Cells whose values can change become
variables assigned by body writes and read by the catch.

This prevents a catch from using a stale value from before the try block.
`try_writes` tracks cells changed relative to try entry. A nested try owns its
record and variables, then passes its changes to the enclosing try as it ends.

When the frame is popped, `close_try_body` moves the body path out of the block
and reads widened cells through their variables. Compatible paths can then
join. Popping an empty exception stack yields `empty-exception-stack`, a bad
state for the walking driver; it is not silently ignored.

## Loops, value sets, and subroutines

### Loop-carried cells

A loop must preserve values across iterations without assuming that the first
iteration's constants remain fixed. The driver identifies changing cells and
asks the session to **widen** them: represent each as a variable declared
before the loop.

`open_loop` emits a labeled `for (;;)`. At a back edge, `continue_loop`
assigns the carried variables and emits `continue label`. It also reports
newly changed cells that were not yet widened, allowing the driver to roll
back and trace again with a larger carried set.

`break_loop` emits a labeled break and moves the exiting path after the loop;
`close_loop` leaves the body. Retracing is analysis work and consumes the
walker's budget. It is not evidence that the emitted program will execute
that many iterations.

### Value sets and concrete VM coordinates

Some widened variables have a bounded set of observed concrete values.
`open_loop_with`, `fork_try_with`, and joins can retain that information.
The path's `assume` and `exclude` facts may then resolve a value used as a
VM jump target, dispatch state, or index.

If it remains ambiguous, `take_dispatch` exposes the variable and remaining
values. The driver can split on `equality_test` rather than guess a target.
`take_used_sets`, `loop_values`, and `try_assigned_values` reveal when more
observations require another trace.

Recovery also has state to undo: `stop_marks` / `restore_stop_marks` restore
the stop state around a recoverable handler attempt. Values escaping a block
are carried through declarations outside it, so their generated names remain
in scope. The [scope checker](../../src/tracing/scope.rs) detects undeclared
generated names in emitted code.

### Outlined VM subroutines

`open_subroutine` starts a top-level function and treats caller stack cells
and memory slots as parameters when the body reads them. Caller-only branch
facts and receiver knowledge are not silently imported into the function.
This allows one outlined body to be used at more than one call site.

After a valid return, `close_subroutine` determines the cells read and changed,
finishes the function, and restores the caller with a `SubroutineSummary`.
`call_subroutine` supplies each call site's cells and writes back the results.
Incompatible call or exception state prevents that summary from being reused.

An opaque outlined result can hide a fact needed by a later branch or VM
coordinate. `outlined_call_in` and `take_inline` identify such a call so the
driver can trace it inline. Loop-variable dependencies are copied before
sequential assignments to avoid overwriting a value still needed by another
assignment. The prefix driver owns these outlining and retry decisions.

## Prefix tracing and capture coverage

The complete-dispatch example does not cover every capture. A mutable image,
a different completion protocol, or partially lifted handlers can invalidate
that preparation even when a useful prefix can be traced.

The [prefix driver](../../src/dispatch/prefix/README.md) has a separate contract
for supported image patches, branches, exceptions, loops, and VM calls. It
also lifts structurally admitted WASM interpreters into the shared walker;
it does not run a general WASM engine. The checked-in WebAssembly root is tested
from raw source and from a source-free kernel artifact.

A prefix walk can produce a completed function when every path returns or
continues a represented loop. A stopped prefix remains diagnostic output and
cannot be emitted as though the missing path had completed. Consult
[`PrefixStopReason`](../../src/dispatch/prefix/types.rs) and the trace completion
together.

Calls and property operations represented by admitted recipes may remain
symbolic in emitted code. That does not mean their host behavior was executed
or that arbitrary source expressions are supported. Static
[WASM boundary facts](../deob/runtime-config.md#what-the-current-proof-rules-establish)
alone do not establish the interpreter lift or a complete trace.

## Live diagnostics and observation rendering

Live debug output and stored observation rendering serve different purposes.
Use `TraceDebugOptions::stderr()` with `trace_handler_with_debug`,
`trace_handlers_with_debug`, or a dispatch/thread `*_with_debug` method to
inspect execution as it happens. Default execution is silent.

Library callers pass the debug options directly to these methods.

A failed debug write disables the sink and records its error without changing
symbolic execution. Keep the debug options' `status()` handle if the caller
needs to report sink failures separately from trace diagnostics.

The typed `tracing::text::Trace` renderer validates and formats observations
already supplied by a producer. It neither runs a trace nor provides a replay
format. See the [text rendering guide](../../src/tracing/text/README.md) for that
contract.

### Find the boundary that stopped the trace

| Symptom | First thing to inspect |
| --- | --- |
| Unsupported handler or nested unknown | Recipe admission and the diagnostic's handler/step provenance |
| `missing-memory-slot-write` | Startup order and whether the target cell was initialized |
| `invalid-computed-input` / `computed-input` | A node ID supplied from outside the session's arena |
| `dynamic-branch` / `unhandled-thread-return` | Whether the caller needs a driver with thread control |
| `dynamic-index` | Whether a program value or joined cell is being used as a VM coordinate |
| `BudgetStopped` / `PrefixStopped` | The relevant symbolic or driver limit and committed prefix |
| `MissingExternal` during emission | Whether every required external name is allowed |
| Arena validation error | References, operator kind, cycles, depth, and expansion cost |

Start with the first failed boundary. A later emission refusal often follows
correctly from an earlier partial trace and should not be “fixed” by changing
the completion flag.

## Validation and implementation references

The runnable examples use checked-in fixtures. The focused suites cover
transactional handlers, state distinctions, emitter validation, dispatch
preparation, and startup order:

```sh
cargo test --test symbolic_tracer --test dispatch --test thread
```

For deeper changes, follow the relevant implementation boundary:

- [Symbolic execution and emission](symbolic-execution.md): values,
  recipe execution, graph validation, and output maps.
- [Symbolic implementation map](../../src/tracing/symbolic/README.md): entry
  points, essential invariants, source ownership, and focused checks.
- [Session driver guide](symbolic-session.md): lifecycle, rollback, branches,
  loops, try paths, subroutine summaries, and shared cells.
- [Session implementation map](../../src/tracing/symbolic/session/README.md):
  essential invariants, source ownership, and focused checks.
- [Dispatch preparation](../../src/dispatch/README.md): source proofs, operand
  decoding, completion contracts, and entry validation.
- [Prefix walker](../../src/dispatch/prefix/execute/README.md): path scheduling,
  image state, convergence, and retries.
- Symbolic regressions,
  thread startup regressions, and
  live sink regressions: concrete edge cases
  behind the contracts above.
