# Prefix execution and control flow

The prefix walker turns an admitted preparation into a symbolic trace. This
reference explains its path state, joins, loops, exceptions, calls, and image
patches. The [execution module guide](../../src/dispatch/prefix/execute/README.md)
provides the file map; [prefix diagnostics](prefix-diagnostics.md) explains stops
and discarded work. [Symbolic sessions](symbolic-session.md) owns the lower-level
state operations used by this driver.

## Place in the pipeline

`run_prefix` is an internal execution entry, not a public source-loading
API. Public callers use the parent's preparation and invocation methods.
Preparation supplies the proven image, selector/handler relationships,
operand descriptors, and optional WASM machine. Each invocation gets a
private image clone and a continuing `TraceSession`; committed patches
cannot modify another invocation.

```text
PrefixPreparation + start + context + limits
  → dispatch/operand step
  → admitted symbolic handler
  → commit record, cursor, and image effects
  → next side / join / loop / call / completion or stop
  → PrefixTraceReport
```

The walker owns control-flow policy. The symbolic session owns arena and
VM state operations, and [`dispatch/step.rs`](../../src/dispatch/step.rs) shares the
bounded dispatch/operand step primitives. Changes to one layer should
retain the other layers' identity and commit rules.

One invocation is a loop over attempts. A new loop header, a split, a
forced fake branch, or a closure sharing an unknown cell rolls the session
back to its entry checkpoint, clears the attempt's records and patches,
and reruns with what it learned. `learned` is keyed by positions, so it
survives; outlined functions' records live outside the attempt's log, so a
rollback cannot lose them. A restart redoes every step from the entry,
which `wasted_steps` counts.

## The state a path carries

A `Cursor` is `pc`, `state`, `image`, `split_image`, and `visited`; a
position is `(pc, state, VM call frames)`, never `(pc, state)` alone.

The image is an `Arc<Vec<u8>>` shared until a patch writes it, so no cursor
writes another's bytes. `split_image` records bytes the merged histories may
patch differently. `visited` is frozen layers plus local positions; `Log<T>`
keeps dropped ranges, and `Learned` is shared and merged three-way.

## Forks in turns

A conditional jump on a program value is the only place a path becomes
two. `fork_sides` creates the `taken` side at the branch target and the
`fall_through` side at the cursor, then runs turns: each turn a running
side takes one dispatch step; a nested fork, loop, or `try` is one step
because `advance` runs it to its end.

A turn is symmetric: both sides start from the same `learned` snapshot and
merge their results, and a halt undoes the other side's step. `reverse_sides`
steps the fall-through first so callers can compare branch-order behavior.

### Meeting points and cut-back

A side that stands where the other has been since the fork found a meeting
point, if the position is ahead of the pre-fork path and not unmergeable.
The lower crossing wins, so which side steps first decides nothing; a
dispatch's own instruction never counts. `learned.meets` caches the point
by the jump.

The side that was there first is cut back to the step that passed the
point: cursor, `split_image`, and `visited` rewind to its mark, patches
are undone latest first, and its statements, events, records, and patches
are dropped. It then retraces that step to the meeting point.

### Joins, and continuing alone

At the same position, `join_conflict` must report none: same call stack,
receiver, try/error state, pending exceptions, stack length, and memory keys.
`join_tagged` merges the arms and tags the join with the jump so a later stop
can re-split that fork (`split_or`).

A side whose other side ended runs on in its own arm. At a target of an
enclosing fork, `continue_alone` closes the `if` and hoists it out with the
ended path's `try` writes; if it cannot, the walk stops with
`UnjoinableBranches` rather than emit a scope error.

### Stacks that differ only by what one arm left

When the only difference is the stack's length (`stack_padding`), the arms
usually compiled `cond ? a : b` with one arm leaving a value it never pops.
The shorter stack gets holes (`pad_stack`) where the part both share ends, so
what each arm pushed above it lines up from the top: every push, pop and read
counted from the top then reads what the VM reads on each path, a hole reads
`undefined` as a read below a stack's end does, and the rest of the function
is traced once. A hole stands for a value only one path has, so the join is
undone (`Halt::Split`, `Learned::unpadded`) when a step reaches the holes or
what lies under them, a handler counts stack positions from the bottom
(`stack_access`), a loop entered before the join continues or breaks, a later
join on the path fails, or a loop's stack length changes. Undoing goes back to
what the walk had learned when the arms joined, so the retrace is the walk
that never padded. A cut to a fixed length no longer than the shared part
(`StackAccess::Reset`) leaves both paths the same stack and clears the holes.
There is no padding inside a subroutine traced as a function, at a known loop
header (a loop entered at two depths stays two loops), or where a parameter or
a `try` the catch reads names a stack cell above the shared part. A stack cell
an enclosing loop names may lie above it (`StackPadding::loop_variable`): the
loop's `continue` or `break` on the padded path undoes the join, and so does a
step reaching the holes. Not for a deficit (below), whose evidence needs its
own join's holes alone: a deficit lined up in the holes of a join under a loop
variable undoes that outer join instead, and is lined up alone on the retrace.
A step that reaches holes of a join already undone (a path a loop or fork saved
kept them) starts the whole walk over (`Halt::Restart`), knowing it undone.

One shape can decide the fork: the longer side pushed nothing of its own and
the shorter side popped values the stack held when the fork opened
(`StackPadding::deficit`). The holes go where they go for any other join. A
read, write or length set from the top that reaches a position below the
number of holes is below the shorter side's own bottom, wherever the holes
are: a VM running that side would read under an empty stack. That decides the
fork (`learned.forced`, reason `stack-deficit`) the same however the walk got
there, which an underflow seen later while some other fork steps does not. A
step reaching the holes above that reads a value the shorter side has: `v ||
x` with `x` pushed before the test pops `v` on one side and keeps it on the
other, and both sides are real. That step, like any other on a stack padded by
more than one join, only undoes the join as above.

### When the sides cannot merge

If `join_conflict` names any other difference, or a cell the sides must agree
on still differs, they do not join: the position joins `unmergeable` and both
step past it, to a later point or each to its end, until `MAX_APART_STEPS`
stops the walk with `UnjoinableBranches`. A wrong join would assign values
only one arm has; stepping apart duplicates the rest of the function.

## Fake branches

Not every conditional jump is a real `if`. A fake branch's test always has
the same outcome, and its other side leads into code never meant to run. A
test known from its operands is decided outright (`FakeByTest { taken }`,
no fork); otherwise the walk forks, and a side that reaches a state real
code never reaches is dropped, rolling back to the jump to follow only the
other side (`FakeByContext { taken, reason }`). The verdict is kept in
`learned.forced`, as a fake branch goes the same way however it is reached:
a retrace of an enclosing loop or fork follows the real side at once instead
of running the junk side to its bad state again.

`bad_state` decides what counts: an out-of-image pc or operand, a missing
state or opcode, a missing call frame, an empty exception stack, an
invalid index, a program-value jump, a primitive call, a WASM trap, or a
junk entry. A stack change counts only outside a catch and a
try-underflow only for WASM; reading below a catch's stack, or after a
catch, is no evidence, and a tracer limit is not a program fact.

A VM jump straight to a program value blames the fork whose side made it;
if the value was carried out of a closed `if`, the walk restarts with that
side known fake (`ForceFake`). Otherwise the stop is no evidence, and the
diagnostic is re-stamped `DynamicIndex` rather than blaming an enclosing
fork.

`junk_entry` probes a created function's straight entry code for
`ENTRY_PROBE_STEPS`. Only the supported VM-invariant failures before its first
real branch establish junk; exhausting the probe is no proof. Startup uses
the descriptor's initialized slots as `undefined` and caller-filled slots as
opaque external values. A `try` normally prevents this proof, except when a
dispatch fault is reached and every recorded handler is itself outside the
image or has a missing state. An entry already outside the image is attributed
to the fork that patched it there (`patched_junk_entry`), not indiscriminately
to any creator. Probes do not recursively check functions they create.

The per-walk `entries` map avoids repeated lookups; preparation-wide
`junk_entries` shares the standardized result across functions and retraces.
Its key includes descriptor, entry PC/state, and whether startup-slot metadata
is present. Re-running that same check in every convergence round was expensive
without supplying new evidence. See [cache lifetimes](symbolic-session.md#why-facts-and-caches-have-different-lifetimes).

With `audit_fakes`, a test-decided branch's ruled-out side is walked alone
for `AUDIT_STEPS` and reported in `fake_audits` as `confirmed`, `runs`,
`unknown`, or `bound`.

An earlier outcome of the same program test does not by itself make a later
test fake. Repeated program logic remains a real branch unless the supported
fake-branch analysis establishes a decision. See
[branch-decision rules](fake-branch-triage.md#rules-that-decide-a-test).

## Loops

A path that comes back to a position it already passed (same pc, state,
and call stack) found a loop header. `advance` records it, counts
`retraces.new_loops`, and raises `NewLoop`; the innermost region entered
before the header traces itself again, and the whole walk restarts only
when the top-level path has no step that reaches it.

Reaching the header opens `label: for (;;) { … }`. A pass that comes back
ends with `continue label`; one that reaches a fork target around the loop
ends with `break label`. A later return to the header proves a `break` was
not an exit, so the position and everything after it join `learned.no_break`
and the loop retraces (`retraces.wrong_breaks`), at most
`MAX_STALE_RETRACES` times for a header that predates the ban.

The body retraces until every iteration agrees with the header. A changed
cell not yet a loop variable, an unseen value or type, or a rewritten shared
cell stops the attempt (`Halt::Rewiden`) and retraces; past
`MAX_LOOP_WIDENINGS` the walk stops with `UnjoinableBranches`. A different
header stack length is `prefix-loop-stack-change`.

## `try`, `catch`, and `finally`

At `enter try` the walk opens `try { … } catch (e) { … }`. The body keeps
the frame until it pops it. The catch starts at the frame's handler the
way the VM leaves it: an error record, a pending exception, and the stack
the `try` was entered with. The VM keeps what the throw left above that
stack, so a catch reading below it stops with `catch-stack-unknown`, no
fake-branch evidence.

`resolve_try` steps deeper stack first, then lower `(pc, state)`, catch
first when equal. A closed body may step only without emitting code, or the
step is undone and the catch goes first, so code after the `try` never runs
inside the `catch`. When the catch reaches a passed position, the `try`
retraces with the body waiting (`TryMeet`) and the two join.

A cell the body writes and the catch reads becomes a `try` variable, as do
its values and types, and the `try` retraces until they settle. A throw
ends its path and routes to the catch inside the body. `finally` is
entered by the same handler and walked as another `try`; the emitter
duplicates it, while the walker only knows the try/error/thrown state that
`join_conflict` compares.

## Subroutines

A handler that pushes a VM call frame makes the next `advance` a `call` at
the new entry. With `outline_subroutines`, the first call traces the
subroutine once: `summarize` opens `vm_sub_<pc>_<state>`, steps until the
frame is popped, and requires the return to the caller's address and
state, the same image and `split_image`; `close_subroutine` also rejects a
changed try/exception state. Then it is a function: its parameters are the
caller cells it reads, it writes the cells it leaves, and later calls jump
to the return and emit the call.

It stays inline (`Subroutine::Inline`) when it branches on an undecided test
or enters a `try` (`NotFunction`), stops, changes the image, or fails to
close: the path steps into it at every call, retracing its code. A call also
goes inline when a test or index needs its return concrete (`inline_call`);
`has_subroutine` retraces one a rollback removed.

## Dispatch and handler execution

`advance` checks open loops (a return to a header is a `continue`), then
known headers, then `visited`, and only then runs `step_once`. `step` admits
one cell: the position, a `split_image` byte, the pc and state row, the next
state, a WASM kernel's banks, the operand entry and patch operands, an
`Executable` template and `Lifted` handler, operand bytes, then decodes and
runs the recipe — never JavaScript.

Only then is a `PrefixExecutionRecord` pushed, with the states,
`actual_next_pc`, decoded operands, recipe identity, and `branch` when the
handler jumped. A stop earlier commits nothing: the admission tests assert
empty `records` and `patches` and zero `trace.handlers_traced`.

## Patches and split images

The image-write handler decodes before the ordinary template path. Its
layout is fixed: a 24-bit little-endian address in `PATCH_ADDRESS_BYTES`
(3) bytes and the value in the fourth (`PATCH_OPERAND_WIDTH` = 4). Another
width is `UnsupportedHandler`; a short operand is `TruncatedOperand`; an
address at or past `image_len` is `AddressOutOfBounds`. The decode returns the
previous byte and `actual_next_pc = operand_entry_pc + width`, so the
write happens only after every check; rewriting the same value keeps the
image shared.

When a fork's sides patched a byte differently, the join keeps one image and
records the address in `split_image`; reading it stops with
`PrefixBranchJoin`. If `image_splits` maps it to a fork, the walk adds it to
`apart_bytes` and retraces with the sides apart, each on its own image, until
they meet in state that merges. A patch both histories make clears the byte;
`image_divergence` lists the rest.

## Limit and budget accounting

Four different limits govern a prefix run, and they are not interchangeable.

| Limit | Type | Scope | Checked | Hitting it yields |
| --- | --- | --- | --- | --- |
| `PrefixLimits::max_dispatch_steps` | `usize` | one walk attempt, every path | `step_once` before each instruction | `PrefixStopReason::Budget` |
| `TraceLimits` | shared struct | one symbolic session (one walk attempt) | the session, per handler | `BudgetStopped`, mapped to `PrefixStopReason::Budget` |
| `PrefixLimits::{max_image_bytes, max_table_cells, max_integer}` | preparation/load bounds | the proven program | preparation and `prepare_with_limits` | `PrefixPrepareError::Limit` / `KernelArtifactError::Invalid` |
| internal walk bounds | constants | one walk | various | a stop or a forced retrace limit |

`max_dispatch_steps` counts fetched instructions within one walk attempt: every
path, every fork side's kept steps, and every step a loop, `try` or fork retrace
re-runs. Discarded steps still count against the attempt they ran in, which is
why `wasted_steps` and the budget can disagree with `records.len()`. A
**whole-walk** restart — a new loop on the top-level path, a split, a forced fake
branch, or the cell restart — begins a new attempt with `steps = 0`, so work
abandoned before that point does not consume the next attempt's budget. The probe
inside `junk_entry` is a separate nested walk with its own `ENTRY_PROBE_STEPS`
bound; it does not consume the outer budget.

`TraceLimits` is the symbolic session's own budget: stack length, memory slots,
node counts, step counts. A handler that exceeds it returns `BudgetStopped`, and
`step` maps that to `PrefixStopReason::Budget` **without** committing a record.
The distinction matters when reading diagnostics: a session-level
`step-budget` code with zero committed records means the recipe stopped, while
`prefix-step-budget` with committed records means the driver's own budget ran
out.

The internal constants are memory and sanity guards, never fixes:

| Constant | Value | Bounds |
| --- | --- | --- |
| `MAX_LOOP_HEADERS` | 64 | distinct loop headers one walk may learn; exceeding it stops with `RepeatedPcState` |
| `MAX_LOOP_WIDENINGS` | 64 | attempts to widen one loop or `try` before `UnjoinableBranches` ("the loop variables do not settle") |
| `MAX_APART_STEPS` | 100_000 | steps two sides may stay apart before `UnjoinableBranches` at the first meeting point |
| `MAX_STALE_RETRACES` | 4 | times a stale header may retrace its region before the walk gives up |
| `MAX_FORK_DEPTH` | 48 | nested forks with both sides running; a fork whose other side ended does not count; exceeding it is `UnjoinableBranches` |
| `ENTRY_PROBE_STEPS` | 200 | straight code the junk-entry probe looks at |
| `AUDIT_STEPS` | 2_000 | steps one `audit_fakes` probe may take, across restarts |

Raising `max_dispatch_steps` cannot repair a proof gap, an unmergeable join, or a
missing recipe. For a `Budget` stop, inspect the final diagnostic: a symbolic
`step-budget` and a driver `prefix-step-budget` require different limits.

## Why each rule exists

| Rule | Prevents | Cost |
| --- | --- | --- |
| turn snapshots + `Learned` merge | the result depending on which side stepped first | two snapshots per turn |
| `join_conflict` + apart cells | merging unequal stacks, slots, or `try` state | sides may step apart to `MAX_APART_STEPS` |
| `split_image` / `apart_bytes` | reading a differently patched byte as one value | a duplicated tail while apart |
| `bad_state` + `junk_entry` | dropping real code, or emitting an `if` for a dead side | a retrace and up to 200 probe steps |
| loop/`try` widening + value dispatch | missed iteration cases; a dynamic jump stop | up to 64 retraces; one fork per value |
| outlining + `CellSharing` + self-checks | repeated trace, cell mismatch, unchecked shortcuts | inline fallback, one extra walk |

## Worked walks

### A fork whose arm steps past the join

```text
10/0  conditional jump     taken → 20/0     fall-through → 12/0
12/0  …                    then 14/0
20/0  jump → 30/0
30/0  where both sides meet
```

Turn 1 both step; turn 2 the fall-through reaches 30; turn 3 finds 30, where
the taken side has been; turn 4 cuts it back to 20, retraces to 30, and
joins. Its step to 32 and record are dropped (`fork: past the meeting
point`); the fall-through's stay.

### A loop that needs a variable

A pass reaches 40/2 (a header reading cell 5), 42/2, 50/2, then jumps back
to 40/2: `advance` records the header and raises `NewLoop`. The path is cut
back to the step that reached 40/2, and `run_loop` opens
`vm_loop_0: for (;;)`. The first iteration writes cell 5, so the return to
the header increments `loop_variables`, adds cell 5 to `widened`, and
retraces with it declared before the loop and assigned before `continue`.

## Invariants

* A position is `(pc, state, call stack)`; a return under another call stack is
  a different position.
* A cursor never writes another's image: sharing is an `Arc`, every write goes
  through `Arc::make_mut`.
* A step commits only after its fetch, decode, admission, and execution checks;
  a stop before commit leaves no record and no patch.
* A cut-back restores the cut side exactly: cursor, `split_image`, `visited`
  mark, patch bytes, and the statement, event, record, and patch ranges.
* A join requires `join_conflict` to report none and no apart cell with an
  unread read; "same position" alone is never enough.
* A stop is evidence only when `bad_state` says so; a catch's stack, a short
  side, and a tracer limit are explicitly not evidence.
* A retrace keeps what it learned and counts what it discarded; it never keeps
  a committed record from a rolled-back attempt.

The [prefix guide](../../src/dispatch/prefix/README.md) explains preparation, JS and WASM paths, and
the public report. [Fake-branch triage](fake-branch-triage.md)
explains how to interpret its branch evidence.
