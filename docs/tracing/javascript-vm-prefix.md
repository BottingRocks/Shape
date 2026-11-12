# JavaScript VM bytecode prefix contract

Terminology follows the canonical [architecture and VM variants](../architecture.md).

The complete-dispatch driver (`DispatchPreparation`) requires a source-proven
terminal selector, an immutable image, and a fully lifted handler table. The
inspected JavaScript VM capture has none of those. This page documents the
separate, explicit **prefix** contract for
a proof-supported subset of JavaScript VM execution and exactly where it stops;
it is not a contract for every JavaScript VM.

## What the prefix proves

`KernelArtifact::compile_from_raw(source, report, limits)` is the source-backed
compilation boundary. `KernelArtifact::compile_from_deobfuscated(source, limits)`
accepts already-standardized deobfuscated JavaScript and rebuilds the artifact
projection without replaying clean/rename: it runs standardized FIND/EXTRACT,
extracts the canonical layout, and structurally recovers the single
operation-table declaration. It also re-states the standardizer's canonical
`vm*` helper roles and `vmContext`/`vmMemory` layout fields, which supply the
same template and operand proofs that RENAME records for a raw compile.
Because the raw capture is unavailable, that artifact records `declared_only`,
not `raw_source_observed`; the original raw capture identity is not re-verified.
Regression coverage compares the raw- and deobfuscated-input runtime payloads
for the compiler's own standardized output. Runtime consumers call
`KernelArtifact::prepare_with_limits(limits)` and use the resulting proof
indexes without loading or re-parsing a pipeline report. The report-only path
loads `KernelArtifact::from_json` and checks its integrity and cross-references;
it does not invoke deobfuscation, Oxc analysis, property extraction, or source
proof again. Nothing is matched by spelling, and no selector, address, hash, or
capture constant is baked in:

1. exactly one `Extracted` dispatch-table fact and its bounded `state -> opcode
   -> [next_state, selector]` literal;
2. the nested dispatch read `table[this.state][image[this.PC++]]`. The proof
   accepts the inspected capture's transparent block wrapper and an immutable
   `var alias = Ctor.prototype` installation target, both by Oxc `SymbolId`;
3. the bytecode binding's initializer. Either the runtime-config artifact
   carries bytes decoded under the exact alphabet established by the
   [runtime decoder proof](../deob/runtime-decoder.md) for that declaration, or the
   initializer is a bounded numeric-literal array;
4. the JavaScript VM run-loop topology
   `var v, O; for (;;) { if (completion !== notComplete) { ...; return ...; }
   v = ctx.dispatch(); if (ctx.<exceptionStack>.length === 0) ops[v](ctx);
   else helper(ops[v], ctx); }`, joined to the proven operation table by its
   code-generation-stable declaration ordinal. The first `var` declaration must
   be exactly two distinct, uninitialized selector/return local bindings used by
   the body; the completion guard must use a not-complete sentinel that is
   declared once and never mutated, the completion register must be initialized
   to exactly that sentinel binding, and the guarded exception stack must be
   initialized to a fresh empty stack by the dispatch method's proven
   constructor (directly `this.G = []` or through a proven empty-array helper
   such as `vmCreateStack`). The run loop and every dispatch/handler function
   must not be async or generator functions, and the run loop's receiver must
   originate from the proven constructor, not merely share the dispatch
   property spelling;
5. exactly one bounded bytecode-write handler whose body is exactly
   `var a = image[pc] | (image[pc+1] | image[pc+2] << 8) << 8;
   var b = image[pc+3]; pc += 4; image[a] = b;`
   against the proven image and PC bindings, with distinct address/value/
   context bindings. The handler is recognized by its body and `SymbolId`s, not
   by any selector constant. Every other reference to the image binding must be
   a computed read (`image[...]`, which yields a byte value rather than an
   alias); identifier rebinding/update, `delete`, member/property writes, and
   object/array/conditional/return/call escapes are rejected.

Any other write to the image, a write in a non-handler function, a missing or
duplicated handler, a wrong shift/index shape, an extra effect in the handler,
a reassigned prototype alias, or a shadowing local fails closed at preparation.

The CLI has the same two input modes. Raw mode preserves the legacy command
and may generate or load a sidecar kernel artifact. `shape deob --report` writes
both deobfuscated JavaScript and the artifact; `shape report` writes only the
artifact:

```sh
shape report capture.raw.js --output capture.kernel.json
shape report capture.deob.js --input-kind deobfuscated \
  --output capture.kernel.json
shape trace thread capture.raw.js --pc 115869 --state 7 \
  --initialslots 0-430 --report capture.kernel.json --out main.js
```

`shape report` writes the artifact JSON to stdout when `--output` is omitted.
It never prints deobfuscated JavaScript and refuses input/output collisions.

Report-only mode is portable and does not need the original raw file:

```sh
shape trace thread --report capture.kernel.json --pc 115869 --state 7 \
  --initialslots 0-430 --out main.js
```

`--report -` is also accepted in report-only mode for bounded artifact JSON
stdin. The compact `shape-kernel-artifact/v1` is required; legacy
`PipelineReport` JSON from `PipelineReport::to_json`/`from_json` and the
properties-only output of `shape kernel show` are rejected. Loading an artifact
skips deobfuscation and reports that phase separately from preparation. The
artifact preserves its `source_basis`: `raw_source_observed` for
`compile_from_raw`, or `declared_only` for a deobfuscated-input compile. The
declared raw capture identity is not re-verified when raw bytes are unavailable;
the artifact integrity digest and structural runtime facts are checked again.

The free function `prepare_prefix_from_saved_report` is narrower than artifact
loading: it prepares only the JavaScript dispatch path, because it goes straight
to `runtime_config::from_saved_report` and `prepare_prefix`. A WASM-dispatched
kernel has no JavaScript dispatch table, so prepare it from raw source or from a
`KernelArtifact` compiled from raw source.

## Execution contract

`PrefixPreparation::thread(entry, descriptor, inputs)` binds the validated
`DispatchStart` (an explicit caller-supplied `pc`/`state`, never a sentinel) plus
a caller-supplied `ThreadDescriptor`/`ThreadInputs`. The entry and the
descriptor/capture inputs are explicit caller parameters, not source-derived
facts; the source proof covers the dispatch read, run loop, image, and patch
handler only. Each `invoke`:

* clones the preparation's immutable initial image, so mutations never leak
  between invocations;
* initializes the descriptor's memory slots (the real boot's `[2,0,3,1]`) in
  the shared canonical thread order;
* dispatches through the proven table and commits one
  `PrefixExecutionRecord` per successful step;
* applies the recognized image write transactionally (`image[address] = value`
  with a bounds check) and records the old/new byte, or decodes operands and
  runs a supported ordinary handler through the shared `TraceSession` recipes;
* forks at a conditional jump whose condition is a program value
  (`if (top) { pc = target; state = s }`): both successors are traced as the
  arms of an AST `if`, in turns of one dispatch step each (a nested fork,
  loop or `try` a side enters is one step). Where both reach the
  same PC, state and call stack the arms join: the stack length, memory slots
  must agree, and a cell holding different program values becomes a
  variable declared before the `if` and assigned at the end of each arm.
  Sides that patched the bytecode image differently still join: the merged
  path keeps one image and the set of bytes its histories may disagree on,
  and reading one of those (an opcode, a decoded operand, a patch's own
  operands) stops the walk; a patch every history makes clears the byte,
  and the report lists the bytes still divergent at the end. A subroutine
  traced as a function is reused only over the image it was traced on, with
  no byte divergent; otherwise the call is traced inline. A condition with a
  known value is followed without a fork;
* joins sides whose stacks differ only in a value one arm left above the part
  both share (`v ? !v : x` with `v` never popped on one arm) with the stacks
  lined up from the top, the shorter padded with holes that read `undefined`;
  a step that reaches the holes, or counts the stack from the bottom, undoes
  that join and the walk goes back to what it knew then. When the longer side
  pushed nothing and the shorter one popped what the other keeps, the code
  after the join reading under the shorter side's own stack makes that side
  fake (`stack-deficit`); a read of a value it still has (`v || x` with `x`
  pushed first) only undoes the join;
* keeps the sides of a fork apart where they meet in VM state that cannot
  merge otherwise (stack lengths it cannot line up, `try` frames or pending
  exceptions): both step on past that position, to a later meeting point
  where the state agrees or each to its own end, so the code after it is
  traced in each arm. A side that reaches the meeting point short of a value
  usually reads below the stack later, which makes the jump a fake branch:
  while that side steps, a stack underflow is evidence for that fork only, so
  a fork nested in its tail does not take it as its own. Once the sides
  stepped apart they run the same code a step apart, so the walk no longer
  looks for a crossing there, only for a position both stand on in state
  that merges.
  Sides that stay apart for more than 100,000 steps (every fork after them
  is traced twice) stop the walk with `UnjoinableBranches` where they first
  met;
* keeps paths that set up different jumps apart. A cell that held a
  concrete value on at least one side and a different value on the other
  (a PC, a state, an index one side set up) becomes a join variable too,
  but one the join remembers. When a VM value read from
  it must be concrete (the jump through slots that ends a `finally`, a loop
  exit or a `try`), the fork is traced again with its sides kept apart while
  that cell differs: each follows its own jump, and they join where they meet
  after both have read it. A merged value only programs read stays a
  variable;
* tracks value sets: a loop variable whose entry value and every value an
  iteration gives it are concrete, a `try` variable whose entry value and
  every value the body assigns are concrete, and a join of two such values
  hold one of a known set of values. A loop that carries its next jump in
  a cell (a state machine: each state sets the PC and state the next one
  jumps through) is such a loop: when a jump needs the variable's value, the
  walk branches on each value in the set, `if (v === 9) { … } else …`, each
  side running the jump knowing its value, and the last value needs no
  test. A set that gains a value after a jump took one from it means the
  attempt missed a case: the loop or `try` is traced again until the sets
  settle. A set of more than 64 values is no jump table, and a jump through
  it stops the walk naming the variable and how many values it holds;
* keeps every emitted name in scope. A path that leaves a block early (a
  lone `if` arm, a loop `break`, a catch carrying on after its `try`, a
  `try` body after its frame is popped) carries values bound inside it:
  each known value becomes itself again, and any other becomes a variable
  declared before the block and assigned where the path leaves it;
* traces `try`. At `enter try` the walk opens `try { … } catch (e) { … }`:
  the body path keeps the frame and its statements stay in the `try` block
  until it pops that frame; the catch path starts at the frame's handler the
  way `vmHandleException` leaves the VM (frame popped, an error record with a
  pending exception, the exception `e` on the thrown-value stack). The VM
  keeps the stack the throw left; the catch path starts with the stack the
  `try` was entered with, so a catch that reads below it stops the walk with
  `catch-stack-unknown`, which is no evidence for a fake branch. The cells
  the catch may see changed are every cell written while the `try` was
  active, on any path, one that ended since too (a throw after the write
  reaches the catch with it); a catch that stops on a jump through such a
  cell traces the `try` again with it as a `try` variable. Past its frame
  the body steps on without emitting code: when the catch path then reaches
  a position the body passed after closing, that is where they meet, and the
  `try` is traced again with the body waiting there. The two
  merge where they meet in the same VM state, like an `if`. After its frame is
  popped, the body only takes steps that emit no code (other than pure
  bindings) until the catch path catches up, so code after a `try` never runs
  inside its `catch`. When the body never falls through (every path in it
  returns or throws), the catch carries on after the `try` once it reaches
  code an enclosing fork meets. A cell the body changes (writes with another
  value than it held at the `try`) and the catch reads becomes a variable
  declared before the `try` and assigned on every write in the body; the
  body itself keeps the values it knows, and the catch reads the variable.
  The `try` is traced again until that set settles. A nested `try` keeps its
  own changed cells and variables and hands its changes to the enclosing one
  when it ends. A `finally` becomes
  `try { body } catch (e) { finally; throw e; } finally; …`: its handler runs
  once for the exception path, rethrowing, and once on the normal path. A
  `throw` ends its path, as `return` does; inside a `try` body JavaScript
  routes it to the `catch`;
* finds where the two sides of a fork meet as it traces them, even when code
  is laid out out of line (a `then` block placed after the function body that
  jumps back to just after the jump): a side that stands where the other has
  been since the fork, nested forks included, found the meeting point. Each
  side keeps a history of its steps: where it stood, and a snapshot of its
  trace path, cursor and what it had traced. The other side is cut back to
  the step during which it passed the meeting point, between two of its
  steps or inside one (a nested region, which it traces again knowing the
  meeting point), keeping what both traced. In a turn each side
  sees the other as it stood when the turn began, and a side that halts (a
  new loop, a call to trace inline, a bad state) undoes the other side's step
  of that turn, so which side steps first decides nothing; the corpus
  self-check walks every function both ways. A loop, a split or a call to
  trace inline at a position only one side passed takes both sides back to
  the start of the turn in which that side first passed it, with what the
  fork knew then (its meeting point, the positions where its sides could not
  merge), as if the fork were traced again from that turn; meeting points
  found since stay known, since sides merge there only if both reach it and
  can merge. The function's own path keeps a history too: such a halt on it
  takes it back to the step that reached the position, not to the start of
  the walk. A VM subroutine traced as a function stays when a cut-back
  drops the steps that traced it, since the other side may call it; its
  records stay with it, reported once for each function the trace still has,
  whichever attempt traced it. At the
  end of the walk a function nothing calls is dropped. A side standing where
  an enclosing fork's other side has been waits there; at an enclosing
  loop's header each side continues the loop. There is no meeting probe and
  no bound on the steps the sides take before they meet. A side
  whose other side ended runs on in its own arm, so a bad state it reaches
  still makes the jump a fake branch, until it reaches a position an
  enclosing fork meets at (that fork's meeting point, or where the fork's
  other side has been): there the `if` closes and the side carries on after
  it, where the enclosing fork joins it. A fork whose sides all ended passes
  the positions they reached up, for the enclosing fork to meet.
  Meeting points, and the cells found to change in each loop and `try` body,
  are remembered by position, so re-traces start from them;
* remembers, per path, how each forked test came out. A later jump on the same
  value (the same traced values combined the same way, after following `let`
  bindings, so two separately evaluated calls never match) is decided from
  that: a `FakeByContext` branch "an earlier branch on this path tested the
  same value". Facts both sides agree on survive a join;
* knows the type of a few kernel results: `vmDecodeString(…)` returns a
  string (built with `+=` from `""`, or taken from its private cache of
  those) or throws, so `typeof (x + vmDecodeString(…))` is `"string"`. A
  variable a join made holds what one of its arms assigned, and one carried
  out of a block what was assigned where the path left it: a test on it is
  decided from those values (their common value, or the union of their
  types), and a test that reads an outlined call through it sends the call
  inline as a direct read does. A value that is one of a few constants (a
  join of constants, `typeof` of a value of a few types, an operator applied
  to such) goes through each operator with each of them, so `(c ? 75 : 97)
  % 1 !== 0` and `typeof (a - b) === "string"` are decided false;
* classifies every conditional jump on a program value, in its record's
  `PrefixBranch` (target, test node and kind): `FakeByTest` when what is known
  about the test's operands decides it, so only one side is traced; `Real` when
  both sides are valid code and become the arms of an `if`; `FakeByContext`
  when one side reaches a state real code never reaches (an opcode or state
  with no dispatch cell, a PC or operand outside the image, a stack underflow,
  a return with no call frame, popping an empty exception stack, or a VM
  jump straight to a program value: an argument, a property, arithmetic,
  never a shared cell's read, which may be a VM constant another function
  wrote, unless every function holding the cell is known to write only
  JavaScript values or none of the addresses it holds matches). When that program value was carried out of an `if` that has
  closed since, it is that `if`'s side that is fake: the walk starts over
  with it known. For a
  fake branch by context the walk rolls back to the jump and follows only the
  other side, emitting no `if`; if both sides reach such a state the walk
  stops. A handler the tracer cannot run is a limit of the tracer, not
  evidence, and stops the walk. A handler that reads or writes a memory slot
  the thread has no cell for throws, as `vmMemory` does
  (`this.slots[slot].value` of a missing cell): the path emits
  `throw new TypeError(…)` and ends there, or goes to the enclosing
  `catch`. Fake branches are also `BranchDecided` events with their
  `BranchEvidence`;
* ends a path at a VM function return (`vmPendingResult = value`), which emits
  `return value`. When one arm returns, the other runs on alone; when every
  path has ended, by a return or a loop `continue`, the walk stops with
  `PrefixStopReason::Completed`;
* traces loops. A path that comes back to a position it already passed (same
  PC, state and VM call stack) found a loop header: the innermost fork, loop or
  `try` entered before the header was first passed traces itself again (the
  whole walk starts over only for a header on the top-level path), and
  reaching that header opens `label: for (;;) { … }`. Every path that comes back
  to the header ends with `continue label`. A path of the body that reaches a
  position an enclosing fork meets at (its meeting point, or where its other
  side has been) left the loop: it ends with `break label` and carries on
  after the loop. A path that later comes back to the header of a loop it
  left was still inside it: those positions are no exits, and the loop is
  traced again without breaking there. A loop with no `break` of its own
  passed on the way back bans those of the loops around it the path passed:
  only loops whose bodies passed this header, so a `break` out of a loop
  the header is not inside stays an exit. One that comes back with no such
  `break` left to ban passed the header in a trace made before a ban that an
  inner region's retrace did not undo (a fork's other side waiting where it
  broke out): the region around both traces itself again, a few times at
  most. A loop exit nothing outside meets
  stays inside the body and ends with `return`;
* shares memory cells the way `vmCaptureThread` does: a closure gets the
  creating function's `memory.slots[slot]` objects for the slots its
  descriptor captures, so both read and write the same cells for as long as
  they live. Such a slot is a cell variable `$CELL_n`: a slot the thread
  captured starts as the cell its caller passed (`$CELL_n = capturedN`), and
  a slot a closure created here captures starts as `{ value: … }` (found when
  the walk reaches that `vmCreateThread`: the walk goes on without the cell
  to find every such closure, then starts over once with all of them as
  cells, keeping of what it learned only the calls to trace inline). A read emits `$CELL_n["value"]` where it happens, a write
  `$CELL_n["value"] = v`, a slot initialization a new `{ value: undefined }`
  (closures made before keep the old cell), and `vmCreateThread(d, pc,
  state, { n: $CELL_n, … })` hands the closure its cells. The code always
  reads the cell; what the path last wrote there only decides tests and
  jumps, and only until a statement that may run other code, once another
  function holds the cell (one the caller passed, or one a closure created
  on this path captured). A cell no closure holds yet keeps its value across
  calls. `shape trace corpus` gathers what every function holding a cell
  writes there (`cell_writes`: VM addresses, and whether JavaScript values
  too) and walks again with it, until the contents settle and every holder
  completed. A read with no value of its own known then holds one of those
  addresses: a VM jump through it dispatches on them with
  `if ($CELL_n["value"] === address)`, a test on the cell rather than on the
  read's variable, since the instruction runs again on each side and reads
  the cell anew (only when no code that may run other code came before the
  read in that instruction). A jump through a cell that may also hold a
  JavaScript value that matches none of the addresses is a jump to a program
  value;
* traces each VM subroutine (code entered by a VM call and left by the VM
  return of that call) once, as `function vm_sub_<pc>_<state>(…)`, and every
  call to it as a call of that function. The function's parameters are the
  caller's stack cells and memory slots it reads; what it leaves in the
  caller's cells it returns, except values that are the same at every call,
  which stay concrete. A subroutine that branches, enters a `try`, ends the
  VM function, patches the image or reads a caller variable stays inline at
  every call. Each call also keeps what it returns as the function's return
  values with the call's arguments for the parameters (built for tests
  only, never emitted), and a test on the call's result is decided on that,
  as it would be traced inline, with the call outlined. A call is traced
  inline instead when a jump or index needs what it returned concrete, when
  a test reads a call whose return values could not be built that way, or
  when a loop variable that a jump dispatched on starts with or carries
  what it returned (the variable then has no value set). The corpus
  self-check walks every function without functions too and finds every
  jump forking the same;
* in a loop, a stack or memory cell that
  changes between iterations becomes a loop variable, declared before the loop
  with its entry value and assigned before each `continue`; the first
  `continue` that shows a new one stops the attempt, and the loop is traced
  again with it until no iteration adds one. Statements leave the VM stack as
  they found it, so a path that comes back to the header with another stack
  length is no iteration real code runs: the fork that led it there is a fake
  branch (`prefix-loop-stack-change`), and without one the walk stops with
  `UnjoinableBranches`, as it does for an iteration that changes the memory
  slots or the call stack. A loop variable widened at another entry of the
  header, one with a longer stack, is no cell of an entry without it;
* emits the VM's own error for a jump to `undefined`: its next
  `context.dispatch()` reads `vmDispatchTable[state][vmBytecode[NaN]]` as
  `undefined` and throws `TypeError` outside `vmInvokeOperation`, so only
  with no VM `try` open is that a plain `throw`. Past a catch it applies only
  to a value taken from a `try` variable's complete value set, not to the
  catch's entry state;
* stops before commit on truncation, address-out-of-bounds, an unsupported
  reached handler, a repeated `(pc, state)`, a recipe that can throw, a
  partial recipe, or the requested step budget, and stops with
  `UnjoinableBranches` when two arms stay apart too long, or when a loop or
  `try` meets itself with VM state that cannot merge.

A completed walk leaves the `TraceReport` `Completed`, and
`emit_javascript` renders it as a function. Every other prefix, including a
zero-step or budget-limited one, leaves it in a non-`Completed` state
(`PrefixStopped` or the recipe's own `BudgetStopped`), so `emit_javascript`
refuses to render the prefix as a whole program. A reached handler whose recipe contains a `throw` is rejected before
execution, so no context/event state commits without a `PrefixExecutionRecord`.

## Tested JavaScript VM fixture run

Use the [trace CLI](trace-cli.md) with the tracked fixture:

```text
shape trace thread tests/fixtures/kernels/javascript/kernel.raw.js \
  --pc 0 --state 0 --initialslots 2,0,3,1 --steps 2 --partial
```

The checked-in fixture currently reaches one committed image patch and then
stops on an unsupported handler. The underlying execution record contains:

```text
prefix ... image_bytes=157803 states=10 patch_selector=222 ... completion_register=vmPendingResult
thread pc=0 state=0 initial_slots=[2, 0, 3, 1] max_steps=2
step=0 pc=0 operand_pc=1 state=0->3 opcode=159 selector=222 effect=image_patch(address=59 old=232 new=232) next_pc=5
stop=UnsupportedHandler { selector: 194, ... }
steps_committed=1 recipe_handlers=0 patches=1 final_image_bytes=157803
```

`steps_committed` counts committed dispatch steps and `recipe_handlers` counts
ordinary handler recipes executed; the first real step is a bytecode patch, not
a recipe handler, so the two must not be conflated. The explicit pc7000 entry
writes a non-neutral image byte (`image[61393]` 5 -> 122) and is exercised twice
from one prepared invocation to show isolation.

## Explicit boundary

Budget-limited and unsupported prefixes are not complete programs and cannot
be emitted as whole programs. A walk that reaches `PrefixStopReason::Completed`
can emit JavaScript, as described above; it does not establish full-capture
coverage. The driver does not model:

* exceptions thrown by calls or property reads inside a `try` body are not
  traced individually: the catch path covers whatever the body throws;
* any handler outside the proven recipe subset;
* arbitrary runtime facts: only the narrow proofs above are execution
  authority, and `PrefixPrepareError` is returned otherwise.

The smallest realistic fixture that preserves this topology is
`tests/fixtures/javascript-vm-prefix.raw.js` (the write handler sits at
selector 0) and
`tests/fixtures/javascript-vm-prefix-write-selector.raw.js` (the write handler sits at
selector 2, proving body-based recognition). `tests/fixtures/javascript-vm-branches.raw.js`
adds conditional jumps, returns, an increment, fake-branch tests, function
literals and `new`; `tests/vm_branches.rs` covers if/else merges, `if` without
`else`, nested `if`s, arms that both return, known conditions, unjoinable arms,
loops, loop variables, nested loops, a loop that grows the stack, fake branches
by test and by context, branch kinds, try/catch, try/finally, try variables,
throws, tests repeated on one path, and out-of-line `then` blocks.
Use `TraceArena::describe` to inspect a jump's kind and test. `tests/prefix.rs` covers
patch-then-consume, per-invocation image isolation, truncation/address/budget
no-commit, emitted-JavaScript refusal on a budget prefix, an honest unsupported
stop, a reached-throw stop, and the run-loop/declaration/reference near-miss
rejections. The real-capture oracle test is ignored by default and requires an
explicit `SHAPE_PREFIX_SEED` path when run.
