# Fake-branch triage

This guide collects what we learned finding and fixing fake branches and the
walker failures that hide behind them. The first part is how to work: the
rules, the procedure and the tools. The second part is what to know: the
rules that decide a test, the evidence that rules out a side, and a casebook
of every kind of failure met so far, each with its symptom, cause, how to
recognize it, the fix and the pitfall. The last part lists the ideas that
were tried and backed out, and why.

Captures of one site expose the same VM structure with different obfuscation,
traps and bytecode layout. When a function walks much more slowly in one
capture, or stops where the same function completes in another capture, compare its
branch decisions with its counterpart. A missed fake-branch proof is the
usual explanation, but the stop, the trace facts and the input differences
must establish it.

Contents:

- [Words used here](#words-used-here)
- [Working rules](#working-rules)
- [What "looks bad" means](#what-looks-bad-means)
- [The procedure](#the-procedure)
- [Tools for one function](#tools-for-one-function)
- [How the walker decides a branch](#how-the-walker-decides-a-branch)
- [Why the extra state exists](#why-the-extra-state-exists)
- [Rules that decide a test](#rules-that-decide-a-test)
- [Evidence that rules out a side](#evidence-that-rules-out-a-side)
- [Casebook](#casebook)
- [Ideas that were backed out](#ideas-that-were-backed-out)
- [Performance lessons](#performance-lessons)
- [Pitfalls](#pitfalls)
- [Read one branch record](#read-one-branch-record)
- [Five small walks that separate the causes](#five-small-walks-that-separate-the-causes)
- [What remains open](#what-remains-open)

## Words used here

- **Fake branch.** A conditional jump whose test always has the same outcome
  at runtime. The obfuscator inserts them; the other successor leads into
  code that never runs: junk instructions, data read as code, traps, or real
  code entered in the wrong state.
- **Real if.** Both sides are code the program can run. They become the arms
  of an `if` in the emitted JavaScript.
- **Fake by test.** The predicate evaluator proves the test's outcome from
  what the path knows of its operands' types and values; only one side is
  traced.
- **Fake by context.** The test is not decided, but one side reaches a state
  no real code reaches (a VM invariant breaks); the walk rolls back and
  follows the other side.
- **Junk function.** A VM function made by `vmCreateThread` whose first
  instructions fail before any real branch: the path that made it is not
  code that runs.
- **Stop.** The walk could not continue; the function is emitted as one that
  throws. A stop deep in a function nearly always sits on the fake side of a
  branch the walker took for real.
- **Cell.** A memory slot shared between VM functions (a closure's captured
  slot), written as `$CELL_n["value"]`, or `$KEY_n` in program mode.
- **Loop variable, try variable, join variable.** A slot or stack cell that
  differs between iterations, between a `try` body and its catch, or between
  two arms that meet. It becomes a variable assigned in each place.
- **Value set, jump set.** The few constants a variable can hold (a value
  set), or the VM addresses it can hold when its other values are program
  data (a jump set).
- **Holder.** A VM function that has a given shared cell. Every holder may
  write it.

## Working rules

These are the defaults. Break one only with a reason written down.

1. **Slow or stopped means a misjudged branch.** A capture far from the corpus
   median, a stop, or emitted code that looks wrong is first assumed to be a
   fake branch treated as real. Look at the branches and their tests before
   anything else.
2. **Never cap it.** A time, step or round limit that makes a slow walk end
   hides the branch that makes it slow. Fix the decision, not the budget.
   The bounds that exist (sides kept apart, value-set sizes) are memory
   guards, not fixes.
3. **Only decide fake-branch shapes.** Decide tests that look like the
   obfuscator's predicates: `typeof` checks, a boolean or literal the path
   knows, a value whose type rules out the comparison, a test of an object
   the path just made. Do not decide a test only because it can be proven
   constant. The program's own guards (`v === v` NaN checks, `0 < a.length`
   loop tests) are real code and must stay `if`s.
4. **When a stop goes away only by deciding a real-looking test, look for
   the walker bug.** Deciding real code mislabels it fake, drops `if`s the
   source has, and usually hides the actual failure.
5. **Follow the VM's order.** A `try` body runs before its catch; a loop's
   writes happen before the next iteration reads them. Walk the writing side
   first, then give the other side the entry state with those writes laid
   over it.
6. **Abstract values in code, concrete values only for decisions.** Emit a
   slot as its variable, never as the value the tracer believes it holds. A
   slot's remembered value can be stale: closures write shared cells the
   walk does not see.
7. **Patched bytecode that differs between sides is a fake branch.** Real
   code does not patch a byte differently on the two sides of an `if`.
8. **Compare against a clean capture of the same site.** Same program,
   different junk: diff the normalized branch tests and explain the extra
   real ifs.
9. **Stops first, full runs last.** While stops remain, run only the captures
   they are in; run the whole corpus once, when every stop is fixed.
10. **Results must not change silently.** Check every change on the affected
    captures, then the corpus and a digest set. Explain each function a capture
    loses: junk dropped is good, a real function lost is a regression.
11. **Ask what happens if the branch went the other way.** If the other side
    could not pass (it jumps to data, reads a missing value, changes the
    stack shape), that side is the fake one.

## What "looks bad" means

- **Time.** Compare like-for-like runs: the same command, flags, build and
  function role across captures. Most captures sit close to the corpus median; a
  capture several times the median is a reason to inspect its walks.
- **Work per function.** Thread mode prints `steps: K kept, H handlers run`
  and a `retraces:` line. Tens of thousands of handlers, many `wrong breaks`,
  or thousands of `unmergeable meetings` identify a comparison target; they
  do not prove that any side should have been pruned.
- **Branch counts.** `branches: R real ifs, T fake by test, C fake by
  context`. A count difference against the same function in another capture
  deserves source and trace inspection.
- **Stops.** A stop deep inside a function (a jump to a program value, a
  stack that does not join, an underflow, a revisited position) marks where
  the model ceased to support execution. It points at the fork that led
  there; it is not itself a fake-side proof.
- **Function counts.** A capture that loses functions between runs either
  stopped tracing a junk function made only on a fake side, or lost a real
  one. Check each one: see
  [lost functions are usually junk](#lost-functions-are-usually-junk).

## The procedure

1. **Find the function.** Compare corpus summaries for equivalent captures
   using the same build and flags. For a supplied entry, `trace thread`
   reports kept steps, handlers, branch classes, retraces, and the stop;
   supply that capture's actual entry, descriptor, and report as documented
   in [trace CLI](trace-cli.md#command-shapes).

2. **Record its branch decisions.** The tracked fixture supplies a runnable
   first example:

   ```sh
   ./target/debug/shape trace corpus tests/fixtures/kernels/javascript/kernel.raw.js \
     --limit 3 --fast --branches /tmp/shape-branch-records.jsonl
   ```

   A full comparison should remove `--limit 3 --fast` and run the same mode
   on each capture being compared. The kernel's main body is the function
   that creates the most others; for other functions, match by the string
   literals in their branch tests. Compare `handlers`, `branches` and
   `retraces` between the slow function and its counterpart.

3. **Diff the tests.** Normalize each branch test (variable, cell and
   subroutine numbers, encrypted strings), count the real ifs per test in
   both, and look at the tests the slow capture has many more of. Those are the
   sites to explain.

4. **Walk back from the stop.** The stop is on a path; the forks before it
   on that path are the candidates. For each, ask what the test reads, what
   the path knows of it, and whether knowing more would decide it. The usual
   answers:
   - a value whose type the walker forgets: a shared cell after a call that
     may run a closure, a loop variable after its loop, a variable carried
     out of one arm of a join;
   - a call whose result the walker cannot type: a VM function made and
     called in place, where another capture tests the function itself;
   - a constant shown as a variable: `typeof 0 === "number"` where `0` is a
     loop or join variable's first value;
   - a value known only to another walk: a cell another function writes, a
     `try` body's write the catch reads before the body was walked.

5. **Inspect both sides.** When two sides meet with different stack heights,
   run apart, or one reaches a stop, record the fork and its test. The
   mismatch may be a missing invariant, an unsupported operation, a true
   difference, or a decoy. The retrace reason locates the investigation; it
   does not decide which side is real.

6. **Fix the rule when evidence supports it.** A missing type, value, or VM
   invariant belongs in the walker only with a source-backed proof. Rerun
   the affected captures, then the corpus once all stops are fixed, and inspect
   each changed completion, stop, branch record, code digest, function count
   and timing.

## Tools for one function

- **`shape trace thread SEED --report ARTIFACT --pc N --state N --descriptor D`**
  walks one VM function. It prints the stop, its diagnostic (`detail:`),
  retrace counts by reason, kept steps and handlers run, and branch counts.
  `--log FILE` writes every step with its PC, state and, for a branch, its
  kind and test. `--out FILE` writes the emitted function.
- **`shape trace program SEED --no-cleanup --out FILE`** walks every function
  of a capture with converged shared-cell contents and writes one program.
  `--no-cleanup` skips the whole-program cleanup passes, which cost far more
  than the tracing itself; use it for every run that checks tracing.
- **Thread mode and program mode differ.** A standalone `trace thread` has no
  shared-cell contents, no creator's cells and no caller-filled slots. A
  function that stops only in program mode has to be reproduced with the
  same inputs, or examined by printing from inside the program run. A
  function that stops in both is easier: start there.
- **Look captures up by full name and site.** Captures of different sites can share
  long name prefixes; a short prefix can pick the wrong capture.
- **Temporary diagnostics.** Most cases below were found by printing from the
  walker at one point: the walk's loop entries, exits and restarts; what the
  evaluator saw for one variable; the join arms of a jump target; the cell
  contents per convergence round; which statement marked an instruction as
  having run code. Such prints are scaffolding; keep them out of commits.
  `trace thread --partial --out FILE` now emits a stopped walk's code with an
  `INCOMPLETE` header: it shows the code on the stopped path while retaining
  the stopped status. See [stopped-thread inspection](trace-cli.md#inspecting-a-stopped-thread).
- **Branch records.** `trace corpus --branches FILE` writes one JSON line per
  decision; `--audit-fakes` walks the ruled-out side of each fake-by-test
  decision, bounded and undone, for corroborating evidence.

## How the walker decides a branch

At every conditional jump on a program value, in order:

1. **Typed decision.** The predicate evaluator gives each node a set of
   possible JavaScript types and, when there are a few, the constants it can
   be, and folds the test. A guaranteed outcome is fake by test.
2. **Opaque-shaped decision.** A test with the shape of the obfuscator's
   opaque predicates is evaluated again on what the path concretely put in
   the cells it reads, and on the value sets of loop and `try` variables.
3. **Fork.** Otherwise both sides are walked in turns of one step. A side
   that reaches a state real code never reaches marks the fork fake by
   context and the walk follows the other side. Both sides bad is a real
   stop.
4. **Join.** Where both sides reach the same VM position in states that can
   merge, they join: differing cells become join variables and type facts
   merge.

A test the program already made earlier on the path (`isUrl` tested twice) is
never decided from the first outcome: that is the program's own logic, a real
`if`, and only fake-branch shapes are decided.

## Why the extra state exists

Fake-branch handling has to answer two different questions: whether a side is
impossible, and whether two still-possible sides can share the rest of a trace.
The extra state records that distinction. A rejected join is not a rejected
branch, and a faster walk is not evidence that pruning was correct.

### Holes let the common suffix run once

One arm can leave an unused stack value below its result. The other arm has
its own result but no extra value. If later instructions only use the top,
both arms can share those instructions. Padding inserts holes below the short
arm's result so top-relative reads keep the VM's meaning. Without this, the
walker either duplicates the suffix or incorrectly merges bottom-relative
positions.

These holes are provisional. A later access into or below the padded region,
a bottom-relative stack operation, or an incompatible loop/join makes the
walker undo the alignment and keep the paths apart. It remembers the rejected
alignment in `Learned::unpadded`, so an enclosing retrace does not repeat it.
Ordinary sparse stack holes can read as `undefined`; join-padding holes must
not silently become values in the reconstructed program.

Only the narrower deficit case establishes a fake side: the longer arm kept
the fork-entry stack, the shorter arm popped some of it, and a later
top-relative access needs more than the shorter arm actually owns. Evidence
requires the holes of that one join. If the short arm can still read one of
its own values, as in a short-circuit expression, the branch remains real.
See [the session's alignment contract](symbolic-session.md#why-joins-have-stack-holes)
for insertion rules and the three regression tests that distinguish these
cases.

### Cached knowledge makes a retrace change something

A retrace removes speculative output so it can reconstruct the region with
better information. If it also forgot where the sides meet, which cell changed
in the loop, or which padded join failed, it could repeat the same work.
The walker keeps `meets`, widening/value/type sets, `apart`, `unpadded`, and
context-proven `forced` decisions with their VM positions. These are reasons
for a retry to do something different; they are not arbitrary memoized states.

The symbolic arena has the opposite identity problem. Rollback can reuse a
node number or generated binding serial. A cached alias, cell-read origin, or
dependency result for the old node would then describe a different value.
`Executor::rollback` removes those entries while keeping the observations
needed for convergence. The [cache inventory](symbolic-session.md#why-facts-and-caches-have-different-lifetimes)
lists the owners and invalidation rules. When a new fact is added, decide
whether it belongs to a path, an arena node, a retry region, or the capture
before deciding how long to keep it.

### Entry checks and return summaries answer different questions

`junk_entry` asks whether a created function fails in its standardized startup
state before any real branch. Initialized descriptor slots start as
`undefined`; caller-filled slots start as opaque inputs. Selected VM faults
can establish junk, including missing dispatch and truncated operands; a
budget stop or ordinary program throw cannot. A valid `try` normally blocks
this straight-entry proof. The dispatch-fault exception requires all recorded
handlers to have out-of-image PCs or missing states. The check is bounded and
does not recursively probe the functions it creates; an inconclusive check
leaves the creator's branch unresolved.

That answer depends on the capture, descriptor, PC/state, and availability of
startup metadata. `PrefixPreparation::junk_entries` shares it across walks.
Only retaining it inside one walk caused the same entry to be checked again
for each creator and convergence round. Both positive and negative answers
are reused; a negative answer means this check did not establish junk, not
that the function has been proven to complete.

A return-shape query instead asks what the callee's own completed walk returns.
`cli::trace::return_shapes::Round` reuses that walk, records which answers
callers used, and checks them in subsequent rounds. A completed callee is a
function maker only when every return gives a VM function it made. A recursive
query uses the previous round's answer to avoid a cyclic wait. This replaced
the separate speculative callee probe: one canonical walk now supplies the
answer and the emitted body. Changed shared-cell dependencies or callee
answers can invalidate reuse.

### State the evidence when checking a change

| Observation | What it establishes | What to inspect next |
| --- | --- | --- |
| `FakeByTest` | Supported predicate analysis decided the test | Operand facts, predicate shape, and any value set used |
| `FakeByContext` | The walker attributed an impossible VM state to one arm | The exact reason and the fork that owns the bad value or stack deficit |
| `PaddedStack` / an `unpadded` retry | An attempted stack alignment could not stand | Whether the later access was valid on each original stack; do not assume either side is fake |
| More retraces or handlers than a counterpart | More analysis work | Missed predicates, stale facts, and repeated entry checks; counts alone do not justify pruning |
| An audit reaches its bound | The bounded audit did not settle the alternative | Keep the original proof and audit result separate |

For implementation, read [`predicate.rs`](../../src/tracing/predicate.rs) for test
rules, [`exec.rs`](../../src/tracing/symbolic/exec.rs) for fact lookup and padded
access checks, [`session/branches.rs`](../../src/tracing/symbolic/session/branches.rs)
for alignment and joins, and [`execute/converge.rs`](../../src/dispatch/prefix/execute/converge.rs)
for rollback, evidence attribution, and entry checks. Preserve the paired
real/fake tests in `vm_branches.rs`; a rule that only
makes the stopped fixture complete can still erase valid program behavior.

## Rules that decide a test


Each rule is a fact about the VM or JavaScript that lets a test be decided.

| Rule | Example it decides |
| --- | --- |
| Objects, arrays and VM functions are truthy | `![]` |
| `typeof` of a known type is a known string | `typeof [] === "object"` |
| Disjoint types are never `===` | `"a" === 1` |
| A non-nullish value is never `== null` | an object `== null` |
| `+` with a string is a string | `typeof (x + "") === "string"` |
| `-`, `*`, `/`, `%`, `**` and the bit operations give numbers, unless both operands are known BigInts | `typeof (a - b) !== "number"`, `typeof (s \| x << m) === "number"` |
| `Number`, `String`, `Boolean` called return their primitive type | `typeof Number(x) === "number"` |
| The same node is itself, unless it may be NaN | `"s" === "s"` |
| The kernel's string decoder returns a string | `typeof (x + vmDecodeString(…)) !== "number"` |
| A string-table read is a string | `typeof vmStringTable[i] === "string"` |
| A value of a few constants folds every combination | `(c ? 75 : 97) % 1 !== 0` |
| `typeof` of a value of a few types is one of their names | `typeof (a - b) === "string"` |
| A join variable is what one of its arms assigned | the union of the arms' types |
| An outlined call is what its body returns, with its arguments | a test on a VM subroutine's result |
| A type test narrows the tested value | after `typeof v === "string"` |
| A test of what the path concretely put in a cell | `typeof $CELL_3.value === "undefined"` |
| Equality with a literal; a null test of an object the path made | `obj == null` where the path made `obj` |
| A null or truthiness test of a cell the function made, when what the path put there and everything each closure holding the cell puts there (asked of the closure's own walk) is of none of the tested types. The closures counted include those an earlier attempt made (a loop whose body makes one after the test is traced again), and a walk kept for a later round stands only while the closures' walks there give the same answer | `counter === null` after `counter = 0`, with a getter that only does `++counter` |
| Two joins are never `===` when no constant one lists for a primitive type both may have equals one the other lists | `([] or typeof []) === (null or "boolean")` |
| A try variable has the types its body and entry give it | `typeof v !== "object"` past a catch |
| A loop variable has the types its iterations give it, inside and after the loop | `typeof v === "number"` in a loop; `v === true` on a string after it |

A rule must be complete when it decides. A decision that read a value set or
a type that grew later is decided again: the loop or `try` is traced again
when a set it read grows.

## Evidence that rules out a side

These are states no real code reaches. A fork side that reaches one makes
the fork fake by context. Each is a VM invariant, not a heuristic.

| Evidence | Why real code never does it |
| --- | --- |
| No dispatch cell for the (PC, state) | the dispatch table covers every reachable state |
| PC or operand outside the bytecode image | the decoder reads only the image |
| Reading below the stack the function started with | compiled code balances the stack |
| A return with no call frame; an empty error record popped | frames are pushed before they are popped |
| An iteration that changes the stack length | loops keep the stack shape |
| A call of a string, number or boolean literal | compiled code calls functions |
| A VM jump to a program value instead of a pushed constant | jump targets are bytecode constants |
| A VM function made that fails before its first real branch | the program never makes a function that cannot run |
| Inside a catch, reading the stack the throw left without setting its length | the handler re-establishes the stack |
| A WASM trap: a trap selector, a full try or record bank, a try entered after more exits than entries | the interpreter's own bounds |
| A bytecode write outside the image | the patch handler writes inside the image |
| Two sides that patch a byte differently, then a read of it | real code does not patch differently on two sides of an `if` |
| A jump through a cell holding one address or a program value | only the address is code the VM runs |
| A read of the string table past its end | the table's operands are indices the compiler emitted |

Things that look like evidence and are not:

- a `try` exit with no `try` open: `tryFrameStack.pop()` on an empty stack
  does nothing in the JavaScript VM (only the WASM interpreter traps on it);
- `undefined` or `null` assumed from a `== null` test: no literal;
- a read of a memory slot the thread has no cell for: the VM throws a
  `TypeError` there, which is observable behavior;
- a junk function the code makes on every path: a decoy it never calls says
  nothing about the path;
- a handler the tracer cannot run: an unsupported operation is a stop, not a
  fake side.

## Casebook

Each entry is a kind of failure, not one capture: what it looks like, why it
happens, how to recognize it, the fix, and the pitfall to remember.

### A repeated program test decided as fake

- **Symptom.** A test such as `/^https?:\/\//.test(url)` is a real if the
  first time and "fake by context, an earlier branch on this path tested the
  same value" the second time, keeping a different side on each path.
- **Cause.** A rule decided any test the path had already made, by the first
  outcome. That is the program checking its own stored value twice
  (`if (isUrl) …; … if (isUrl) cache[url] = link`), real code, not an
  obfuscator predicate: by definition a fake branch goes the same way however
  you reach it.
- **Recognize it.** The same position decided differently depending on the
  path, or a decided test that is a plain program call or variable rather than
  a fake-branch shape.
- **Fix.** Never decide a test from an earlier outcome on the path. A
  fake-branch shape is decided by its types on every path anyway; everything
  else stays a real if.
- **Pitfall.** The rule existed to stop functions that test one value many
  times from forking at every test. Without it some walks do more work, and a
  function whose repeated tests nest deeply can reach the fork-depth guard; that
  is a walker cost to solve on its own (by joining the sides sooner), not a
  reason to decide real code.

### A kernel helper's result is untyped

- **Symptom.** `typeof (x + vmDecodeString(…)) !== "number"` stays a real if
  and its other side leads into junk.
- **Cause.** The kernel's string decoder returns the decoded string or
  throws; the evaluator did not know its contract.
- **Fix.** Give kernel helpers their proven types: the decoder returns a
  string, a string-table read is a string.
- **Pitfall.** Kernel helpers have contracts as strong as built-ins; type them
  from their proven source, not by name guessing.

### Arithmetic and bit operations are untyped

- **Symptom.** Tests like `typeof (timing[e] - timing.navigationStart) !==
  "number"` or `typeof (s | x << m) === "number"` stay real ifs; their other
  sides make junk closures or patch bytecode.
- **Cause.** Only some operators were typed. A BigInt minus or times any other
  type throws, so arithmetic is a BigInt only when both operands are BigInts;
  bit operations always give numbers.
- **Fix.** `-`, `*`, `/`, `%`, `**`, `<<`, `>>`, `>>>`, `|`, `^`, `&` give a
  number unless both operands are known BigInts.
- **Pitfall.** The opaque predicates count on ordinary JavaScript facts. When
  the obfuscator writes `x | 0` or `a << b`, read it as "this is a number".

### A value rebound many times loses its type

- **Symptom.** `typeof v !== "object"` on an array the path made, or
  `typeof n === "number"`, passed to an outlined test, stays a real if;
  its other side holds a second copy of a collector (southwest 02bd
  wave:4:3: the readback collector again plus Intl code) or leaves extra
  loop variables that keep a loop from being rebuilt (puma 0e3e: the
  namespace callback stayed inside a gradient loop, so it was never
  written as an arrow and the recognizer did not claim it).
- **Cause.** The walk rebinds a value once for each element written into
  it (fifty readback samples are a chain of about a hundred `let`
  bindings). The evaluator followed binding chains only as far as its
  expression-depth limit (64), so the test's operand never reached the
  array literal. The same subroutine on a short array was decided.
- **Fix.** Follow binding and cell-read chains to their end. A chain is
  not nested syntax; only the map's size bounds it.
- **Pitfall.** A depth guard meant for expression trees is wrong for a
  chain of copies. Compare with the same test on a smaller value first.

### Constants hidden in a join or a conditional

- **Symptom.** `(c ? 75 : 97) % 1 !== 0` stays a real if.
- **Cause.** The evaluator kept only types, not the few constants a value can
  be.
- **Fix.** An abstract value lists its constants when there are a few;
  operators fold every combination; `typeof` of a few types is one of their
  names.

### A test reads an outlined call's result

- **Symptom.** The walk goes back and traces a subroutine call inline every
  time a test reads its result; handler runs multiply.
- **Cause.** The test could only be decided by seeing the call's body.
- **Fix.** Each outlined call keeps the function's return values with its
  arguments substituted, for tests only, and the evaluator decides on those.
  The call stays outlined.
- **Pitfall.** Do not change what is emitted to decide a test; give the
  evaluator the facts instead.

### A loop loses its real exit

- **Symptom.** The rest of the function is traced inside a loop.
- **Cause.** A path back at a loop header with no break of its own to ban
  banned every recorded break it had passed, of any loop, so a real exit of
  an unrelated loop was banned.
- **Fix.** Remember each loop's body; only ban the breaks of loops whose
  bodies passed the header.
- **Pitfall.** "Back at a header" is evidence about the loops around that
  header only.

### An iteration changes the stack length

- **Symptom.** `branches cannot join at pc P: an iteration changes the stack
  length from n to n+1`.
- **Cause.** A path came back to a loop header with another stack length.
  Compiled loops keep the stack shape, so the fork that led there is fake.
- **Fix.** Treat the change as fake-branch evidence for the fork that led to
  it.
- **Pitfall.** When the stop appears anyway, the fork that should take the
  blame was decided real because a test lost a type. See
  [a loop variable forgets its type after the loop](#a-loop-variable-forgets-its-type-after-the-loop).

### A junk side walked again on every retrace

- **Symptom.** One function is most of a seed's time; its waste is mostly
  `fork: a fake side`, yet it ends with only one or two fake-by-context
  branches.
- **Cause.** The same fork's junk side was found fake again on every retrace
  of an enclosing loop (loop variables widening, meeting points): each time
  the walk stepped hundreds of handlers into it until it jumped to a program
  value. A seed whose tests are flipped (`===` for `!==`) steps the junk side
  first, the worst case.
- **Fix.** Keep the verdict in `learned.forced`: a retrace follows the real
  side at once. 502 seeds gave every branch decided as before and one
  function's code different, nine subroutine calls staying calls.
- **Pitfall.** The walk can do things on a junk side that outlive its undo:
  a loop rule marked a loop as dispatched on its values there and traced
  calls inline for it. Fewer junk walks means fewer of those, so code can
  change without any branch changing.

### Sides that cannot merge

- **Symptom.** Two sides meet with different stacks, patches, try frames or
  pending exceptions, and the walk stops at the meeting point.
- **Fix.** Step both sides on past that position, to a later meeting point or
  each to its own end; a stack underflow on the side that met short of values
  is evidence for that fork only.
- **Pitfall.** An unmergeable meeting is not a stop; it is a reason to keep
  the sides apart until one shows a bad state.

### One arm leaves a value it never pops

- **Symptom.** One function is far bigger and slower than its neighbours; its
  `unmergeable meetings` all come from one fork, every meeting one stack value
  apart, and the same code appears several times at deeper and deeper nesting.
- **Cause.** The fork compiled `v !== undefined ? !v : LIT`: one arm replaces
  `v`, the other pushes its literal on top of it and never pops `v`. Both are
  real code; the leftover is never read. Joins required equal stack lengths, so
  the sides ran the whole rest of the function apart and it was emitted once
  per arm.
- **Fix.** Line the stacks up from the top: holes where the shared part ends on
  the shorter side, which read `undefined` as a read below a stack's end does.
  Any step that would read what only one path has there undoes the join.
- **Pitfall.** Undoing must go back to what the walk had learned when the arms
  joined. A sweep found a fork whose sides met at an inner loop's header at two
  depths: padded, the loop's iteration changed its stack length and ruled a
  real arm fake, and after a plain undo the meeting points and loops learned on
  the padded path still decided it so. A loop entered at two depths stays two
  loops.

### One arm leaves a value under a loop variable

- **Symptom.** A function inside nested loops is slow, with hundreds of
  unmergeable meetings from one fork, every meeting one stack value apart.
- **Cause.** `x = test ? v : f()` where `v` is a stack cell every enclosing
  loop names: the arm that calls `f` leaves `v` under its result. Lining the
  stacks up from the top moves `v` on the shorter side, so joins refused, and
  the sides ran the rest of the loop body apart.
- **Fix.** Line them up anyway: the loop's `continue` or `break` on the padded
  path, or a step reaching the holes, undoes the join.
- **Pitfall.** Not for a deficit. Lined up under a loop variable, a deficit's
  evidence was lost when a later deficit nested in the holes, and the
  underflow went back to blaming the innermost fork: a real branch was ruled
  fake. A deficit nested in such holes undoes the outer join. And an undo must
  not leave holes behind in a path a loop or fork saved: a step reaching holes
  of a join already undone starts the walk over.

### A side that pops a value the code after the join uses

- **Symptom.** A function is slow and its fork decisions change with retrace
  order: the same jump is fake on one walk and real on another, and the
  evidence is an underflow found far past the meeting point, while some
  nested fork was stepping.
- **Cause.** One side of a junk jump pops values the other keeps, and the code
  after the meeting point uses them. The underflow only shows up once that
  code runs down to the missing value, so whichever fork is innermost then
  takes the blame.
- **Fix.** Join the sides lined up from the top, as any padded join; the code
  after the join reading under the short side's own bottom (a position below
  the number of holes) is the evidence, for this fork, wherever it is reached
  from.
- **Pitfall.** Reaching the holes is not enough. `a || b` and `a && b` with the
  fallback pushed before the test pop on one side and keep on the other, the
  same stack shape, and the code after reads each side's own top. Fake
  branches are rarely logical expressions; only a read under an empty stack
  proves the short side junk.
- **Pitfall.** Do not move the holes for this shape. Putting them where the
  short side cut the stack lines `v || x` up wrongly (its short side's top is
  `x`, its own result): the join is undone, the walk takes another route, and
  one function followed a junk side into an unsupported handler. Holes at the
  bottom shift every position, so later joins refuse and a function ran more
  than twice as slow with the same output. Whether a read is under the short
  side's bottom does not depend on where the holes are.

### Two sides patch a byte differently

- **Symptom.** A merged path reads a bytecode byte that the two sides of a
  fork patched differently, and the walk stops.
- **Cause.** One side was not code that runs.
- **Fix.** Keep the fork's sides apart while their images differ at that
  byte, each running on its own image, so the side that is not code that runs
  shows a bad state.
- **Pitfall.** Never pick one image; disagreeing patches mean a fake branch
  the walker missed.

### Junk functions decide the side that made them

- **Symptom.** A walk goes down a side that makes a VM function and stops
  later; the made function, walked alone, fails at once.
- **Cause.** A function made on a fake side is often garbage: its first
  instructions read below its empty stack, pop an error record it has none of,
  trap, read a constant table past its end, or patch outside the image.
- **Fix.** Check each made function from its entry: any failure before its
  first real branch makes the side that made it fake, past a catch too.
  Branches its own tests decide do not count as branching. Start the check the
  way a real call starts the function: the slots its descriptor initializes as
  `undefined`, and its captured, argument and arguments-object slots as values
  it does not know.
- **Pitfall.** A check that starts the function differently from a real call
  answers a different question: a function whose first instruction reads a
  captured cell goes another way without it. And share the result: it depends
  only on the capture and the made function, so checking it again in every walk is
  pure cost.

### Lost functions are usually junk

- **Symptom.** After a fake-branch fix, captures trace a function or two fewer.
- **How to check.** Emit the capture with both builds, diff the function names,
  and read each lost body. Junk shows itself: no `if`, loop or `return`, a body
  that throws at once (`Cannot read properties of undefined`), and the old
  output using the function as a number (`fn * x`, `fn >= 2`), which means a
  number operand was misread as a function made there. Other legitimate drops:
  a junk copy of a function whose real copy stays, a copy nested in a junk
  parent, and a function made on the side of a `typeof` test that is now
  decided.
- **Pitfall.** A drop in function counts is usually the tracer getting more
  accurate, but it must be checked function by function.

### A jump to a value the path knows is not a number

- **Symptom.** A walk follows a side that jumps through a shared cell.
- **Cause.** A shared cell's read was never treated as a program value,
  because another holder may have written a VM constant there. But a cell no
  other function holds, made `{ value: void 0 }` and never written, the path
  knows exactly.
- **Fix.** A jump inside a `try` (where it is no dispatch `TypeError`) to such
  a known non-number is junk evidence like any program-value jump.

### A cell holding one address or data

- **Symptom.** A jump through a cell stops, and the shared-cell rounds do not
  settle.
- **Cause.** The cell holds one VM address plus program data. A jump to the
  data leaves the code the VM runs, so only the address is real code, even for
  a read after code that may write the cell.
- **Fix.** A VM jump through a cell holding one address or a JavaScript value
  goes to the address; no test is needed.
- **Pitfall.** Ask whether the other value could ever be a real jump target. If
  not, there is nothing to dispatch on.

### A catch needs a write its body has not made yet

- **Symptom.** The catch path stops on a VM value the `try` body may write.
- **Cause.** The fork walked the catch before the body reached the write; the
  VM runs the body first.
- **Fix.** Trace the `try` again with the body walked to its close first, so
  the catch starts from the entry with the body's writes laid over it and
  reads the written slot as a try variable.
- **Pitfall.** Walk in VM order: the writing side first.

### A catch does not know the body's types

- **Symptom.** A catch stops on a VM value.
- **Cause.** The body gave a variable types the catch had not been told, so the
  catch could not decide `typeof v !== "object"` on a try variable both the
  entry and the body made an object.
- **Fix.** A catch that stops on a VM value learns the types the body gives
  its variables, and the `try` is traced again.

### A handler choice mistaken for an ambiguous operand

- **Symptom.** A handler's operands are "unproven" and the walk stops in
  preparation, not on a branch.
- **Cause.** Operand separation flagged every `?:` and `||`/`&&` in a handler as
  an ambiguous operand decoder. A handler that makes an object inheriting from
  the top of the stack (a type test `||` a type test) reads no bytecode and
  leaves the PC alone, so it has no operands at all.
- **Fix.** Only a choice that reads the bytecode or touches the PC affects
  decoding.
- **Pitfall.** Not every stop is a branch: check whether it is in preparation
  or in the walk.

### VM bookkeeping counted as code

- **Symptom.** A jump through a cell stops with "read from a shared cell after
  code that may write it", and the shared-cell rounds oscillate: with
  data-only contents the function completes and writes addresses into the
  cell; given those addresses, it stops and writes nothing.
- **Cause.** The function keeps return addresses in its own cell (a call setup)
  and jumps back through it. The read came after `i + 1` with `i` bound to a
  number in the same instruction, and the "runs code" check counted that `+`
  as code that may run a closure, because it only looked at number literals,
  not `let` copies of them.
- **Fix.** Arithmetic on numbers, through bindings, runs no code; the read is
  before any code that may write the cell, and the jump dispatches on the
  cell's addresses.
- **Pitfall.** An oscillating convergence means a walk depends on its own
  writes. Find which read is wrongly treated as stale.

### A loop signal dropped by a fork

- **Symptom.** `dispatch revisited pc P state S without progress`, from a loop
  header the path reaches again from inside a nested loop.
- **Cause.** The path reached the header again in a nested fork while the
  outer fork's other side halted in the same turn on a loop inside the fork.
  When both sides of a fork halt, the fork handled only the first halt; the
  dropped one had already marked the position as a known header, so every
  later arrival was taken as a stale header until the walk gave up.
- **Recognize it.** Log loop entries, exits, restarts and the new-loop
  signals: the header is entered only with "already passed", and the signal
  for it never reaches a region that could retrace it.
- **Fix.** When both sides halt, a new-loop halt whose header the path passed
  before the fork goes first: its retrace outside the fork redoes the fork
  anyway.
- **Pitfall.** This stop could be made to disappear by deciding real tests
  (see [backed out](#ideas-that-were-backed-out)). That was the wrong fix. A
  side effect of a halt that gets dropped must not outlive it.

### A loop variable forgets its type after the loop

- **Symptom.** An iteration changes the stack length at a loop header; the
  step log shows a test decided fake on the first pass and real on the
  retrace.
- **Cause.** Two gaps. A loop variable's types reach the trace only when the
  loop is traced again, which happens only if a decision inside the loop read
  a type that grew, so a decision after the loop never sees them. And the join
  of the `if` around the loop kept only type facts both arms had, while the
  other arm never held the loop variable.
- **Recognize it.** Print the arms of the variable the test reads: one arm has
  a type, the other is the loop variable with none.
- **Fix.** When a path leaves a loop, give each loop variable the types of its
  start or of what iterations carried back. A join keeps what one side knows
  of a variable named after the fork, since only that side holds it.
- **Pitfall.** Types learned in a loop are facts about the variable, not about
  one iteration; carry them wherever the variable goes. Path facts (what a
  test narrowed) are different: those still need both sides to agree.

### A jump through a copy of a jump-set variable

- **Symptom.** `jump target is not a concrete non-negative integer`, in a loop
  around a `try`, often only in program mode.
- **Cause.** The `try` body saves where to resume (an address and a state)
  before code that may throw. In a loop the cell's entry is unknown, so the
  try variable gets a jump set: an address, or a program value. The jump goes
  through a `let` copy of that variable, and the lookup from a value to its
  variable only knew variables with a value set or an assumption.
- **Fix.** A variable with a jump set is found too; a VM position it holds is
  one of those addresses.
- **Pitfall.** Every way a variable can carry knowledge (value set, jump set,
  assumption, types) has to survive copies; check the lookup, not only the
  variable.

### A catch takes one write of a saved position

- **Symptom.** `after a catch, pc P state S reads a stack the throw left`, or
  the catch jumps to a known address and decodes garbage there.
- **Cause.** The body saves where to resume in two slots, an address and a
  state. The catch reached the jump when the body had written only the
  address, so it paired that address with the state slot's entry value, and
  the `try` traced with the body first did not learn the state it wrote.
- **Fix.** A catch stopped there learns what the body assigned before it is
  traced again, and a jump to an address only the body wrote takes its state
  from the body's writes too.
- **Pitfall.** An address and a state are one value in two slots: never pair
  one from the entry with one from the body.

### Sequential tests that nest

- **Symptom.** `forks nest more than 48 deep` in a run of `x = c && f()`, on a
  function that completes in other captures.
- **Cause.** Layout. The side with the shorter arm reaches the join first,
  passes it, and takes the next fork inside this one before the longer arm
  steps; so down the whole run, with no meeting point found.
- **Fix.** A walk that reaches the guard starts over patient: a side at a fork
  of its own waits for the other, which then crosses it at the meeting point.
- **Pitfall.** The guard itself stays: a walk that still reaches it is patient
  and nested for real.

### A function made inside itself

- **Symptom.** `trace program` fails the whole capture: "it is nested in itself".
- **Cause.** Real code, not junk. An event handler removes its listeners and
  makes a module factory again, with no shared cells; the factory makes the
  function that registers the handler. Each function is nested where it is
  made, and only direct self-creation was allowed.
- **Fix.** A function that shares no cells, made inside a placement of itself,
  refers to that placement. A function that shares cells, made again, is a new
  instance placed again, as long as the loop passes one that shares none. A
  loop of functions that all share cells still fails: real code cannot write
  one.
- **Pitfall.** Before calling a structure junk, read it: listeners and
  factories are real.

### Catch addresses, traps and underflow in the WASM VM

- **Symptom.** WASM functions stop past a catch or on try bookkeeping.
- **Causes and fixes.**
  - Past a catch, a VM jump goes to a VM address the `try` body wrote; a catch
    keeps the VM positions its body does not write.
  - A side inside a catch that reads the stack the throw left, a trap
    selector, and a made function that underflows its empty stack before any
    branch are fake-side evidence.
  - A shared cell this path last wrote a non-position to makes a jump through
    it a program-value jump; where paths meet, a position one wrote stands
    over a value the other wrote.
  - A `try` exit with no `try` open is a no-op in the JavaScript VM; only the
    WASM interpreter traps on a try entered after more exits than entries.
- **Pitfall.** The two VMs share the bytecode contract but not their bounds:
  check which VM an invariant belongs to before counting it as evidence.

## Ideas that were backed out

Recorded so they are not tried again without new evidence.

- **Deciding `v === v` on numbers that cannot be NaN.** A function that mixed
  four entropy sources into a capture (`Math.random`, `performance.now`, `Date`,
  `crypto.getRandomValues`) guarded each with `v === v`: the program's own NaN
  guard. Deciding them dropped real `if`s and did not fix the stop it was
  meant for. Self-comparisons on a possible number stay real ifs.
- **Deciding `k < arr.length` from writes the path made.** `arr = []; arr[0] =
  v; 0 < arr.length` is a real loop test. The rule was expensive and only hid a
  walker bug (the dropped loop signal). The only `.length` rule worth having is
  a string literal's.
- **Matching separate calls by structure.** Comparing two call nodes by shape
  would make `Math.random() === Math.random()` true. Only the same node is
  itself.
- **Deciding a loop test from a slot's remembered value.** Deciding
  `(Number(i) + 1) < undefined` as always false from a slot belief deleted a
  real loop body. Slot facts decide only when nothing else can write the slot.
- **Shared-cell types at loop headers, joins and catches.** They fixed rare
  cases and slowed everything else. Only concrete values are kept at loop
  headers; exact facts there cost far more than they decided.
- **Loop-cell types for jumps through cells.** No effect on any stop and a
  cost everywhere.
- **A probe walk inside a walk.** A budgeted walk to learn what a callee
  returns was replaced by the callee's own walk, walked once and shared. A
  probe is a cap in disguise and answers a different question than the real
  walk.
- **Budgets as fixes.** Every time-, step- or round-limited fix was rejected
  in favor of finding the branch. A step-limited meeting-point probe gave way
  to walking both sides in symmetric turns with cut-back.

## Performance lessons

- **Share a result that depends only on the capture.** A junk-function check kept
  per walk was repeated by every function that made the same descriptor, by
  every convergence round and by every retrace, and it dominated the cost of
  the whole run. Kept once per capture, the cost disappeared.
- **A check's inputs set its cost.** Giving a check more realistic starting
  state made each check walk further before failing; that is what made the
  repeats expensive.
- **Find a cost by bisecting with best-of-three timing on fixed captures.** A
  fixed set of slow captures plus random ones, each run three times, separates
  one change from another reliably. One run per capture does not.
- **Describe only what stands.** Rendering a diagnostic that scans every
  statement, at every dispatch that may replace the stop, made deep walks take
  minutes. Describe only a stop that stands.
- **Do not reparse the whole program per pass.** Cleanup passes that each parse
  and print the whole emitted program cost many times the tracing; keep them
  out of tracing runs, and run passes on one parsed tree.
- **Cut back, do not restart.** A new fact found on one side (a loop, a
  meeting point, a call to trace inline) takes only that side back to the step
  that found it, not the whole fork or function.

## Pitfalls

- A decision may only use knowledge that is complete. Loop value sets are
  partial while the loop is still traced; a decision on a partial set can
  prune the path that would complete it. Record such decisions and retrace
  when the set grows.
- Shared-cell contents gathered across functions are sound only when every
  holder completed; guessed contents can miss addresses and make a real jump
  look fake. Keep guessed contents only if the strict contents complete fewer
  functions.
- A value is a fact about a path; a type is often a fact about a variable.
  Type facts that survive a join only when both sides agree lose a variable
  that only one side holds.
- Never bound the cost of a misjudged branch with a time or step limit and call
  it fixed.
- A stop in program mode may not reproduce in thread mode, and the reverse:
  compare with the same inputs.
- When both sides of a fork halt, any side effect the dropped halt left behind
  (a learned header, a counter) must not outlive it.
- A code cleanup pass that fails on one overlap must decline that one edit,
  not fail the whole program; a pass that chains edits must take one link per
  round.

## Read one branch record

`trace corpus --branches FILE` writes JSON Lines. A limited, fast run of the
tracked JavaScript VM fixture produced this decision record:

```json
{"at":"116981/5","function":"d31@115869/7","kind":"real","seed":"kernel.raw.js","test":"!(\"undefined\" !== typeof host[\"Reflect\"])","values":"unknown"}
```

The coordinate `at` is this run's PC/state, and `function` identifies the
walk that encountered it. `kind: "real"` with `values: "unknown"` means the
tracer did not prove a side impossible. It is **not** evidence to prune either
side. Compare that test and its source/provenance in another capture, then inspect
what each side reaches. If a branch is `fake by test` or `fake by context`, the
record includes why and which side was kept; `--audit-fakes` can walk the
ruled-out side for additional diagnostic evidence. That audit does not itself
turn the alternative into a regression or a source-level impossibility proof.

## Five small walks that separate the causes

The image arrays below are the complete instruction bytes passed to `walk` in
`tests/vm_branches.rs`. In that fixture opcode `1`
pushes `host.flag`, `2` pushes 1, `3` pushes 2, `4` is a conditional jump,
`5` an unconditional jump, `6` returns the top stack value, `17` pops, and
`26` reads memory slot 0. The second byte of a conditional jump is its target;
its third byte is the dispatch state. Opcode 200 has no dispatch cell. These
are fixture semantics, not a claimed decoder for arbitrary captures.

**Known test.** `[9, 4, 6, 0, 3, 6, 200]` computes `![]`, then would jump
into opcode 200 if the value were true. An object is truthy, so `![]` is
false. The trace records `FakeByTest { taken: false }`, does not fork, and
emits no `if`. The unreachable garbage is corroboration; the decision follows
from the test value.

**Bad target, unknown test.** `[1, 4, 6, 0, 3, 6, 200]` tests `host.flag`,
whose value is unknown. The target at PC 6 has no dispatch cell, while the
fall-through returns 2. The record is `FakeByContext { taken: false }` with a
reason containing `opcode 200`. Generated code retains the `host["flag"]`
expression but has no `if`. This is a context proof, not constant folding of
`host.flag`.

**Stack underflow.** `[1, 4, 5, 0, 17, 3, 6]` has an unknown test. The
fall-through pops an empty stack; the jump reaches the value-push and return.
The decision is `FakeByContext { taken: true }` with `invalid-index` evidence.
A later failure can resolve an earlier fork, but only when the failure is a
VM invariant violation on that path.

**A real exception is not a fake side.** `[1, 4, 6, 0, 3, 6, 26, 6]` reads
an absent memory slot on one side. The VM can throw there. The trace retains
an `if` and emits `throw new TypeError` for that side; it records no
`FakeByContext`. Treating every error as impossible would erase observable
program behavior.

**Loop invariant.** `[1, 4, 8, 0, 2, 5, 0, 0, 3, 6]` enters a loop whose
body pushes one value per iteration. The branch into the body is classified
`FakeByContext { taken: true }`; its reason includes
`prefix-loop-stack-change`. The analysis uses the loop's stack contract, not
an arbitrary iteration cap.

The more involved case
`an_underflow_past_an_unmergeable_join_decides_the_fork_that_left_the_side_short`
shows why branch counts cannot be judged independently: its first fork is
`FakeByContext`, its second is `Real`, and the output has exactly one `if`.
Compare the ordered decisions and the reachable state at each fork, not just
the final total.

**Stack deficit.** `[3, 1, 4, 6, 0, 17, 6]` pushes one value before an
unknown test; the fall-through pops it, and the return after the branch reads
the top. On the fall-through that is under an empty stack. The decision is
`FakeByContext { taken: true }` with a reason starting `stack-deficit`, and
the output has no `if` (`a_side_whose_stack_the_code_after_it_empties_is_fake`).
`[2, 2, 3, 1, 4, 8, 0, 17, 11, 6]` has the same shape with two more values
under it: the fall-through compares those, a value of its own, and the branch
stays real (`a_side_that_pops_to_a_value_pushed_before_the_test_is_real`).

## What remains open

The model still has known edges that a new capture may reach:

- a shared cell holding two or more real VM addresses, read after code that
  may write it, needs a dispatch after a call inside one instruction, which is
  not built;
- a loop of VM functions that all share cells and make each other cannot be
  nested (real code cannot write one, so it would be junk);
- shared-cell contents that do not settle across rounds stop the function that
  depends on them.

A useful record for a new stop contains the exact input, build and flags, the
function and PC/state, the test provenance, both sides' outcomes, the retrace
reason, and any changed emitted code. A new rule needs a VM invariant or
source proof plus a corpus comparison across the affected captures.
