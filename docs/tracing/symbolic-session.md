# Symbolic session driver guide

`TraceSession` keeps one symbolic arena and VM context alive while a driver
walks a function. This guide explains the state operations a driver uses:
transactional handler execution, checkpoints, path merges, loop widening,
exception paths, outlined calls, and shared cells.

Each tracing worker owns its session. `TraceSession` is `Send` but not `Sync`:
ownership can move between threads, while queries update internal caches and
shared session references cannot cross threads. The former `Send + Sync` test
contract has been relaxed to `Send`; consumers that relied on `Sync` must use
exclusive session ownership or synchronize access. Completed reports and debug
status support observation outside the owning worker.

Start with the [symbolic tracer guide](symbolic-tracer.md) to choose an entry
point and emit JavaScript. Use the [session module README](../../src/tracing/symbolic/session/README.md)
for the ownership boundary, implementation map, and essential invariants.
This page is the detailed reference for extending or debugging a driver.

| Need | Section |
| --- | --- |
| Choose return semantics and configure a session | [Construction](#constructing-a-session) |
| Understand an instruction's outcome | [Stepping](#stepping-one-handler) and [hand-offs](#hand-offs-and-finishing) |
| Inspect emitted structure and diagnostics | [Reading a report](#reading-a-report) |
| Retry speculative work | [Checkpoints and rollback](#checkpoints-rollback-and-facts-that-survive) |
| Merge branches or explain a pruned side | [Branches](#branches-fork-walk-join) and [predicate facts](#predicate-facts-and-value-sets) |
| Represent repeated or exceptional control flow | [Loops](#loops) and [exceptions](#try--catch--finally) |
| Reuse a traced call | [Outlined subroutines](#outlined-subroutines) |
| Preserve captured-slot identity | [Shared cells](#shared-cells-and-closures) |
| Locate a state bug | [Troubleshooting](#where-a-state-bug-belongs) and [driver mistakes](#common-driver-mistakes) |

## The shape of a walk

This sketch shows the order of operations inside a driver. It omits successor
selection and path management; the comments mark work the driver must perform
before it can collect a complete report.

```rust
let mut session = TraceSession::new_continuing(context, limits).with_thread_control();
session.set_vm_cursor(VmCursor { byte_offset: next_pc, dispatch_state: next_state });
let result = session.execute_handler(&templates, &DecodedInstruction::new(selector, operands));
if let Some(branch) = session.take_branch() { /* fork, walk both, join */ }
if let Some(value) = session.take_thread_return() { /* this path returned */ }
let report = session.report();
```

Choosing successors, switching to a forked path, joining it back, retracing a
loop, and ending a path at a VM return are policy and belong to the driver.

## Constructing a session

Pick the return convention first. `TraceSession::new` treats a handler `return`
as terminal: the return statement is emitted and the session stops, which is
right when the supplied sequence is the whole unit. `new_continuing` treats it
as a dispatch continuation: the value is still evaluated and recorded, but as an
`Evaluate` statement, never as `return`, so emitted JavaScript cannot leave
early. A VM function return is a different event (`vmPendingResult = value`,
exposed through `take_thread_return`).

Both constructors check the incoming context immediately: a caller-supplied
`Computed` node, an oversized stack, or too many memory slots stop before any
handler runs. Further handler calls on a stopped session return
`Partial`/`BudgetStopped` without running the recipe. An unexpected
`computed-input` or `initial-*-budget` therefore means inspecting the initial
context before investigating the walk.

The builders add facts only the driver owns:

| Builder | Adds and why |
| --- | --- |
| `with_debug` | A live per-operation writer; the driver owns where output goes |
| `with_constant_table` / `with_number_table` | An immutable string/number table: a concrete in-range read is that entry, any other read stays abstract or stops. The table identity is proved during preparation |
| `with_try_depth_traps` | A `try` exit below zero traps, following the lifted image's semantics |
| `with_cell_contents` | What each shared cell may hold (addresses and/or data), from analysing every function holding it |
| `with_captures` / `with_created_hook` / `with_returns_hook` | Which slots a descriptor captures, what a created VM function is, and what a call of one returns; lets the driver trace it later or leave the call symbolic |
| `with_thread_control` | Driver hand-offs for program-value branches, VM returns, `try` entries. Without it those stop with `dynamic-branch`, `unhandled-thread-return`, `unhandled-try` |

## Stepping one handler

`execute_handler` resolves the instruction's `handler_index` in the template
report and delegates to `execute_handler_at`. Order matters when reading a stop:

1. Take the VM cursor. It describes exactly one dispatch, so it is consumed even
   from a stopped session; a later handler must not push a stale return address.
2. If stopped, report `BudgetStopped` for a budget stop, `Partial` otherwise.
3. Check the step budget, then reject a missing handler and a caller-supplied
   `Computed` operand (its node belongs to another arena).
4. Preflight the recipe once. The verdict is cached under the template's identity
   and step slice, so thousands of occurrences of one immutable recipe reuse it.
5. Checkpoint, run, map the outcome.

The checkpoint precedes execution, so an unsupported or over-budget attempt is
rolled back whole — no partial value, statement, event, diagnostic, or binding
serial survives — while earlier committed handlers stay. The cursor is restored
for the run and cleared afterwards; a successful handler also clears
`cell_assumed`, the per-instruction assumptions made by a value dispatch.

| Result | Meaning | Driver response |
| --- | --- | --- |
| `Continued` | The handler finished | Fetch the next instruction |
| `Returned(value)` | A recipe return produced a value | With `new_continuing`, keep walking |
| `Thrown(value)` | A JavaScript throw | Without thread control the session is stopped; with it, end only this path |
| `Partial` | Unsupported input or already stopped | Read `last_diagnostic`; retry only if the cause can go away |
| `BudgetStopped` | A step, node, stack or memory bound | Report a budget stop; raising a limit is policy, not a fix |

Without thread control a throw is a terminal, emittable completion (`Thrown`);
with thread control the emitted `throw` stands where the VM threw, the session
stays runnable, and other paths can still complete the report.

## Hand-offs and finishing

Every "the last handler did X" accessor takes the value out, so a driver cannot
act twice on one event or silently miss that it already did: `take_branch` (a
program-value conditional with its test, target, and whether a proof decided it),
`take_thread_return`, `take_try_entry`, `take_created_now`,
`take_control_transfer` (PC/state assignments replacing fall-through),
`take_dispatch` (a coordinate that is a variable with a known value set), and
`take_wanted_cells` (slots a new closure captured, for a restart). Read-only
inspection is `call_frames`, `try_depth`, `exception_depths` (the number of `try`
frames and error records, recorded by the image patcher), `recipe_steps`,
`is_stopped`, and `last_diagnostic`; `cell_facts` snapshots every cell's concrete
value plus the stack length, which is how a driver compares two fork sides.

Completion is a driver decision. `finalize_stack(index, provenance)` reads the
slot (holes read as `undefined`), records a `StackRead` event and a terminal
`return`, and completes `Returned`; it refuses an out-of-range index and a
stopped session. `finalize_undefined` does the same for `undefined`.
`stop_prefix(code, message, provenance)` records a driver stop while keeping the
committed prefix, so a truncated image or exhausted walk budget never
masquerades as completion. `report()` consumes the session, flushes the debug
`STOP` line, and returns the arena, completion, diagnostics, committed handler
count, external inputs, and a final-state summary — data about what happened,
not a resumable context.

Collecting a report also does not prove the driver's
contract: a trace can be `Completed` while the driver stopped, and emission
refuses that. The free functions `trace_handler`, `trace_handlers`, and their
`_with_debug` variants are thin wrappers for the direct-recipe case: terminal
session, fixed sequence, break on return/throw/stop.

Thread control is per path; completion is per session. Each forked path carries
its own VM state, but completion and diagnostics are global, so an ended path is
closed by the driver (`continue_alone`, or simply dropping it) rather than by
setting a stop. That is also why `Thrown` is a completion only without thread
control: one path's throw must not end the walk for its sibling.

## Reading a report

`TraceReport` is smaller than it looks, and its fields answer different
questions. `arena.statements` is the emitted structure — blocks, declarations,
assignments, control flow; `arena.events` is the ordered observation log (reads,
writes, initializations, branch decisions), and an event often has no matching
output line. Follow event provenance to the nodes and statements it produced
instead of looking for a one-to-one mapping. `TraceArena::describe` and
`describer()` render a node as a compact expression with bindings inlined, which
is the fastest way to understand a test or a merged value.

`diagnostics` carries
a `TraceStopCode` plus optional provenance; `handlers_traced` counts handler
occurrences, not emitted lines; `external_inputs` is what the emitter must be
allowed to name; `final_state` is only a stack length and slot count. Emission
reads this same report, so a diagnostic and a later emission refusal usually
point at one boundary: fix that, and start with the first one.

## Checkpoints, rollback, and facts that survive

`checkpoint` saves arena lengths, the whole `TraceContext`, diagnostic count,
external inputs, binding and loop serials, completion, and the current block and
its length. `rollback` truncates the arena, restores all of it, and clears the
one-shot hand-offs. It is cheap because `TraceContext` shares its bulk through
`Arc`: memory slots and fact maps copy only when written, so a checkpoint mostly
clones handles.

Two undos are easy to confuse. `learned_facts`/`forget` (crate-only) preserve what
a retrace must not rediscover — markings, used sets, cell write observations,
constant tables, step counts — because a rollback drops them and a restarted
region would otherwise rediscover the same boundaries forever.
`stop_marks`/`restore_stop_marks`/`resume` handle a stopped handler attempt:
`restore_stop_marks` undoes it, while `resume` keeps the diagnostic and restores
only completion, which is what a recovered attempt should report.

## Branches: fork, walk, join

There is one executor, so a fork does not run two. `fork(test)` copies the current
context into a `TracePath` per arm, emits an empty `Declare` (where join variables
go, before the `if`) and an empty `If`, and learns per-arm branch and type facts.
The session keeps running the current path until `switch_path`, which is a swap —
the path passed in receives the state that was running, which is how a driver
rotates sides without copying. `current_path` copies when the driver needs to
keep the running state; `cut_back` restores a side to an earlier copy.

`joinable` / `join_conflict` state the merge precondition and name the first
mismatch: call frames, receiver, `try`/underflow/error state, pending exceptions,
stack length, and the set of memory slots. The driver's position (PC, dispatch
state, image) is not checked, so it must verify that both arms stand on the same
instruction.

`join` then merges cells: identical nodes and equal facts stay; two
reads of the same variable stay that variable if their facts agree; equal
concrete facts are rebuilt outside the arms (an arm-local binding is not in scope
after the `if`); otherwise the cell becomes a fresh `trace_value_N`, declared
before the `if` and assigned at the end of each arm.

Facts intersect — branch
facts only where both arms agree, type facts intersected (except variables named
inside the fork, which one arm alone can know), `try` writes and escaped cells
unioned, `from_catch` conjoined — and cell position/value marks join via
`CellMark::join`, so a VM position one side wrote survives unless both sides wrote
a non-position value.

The emitted shape of such a merge:

```js
let trace_value_0;
if (test) {
  trace_value_0 = 3;
} else {
  trace_value_0 = 5;
}
use(trace_value_0);
```

The declaration is outside the `if` because either arm may have produced the
value, and the assignment is at the end of each arm so it observes that arm's
final state. A cell both arms left equal keeps its value instead of getting a
merge variable.

`join_tagged` tags merges that had a concrete value on at least one side; a later
coordinate that needs the cell concrete reports the tag through `take_split`, and
`cells_differ` tells the driver whether the arms still disagree there. That is the
retry loop for "this join cannot be a jump target, walk the paths apart."

`continue_alone` closes a fork when one arm ended: it hoists values out of the
surviving arm, returns to the parent block, and carries the ended arm's `try`
writes.

`only_pure_bindings_since` lets a driver discard an abandoned arm whose
only effect was literal binding. `cut_back` neutralizes what the removed side
traced: ordinary statements become empty `Declare`s (IDs are stable and may be
referenced), events become `Undone` (the log is ordered, so deleting would
renumber later observations), and functions stay with their bodies because the
other side may still call them. Deleting instead of neutralizing breaks every
later reference.

Walking a branch, in the driver's order:

1. `take_branch`. If `decided` is set, follow the chosen successor; otherwise
   `fork`.
2. Walk each side in turns with `switch_path`, taking a checkpoint before any
   region whose result might be discarded.
3. When a side reaches a position the other passed since the fork, check
   `joinable` and the driver's own position; if both agree, `join`; if not, keep
   the sides apart or `cut_back` and retry with more facts.
4. If one side ends, `continue_alone` on the other and carry on after the `if`.
5. If an arm is proven fake, roll back to the fork and `record_fake_branch`.

## Stack alignment and cached knowledge

### Why joins have stack holes

Not every stack-length mismatch makes a branch fake. One arm can leave an
unused value below its result while both arms go on to use their top value.
Walking the entire suffix twice duplicates work and can hide a later meeting;
merging stacks by their bottom indices would instead read the wrong value.
`stack_padding` recognizes the narrower case where other VM state agrees and
`pad_stack` inserts holes below the shorter arm's result to align the tops.

For example, with `base` the unchanged shared prefix:

```text
long arm:   [base, unused, resultA]
short arm:  [base,         resultB]
aligned:    [base, <hole>, resultB]
```

The join can now merge `resultA` and `resultB` and trace their common use once.
The hole is analysis bookkeeping, not a value the VM pushed. Ordinary sparse
stack holes read as `undefined`; a live join-padding region is guarded before
that read can happen. `resolve_stack`, stack writes, and length changes report
`PaddedStack` if they reach the padding or the values below it. The driver uses
`take_pad_conflict` to undo the responsible join and retrace the arms apart.
A bottom-relative access also invalidates the alignment. It must never quietly
consume a fabricated `undefined` from a padded join.

`StackPadding` records the shorter arm, insertion point, count, earlier join,
and the `deficit` / `loop_variable` cases. A deficit means the longer arm kept
only fork-entry values while the shorter arm popped some of those values.
Only a top-relative access below the shorter arm's own bottom, with one padding
join and that deficit established, supplies fake-side evidence. Other padding
conflicts only reject the join. In particular, popping one alternative in
`v || x` can expose another value already on the shorter stack; that is real
code, not an underflow proof.

Padding is refused inside an outlined subroutine or where moving a cell would
invalidate parameter or catch-entry identities. An enclosing loop's named
stack cell can move in the supported non-deficit case, but its `continue` or
`break` must undo that join (`loop_before_padding`). A fixed stack reset to
the shared prefix can clear the padding because the stacks agree again.
An earlier join's holes retain their origin when propagated; overlapping
joins cannot borrow each other's holes as deficit evidence.

The session owns the alignment and reports conflicts; the dispatch walker owns
the rollback, `Learned::unpadded`, and any `stack-deficit` branch decision.
See [the walker contract](prefix-execution.md#stacks-that-differ-only-by-what-one-arm-left),
[`branches.rs`](../../src/tracing/symbolic/session/branches.rs), and
[`padded_stop` in `exec.rs`](../../src/tracing/symbolic/exec.rs). The paired tests
`branches_with_different_stack_lengths_join_lined_up_from_the_top`,
`a_side_that_pops_to_a_value_pushed_before_the_test_is_real`, and
`a_side_whose_stack_the_code_after_it_empties_is_fake` in
`vm_branches.rs` separate safe alignment, a rejected
alignment with a real branch, and actual deficit evidence.

### Why facts and caches have different lifetimes

Retries are part of tracing: a later instruction can reveal a loop variable,
invalidate a join, or require an outlined call to be traced inline. Keeping no
knowledge repeats the same failed attempt; keeping every old fact attaches
discarded values to new nodes. The cache boundary follows the identity of the
answer's inputs:

| Stored result | Scope and reason | What must be forgotten or rechecked |
| --- | --- | --- |
| `Executor::preflights` | One session, keyed by template address, handler index, step-slice address and length. Repeated occurrences reuse recipe admission, not execution results. | Operands and current VM state still run through the normal checks on every occurrence. |
| `Executor::reaches_marked` | One arena, indexed by node ID. Memoizes whether an expression depends on a marked join or outlined-call result, avoiding repeated graph searches. | Rollback truncates entries for removed nodes; a reused numeric ID identifies a different value. |
| `aliases`, `joins`, `call_returns`, `concrete_reads`, `position_reads` | Facts about particular graph nodes, used to explain predicates and coordinates through copies, joins, and calls. | Rollback removes entries for discarded nodes. These are decision facts, not permission to replace emitted shared-cell reads with constants. |
| `read_slots`, `read_names` | Which cell a particular read node or generated binding read. Needed to record dependencies on shared-cell contents. | Rollback removes freed node/name identities, while `consulted`, `data_consulted`, and `set_consulted` retain what the attempts asked of cells. |
| Walker `Learned` | Positions, meetings, widenings, value/type sets, rejected padding, inline-call requirements, and context-proven fake sides survive the appropriate retrace. | Restore the knowledge at the right region boundary when undoing a speculative join; widening a set used by a decision requires retracing it. |
| Preparation `junk_entries` | Shared by walks of the same capture; caches the standardized created-function entry check. | Descriptor, entry PC/state, and whether descriptor startup slots were supplied belong in the key. This is not a summary of arbitrary caller state. |
| Worker `Round` walks | Reuses a function's own trace for return-shape answers instead of running a second speculative callee trace. | Reuse requires matching cell dependencies and the answers previously supplied about callees; changed inputs can require another round. |

The last three are driver/worker caches, not `TraceSession` state. Their source
owners are [`state.rs`](../../src/dispatch/prefix/execute/state.rs),
[`converge.rs`](../../src/dispatch/prefix/execute/converge.rs), and
[`return_shapes.rs`](../../src/trace/return_shapes.rs). A completed worker
walk can be reused when the portions of cell contents it queried are unchanged
(`CellReads::same`); a stopped walk is retried for any changed contents, which
may contain its missing proof. Recursive return-shape queries use the preceding
round's answer instead of waiting on themselves, and the recorded answers
determine whether that walk stands in the next round.

`TraceContext`'s `Arc` maps serve another purpose: cheap snapshots with
copy-on-write isolation between arms. They are not a shared mutable store of
facts every path can trust. Keep this distinction when adding caches or moving
a field out of path state.

## Predicate facts and value sets

Fake branches — a test whose unused successor leads into state real code never
reaches — make forking waste work, so the session carries three mechanisms. Path
facts: `learn(name, cell, value, is)` records `variable === value` (or not) on the
running path; `assume`/`exclude` do it on a non-running `TracePath`, which is how
a driver splits a value-set dispatch before stepping either side; `equality_test`
builds the test node (`name === value`, or `$CELL_<slot>["value"] === value` for
a cell read).

Test proofs: a handler whose `JumpIf` test is provably decided
records `PendingBranch::decided` with `BranchEvidence::Test` and a
`BranchDecided` event.

Context rejections: when the driver proves an arm reaches
impossible VM state, it rolls back and calls `record_fake_branch(test, taken)`,
recording the surviving direction with `BranchEvidence::Context`. Keeping the
evidence kinds distinct is what makes branch triage explainable: a proof and a
state rejection are different claims.

Value sets extend this to VM coordinates. A loop or `try` variable may hold one of
a bounded set of concrete values (at most 64; more is a jump table, not a
dispatch). When a jump target or dispatch state is such a variable,
`take_dispatch` returns the variable, its cell, and the remaining candidates, and
the driver splits on them. `take_used_sets` names the variables a VM value was
taken from: if a later retrace grows one of those sets, the earlier decision was
incomplete and the walk restarts, which is why a value-set decision is not
silently trusted across iterations. `take_junk_origin` identifies the fork whose
arm carried out a program value later used as a jump target;
`outlined_call_in`/`unresolved_call_in` say whether a test depends on an
uninlined outlined call (`take_inline` reports the call that needed it);
`copies_variable` answers whether a name is derived from another name.

## Loops

The session never executes a loop. The driver reaches a header twice and asks the
session to widen the cells that changed. `open_loop` emits `let trace_value_N =
<entry value>` before a labeled `for (;;)`, sets each widened cell to that
variable, and returns a `TraceLoop` with the header state; later code reads the
cell as the variable until something writes the cell.

At a back edge,
`continue_loop` verifies that the iteration returns with the same call frames,
receiver, stack length, and memory-slot set, emits assignments for every tracked
variable, emits `continue label`, and returns changed cells that were not widened
yet. A non-empty return is the driver's signal to roll back and retrace with
those cells included — which is why first-iteration constants are not frozen into
the emitted loop.

Assignments are sequential, so a value reading another loop
variable is copied first; otherwise `a = b; b = a` would rotate values the VM had
snapshotted.

Helpers answer driver questions without exposing context fields: `loop_values` /
`loop_types` (what each variable holds, and its possible types),
`loop_outlined_call` (whether it holds an outlined result, so tracing inline might
yield a value set), `loop_cell_changes` / `loop_cell_writes` (which marks
changed, and which concrete cells the iteration overwrote, for `open_loop_with`),
`loop_stack_change`, and `type_loop_variables` (start or carried types after the
loop).

`break_loop` hoists values out of the body (a `break` leaves the block, so
an inner binding is out of scope), emits `break label`, and moves to the parent
block; `close_loop` only leaves the body when the loop ends normally. Widening is
not free — each new cell restarts the loop region, and the walker bounds headers
and widenings — so widen only cells that actually change.

## `try` / `catch` / `finally`

A catch does not start from the state before the `try`. The VM's exception
handler pops the frame, pushes an error record, and makes the thrown value the
catch parameter. `fork_try` builds exactly that catch state next to a body path;
without it a catch would read values the body had already overwritten.
`fork_try_with` adds earlier observations: each widened cell gets a variable
declared before the `try`; every body write assigns cell and variable
(`note_try_write`), and the catch reads the variable, so it sees whatever the body
left there rather than a stale entry value.

`values` supplies initial value sets, `types`
supplies initial type facts, and `cells_written` tells the catch which cells the body may
write so it can keep facts about the rest. `value_sets` is the general bounded
set; `jump_sets` is narrower — only non-negative integer addresses, and only VM
jumps read it, because a jump through a cell that may hold a program value leaves
the code the VM runs.

`close_try_body` runs when the body pops its frame normally. It requires the body
block to be the `try`'s own arm (a body ending inside a nested block makes the
hoist ambiguous), hoists carried values, rewrites widened cell reads as their
variables so a join with the catch sees the same variable on both sides, records
`left_try` for the join's scope repair, restores the enclosing `try` scope, and
merges changed cells into the outer `try`'s writes. Nested `try`s stack in
`try_outer`, each owning its variables and writes until it closes. `from_catch`
marks a path whose stack below the `try` entry is whatever the throw left: a later
stack-index failure there is under-specification, not evidence of a fake branch,
which is why the walker stops with `CatchStackUnknown` instead of pruning.

## Outlined subroutines

A subroutine called many times is traced once as a top-level function and called,
keeping emitted code linear and stopping the walker from re-tracing the body at
every call site. The cost is that the function may use only what a function can
see. `open_subroutine` emits the function, turns every caller stack cell and
memory slot into a parameter placeholder, and resets facts a function cannot
inherit: receiver, branch/type facts, merged concretes, loop variables, value
sets, jump sets, assumptions, and `try` state.

`close_subroutine` validates the return shape before accepting a summary: call
frames must match the caller's minus the returning frame, `try` and exception
state must be unchanged, and every identifier the body reads must be declared in
the body or be a placeholder. A body reading a caller variable has no valid
parameter list, so the driver must trace that call inline. Changed caller cells
are recorded relative to the call (stack offset from entry length, or a memory
slot) as either a constant or one of the returned values; several return values
become an array literal. The parameters are the placeholders actually read.

`call_subroutine(summary, tag)` resolves relative cells against the current stack,
refuses (changing nothing) if a needed cell is missing, emits
`callee.call(undefined, arguments...)`, binds the result, resizes the stack by
`stack_delta`, writes the outputs back, and records the return values in terms of
the arguments (`call_returns`) so a later test on the result may be decided
without inlining. Written output values are tagged in `outlined`; if a test or
coordinate later depends on one, the driver rolls back and retraces that call
inline. `has_subroutine` reports whether the function survived a rollback, and
`drop_unused_functions` removes functions orphaned by a cut-back side.

## Shared cells and closures

Shared cells model `vmCaptureThread`-style aliasing: a VM function and the
closures it creates read and write a slot through a cell object. `open_cells`
makes the listed slots cells for the walk: one `let $CELL_n;` per slot, then
either the incoming cell value (a caller-passed cell, which also becomes escaped
because another function may write it) or a fresh `{ value }` object. From then
on the slot is `$CELL_n.value` in emitted code, and a new cell is a new object. It
must be called before the first step, and a walk that restarts rolls back to a
checkpoint taken before it, which is why the driver opens cells again on the next
attempt.

A closure capturing a slot that is not yet opened cannot be traced
correctly; the session records those slots in `wanted_cells`, and
`take_wanted_cells` hands them to the driver, which opens them and restarts.
`with_captures` supplies the per-descriptor slot lists so a created closure is
handed its existing cells.

`with_cell_contents` supplies what each shared cell can hold (addresses and
whether any function writes data); a read with the same effect under two analyses
traces the same, which lets a walker reuse a trace when only irrelevant contents
changed. `cell_reads` reports what the walk used — the cells read, those asked for
data, and those asked for a value set — so a caller can tell whether a trace is
still valid after the analysis improves.

`CellRead { slot, read }` identifies one read of one shared cell by one
instruction, where `read` counts earlier reads of that slot by the same
instruction. Assumptions and re-runs are keyed by that identity, so the same
instruction can be given different values for its first and second read of a
cell. `CellReads` summarizes what a walk consulted: the cells, those it asked
for data, and those it asked for a value set.

Shared-cell facts form a ladder:

| Record | Holds | Invalidated by |
| --- | --- | --- |
| `cell_facts` / `cell_nodes` | What this path last wrote | Any statement that may run other code |
| `cell_concrete` | The node the cell concretely holds | A write by an escaped closure, or an iteration that writes it |
| `cell_writes` | The executor's write serial per slot | Never; used to tell which iteration wrote |
| `cell_positions` / `cell_values` | Whether the cell holds a VM position or a program value | Only VM jumps read them; `CellMark::join` merges them |

Keeping them apart is what lets the tracer decide an opaque predicate on a cell
while still emitting a read of the cell rather than a constant.

`with_debug`
attaches a writer (owned by the session, no global state) that prints a preamble
per operation, effect lines, and a final `STOP:` line; `take_debug` moves it to a
replacement session so a restart stays in one stream. A failed write disables the
sink and records the error in `TraceDebugStatus`; it never changes execution.

## The public API at a glance

Reachable from `tracing::symbolic` and the crate root. `forget`,
`learned_facts`, `execute_handler_at`, and `take_debug` are crate-only helpers
for the in-crate drivers.

| Task | Methods |
| --- | --- |
| Construct | `new`, `new_continuing`, the `with_*` builders above |
| Step | `execute_handler`, `set_vm_cursor`, `take_control_transfer`, `call_frames`, `recipe_steps` |
| Observe | `is_stopped`, `last_diagnostic`, `exception_depths`, `try_depth`, `cell_facts`, `cells`, `statement_count`, `event_count`, `block_length` |
| Finish | `finalize_stack`, `finalize_undefined`, `stop_prefix`, `report`, `with_debug`; `trace_handler(s)`, `trace_handler(s)_with_debug` |
| Hand off | `take_branch`, `take_thread_return`, `take_try_entry`, `take_created_now`, `take_dispatch`, `take_used_sets`, `take_split`, `take_inline`, `take_junk_origin`, `take_wanted_cells` |
| Snapshot | `checkpoint`, `rollback`, `stop_marks`, `restore_stop_marks`, `resume` |
| Branch | `fork`, `switch_path`, `current_path`, `join`, `join_tagged`, `joinable`, `join_conflict`, `continue_alone`, `cut_back`, `cells_differ`, `only_pure_bindings_since` |
| Facts | `learn`, `assume`, `exclude`, `equality_test`, `record_fake_branch`, `copies_variable`, `outlined_call_in`, `unresolved_call_in` |
| Loop | `open_loop`, `open_loop_with`, `continue_loop`, `break_loop`, `close_loop`, `loop_values`, `loop_types`, `loop_outlined_call`, `loop_cell_changes`, `loop_cell_writes`, `loop_stack_change`, `type_loop_variables` |
| `try` | `fork_try`, `fork_try_with`, `close_try_body`, `try_variable_names`, `try_assigned_values`, `try_assigned_types`, `take_try_body_writes`, `take_try_body_cell_writes`, `try_writes`, `from_catch` |
| Calls | `open_subroutine`, `close_subroutine`, `call_subroutine`, `has_subroutine`, `drop_unused_functions` |
| Cells | `open_cells`, `take_wanted_cells`, `cell_writes`, `cell_reads` |

## Where a state bug belongs

| Symptom | Inspect |
| --- | --- |
| Unsupported handler leaks work into a committed prefix | Checkpoint/rollback order in `lifecycle.rs` |
| Wrong handler's diagnostic reported | The preflight cache key and missing-handler check |
| Branch arms join with incompatible VM state | `join_conflict` (names the first mismatch), then `merge_cell` for differing cells |
| A join uses a value that is not in scope | `hoist`, `left_try`/`in_scope_at`, and whether `close_try_body` ran first |
| A loop needs another iteration's values | Widening; check the driver feeds `loop_cell_changes` back into `open_loop_with` |
| A loop's emitted values rotate | The sequential-copy step in `continue_loop` |
| An outlined call needs caller-only facts | `close_subroutine`'s caller-variable rejection, then the inline fallback |
| A catch reads stale values or loses completion | `fork_try_with`'s cell filtering, then the driver's try handling |
| A retrace keeps facts from the wrong path | `learned_facts`/`forget` pairing and the matching checkpoint owner |
| A discarded arm still references nodes | `cut_back`: neutralize, do not delete |
| A closure silently misses a cell | `take_wanted_cells` was dropped, or `open_cells` ran late |
| A session stops before the first decision | `check_initial_context` and the initial context |
| Emission refuses a supposedly complete trace | Completion is `PartialUnsupported`, `BudgetStopped`, or `PrefixStopped` |

## Common driver mistakes

Each of these leaves a plausible-looking trace with a silent gap, which is why
they are worth checking first:

- Consuming a branch (or a VM return, or a `try` entry) without acting on it.
- Joining two arms without checking the driver's own PC/dispatch state — the
  session checks VM state, not the instruction both sides stand on.
- Keeping node or statement IDs created by work that a rollback discarded.
- Opening cells after the first handler, when earlier reads already traced the
  slot as a plain value.
- Treating a value set as monotone: it can grow after a retrace, which is what
  `take_used_sets` exists to catch.
- Widening every changed cell instead of the ones that matter, paying a full
  loop retrace for bookkeeping.
- Deleting statements or nodes in `cut_back` instead of neutralizing them.
- Setting completion by hand to make emission accept a stopped boundary.

## Invariants worth keeping

- One session owns one arena. Never feed a `Computed` from another report into a
  context or operand; the API rejects it, and a workaround would attach a node ID
  to unrelated data.
- Hand-offs are consumed once. Ignoring a `take_*` result is how a driver stops
  noticing branches or VM returns.
- The cursor describes one dispatch; set it only for a handler that pushes a call
  frame, and do not expect it to persist.
- A failed attempt is gone. Discard IDs created by that attempt; earlier
  committed values remain valid.
- Retries preserve knowledge deliberately: use `learned_facts`/`forget`, or the
  walk rediscovers the same boundaries forever.
- Every emitted value must remain in scope where it is used; hoisting and join
  declarations exist for that.
- A stop is not a completion; `PartialUnsupported`, `BudgetStopped`, and
  `PrefixStopped` never become emittable by changing a flag.
- Paths carry state, not just values; call frames, receiver, `try` frames, error
  records, and pending exceptions decide joinability before values are compared.
- Fork sides share one arena; a statement belongs to whichever block the driver
  set, so switching paths without switching blocks misfiles output.
- Values and facts have different lifetimes: a value node is immutable once
  built, a cell's contents are not; do not reuse a cell fact after code that may
  write it.

A checkpoint allows a speculative handler or path to roll back without re-running
earlier work. Partial and budget-stopped results remain explicit; emission does
not turn them into complete JavaScript. Direct recipe behavior is covered by
`symbolic_tracer.rs`; walking control flow
by `vm_control_flow.rs`,
`vm_branches.rs`,
`thread.rs`, and
`prefix.rs`; stop codes by
`trace_stop_codes.rs`. See also the
[execution and emission guide](symbolic-execution.md), the [prefix execution
guide](../../src/dispatch/prefix/execute/README.md), and the [symbolic tracer
guide](symbolic-tracer.md).
