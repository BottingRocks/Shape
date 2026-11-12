# Symbolic tracing and rendering

`symbolic/` executes admitted handler recipes over bounded VM state and emits
JavaScript from a complete trace. `text/` validates and renders
observations supplied by a producer. These are separate models: rendering
never executes or infers a handler.

For a runnable whole-function trace, start with [Trace a whole program](../../README.md#trace-a-whole-program). The [symbolic tracer guide](../../docs/tracing/symbolic-tracer.md)
explains values, stops, and emission.

For the reasons behind fake-branch handling, start with
[fake-branch triage](../../docs/tracing/fake-branch-triage.md#why-the-extra-state-exists).
The session guide explains [stack alignment holes](../../docs/tracing/symbolic-session.md#why-joins-have-stack-holes)
and [cache lifetimes](../../docs/tracing/symbolic-session.md#why-facts-and-caches-have-different-lifetimes):
which facts survive a retrace, which must be discarded, and why an invalid
join is not automatically a fake branch.

## Files

| File | Responsibility |
| --- | --- |
| [`symbolic/`](symbolic/mod.rs) | `TraceSession`, symbolic values, events, stops, and Oxc emission |
| [`predicate.rs`](predicate.rs) | Conservative decisions for known branch tests |
| [`scope.rs`](scope.rs) | Generated-name scope checks after emission |
| [`text/`](text/README.md) | Observation-only validation, text, and line/details maps |
| [`mod.rs`](mod.rs) | Module facade and public aliases |

## Subgroup layout

The two large tracing modules are split by responsibility. The public paths
(`tracing::symbolic` and `tracing::text`) are unchanged.

| Module | Files |
| --- | --- |
| `symbolic/` | [`mod.rs`](symbolic/mod.rs) — public surface and re-exports; [`model.rs`](symbolic/model.rs) — values, nodes, events, stops, and the session-facing model; [`session/`](symbolic/session/mod.rs) — `TraceSession` split by state/try/branches/loops/subroutines/convergence/lifecycle; [`exec.rs`](symbolic/exec.rs) — the symbolic `Executor`; [`emit.rs`](symbolic/emit.rs) — `emit_javascript`, `emit_program_function*`, and the Oxc `Emitter` |
| `text/` | [`mod.rs`](text/mod.rs) — the `Trace` type, contracts, and validation entry; [`validate.rs`](text/validate.rs) — event/scope/provenance validation and the structure validator; [`render.rs`](text/render.rs) — the text renderer and formatting |

## Execution and emission

A `TraceSession` owns one symbolic context for a run. The dispatch drivers decide
which admitted recipe to step and how to handle branches, loops, calls, and
`try`. The session rolls back an unsupported handler attempt and retains
an explicit partial stop; it does not turn an unknown value into a guessed
concrete value. `operations::templates::admit_recipe` is the shared admission gate, and `js_number` owns
numeric conversions.

`emit_javascript` constructs an Oxc AST. It emits completed, returned, or thrown
traces, and by default rejects unsupported or budget-stopped traces. The
explicit `allow_incomplete` option is for stopped-trace inspection
(`trace thread --partial`); it does not establish completion. Generated bindings
and source maps come from trace facts. After emission, `vm_string` and
`emitted_cleanup` can simplify only proven-safe patterns.
In `trace program`, generated values use a local serial and a short function
number, such as `$SSA_9965_7`. The number follows the final program's sorted
function order, independent of parallel trace scheduling. Mutable branch and
loop values use `$CARRY` instead of `$SSA`.

## Captured-slot contract for linked functions

A captured VM slot is a shared cell, not a copy of its last primitive value.
Emitted linked functions receive a `capturedN` cell object and read or write
its `.value`; the program linker can project a shared slot into the
creator's binding when the capture relation is proven. An isolated invocation
invocation instead receives a caller-supplied snapshot. Do not substitute one
model for the other.

## Text rendering

`tracing::text::Trace` accepts typed scopes and events with evidence and optional
provenance. It validates nesting, call/resume relationships, branch joins,
stops, and output bounds before rendering text and line maps. The producer's
claims remain producer claims; the renderer does not certify a solver, source
span, or runtime observation. See [trace renderer](../../docs/tracing/trace-text-renderer.md).
