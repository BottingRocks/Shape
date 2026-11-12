# Symbolic execution and emission

The symbolic executor evaluates admitted handler recipes over bounded VM
state and records values, statements, and events in one arena. The emitter
validates an eligible report and turns its statements into an Oxc AST and
JavaScript. This guide explains the value model, recipe evaluation, emission
rules, and linked-program contracts in detail.

For runnable examples and the choice of direct, dispatch, or prefix tracing,
start with the [symbolic tracer guide](symbolic-tracer.md). For path scheduling,
rollback, branches, loops, and calls, use the [session driver guide](symbolic-session.md).
The [symbolic module README](../../src/tracing/symbolic/README.md) keeps the source
map, essential invariants, and focused test pointers.

| Need | Section |
| --- | --- |
| Choose a library entry point | [Execution boundary](#choose-the-execution-boundary) |
| Distinguish inputs, computations, and unsupported values | [Value model](#the-value-model) |
| Follow nodes, statements, events, and provenance | [Trace graph](#the-trace-graph) |
| Initialize VM storage and set work budgets | [Context](#context-the-vm-state-the-graph-is-built-from) |
| Understand a recipe result or stop | [Executor](#the-executor) |
| Emit a standalone function | [Emission](#emission) |
| Emit a function for linking | [Program mode](#program-mode) |
| Explain generated JavaScript | [Reading an emitted function](#reading-an-emitted-function) |
| Find a method or diagnose a failure | [Public API](#the-public-api) and [troubleshooting](#where-a-bug-belongs) |

```
decoded VM instruction ─┐
handler recipe ─────────┴─► Executor::run_handler ─► TraceNode/TraceStatement
                                                        │        ▲
                                        TraceContext ────┘        │
                                                                  │
TraceReport (arena + completion + diagnostics) ─► validate ─► Oxc AST ─► code
```

## Choose the execution boundary

| Need | Entry point | Caller supplies |
| --- | --- | --- |
| Trace one handler or an already-decoded sequence | `trace_handler` / `trace_handlers` | Templates, decoded operands, `TraceContext`, limits |
| Drive multiple dispatch steps with shared state | `TraceSession` | Handler selection, coordinates, branch/loop decisions, completion policy |
| Generate a function from an accepted trace | `emit_javascript` | `TraceReport` and `EmitOptions`, including allowed external names |
| Emit a function for the linked program | `emit_program_function` / `emit_program_function_in_namespace` | Trace and program naming/placement context |

For a raw capture, start with [`dispatch`](../../src/dispatch/README.md) or the
[trace commands](../../cli/trace/README.md); they own image decoding and
preparation. A `VMInstruction` here already has its handler index and operands.
Recipe operand position 1 is element 0 of that instruction's operand vector,
because the source handler's context parameter is separate.

The free functions are conveniences: they construct a terminal session, run
the supplied sequence, and stop at the first return, throw, or stop. A caller
that needs to continue after a handler return must use `TraceSession` directly;
see the [session guide](symbolic-session.md). The symbolic layer does not
discover an entry, decode a bytecode format, or assign semantic meaning to a
handler key.

## The value model

`SymbolicValue` is the input and output vocabulary. It deliberately has no
"any JSON" case and no implicit coercion:

| Variant | Meaning |
| --- | --- |
| `Undefined`, `Null`, `Boolean`, `Number`, `String` | Concrete JavaScript primitives with their distinctions preserved |
| `Object { identity }` | A stable object-like identity; references to the same identity stay the same object and are never coerced to a number |
| `External { name }` | An opaque but valid input, emitted as a function parameter and admitted by `EmitOptions` |
| `Computed { node }` | A reference to a computation in this trace's arena; valid internally, rejected when supplied from outside |
| `Unknown { reason }` | An unsupported value, not a symbolic one: execution stops with a diagnostic |

The `External`/`Computed`/`Unknown` split is the important one. `External` says
"the caller may pass anything here, and the output may name it". `Computed`
says "the output will compute this, and a node ID identifies it". `Unknown`
says "the tracer cannot represent this safely". Treating any two as
interchangeable produces output that looks plausible but either loses a
dependency or claims a value the trace never established.

A node ID is local to one arena. That is why a `Computed` value is rejected on
input: copying the ID into another report would not copy the computation, and
emission would point at an unrelated node.

## The trace graph

A `TraceArena` has three append-only vectors. IDs are indices into those
vectors, so a finished report has exactly one node, statement, or event per
index.

- Nodes are expression-like values. They are not mutated after construction, so
  a node ID is a stable description of one computation.
- Statements are the emitted program's structure: bindings, assignments,
  expression statements, `if`, loops, functions, `try`, `return`, `throw`.
- Events are ordered observations of VM storage traffic: stack and memory
  reads/writes, initialization, branch decisions, and `Undone` markers left by
  a walk that cut a speculative side back.

Events are not a second program. A stack read event often corresponds to no
emitted line, because the value it read was already materialised as a binding.
Use event provenance to find the statement and nodes involved instead of
looking for a one-to-one line mapping.

### Values (`TraceNodeKind`)

| Group | Kinds |
| --- | --- |
| Literals | `Undefined`, `Null`, `Boolean`, `Number` (rendered text), `String` |
| Inputs and names | `Object { identity }`, `External`, `Identifier`, `Global` (free name such as `host`) |
| Storage (graph-only) | `StackLength`, `StackRead`, `MemoryRead` — retained for provenance, rejected by emission |
| Computation | `Binary`, `Unary`, `PropertyRead`, `ObjectLiteral`, `ArrayLiteral`, `Call`, `New`, `Delete`, `Function` (closed source text) |
| VM closure | `VmFunction { descriptor, byte_offset, dispatch_state, memory }` |

The storage nodes are the visible sign of the "graph, not run" design. The
executor records that a value came from stack slot 3 or memory cell 7, and
resolves it to a value node for later use; the emitter never has to read a VM
object. That is why emitted functions have no `context` parameter.

### Statements (`TraceStatementKind`)

| Kind | Emitted as | Purpose |
| --- | --- | --- |
| `Let { name, value }` | `let trace_value_N = value;` | Materialise one computed value at the step that observed it |
| `Assign { name, value }` | `name = value;` | Merge and loop-carried variables, shared cells |
| `Evaluate { value }` | `value;` | Side-effecting expression, including non-terminal handler returns |
| `Return` / `Throw` | `return value;` / `throw value;` | Terminal completions or a path-local throw |
| `PropertyWrite` | `object[key] = value;` | A property write the recipe performed |
| `Declare { names }` | `let a, b;` | The variables a following `if` merges its arms into; empty blocks are skipped |
| `If { test, taken, fall_through }` | `if (test) … else …` | A program-value conditional jump; each arm owns its statement IDs |
| `Loop { label, body }` + `Continue`/`Break` | `label: for (;;) { … }` | A VM loop and its edges |
| `Function { name, params, body }` | `function name(params) { … }` | A VM subroutine traced once and called from each site |
| `Try { param, body, handler }` | `try { … } catch (param) { … }` | A VM `try`; the handler is the catch or `finally` body the VM runs |

### Events (`TraceEventKind`)

`StackRead`, `StackWrite`, `StackLengthRead`, `StackLengthSet`, `MemoryRead`,
`MemoryWrite`, `MemoryInitialize`, `BranchDecided { test, taken, by }`, and
`Undone`. `by` is `BranchEvidence::Test` when predicate facts proved the
outcome and `BranchEvidence::Context` when a driver rejected an impossible
path. The distinction is visible in reports so a triage can tell a proof from a
state-based rejection.

Every node, statement and event carries `TraceProvenance`: handler index,
table key, semantic identity, step order, monotonic trace step, and source
span. This is what makes an emitted line explainable back to the recipe step
that produced it, and it is why emission does not simply drop unused-looking
bindings by default.

`TraceArena::describe` renders a node as a compact JavaScript-like expression
with never-reassigned `let` bindings inlined; `describer()` reuses the binding
map when describing many nodes. `live_nodes()` walks statements' values to mark
nodes reachable from code, which is how a caller can find nodes built on a path
that was later cut back. Neither is emission: `describe` is for diagnostics and
does not enforce the emitter's limits.

## Context: the VM state the graph is built from

`TraceContext` is mutable VM state, not trace output. Its distinctions are
semantic, and collapsing them changes behavior:

- A stack hole is not a stored `undefined`. Both read as `undefined`, but only
  one is a deliberate write, and only a write records a `StackWrite` event.
- A missing memory slot is not an initialized one. Reading or writing it
  produces the VM's own `TypeError` (a represented throw), because
  `this.slots[slot].value` is being accessed on `undefined`. `initialize`
  creates a cell deliberately, and replaces an existing one.
- No receiver is not a receiver of `undefined`. `set_receiver` is explicit;
  reading `receiverValue`/`receiverObject` without one stops with an unknown
  input diagnostic.
- `try_underflow` records `try` exits below zero. A JavaScript VM's
  `tryFrameStack.pop()` on an empty stack does nothing; the WASM interpreter
  traps. `with_try_depth_traps` selects the trapping behavior.

Checked initialization is the safe default:

| Method | Effect |
| --- | --- |
| `set_stack_value` / `_bounded` | Write a value, growing the stack with holes |
| `set_stack_length` / `_bounded` | Grow (or shrink) with holes, no value write |
| `set_memory_value` / `_bounded` | Create or replace a slot with a value |
| `initialize_memory_slot` / `_bounded` | Create a fresh `undefined` cell, discarding an existing value |
| `set_receiver` | Set the `this` of the running VM function |

The bounded variants take the caller's own limit instead of `TraceLimits`'
default; use them for externally supplied indices and lengths, and validate
before calling rather than relying on a later failure. All value-taking setters
reject a caller-supplied `Computed`.

`TraceLimits` bounds work, not correctness:

| Field | Default | Counts |
| --- | --- | --- |
| `max_steps` | 10,000 | Recipe steps executed |
| `max_nodes` | 50,000 | Trace nodes constructed |
| `max_stack_length` | 4,096 | VM stack length |
| `max_memory_slots` | 4,096 | Distinct memory cells |

One decoded instruction can run many recipe steps, so `max_steps` is a work
limit, not an instruction limit. Drivers have their own dispatch/walk budgets,
separately. Raising a limit can admit a longer supported trace; it cannot make
an unsupported recipe or missing proof valid.

## The executor

`Executor` is crate-visible (`pub(crate) use exec::*`) and created by the
session. Understanding it means understanding four things: temporaries,
binding, storage, and failure.

### One handler

`run_handler` clears per-handler temporaries and one-shot hand-offs, then runs
the recipe's steps in source order. Each step increments `steps` and stops on
`max_steps`. The first terminal step (`Return`, `Throw`, or a `throw` raised by
a step) ends the recipe; a pending `CompleteThread` value is emitted as a
`return` statement after the step loop. An ordinary `return` is either
terminal (`TraceSession::new`) or an `Evaluate` statement with the value
reported (`new_continuing`); the difference is in the session contract, not in
the recipe.

`EvalValue` carries three things: the `SymbolicValue` identity, an optional
concrete `fact` when the exact value is known, and the graph `node` that
represents it. The `fact` is the only channel for static knowledge; the node
is what emission uses. Keeping them separate is what lets the tracer emit
`x + "!"` while knowing nothing about `x`.

`Let`/`Assign` to a recipe temporary materialises a `trace_value_N` binding and
records an alias from the identifier node to the value node. Reads of the
temporary resolve through `temp_values`; reads of a bound name resolve through
the alias map. Temporaries are cleared for each handler, so a local cannot leak
from one instruction to the next.

### Recipe operations and what they do

| Group | Operations | Behavior |
| --- | --- | --- |
| Temporaries | `Let`, `Assign` | Evaluate, bind a fresh SSA name, record the temporary. `Assign` requires a prior `Let` |
| VM storage | `StackWrite`, `StackLengthSet`, `MemoryWrite`, `MemoryInitialize` | Checked writes; a write to an absent slot is the VM's `TypeError`; initializing replaces |
| Object storage | `PropertyWrite` | `object[key] = value` for non-VM objects |
| VM calls | `PushCallFrame`, `PopCallFrame` | Push uses the driver-supplied `VmCursor`; popping exposes fields via `PoppedCallFrame` |
| VM position | `SetProgramCounter`, `SetDispatchState` | Recorded as a `ControlTransfer` the driver consumes |
| `try` state | `PushTryFrame`, `PopTryFrame`, `PushFinallyRecord`, `PopErrorRecord`, `RethrowIfHandled`, `DropThrownIfHandled`, `DropThrownUnderIfHandled` | Maintain the try/error stacks and the represented throw behavior |
| Control | `JumpIf` | Evaluate condition/target/state; a known condition sets the transfer, an unknown program value becomes a `PendingBranch` for a thread-controlling driver |
| Threads | `CompleteThread` | `vmPendingResult = value`; without a thread-controlling driver this is `unhandled-thread-return` |
| Program | `Evaluate`, `Return`, `Throw` | Value statement, handler return, JavaScript throw |
| Fail closed | `Unknown` | Admission rejects it before execution |

`SemanticValue` mirrors this: operands (position 1 is operand 0), temporaries,
literals, stack length/read, memory read, property access, object/array
literals, popped frame/try/error fields, stack slices, calls, thread creation,
receiver reads, and bindings (free names such as `host` or `vmStringTable`).
The context object itself is never a value; the recipe defines
`semantic-value` cases that would need one, and those are rejected.

### Storage reads

A stack read of an ordinary hole or an index past the end yields `undefined` and
materialises the value when first read. A memory read of an absent slot is not
`undefined`: it raises the VM's `TypeError` through the same represented-throw
path a handler would hit at runtime. An initialized slot reads as `undefined`.
A shared cell (`$CELL_n`) is read through `read_cell`, which consults the
path's known cell facts for decisions while still emitting a property read.

Join-padding holes are guarded separately: they align unequal stacks for a
tentative join, and an access into that region reports `PaddedStack` so the
driver can undo it. They cannot supply a fabricated `undefined`. See
[why joins have stack holes](symbolic-session.md#why-joins-have-stack-holes).

Writes check capacity and slots first. Stack writes grow to the index within
`max_stack_length`; a memory write to an absent slot is the VM's
`TypeError`; `MemoryInitialize` allocates or replaces a cell, bounded by
`max_memory_slots`. Shared cells are written as property writes, and every
shared-cell write advances a serial so a later loop or join can tell which
iteration wrote last.

### VM coordinates

A jump target, dispatch state, slot number, or stack length must become a real
`usize`. `concrete_index` is where symbolic execution either succeeds or hands
the problem back to the driver:

- A known numeric fact is used directly.
- A value read from a shared cell this path wrote a VM position to resolves to
  that position (`cell_positions`), never to the JavaScript value.
- A merge of a concrete value with a symbolic one sets `split`; the session
  exposes it through `take_split` so the driver walks the paths apart.
- An outlined call result sets `inline`; `take_inline` lets the driver retrace
  the call inline.
- A variable with a bounded value set sets `dispatch` (`take_dispatch`) and
  records the set in `used_sets`, so a later growth of that set invalidates the
  decision and forces a retrace.
- Anything else is `dynamic-index` (or `invalid-index` when the concrete number
  is out of range). A jump to `undefined` with no `try` open is the VM
  dispatch's own `TypeError`, not a tracer limitation.

These are deliberate escapes from "the tracer guesses". A guessed coordinate
would emit code that never runs the way the VM does.

### Branches and falls

`JumpIf` first tries constant folding, then path facts, then typed predicates,
then opaque-predicate shapes; `predicate.rs` owns those rules. A decided
branch records a `BranchDecided` event with its evidence and sets the control
transfer. An undecided branch becomes a `PendingBranch` when the driver walks
threads; otherwise it is a `dynamic-branch` stop. A second program-value branch
in one handler is also a stop, because the driver protocol assumes one branch
per step.

A known branch is a VM decision even without thread control. That distinction —
"the test is known" versus "the walker must fork" — is why reports can show a
single path through a conditional without any fork records.

### Failure taxonomy

`ExecutionStop` has three shapes, and the session maps them differently:

| Stop | Meaning | Session result |
| --- | --- | --- |
| `Budget(message, provenance)` | A step/node/stack/memory bound | Rollback, `BudgetStopped` |
| `Unsupported(code, message, provenance)` | Invalid input, impossible state, unsupported operation | Rollback, `Partial` with a diagnostic |
| `Throws(value)` | The VM itself throws here (including a missing memory slot) | Emitted `throw`, reported as `Thrown` |

That last row is easy to misread. A represented throw is behavior: the
emitter can print it. An unsupported stop is analysis failure: the emitter
must refuse the trace. Do not convert one into the other to make output look
complete.

### Admission

Before a handler runs, `preflight` calls the shared `admit_recipe` with the
default `RecipeBounds`: template status must be executable, no unsupported
fragments may remain, steps must be source-ordered and within 100,000, no
`Unknown` operation or value may appear, the context cannot be a recipe value,
temporaries must be defined before use and defined once, and expression work
(depth 256, node visits 100,000) must be bounded. The reason string becomes the
`unsupported-handler` diagnostic. The session caches the verdict per recipe
identity, so the walk pays for it once.

### Folding and numbers

`fold_binary` and `fold_unary` are used by both execution and emission, so a
folded value cannot differ between the trace and the printed code. They fold
only what is provably JavaScript:

- Arithmetic and bitwise operators on primitives, with ToNumber/ToInt32
  semantics where the input is numeric.
- `===`/`!==` on primitives, including the IEEE rules (`NaN !== NaN`,
  `-0 === 0`); objects never fold.
- Numeric comparisons; strings and mixed types do not fold because relational
  comparison is code-unit based and coercion-sensitive.
- String concatenation only when neither operand is an object and no number
  needs `Number::toString` formatting; otherwise the expression is kept.

`number_text` renders a *literal*, not ECMA-262 `Number::toString`: it must
round-trip to the same binary64 value and must preserve `-0`. `parse_number_text`
is the inverse. Emission uses the same text, and renders `NaN`, `Infinity` and
`-Infinity` as arithmetic expressions (`0 / 0`, `1 / 0`) so a parameter named
`NaN` or `Infinity` cannot change their meaning.

## Emission

`emit_javascript` is a projection: it consumes established facts and refuses to
invent one. Its eligibility rule is exact: `PartialUnsupported`,
`BudgetStopped`, and `PrefixStopped` are by default refused with
`EmitError::IncompleteTrace`; `Completed`, `Returned`, and `Thrown` are
candidates. Eligibility is not enough — arena validation and the external
allow-list still apply.

`allow_incomplete: true` is the explicit diagnostic exception used by
`trace thread --partial`. It permits inspecting a stopped trace while retaining
arena and external-name validation; it does not change the report's completion.
Program and corpus emission never enable it.

`EmitOptions` is intentionally small:

| Field | Meaning |
| --- | --- |
| `function_name` | Name of the generated function (default `trace`) |
| `context_parameter` | Retained for source compatibility and ignored; emitted functions have no VM context |
| `external_parameters` | Allow-list of `External` names; any other required name is `MissingExternal` |
| `simplify_bindings` | Inline never-reassigned constant/copy bindings and drop unread pure `let`s |
| `allow_incomplete` | Permit diagnostic emission of a stopped trace (default false); does not certify completion |

`simplify_bindings` is off by default so the output keeps one statement per
trace statement, which is what the separate `cleanup_emitted_javascript`
pipeline and the provenance maps expect. Turning it on is an output-shape
decision, not a correctness switch.

### The pipeline

1. Reject an ineligible completion.
2. `validate_arena`: IDs must be indices, references must resolve, storage
   nodes must not be emitted as expressions, operators must be binary/unary,
   blocks must form a tree in statement order, and the expression DAG must be
   acyclic, at most depth 256, and at most 100,000 expanded nodes.
3. Reserve free names first: `Global` names (`host`, helper names) are emitted
   verbatim, so they must be valid identifiers and are reserved before any
   generated name.
4. Reserve the function name, external parameter names, and object parameter
   names.
5. Plan slot emission from memory events (see below).
6. If simplifying, compute inline bindings and dead bindings.
7. Reserve every local name in statement order, preferring `$SSA_N`,
   `$CARRY_N`, `$ARRAY_N`, `$OBJECT_N`, `$THREAD_N`, and `$SLOT_N` based on the
   value's kind and whether the binding is reassigned.
8. Build the Oxc AST with `AstBuilder`, prepend any helper declarations the
   code calls, add `"use strict"` in program mode when `this` is read, print
   with `Codegen`, and return the code plus `statement_map`, `node_map`, and
   `object_parameters`.

No generated trace is printed and reparsed to build the AST. Helper
declarations and closed-function sources are parsed directly from their source
text, but the trace itself is constructed with `AstBuilder`, which is why
statement and node maps point at the same AST nodes the printer sees.

### Names

One namespace covers the function, parameters, locals, slots, cells, and
helpers, so a user-supplied name cannot collide with a VM-generated one. Names
are sanitized deterministically: characters outside `[A-Za-z0-9_$]` become `_`,
a leading digit is prefixed with `_`, and reserved words get a trailing `_`.
Collisions get `_2`, `_3`, and so on.

Object identities become stable parameters named `object_<identity>`, never
fresh `{}` literals, so identity is preserved across references.
`EmittedJavaScript::object_parameters` exposes the mapping. External names
become parameters in the order `external_parameters` lists them.

### Slot emission planning

The VM stores values in memory cells; the graph mostly uses SSA bindings.
`plan_slot_emission` bridges the two without letting memory events disappear:
it finds memory reads that can be projected to `$SLOT_n` immediately before
the matching statement and writes projected to `$SLOT_n` immediately after the
defining statement. It only does so when the event maps to exactly one
statement with the same provenance, the value is reusable (a literal, external,
object, or identifier), and the statement's block path contains no loop, `try`,
or function (where a slot's liveness across the boundary would be wrong).
Otherwise the memory value stays in SSA form.

This is why a small trace can contain both `var $SLOT_4 = $SSA_3;` and
`$SLOT_4 = $SSA_3;`: the read is projected before one statement and the write
after another, while the SSA binding preserves provenance. The plan is
conservative: an event seen at more than one statement, more than one read at
one statement, or a value that is not reusable, leaves the memory access in SSA
form rather than guessing.

### Validation limits

| Check | Limit | Why |
| --- | --- | --- |
| Reference integrity | Every child ID < node count | Public arenas can be constructed by callers |
| Block ownership | Child ID > parent, exactly one owner | Prevents cycles and two-parent statements |
| Expression depth | 256 | Bounds recursive printing |
| Expanded DAG cost | 100,000 visited nodes | Rejects shared DAGs whose textual expansion is exponential |

A small graph can still be too expensive to print. Both limits are reported as
`EmitError` variants with the node and limit, not panics.

### Errors

`EmitError` is closed and specific: `IncompleteTrace`, `MissingExternal`,
`InvalidIdentifier`, `InvalidNode`, `InvalidArena`, `ExpressionCycle`,
`ExpressionTooDeep`, `ExpressionTooLarge`. `InvalidArena` covers both
structurally bad reports and program-mode contract violations. When emission
refuses, the first error is the boundary to fix; changing the completion flag
or adding an external name blindly usually hides a real gap.

## Program mode

`emit_program_function` targets the linked-program pipeline (`shape trace
program`), not a standalone function. Its contract differs from
`emit_javascript` in ways the arena must satisfy:

- No parameters. Thread inputs are read in place: `arguments[i]`,
  `arguments`, `this`, the function itself, and captured cells through the
  enclosing scope.
- Every memory slot is a variable `$KEY_<slot>`; a shared cell's box is
  unwrapped to that variable. A slot's cell can be declared only at the top
  level, because a `let` inside a block would give each pass its own cell while
  the VM keeps one.
- A created VM function is emitted as an empty
  `function $THREAD_<state>_<pc>() {}` marker and reported in
  `ProgramFacts::threads`; the program nests that function's own emitted code
  there. This is why creation must be in a binding and cannot sit inside a
  function body.
- A shared slot that becomes a fresh cell after a creation captured the old
  cell is rejected: the creation holds the old object, and the trace would
  claim otherwise.
- Object identities are rejected: a standalone function can take them as
  parameters, a program function cannot.
- Reading `this` emits `$THIS` and a strict directive. The caller replaces
  `$THIS` with the real receiver after cleanup, because the cleanup passes
  treat `this` as the global object.

`emit_program_function_in_namespace` appends an owner namespace to every
generated `$SSA`/`$THREAD`/`$KEY` name, so independently traced functions can
be linked without identifier collisions. The namespace must be non-empty and
ASCII alphanumeric/underscore.

`ProgramFacts` returns what the linker needs: created threads (name,
descriptor, PC, state, cells), captured slots, and how often the function named
itself or read its receiver.

## Reading an emitted function

The tracked example traces a handler that appends `"!"` and one that moves the
value through a memory slot. Its output is:

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

Read it as three layers. `$SSA_n` bindings are one per value the trace
materialised; the copies are the trace's own stack-to-stack transfers, kept
because default emission preserves one statement per trace statement.
`$SLOT_4` is the slot plan: a memory read is projected before the statement
that reads it, the write after the statement that produced it, and the final
read before the return, so the emitted function uses a real variable instead of
a VM object. `return $SLOT_4` is the terminal completion. The values, not the
lines, are the point: the external input flows through the stack and memory and
becomes a return value. Dead-copy removal and renaming belong to optional
binding simplification or the separate cleanup pass, not to the symbolic
tracer.

## Reports and completion

A `TraceReport` is the emitter's input and the diagnostic surface: arena,
completion, diagnostics, handler count, external inputs, and final-state
summary. Completion semantics, the difference between a converted throw and an
unsupported stop, and the driver-side rules live in the [session
guide](symbolic-session.md); the emitter still validates that report before producing code.

`handlers_traced` counts handler occurrences, not emitted lines. `final_state`
is a stack length and slot count, not a resumable context. `external_inputs`
must be admitted by `EmitOptions` (or be program inputs in program mode).
Diagnostics carry a `TraceStopCode`, a message, and optional provenance; the
code is the stable identifier used in reports and tests.

## The public API

All of it is public through `tracing::symbolic`, and most of it is also
re-exported at the crate root.

| Area | Items |
| --- | --- |
| Direct tracing | `trace_handler`, `trace_handlers`, `trace_handler_with_debug`, `trace_handlers_with_debug` |
| Session | `TraceSession` (see the session guide for its operations) |
| Emission | `emit_javascript`, `emit_program_function`, `emit_program_function_in_namespace`, `EmitOptions`, `EmittedJavaScript`, `EmittedStatementMap`, `EmitError`, `ProgramFacts`, `ProgramThread`, `PROGRAM_SELF`, `PROGRAM_THIS` |
| Values and steps | `SymbolicValue`, `DecodedInstruction`, `VMInstruction`, `LiteralValue`, `PrimitiveOperator` |
| State | `TraceContext`, `TraceContextError`, `TraceLimits`, `CallFrame`, `TryFrame`, `ErrorRecord`, `VmCursor`, `ControlTransfer` |
| Graph | `TraceArena`, `TraceNode`, `TraceNodeKind`, `TraceNodeId`, `TraceStatement`, `TraceStatementKind`, `TraceStatementId`, `TraceEvent`, `TraceEventKind`, `TraceProvenance`, `TraceValue`, `TraceCell` |
| Reports | `TraceReport`, `TraceCompletion`, `TraceDiagnostic`, `TraceStateSnapshot`, `TraceStopCode`, `TraceHandlerResult`, `TraceFinishError`, `TraceJoinError` |
| Driver types | `TracePath`, `TraceFork`, `TraceLoop`, `TraceCheckpoint`, `StopMarks`, `ValueDispatch`, `CellRead`, `CellContent`, `CellReads`, `MergedConcrete`, `SubroutineFrame`, `SubroutineSummary`, `PendingBranch`, `BranchEvidence` |
| Debug | `TraceDebugOptions`, `TraceDebugStatus` |

## Where a bug belongs

| Symptom | Inspect |
| --- | --- |
| A handler reports `unsupported-handler` with a vague reason | `admit_recipe` bounds and the template's unsupported fragments; the reason is the admission vocabulary, not the executor |
| A value reads as `undefined` unexpectedly | Stack hole versus stored `undefined`, and whether the stack write actually ran |
| A memory write reports a `TypeError` | The slot was never initialized; initialization is deliberate and replaces |
| `dynamic-index` on a number that looks concrete | The value has no `fact`, or came from a cell/merge/outlined call; check `take_split`/`take_inline`/`take_dispatch` |
| A branch that should be decided forks instead | Predicate rules: a type or branch fact did not survive, or the value is an outlined call result |
| A constant looks different in emitted code | `fold_binary`/`fold_unary` disagreement, or `number_text`/`parse_number_text` round-trip |
| Names collide or change unexpectedly | The single `NameMap` reservation order, sanitization, and the `$SSA`/`$CARRY`/`$SLOT`/`$KEY` choice |
| A slot disappears from output | Slot planning rejected the event as ambiguous or unsafe, falling back to SSA; that is conservative, not a bug |
| Emission refuses with `InvalidArena` | ID/reference integrity, block ownership, or a program-mode contract |
| Emission refuses with `ExpressionTooDeep`/`TooLarge` | Expanded DAG cost; the graph is shared, not necessarily small |
| `MissingExternal` | The name is in `report.external_inputs` but absent from `EmitOptions::external_parameters` (or not a program input) |
| Program mode rejects a creation or a later new cell | The creation is not in a binding or sits inside a function, or a captured slot gets a fresh cell afterwards |

## Boundaries worth keeping

- The model is data, not a run. `TraceArena`/`TraceReport` record what
  happened; the executor in `exec.rs` steps a recipe, and the session in
  `session/` owns one arena across handlers.
- Emission is a projection. It refuses unsupported, incomplete, or
  budget-stopped traces and never invents a concrete value; it consumes trace
  facts, it does not re-derive them.
- Constant folding used by both execution and emission lives in `exec.rs`
  (`fold_binary`, `fold_unary`), so the two never disagree on a folded value.
- The session/model split is what makes a rollback cheap: an unsupported
  handler attempt restores a checkpoint instead of re-running earlier handlers.
- Storage facts are for decisions; emitted code reads the graph. Mixing the
  two is how a value gets claimed as concrete when only one path wrote it.
- Node IDs never cross arenas. A `Computed` from another report is not a value
  this trace can use, and the API rejects it for that reason.

The tracked example and its expected output are documented in the [symbolic
tracer guide](symbolic-tracer.md), together with the dispatch and
prefix contracts. The [session guide](symbolic-session.md) covers the driver
operations; [dispatch preparation](../../src/dispatch/README.md) covers source
proofs and decoding; the [prefix walker](../../src/dispatch/prefix/execute/README.md)
covers the control-flow strategy that uses the session.
