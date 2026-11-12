# Symbolic tracing

This module evaluates admitted VM handler recipes over bounded symbolic state
and emits JavaScript from eligible traces. `TraceSession` keeps one arena alive
across handlers; `emit_javascript` validates a report, builds an Oxc AST, and
prints the resulting function. The public API is `shape::tracing::symbolic`,
with many types also re-exported from [`lib.rs`](../../lib.rs).

Use this README for entry points, ownership, invariants, and source navigation.
The [execution and emission guide](../../../docs/tracing/symbolic-execution.md) holds
the detailed value, graph, recipe, and emitter reference. The
[symbolic tracer guide](../../../docs/tracing/symbolic-tracer.md) provides runnable
examples; the [session guide](../../../docs/tracing/symbolic-session.md) explains
state operations for drivers.

## Ownership

| Layer | File | Responsibility |
| --- | --- | --- |
| Model | [`model.rs`](model.rs) | Values, graph records, mutable VM context, reports, and stop types |
| Execution | [`exec.rs`](exec.rs) | Admission, recipe evaluation, storage effects, and constant folding |
| Session | [`session/`](session/README.md) | Transactions, checkpoints, shared state, and path operations |
| Emission | [`emit.rs`](emit.rs) | Report validation, names, slot projection, Oxc AST construction, and provenance maps |

Dispatch drivers supply decoded instructions and decide how to walk branches,
loops, and VM calls. This module builds a symbolic representation of their
effects. It does not run the captured JavaScript or WebAssembly in a runtime.

## Choose the entry point

| Need | Entry point | Caller supplies |
| --- | --- | --- |
| Trace one handler or a decoded sequence | `trace_handler` / `trace_handlers` | Templates, operands, context, and limits |
| Keep state across driver-selected steps | `TraceSession` | Instruction selection, coordinates, path decisions, and completion policy |
| Emit a standalone function | `emit_javascript` | Eligible `TraceReport` and `EmitOptions` |
| Emit a function for a linked program | `emit_program_function` / `_in_namespace` | Trace, naming namespace, and program placement context |

For raw captures, start with [dispatch preparation](../../dispatch/README.md)
or the [trace commands](../../../cli/trace/README.md). A `VMInstruction` here
already identifies its handler and decoded operands. Recipe operand position
1 reads vector element 0; the source handler's context parameter is separate.

The direct tracing helpers use a terminal session and stop at the first
return, throw, or unsupported/budget boundary. A dispatch driver that must
continue after an ordinary handler return uses `TraceSession::new_continuing`.
A VM function return is a separate driver hand-off. See the
[session lifecycle](session/README.md#the-driver-lifecycle) before changing that
return convention.

## Values and VM state

`SymbolicValue` separates values that can remain symbolic from values the
analysis cannot represent:

| Value | Meaning |
| --- | --- |
| Primitive variants | Concrete JavaScript values, preserving `undefined`, `null`, and numeric distinctions |
| `Object { identity }` | Stable object identity, preserved across references |
| `External { name }` | Opaque valid input whose name the emitter must allow |
| `Computed { node }` | A computation owned by this session's arena |
| `Unknown { reason }` | Unsupported information that stops execution with a diagnostic |

An external value can participate in emitted expressions without becoming
concrete. An unknown value cannot be substituted for it. Node IDs are local to
one arena, so context setters and decoded operands reject caller-supplied
`Computed` values.

`TraceContext` holds mutable stack, memory, receiver, call, and exception state.
Preserve these storage distinctions when changing evaluation:

- A stack hole reads as `undefined` but is distinct from a deliberate write.
- An absent memory slot raises the VM's represented `TypeError` when read or
  written; an initialized slot contains a value, possibly `undefined`.
- Initializing a memory slot replaces its cell and discards its old value.
- A missing receiver is distinct from a receiver explicitly set to `undefined`.
- JavaScript and WebAssembly differ on an empty `try`-stack pop; the driver
  selects trapping behavior with `with_try_depth_traps`.

Join-padding holes have an additional contract: they temporarily align stacks
from the top, and a step reaching that region must undo the join before using
it. They cannot supply an ordinary `undefined` read. See
[why joins have stack holes](../../../docs/tracing/symbolic-session.md#why-joins-have-stack-holes)
for the distinction between a failed alignment and actual stack-deficit
evidence. The [cache inventory](../../../docs/tracing/symbolic-session.md#why-facts-and-caches-have-different-lifetimes)
also explains why arena-node facts, path snapshots, and capture-wide entry
checks cannot share one rollback policy.

Use the checked context setters for incoming values and indices. Their bounded
variants accept explicit caller limits. `TraceLimits` defaults to 10,000 recipe
steps, 50,000 nodes, 4,096 stack entries, and 4,096 memory slots. A recipe can
consume many steps per decoded instruction; dispatch budgets are separate.

See the [value model](../../../docs/tracing/symbolic-execution.md#the-value-model) and
[context reference](../../../docs/tracing/symbolic-execution.md#context-the-vm-state-the-graph-is-built-from)
for all variants, setters, and limits.

## Trace records and execution outcomes

The arena records three different things:

- **Nodes** describe expressions and values.
- **Statements** describe emitted structure: bindings, writes, branches,
  loops, functions, returns, and throws.
- **Events** record ordered observations, including storage traffic and
  branch decisions. An event need not produce an output line.

Follow `TraceProvenance` from an observation to its handler, recipe step,
source span, and generated records. `TraceArena::describe` is useful for
inspecting a node; it is a diagnostic formatter, not JavaScript emission.

The executor preflights a recipe before running it. Unsupported or over-budget
handler attempts are rolled back by the session, preserving earlier committed
work. A VM throw is different: it is represented behavior that can appear in
emitted code. Keep these outcomes distinct when mapping failures:

| Outcome | Meaning |
| --- | --- |
| Represented throw | The modeled VM operation throws a value |
| Unsupported stop | Analysis cannot establish or represent the requested behavior |
| Budget stop | A configured work or storage limit was reached |

A `TraceReport` contains the arena, completion, diagnostics, committed handler
count, external inputs, and final stack/slot counts. It cannot resume the VM.
The driver's completion contract still matters when deciding whether a walk
succeeded.

## Emission contract

`emit_javascript` considers `Completed`, `Returned`, and `Thrown` reports.
By default it rejects `PartialUnsupported`, `BudgetStopped`, and `PrefixStopped`,
then validates the arena and required external names before constructing output.
Eligibility alone does not guarantee successful emission.

`EmitOptions::allow_incomplete` defaults to false. `trace thread --partial`
sets it for diagnostic output from a stopped walk; arena and external-name
validation still apply. Program and corpus emission keep it false. Such a
diagnostic function is an inspected prefix, not a completed reconstruction.

Callers choose a function name and allow external parameter names through
`EmitOptions`. Object identities become stable parameters. Generated names
share one namespace with parameters and free names, so naming changes must
preserve collision avoidance and identity.

Binding simplification is optional and off by default. The default output
retains trace statements for provenance; turning simplification on changes
output shape. Memory events may be projected to local slot variables when
provenance and scope permit it; otherwise values remain represented by their
existing bindings.

The emitter validates ID references, block ownership, expression cycles, depth,
and expanded graph cost. The expression limits are depth 256 and 100,000
expanded node visits. A compact graph can still exceed the output expansion
bound. Read `EmitError` before changing completion or external-name settings.

Program emission has a separate contract: inputs come from arguments,
receiver, and enclosing cells, while created VM functions become markers for
the linker to place. It rejects object parameters and cell/creation patterns
that cannot preserve the capture relationship. Consult
[program mode](../../../docs/tracing/symbolic-execution.md#program-mode) when touching
that path.

## Boundaries to preserve

- Keep concrete facts used for decisions separate from value nodes used for
  emission. One path's known value must not become another path's constant.
- Share numeric folding between execution and emission; preserve JavaScript
  number semantics, including `NaN`, infinities, and negative zero.
- Keep computed node IDs within their owning arena and discard references to
  work removed by rollback.
- Keep unsupported analysis distinct from a represented VM throw. Increasing
  a budget cannot supply a missing semantic proof.
- Preserve source provenance and output maps when changing binding or slot
  emission. Events and output lines are not a one-to-one sequence.
- Let the driver establish completion. Emission must not hide a partial walk
  by changing its completion flag.

## Source map and deeper reference

Read `model.rs` first, then follow `exec.rs` through `run_handler`, `run_step`,
and `eval`. For output changes, start at `emit_with` in `emit.rs`: eligibility,
validation, naming, and slot planning precede AST construction.

| Location | Start here for |
| --- | --- |
| [`mod.rs`](mod.rs) | Module wiring and public exports |
| [`model.rs`](model.rs) | Graph types, VM context, completion, and stop codes |
| [`exec.rs`](exec.rs) | Recipe evaluation, facts, cells, admission, and numeric folding |
| [`emit.rs`](emit.rs) | Emitter options, arena checks, naming, slots, and program emission |
| [`session/`](session/README.md) | Checkpoints and branch/loop/exception/call operations |
| [`predicate.rs`](../predicate.rs) | Predicate rules consulted by the executor |
| [`tests.rs`](tests.rs) | Small executor/emitter arenas and focused edge cases |

The detailed guide provides the [recipe operation reference](../../../docs/tracing/symbolic-execution.md#recipe-operations-and-what-they-do),
[emission pipeline](../../../docs/tracing/symbolic-execution.md#the-pipeline),
[public API inventory](../../../docs/tracing/symbolic-execution.md#the-public-api), and
[symptom-to-code map](../../../docs/tracing/symbolic-execution.md#where-a-bug-belongs).

## Tests

```sh
cargo test --test symbolic_tracer
```

The tests use tracked fixtures and explicit operands. The execution model is
explained in the [symbolic tracer guide](../../../docs/tracing/symbolic-tracer.md).
Choose additional suites for the boundary being changed:

- `live_trace_debug.rs`: debug output.
- `vm_control_flow.rs` and
  `vm_branches.rs`: driver control flow.
- `thread.rs`: thread startup and input layout.
- [`emitted_*` suites](../../../tests): output cleanup and shape guarantees.

For driver retry policy, continue with the
[prefix walker](../../dispatch/prefix/execute/README.md).
