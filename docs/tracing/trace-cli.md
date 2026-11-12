# Trace CLI

`shape trace` includes `thread` for one function, `dispatch` for one explicit invocation to a completion selector,
`corpus` for reachable-function comparisons, and `program` for linked program recovery. `cleanup-thread` cleans an emitted
function. Add `--json` to any of these commands to get a structured result on stdout using the same flags.

Build as described in [Build once](../../README.md#build-once). Terminology follows the
[glossary](../reference/source-modules.md#glossary).

For cleanup of an already emitted function without tracing or linking, use
`shape trace cleanup-thread thread.js --out thread.cleaned.js`. This standalone
entry point is described in the [individual thread cleanup guide](../transforms/thread-cleanup.md).

## Command shapes

```text
shape trace cleanup-thread <thread.js> [--out FILE]

shape trace thread <capture.raw.js> --pc N --state N \
  (--descriptor N | --initialslots SLOTS) [--report PATH] [--steps N] \
  [--out PATH] [--log PATH] [--self-check] [--no-outline] [--reverse-sides] [--partial] \
  [--fake-branches FILE] [--not-fake-branches FILE]

shape trace thread --report ARTIFACT --pc N --state N \
  (--descriptor N | --initialslots SLOTS) [--steps N] [--out PATH] \
  [--log PATH] [--self-check] [--no-outline] [--reverse-sides] [--partial] \
  [--fake-branches FILE] [--not-fake-branches FILE]

shape trace corpus <capture.raw.js>... [--report-dir DIR] [--baseline FILE] \
  [--update-baseline] [--fast] [--cleanup] [--seed-by-seed] [--branches FILE] \
  [--audit-fakes] [--steps N] [--limit N]

shape trace program <capture.raw.js> [--out FILE] [--report-dir DIR] [--steps N] \
  [--dump-stages DIR] [--thread-cleanup] [--no-cleanup] [--check-reuse] \
  [--fake-branches FILE] [--not-fake-branches FILE]
```

Input policies, output shapes, and exit behavior are described below;
[`cli/trace/README.md`](../../cli/trace/README.md) maps them to their
implementation. Use `shape trace <mode> --help` for the full flag list. `thread` needs
`--pc` and `--state` and one of `--descriptor` or `--initialslots`; `program`
takes exactly one capture and writes the linked program to `--out` (or stdout
when it is omitted); `corpus` needs at least one capture
and never writes a program. `thread` has a report-only mode and can read an artifact from stdin with `--report -`.

The shortest choice rule is about the question being asked. Use `ops show`
before tracing if the handler itself is unknown; use `trace thread` when PC,
state, and a descriptor or initial slots are known; use `trace corpus` to
compare reachability and branch behavior across a capture set; use
`trace program` when the desired artifact is one linked JavaScript program.
Use `--json` on the same command for automation; [JSON output](trace-json.md) defines the result and input snapshots.

## Input modes

`corpus` accepts only raw captures. `program` always reads the capture itself.
`thread` can walk a raw capture or an existing source-free kernel artifact
(report-only mode). `dispatch` reads raw source and an optional explicit input snapshot. `corpus` and `program` share `--report-dir DIR`: the artifact is
compiled from the capture once, published, and loaded on later runs. A legacy
`<capture-file-name>.report.json` in that directory is rejected, not converted;
regenerate artifacts with `shape report SOURCE --output ARTIFACT` (or
`shape deob SOURCE --report ARTIFACT` when the standardized JavaScript is also
wanted). Standardized deobfuscated input can be compiled with
`shape report DEOB --input-kind deobfuscated --output ARTIFACT`; that artifact
records a `declared_only` source basis and emits no JavaScript.

## Shared cells

`vmCaptureThread` gives a closure the creating function's `memory.slots[slot]`
objects for the slots its descriptor captures. A cell is shared by its owner
and every closure that captures it, and can hold VM addresses written as jump
setups and/or JavaScript values. Each walk reports what it wrote to each cell
and which contents it used; `trace corpus` gathers the union over every holder
of a cell and walks again until the contents settle. Which consumer does that
is policy, not a side effect of sharing the engine:

| Mode | Shared-cell contents |
|---|---|
| `trace corpus` | converges over every reachable function; reported walks use them |
| `trace corpus --limit N` | none; functions also walk one at a time |
| `trace thread` | none; the walk gets only the capture mapping |
| `trace program` | a walk that stops on a shared cell is walked again with its ancestors' contents; falls back to converged contents only if those are not what every holder puts there or functions still stop, and keeps that retry only if fewer stop and the program still links |

A full corpus run and a limited one are different analyses. A thread uses only
its supplied captures. Program tracing first discovers return shapes and walks
reachable functions once, giving a walk that stops on a shared cell what its
ancestors put there; when stops remain or those contents do not hold, it can
retry with converged shared-cell facts (`--check-reuse` also walks every
function afresh and fails if a kept walk differs). The retry must reduce
stops and still form a valid linked program. Remaining
stops become throwing functions.

Within corpus mode, `--fast`, `--cleanup`, `--seed-by-seed`, and `--branches` do
not change convergence policy; `--limit` does. See
[trace orchestration](trace-orchestration.md#shared-cell-analysis) for ownership
and the conditions under which facts are reused.

## Deciding branches by hand

`thread` and `program` take the branch overrides `shape/`'s trace took. A
branch is named by its conditional jump's position, `"pc/state"`: the `pc=`
and `state=` of the jump's line in `--log`, or the `at` of a branch record from
`trace corpus --branches`.

- `--not-fake-branches FILE`, a JSON array such as `["114071/9"]`: these
  branches are real. Neither their test nor what one of their sides reaches
  decides them. A side that reaches a state real code never reaches (an
  underflow, a jump to a program value, a function it creates that fails at
  entry) stops the walk there instead of making the branch fake, and no
  enclosing branch takes that evidence either: it would only move the
  verdict outward and drop more code. A walk that completes with the branch
  real shows the side never failed; one that stops shows where it does.
- `--fake-branches FILE`, a JSON object such as `{"114071/9": true}`: these
  branches are fake and always take the side given, `true` the jump and
  `false` the fall-through. The record reads fake by context, `forced by
  --fake-branches`.

A position may not be in both files. Without either option the walk is
unchanged. In `program` the overrides apply to every function's walk, a
position naming the same jump wherever it runs.

## Inspecting a stopped thread

`trace thread ... --partial --out /tmp/stopped-thread.js` writes the traced
prefix even when the walk stops. The output starts with `// INCOMPLETE:` and
the stop reason; the summary still reports the stop and the command still
returns a stopped status. This is diagnostic output, useful for locating the
branch and values before a failure, not a completed function reconstruction.
Arena validation still applies and can refuse emission. Program and corpus
mode never emit incomplete traces this way.

Pair `--partial` with `--log FILE` to follow the handler coordinates. Use
`--reverse-sides` to check whether fork scheduling changes the result and
`--no-outline` to compare inline subroutine tracing. A difference locates a
modeling or reuse issue; it does not prove the discarded branch fake. See
[fake-branch triage](fake-branch-triage.md#why-the-extra-state-exists).

## `program` output

`--out` receives one JavaScript file: a leading comment, the entry function as
`function $THREAD_<state>_<pc>()`, and a call of it. Each function a traced
function creates is written as an empty `function $THREAD_<state>_<pc>() {}`
where it is created, and the created function's code is placed inside that
marker, so nesting follows creation, not call order:

```js
// Traced from a JavaScript VM capture by `shape trace program`.

function $THREAD_<entry state>_<entry pc>() {
	…
	function $THREAD_<state>_<pc>() {
		…
	}
	…
}

$THREAD_<entry state>_<entry pc>();
```

The angled parts are each thread's state and PC numbers, and `…` elides a
body. Every function is emitted in program mode: it takes no parameters, reads
its inputs as `arguments[i]`, `arguments`, `this` and its own name, keeps
memory slot `K` in `$KEY_K` (a shared cell's box unwrapped, so a slot a
function shares with its creator is the creator's variable), and nothing is
renamed during placement. A function that did not complete is written as one that
throws and names its stop; a function created at several sites is written at
each. A function that creates itself makes a fresh instance of the same code
with the same shared cells, so its creation refers to the function itself
instead of writing a second body. Emitted code, markers and free/declared
names are found on each function's walk thread; the tree is then written in
one pass. Placement and name checks are in
[`program_nest.rs`](../../src/trace/program_nest.rs).

Cleanup runs in two places. Before linking, each function goes through its
emission stages (string fold, property writes, array constructions, receiver
`this`), the optional `--thread-cleanup` driver, and then its own cleanup
stages (`clean_function`) on its walk thread. After linking, the whole program
is named and tidied once (`finish_program`). Both orders live in
[`pipeline/stages.rs`](../../src/transforms/emitted_js/pipeline/stages.rs);
[`program.rs`](../../cli/trace/commands/program.rs) only traces, links and
calls them. The [pipeline table](../transforms/emitted-program-cleanup.md#pipeline)
maps every dump to its pass and file.

`--dump-stages DIR` saves `DIR/00-linked.js` and the named whole-program
stages (`12-short-generated-names.js`, `13c-…`, `13d-positional-names.js`,
`13e-dotted-properties.js`). For each traced function,
`DIR/functions/<function-identity>/` holds `00-emitted.js` through
`04-receiver-this.js`, `05-thread-cleaned.js` with `--thread-cleanup`,
`06-thread-pipeline.js` (the cleaned function, taken back out of its unit),
and `pipeline/01-…13b-…` for each per-function stage. Without
`--thread-cleanup`, `13e-dotted-properties.js` matches the emitted program
(`--out` or stdout) byte for byte; with it, the output is that program
reprinted.

With `--no-cleanup`, `--out` (or stdout) receives the linked program
immediately after the per-function string, property-write, array, and receiver
stages, with arrays laid out. It skips both the per-function and the
whole-program cleanup stages and is useful when checking tracing or linking
without paying for those reparses. If stages are requested at the same time,
`00-linked.js` is the only whole-program snapshot, and each function has its
`00-…04-` (and `05-`) snapshots but no `06-` or `pipeline/`.

The first per-function stages remove direct `x = void 0` assignments that
immediately follow an uninitialized `let x` in the same function body, and
inline a generated `$SSA` binding whose only use is the next direct return,
throw, branch test, call, or lexical `$KEY` assignment. They leave assignments
after calls or branches in place and never move an initializer across another
statement. The literal SSA stage replaces reads of proven single-assignment
primitive literals, then removes their generated declarations.
`06-readable-property-writes.js` lowers remaining exact data descriptors,
including generated helper parameters and dynamic keys. It assumes ordinary
extensible receivers and primitive keys; see
[property-write cleanup](../transforms/emitted-property-writes.md#linked-program-output).
The alias SSA stage removes generated copies of resolved local bindings when
their values stay stable through every use, including loop-head snapshots
whose uses precede the next loop-state write. It retains captured values,
shadowed names, shorthand object keys, and copies across intervening writes. A
second adjacent pass consumes handoffs exposed by those removals.
The carry stages inline immediate single-use joins and coalesce branch-local
carry copies into their destination binding, including relays through loop
branches that preserve the previous value. A following pass removes local
`$CARRY = $CARRY` stores exposed by that coalescing. They also remove unread
carry bindings and their dead stores, retaining any right-hand-side effects;
the dead-carry sweep runs again after the helper and tuple stages. An exact
two-arm branch, including a nested assignment-only diamond, becomes one
conditional immutable value; side effects in the tests and selected value
retain their original order.
`11-vm-sub-parameters.js` gives `vm_sub_*` parameters stable positional names
(`$PARAMA`, `$PARAMB`, …) in signature order. It does not infer a semantic role
from the generated body or expose the original VM stack and memory slot
numbers. `11b-shared-vm-helpers.js` shares equivalent `vm_sub_*` definitions
within one top-level function, including different generated names and
identical unresolved global references. It only moves helpers whose bindings
are used by direct calls. Capturing helpers, observed function values, dynamic
scope, and definitions in separate top-level roots stay where they are.
`11b1-simplified-vm-subs.js` compacts fresh arrays and single-use temporaries
inside linear subroutines. `11b2-inline-vm-wrappers.js` inlines small
single-return wrappers at calls with simple lexical arguments. It also accepts
a computed argument for a one-parameter wrapper that reads it exactly once
before any observable operation. `11b3-scalar-tuples.js` replaces fresh literal
return arrays used only by immediate indexed reads with direct scalar
bindings. `11b4-ssa-after-tuples.js` removes the simple aliases those bindings
expose. `11b5-unpacked-tuples.js` collapses an immutable helper call followed
by immediate ordered reads of its fresh array result into one destructuring
binding. For example, `let tuple = helper(); let a = tuple[0]; let b =
tuple[1];` becomes `let [a, b] = helper();`. This assumes the standard Array
iterator. The helper still creates its array; this stage removes the temporary
binding and indexed reads from the emitted source, not the allocation.
The `11c-dead-ssa.js` stage removes unread generated SSA declarations. It
retains the initializer as an expression statement when evaluating it may
still call code, read a property, or throw. A generated local read immediately
after a direct write to the same resolved binding needs no retained expression.
`11c3-dead-key-stores.js` removes direct stores to local `$KEY` bindings that
have no resolved reads, while retaining an effectful right-hand side.
`11c4-dead-after-keys.js` repeats that removal with dead SSA, bare generated
reads after a direct declaration, and dead carries until those passes expose
no more work. `11c5-argument-reads.js` removes discarded numeric
`arguments[index]` reads when the function uses its unescaped arguments object
only through numeric indexing. That rewrite assumes ordinary built-ins without
numeric prototype getters. Loop recovery, helper expansion, forwarding and the
final tidy follow (`11d2-…13b-`). The whole-program stages after linking
rebuild classes and arrow callbacks, give threads parameters, name bindings,
and run late tidies whose assumptions (built-ins and host globals do not
change; property reads in the fingerprint probes run no code; `.length` is a
number) are listed under
[late whole-program passes](../transforms/emitted-program-cleanup.md#late-whole-program-passes).

## Scheduling and determinism

`dispatch` and `thread` each run one invocation and schedule no parallel work.
`corpus` works at two levels: by default every capture walks at once (one thread
per capture) and one capture's functions walk one at a time; `--seed-by-seed` walks
the captures in order, one at a time, each capture's functions on every core, which
is the shape for timing that capture. `--limit` always walks one function at a
time, so the cap cuts one queue order rather than whichever walk finishes
first.
`program` walks one capture's functions on every core and starts each function the
moment a walk creates it, not when its creator finishes.

A function's walk depends only on its own inputs (descriptor slots, the cell
contents it is given, the cells an earlier walk of it found), never on when it
runs; its code is emitted and checked on its own walk's thread. Walk worker
threads reserve a 512 MiB stack because walks recurse; the reserved memory is
not committed until used.

Results are collected in a fixed order, not completion order: per function by
`descriptor`/`pc`/`state` (a corpus baseline uses the keys `entry@pc/state`
and `dN@pc/state`), and per capture by file name in a baseline. Only functions
reachable through the created-function lists of finished walks stand, so an
attempt that announced a function and was later abandoned does not add one.
Fingerprints, baselines, reports and the generated program text are therefore
independent of the worker count and of which walk finishes first. The timings
are not: corpus stores a per-function `millis` in its fingerprints, and
`thread`, `corpus` and `program` print a `time:` line on stderr. The comparison
never reads `millis`, so keep timings out of semantic comparisons.

## Streams and exit codes

- `dispatch`: JavaScript on stdout or `--out`. 0 proven completion, 3 stopped with no JavaScript, 1 runtime/emission error, 2 usage.
- `thread`: JavaScript on stdout or `--out`. 0 complete, 3 stopped, 5 a
  self-check difference, 4 an undeclared generated name, 1/2 errors.
- `corpus`: baseline JSON only with `--update-baseline` (and the `--branches`
  file when asked). 0 no regression, 1 a regression or an error, 2 usage.
- `program`: the program at `--out` (or stdout when it is omitted). 0 every function traced, 3 some function
  did not complete, 1 a capture/artifact/walk/emission/placement error, 2 usage.

`thread` checks in order: a stopped walk is 3 even if it also has self-check
differences; self-check differences are 5; an undeclared generated name is 4.
`program` still writes the program when it exits 3 (stopped functions become
throwers) and writes nothing when it exits 1. Summaries go to stderr; help goes
to stdout.

## Failure modes

| Symptom | Mode | Exit | Side effect |
| --- | --- | --- | --- |
| Unknown option, missing flag, or raw `--report -` | any / `thread` | 2 | Usage on stderr; no analysis |
| Unreadable or malformed input snapshot | `thread` / `dispatch` | 1 | No JavaScript; failure JSON with `--json` |
| Missing/legacy/foreign artifact or bad descriptor | `thread` | 1 | No JavaScript or log |
| Stop: budget, unsupported dispatch, unmergeable join | `thread` | 3 | Step log may be written; no JS |
| Self-check difference / undeclared generated name | `thread` | 5 / 4 | JS already written |
| Missing capture, legacy cache, or bad baseline | `corpus` | 1 | No baseline rewrite |
| Regression or run-level `undeclared`/`emission`/`self_check` | `corpus` | 1 | Branch file may exist |
| Some functions stopped | `program` | 3 | Program written; throwers explain |
| Placement/name/pass/output error | `program` | 1 | No program written; dump stages may exist |

## Diagnostics

Every command explains the same underlying stops, but routes them differently.
The public stop reason is the same enum for all prefix walks: `completed` means
every path returned or continued a loop and the function emits; anything else
is an explicit partial boundary.

- `completed` — the function is complete; it emits, and `program` places it.
- `dispatch step budget reached` — `--steps` ran out; raise it or accept a
  partial result.
- `state N has no proven table row`, `state N opcode M has no proven dispatch
  cell` — the capture's table does not cover this dispatch; it is not a
  supported kernel on this path.
- `selector N is unsupported: …` — the handler exists but no recipe admits it:
  a coverage gap, not malformed input.
- `dispatch PC … is outside the … image`, `handler operands … need … bytes but
  only … remain`, `bytecode write address … is outside … image` — malformed
  bytes, or an entry/slot declaration that does not match the image.
- `branches cannot join at pc … state …` — the two sides met with VM state the
  tracer cannot merge.
- `dispatch revisited pc … state … without progress` — a loop the walk did not
  recognize.

`thread --json` and `dispatch --json` return diagnostics with stable codes, messages, and source provenance when present.
Normal thread output prints `stop:` and the last diagnostic as `detail:`. Corpus fingerprints and program stopped-function
lists expose their own stop information. See [JSON output](trace-json.md).

`thread` also reports what the walk did: `retraces:` by cause, `branches:`
counts with each fake-by-context decision and its reason, `scope:` for
generated names read where none is declared, `vm strings:` folded/declined with
reason counts, `property writes:` and `array constructions:` for the cleanup
passes, and `self-check:` differences.

`corpus` reports per capture: functions, completed, elapsed, overstep
(VMInstructions run per kept one) and the worst function; with the self-check
on, the inline overstep and how many functions depend on side order. It then
lists `N stopped: <kind>`, the steps run/kept/thrown away by cause (loop, `try`
or whole-walk restarts, and shared cells found late), and the ten highest
overstep functions.

A corpus `--baseline` comparison separates failures from observations.
`regression:` means a completed function now stops, a function is no longer
reached, a completing function lost positions/effects/created functions, a new
branch is decided by context, or the current run has an `undeclared`,
`emission` or `self_check` entry — any of these exits 1, with or without a
baseline for the last three. `changed:` means now completes, stops differently
without completing, gained positions/effects/created functions, a different
branch count, newly reached, or a capture not walked; it does not fail the run.
Branch-order (side-order) differences are counted as
`depend on the side order`, not yet as regressions. `--branches FILE` writes
one JSON line per function whose code was emitted and one per conditional
jump, which is where `--audit-fakes` records what a branch's ruled-out side
reached.

`program` reports `functions: N traced, M stopped` with one
`  stopped <key>: <reason>` line per thrower, the `kernel globals read:` list
of outside names the program still needs, and the output size and time. A
placement or name error aborts with exit 1 and no output; its text names the
function and the failed check (a generated name read from outside that no
creator gives, a shared slot its creator does not pass, a function nested in
itself, a creation marker that is missing or duplicated).

## Supported boundaries

These commands analyze source and run admitted recipes symbolically; they do not execute captured JavaScript or WASM.
`--inputs` supplies a snapshot, not live parent memory. Tagged `computed` and `unknown` values are output-only and are
rejected as inputs. Structurally admitted WASM interpreters use prefix preparation, with the limits in the
[prefix guide](wasm-prefix.md#wasm-dispatched-kernels-prefixwasmrs).

## Run an analysis

For a first look at a real tracked capture, use the CLI that accepts a source
path directly:

```sh
shape ops show tests/fixtures/kernels/javascript/kernel.raw.js --op 0 --format markdown
shape trace corpus tests/fixtures/kernels/javascript/kernel.raw.js --fast
shape trace thread tests/fixtures/kernels/javascript/kernel.raw.js \
  --pc 115869 --state 7 --descriptor 31 --out /tmp/shape-thread.js
```

The first command explains one proven handler. The second summarizes all
reachable functions and their stops. The third traces one known function,
prints its stop and branch counts, and writes its JavaScript to the selected
path. Remove `--fast` to run corpus self-checks.
Add `--json` to any trace command for automation. See [JSON output and snapshots](trace-json.md).
