# Symbolic trace session

`TraceSession` owns the symbolic state a bounded driver keeps while walking a
VM function. One session holds one `Executor`: its node, statement, and event
arena; mutable `TraceContext`; completion state; diagnostics; and learned
facts. A value keeps its node identity across committed handlers in that
session.

Use this README to find the implementation and understand its essential
contracts. The [session driver guide](../../../../docs/tracing/symbolic-session.md)
covers construction options, rollback, branches, loops, exceptions, outlined
calls, shared cells, and the full method reference. For runnable tracing and
emission examples, start with the
[symbolic tracer guide](../../../../docs/tracing/symbolic-tracer.md).

## Ownership and boundaries

The executor runs an admitted handler recipe. The session makes that work
transactional and exposes operations for manipulating paths. The dispatch
driver selects instructions, controls retries, and decides when a VM function
has completed.

| Layer | Responsibility |
| --- | --- |
| [`model.rs`](../model.rs) | Values, nodes, statements, events, paths, context, and completion types |
| [`exec.rs`](../exec.rs) | Recipe evaluation, admission preflight, folding, and cell effects |
| `session/` | One arena across handlers, transactions, checkpoints, path operations, and facts |
| [`dispatch/`](../../../dispatch/README.md) | Instruction decoding, VM coordinates, image state, walk budgets, and completion policy |

The JavaScript emitter lives in the parent symbolic module. It consumes a
report after the driver has established the appropriate completion boundary.
A report is a record of the trace, not a resumable execution context.

## The driver lifecycle

### Choose the return convention

`TraceSession::new(context, limits)` treats an ordinary handler `return` as
terminal. Use it when the supplied handler sequence is the whole unit.
`new_continuing` evaluates and records the returned value while keeping the
session runnable; emitted JavaScript contains an evaluation rather than an
early `return`.

A VM function return is a separate hand-off. A driver that handles VM branches,
returns, and `try` entries enables `with_thread_control` and consumes the
corresponding `take_*` results. Without that mode, encountering those effects
stops at the unsupported boundary.

Both constructors validate the incoming context. A caller-supplied `Computed`
node, oversized stack, or excessive memory-slot count can stop the session
before the first handler. Check the initial diagnostic before debugging the
walk itself.

### Step and handle the outcome

1. Supply any proven tables, capture mappings, and shared-cell facts the driver
   owns. Open shared cells before stepping; checkpoint before work that may
   need to be restarted.
2. For a handler that pushes a call frame, set the VM cursor to the PC after
   its operands and the selected dispatch state. The next handler consumes it.
3. Call `execute_handler` with the template report and decoded instruction.
   Admission and execution are bounded; a failed attempt rolls back its work
   while earlier committed handlers remain.
4. Consume and act on the hand-offs before choosing the next instruction.
   Branches, VM returns, `try` entries, control transfers, value-set dispatches,
   and newly captured cells can all require driver action.
5. Finalize only when the driver's completion contract is satisfied. Otherwise
   record a typed stop and retain the committed prefix.

| Handler result | What the driver must account for |
| --- | --- |
| `Continued` | The handler finished; process its hand-offs and select the successor |
| `Returned(value)` | Under `new_continuing`, this is a handler result and walking continues |
| `Thrown(value)` | With thread control, end this path; without it, the session is terminal |
| `Partial` | Inspect `last_diagnostic` for unsupported input or an existing stop |
| `BudgetStopped` | Report the exhausted bound; increasing it is a driver policy decision |

The detailed [stepping contract](../../../../docs/tracing/symbolic-session.md#stepping-one-handler)
explains check order, preflight caching, rollback, and throw behavior.

### Finish and inspect the report

`finalize_stack` and `finalize_undefined` record a terminal return under an
explicit completion convention. `stop_prefix` records a driver-level stop
without discarding the committed prefix. `report()` consumes the session and
returns the arena, completion, diagnostics, handler count, external inputs,
and final stack/slot counts.

Check the driver result as well as the report. Collecting a report does not
prove the walk completed. A stopped boundary must remain explicit, and a
partial or budget-stopped report cannot be made emittable by changing a flag.

Statements describe emitted structure; events record observations and may
have no corresponding output line. Use node descriptions and provenance to
follow a diagnostic back to the operation that produced it. See
[reading a report](../../../../docs/tracing/symbolic-session.md#reading-a-report) for
how those fields relate.

## Invariants to preserve

- **Arena ownership:** accept computed values only from this session. Node IDs
  from another report do not identify values in the current arena.
- **Transactional attempts:** discard references created by work that was
  rolled back. Earlier committed values remain valid.
- **One-shot hand-offs:** every `take_*` consumes pending state. A driver must
  act on that result before it is lost.
- **Join compatibility:** `joinable` checks VM context; the driver must also
  check PC, dispatch state, and image position. Compatible values alone are
  insufficient.
- **Scope:** a value used after a branch, loop, or `try` must remain in scope.
  Preserve hoisting and merge declarations when changing path operations.
- **Path-local state:** call frames, receiver, exception state, stack shape,
  and memory-slot membership travel with each path. Fork sides share one
  arena, so block ownership matters when switching paths.
- **Deliberate retries:** preserve the learned facts needed by a retrace with
  the matching checkpoint protocol. Otherwise the same boundary can be
  rediscovered indefinitely.
- **Shared-cell identity:** open cells before their first use and restart when
  `take_wanted_cells` requests additional captured slots. Invalidate facts
  when other code may write those cells.

## Detailed driver operations

| Work you are changing | Read |
| --- | --- |
| Session builders and immutable inputs | [Construction](../../../../docs/tracing/symbolic-session.md#constructing-a-session) |
| Speculation, rollback, and retained knowledge | [Checkpoints](../../../../docs/tracing/symbolic-session.md#checkpoints-rollback-and-facts-that-survive) |
| Forking, switching paths, merging, and abandoning an arm | [Branches](../../../../docs/tracing/symbolic-session.md#branches-fork-walk-join) |
| Why unequal stacks get holes, and when that join must be undone | [Stack alignment](../../../../docs/tracing/symbolic-session.md#why-joins-have-stack-holes) |
| Why a cache survives a retry, or must lose discarded node IDs | [Cache lifetimes](../../../../docs/tracing/symbolic-session.md#why-facts-and-caches-have-different-lifetimes) |
| Decided tests and coordinate value sets | [Predicate facts](../../../../docs/tracing/symbolic-session.md#predicate-facts-and-value-sets) |
| Widening and back-edge assignments | [Loops](../../../../docs/tracing/symbolic-session.md#loops) |
| Catch state, body writes, and scope repair | [Exceptions](../../../../docs/tracing/symbolic-session.md#try--catch--finally) |
| Outlining, summaries, and inline fallback | [Subroutines](../../../../docs/tracing/symbolic-session.md#outlined-subroutines) |
| Closures, read identities, and fact lifetimes | [Shared cells](../../../../docs/tracing/symbolic-session.md#shared-cells-and-closures) |
| A method name or suspicious trace result | [API reference](../../../../docs/tracing/symbolic-session.md#the-public-api-at-a-glance) and [troubleshooting](../../../../docs/tracing/symbolic-session.md#where-a-state-bug-belongs) |

## Source map

Start with `mod.rs` for session state and `lifecycle.rs` for a transactional
step. Read `branches.rs` and `convergence.rs` before changing merge behavior;
then follow the control-flow operation involved. Keep `exec.rs` and `model.rs`
open for the underlying evaluation and value types.

| File | Responsibility |
| --- | --- |
| [`mod.rs`](mod.rs) | Constructors, accessors, cell opening, and module wiring |
| [`lifecycle.rs`](lifecycle.rs) | Handler transactions, finalization, reports, and `trace_handler*` wrappers |
| [`branches.rs`](branches.rs) | Fork/join, stack padding and deficit checks, checkpoints, rollback, path switching, and learned facts |
| [`convergence.rs`](convergence.rs) | Merge variables, join-arm knowledge, and initial-context checks |
| [`loops.rs`](loops.rs) | Widening, loop headers, back-edge assignments, and carried values |
| [`try_blocks.rs`](try_blocks.rs) | Body/catch paths, `try` variables, and body closure |
| [`subroutines.rs`](subroutines.rs) | Outlining, call summaries, call-site writes, and unused-function cleanup |

## Focused checks

`symbolic_tracer.rs` covers direct
recipe execution and emission. Walking behavior is covered by
`vm_control_flow.rs`,
`vm_branches.rs`,
`thread.rs`, and
`prefix.rs`. Typed stop behavior belongs to
`trace_stop_codes.rs`.

Run the suites relevant to the changed boundary. The
[prefix execution guide](../../../dispatch/prefix/execute/README.md) explains
the walker policy that uses these session operations.
