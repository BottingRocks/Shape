# Individual thread cleanup

`shape trace cleanup-thread` cleans an already emitted thread independently of
the tracer and program linker:

```sh
cargo build --release
./target/release/shape trace cleanup-thread thread.js --out thread.cleaned.js
```

The input is exactly one JavaScript function declaration, optionally containing
local helpers. A raw capture or a linked program is not the intended input.
Omit `--out` to write JavaScript to stdout. Counts and elapsed time go to stderr;
the input file is not modified unless explicitly chosen as the output file.
The command parses and transforms source; it never executes captured code.

The reusable API is `shape::emitted_cleanup::cleanup_thread(&source)`. It owns
no global mutable state and returns code, total inlined bindings, computed
snapshots inlined, helper calls expanded, helper declarations removed/remaining,
property writes simplified, arrays folded, method calls restored, numeric slot
values reused, and cleanup rounds.
It also reports reconstructed loops and flattened standalone blocks.
Original source maps are invalid
after cleanup. `trace program --thread-cleanup` calls it on each tracing worker
with captured/exported slot metadata, checks scope before and after cleanup,
then links the results. The default program mode and the `thread`
and `corpus` commands keep their existing cleanup policies.

To inspect a complete linked program with just per-thread cleanup:

```sh
./target/release/shape trace program capture.raw.js --thread-cleanup \
  --no-cleanup --out program.threads-cleaned.js
```

Omit `--no-cleanup` to also run each function's cleanup stages
(`clean_function`) after this driver and the whole-program stages
(`finish_program`) after linking; see the
[pipeline](emitted-program-cleanup.md#pipeline). The two cleanup switches are
independent. With `--dump-stages DIR`, each function gets
`DIR/functions/<function>/05-thread-cleaned.js` before its own stages run.

The driver is `cleanup_thread_impl` in
[`pipeline/thread.rs`](../../src/transforms/emitted_js/pipeline/thread.rs).
After `vm_sub_*` and runtime-helper expansion it runs seven phases in order:
`expression_sweep` (the main fixed-point sweep, then loops), `storage_sweep`,
`late_propagation`, `branch_cells`, `literal_relocation`, `recycling`, and
`reprint`.

## First transformation group

The pipeline first expands straight-line `vm_sub_*` helpers, including nested
helper calls, and removes their declarations once all uses are expanded.
Arguments are captured once, left-to-right, before the body runs; parameters and
locals receive fresh names. This preserves snapshots when a later argument
changes a `$KEY` used by an earlier argument. Embedded calls use comma expressions
at their original evaluation point, preserving branches, loops, and detached
method calls. Helpers exposing `this`, `arguments`, nested closures, dynamic
scope, recursion, or unsupported body shapes are retained and counted.

Up to twenty-four rounds simplify exact generated property writes and fresh array
construction, remove dead carries, coalesce carry relays, merge equivalent
carries, fold branch joins, scalarize literal tuples, substitute literals and
stable `this` values, inline dependency-checked snapshots and aliases, reconstruct
adjacent expressions, forward SSA results directly into existing slots, and
remove dead SSA bindings. The repeat stops when no pass changes the source.
Adjacent reconstruction also removes single-use owned slot initializers when
that permits a whole call or assignment to recover its original evaluation
order. A dominating local read can precede a moved expression only when it is
initialized, uncaptured by writes, unaffected by that expression, and outside
mapped parameters and shared slots. Incoming slot assignment targets may
consume an adjacent RHS: creating their lexical reference does not read the
slot value, and store errors still occur after RHS evaluation.

Repeated local conditional tests such as `v ? next : v` become `v && next`
(and `v ? v : fallback` becomes `v || fallback`). The actual falsy value and
lazy branch evaluation survive; this often reduces two carry reads to one and
allows the carry itself to inline. Duplicate carry channels can also merge
across other nonthrowing carry copies that neither observe the old channels
nor overwrite their common source.
Unused local generated stores and declarations are removed using resolved
bindings, with shared cells, parameters and captures protected. Observable RHS
evaluations survive as expression statements. A later harmless overwrite can
kill an earlier store in the same straight-line block. For an uncaptured local
in a function without exception handlers, intervening calls may be crossed while
retaining their evaluation; otherwise potentially throwing evaluations stop the
proof. Control flow remains a barrier. Initialized local reads and strict comparisons
can disappear when their result is discarded, while unknown property reads,
calls and coercions retain their evaluation.
Each pass reparses its output. A final AST print removes redundant substitution parentheses while
preserving precedence, call receivers, and detached calls. Program mode prints
the final linked output too, so later passes cannot leave unnecessary wrappers.
After expression cleanup, a separate loop pass reconstructs recognizable
conditions, backedge updates, and post-loop continuations. A final bounded
lifetime cleanup eliminates local storage handoffs exposed by the recovered
control flow and removes newly dead cells. Storage reuse runs after expression
reconstruction so mutable cells do not obstruct snapshot and descriptor proofs.
Loop reconstruction itself runs once. Other legacy
transformation families remain separate work.

Property writes use the existing fresh-receiver program policy under standard
`Object.defineProperty` and built-in prototype behavior. The receiver must be a
proven fresh ordinary allocation and unobserved through its construction.
Direct assignments to static ordinary properties are allowed during that
construction, including assignments produced by earlier cleanup rounds. Reads,
RHS escapes, nested writes, and special/dynamic assignment keys still prevent
this proof.
Accessors, restricted descriptors, unknown/proxy receivers, `__proto__`, array
`length`, and observed call results retain their descriptor calls. Existing
strict `trace thread`/`corpus` policies are unchanged.

Fresh object and array construction also follows the intent of
`shape/src/transformations/poscontainer/rename-single-use-array-object-identifiers.js`.
Tuple and alias relays settle before lowering so the complete construction
remains recognizable. Exact data-descriptor writes become literal fields when the object has no
intervening observation or capture. Field snapshots keep their
evaluation points until adjacent-expression reconstruction can safely inline
them. Computed literal keys preserve `__proto__` as an ordinary own property.
The transform does not assume arbitrary property assignments are descriptor
writes.
Dense fresh arrays built by exact descriptors at indices `0..N-1` become array
literals, with each original field evaluation captured once in order. Gaps,
dynamic indices, observations and captures decline this reconstruction.

Fresh helper tuples with 2–16 elements scalarize when their only uses are the
immediate indexed projections. Comparisons, arithmetic and calls remain
evaluated once in element order. Captured projection bindings and projection
reads during construction decline, preserving TDZ and snapshot behavior.
For example, `[snapshot === void 0, snapshot]` loses its allocation without
repeating the property read that produced the snapshot. Number conversions of
proven Number-valued SSA results disappear; the first conversion of an unknown
input remains. Counted loops with a numeric initial value and no other counter
writes can use `counter++`/`counter--`. These follow the legacy thread update
transform with explicit type and binding checks, retaining unknown coercions
and BigInt conversions.

Method restoration follows the legacy `replace-apply-funcs` intent while checking
resolved receiver identity. `obj.method.call(obj, args)` becomes
`obj.method(args)`, and a dense literal `.apply` argument list can become normal
arguments. This readable VM-op rewrite assumes ordinary native call/apply
lookup behavior; visible call/apply customization and dynamic scope disable it.
Captured receiver writes, mapped parameters, mismatched receivers, and a saved
method lookup separated from invocation by effects keep the explicit call.
Hoisted function declarations do not block adjacent expression reconstruction;
Annex-B block declarations remain barriers.
Under the same native-call policy, saved receiver and method captures can fold
into nested calls when their original getter and argument evaluation order is
preserved. Intervening slot writes remain barriers.

## Thread scope

Incoming `$KEY` cells remain references to the creator's bindings. Local cells
keep their declarations, including cells exported to child threads. Exporting a
cell does not turn it into a free variable in its owning thread. Cleanup neither
declares an inherited cell nor invents a replacement `$KEY` name.

Before linking, child function bodies are empty markers, so their captures are
not visible to AST analysis. Program mode supplies the union of incoming and
child-exported cells through `cleanup_thread_with_options` and
`ThreadCleanupOptions::shared_slots`; these cells are protected from rewrites
that require exclusive local ownership. Scope checks reject inherited-cell
declarations and missing exported-cell declarations before and after cleanup.
Standalone `cleanup_thread` conservatively protects all declared numeric slots
when it encounters empty child markers without tracer metadata.

## Structured loops

Generated labeled `for (;;)` regions become `while (condition)`, `do…while`, or
counted `for (initializer; condition; update)` loops when their backedges and exits can
be recovered. The pass handles reversed branches, early-return search loops,
try/catch backedges, and nested loops from the inside out. False-condition continuations move after
the loop while retaining their original block scope. Unnecessary terminal
continues and labels disappear; jumps from nested loops keep their targets.

Condition-prefix SSA declarations may become assignments in the condition,
with declarations immediately outside the loop. This preserves getter order
and snapshots, including values needed by the exit continuation. Captured
per-iteration cells, TDZ dependencies, and dynamic scope prevent lifting. A
counter update moves into a `for` header only if no other continue can bypass
it and it does not depend on a body-local binding. Initializer movement checks
scope, closure capture, shared cells, later uses, and newly lifted name shadows.
Existing variable names remain unchanged.

Shared search continuations execute after either success or exhaustion. The
original exhaustion writes execute only on the false condition; an early
successful iteration breaks to the common continuation. Uninitialized local
join cells reset on each iteration. Process-before-test loops use `do…while`,
retaining ordered prefix statements in the body and conditional backedge work
at the test. Branch-local declarations lift only after capture, const-write,
TDZ, and name-resolution checks. Class instance field initializers count as
captures because they can read a cell after the iteration has ended.

Loops with body breaks that must skip the false-condition continuation remain
in their original form. The pass does not guess a new condition for arbitrary
control flow. Runtime regressions exercise condition getters, nested targets,
early returns, `try/finally`, closure cells, post-loop values, and TDZ errors.
The [remaining-loop audit](remaining-loop-audit.md) records the individual
rewrites for all 29 loops left after the first reconstruction group.

A computed snapshot records the versions of every lexical binding read by its
initializer. A later assignment or update changes the binding's version, even
if it happens to assign the same value again. The snapshot may move to its
single use only if all dependency versions still match and each name resolves
to the same symbol at that use. Writes inside the consuming statement also
prevent substitution.

```js
let $KEY_1 = 10;
let $SSA_1 = $KEY_1 + 2;
$KEY_1 = 20;
return [$SSA_1, $KEY_1]; // must still return [12, 20]
```

The first implementation is deliberately local. Calls, member accesses, and
control-flow boundaries end computed-snapshot regions. Dependencies must be
initialized lexical locals in that region, and captured writes reject them.
Arithmetic requires primitive type facts from preceding declarations or
assignments; unknown objects, Symbols, BigInt arithmetic, getters, calls, and
allocations are not guessed safe. Parameters and incoming/free `$KEY` bindings
are not assigned invented primitive types. The existing adjacent pass can
still move effectful expressions when it proves the exact evaluation order.

Binding-reference contexts need separate handling: `delete $SSA` must not
become deletion of an expression value, and shorthand properties must keep
their keys. Parameter aliases are retained when their owning function exposes
`arguments`, because sloppy-mode mapped arguments can mutate them indirectly.
Ordinary `var` locals are not mapped parameters. Repeated `var` initializers
are tracked as writes even when Oxc does not represent them as identifier writes.

## Dense array construction and layout

Contiguous writes to a fresh dense array can reconstruct its literal even when
it starts with real values such as `[0]`, rather than only `void 0` seeds.
Writes must proceed through consecutive numeric indices without gaps, and the
initial elements cannot contain holes or spread. Effectful appends preserve all
existing element evaluations and then each RHS in order. Overwriting existing
indices requires inert original elements, so their effects cannot disappear.
Getters, calls and saved lexical values may become literal elements only when
the receiver has no captures or observations during construction; pending child
markers require the tracing path's checked capture contract. Self-references,
early escapes and reassignment retain the construction. Direct function-body
function declarations can be crossed because they are hoisted; block function
declarations retain their Annex B evaluation position. Existing
own indices can be overwritten without a prototype assumption; appended indices
use the same ordinary built-in prototype policy as thread property cleanup.
Visible native prototype/constructor mutations or unproven prototype escapes
retain append writes. Native aliases are tracked as unions across assignments;
reassigning a VM local is not itself a global mutation. Recognized native
inspection calls remain in place and do not falsely pin an unobserved array.

Later single-use literal cleanup can substitute the completed array into its
final assignment, preserving one allocation per value and keeping shared or
reused identities in a binding. No replacement variable names are introduced.

Final thread and linked-program output place array literals on one line.
Only whitespace in array spans changes; string, regex, template and comment
contents are protected. Mandatory newlines inside raw template literals or line
comments remain. Reparsed output must reproduce the same canonical AST print,
protecting ASI and function-valued elements. Objects outside arrays keep their
normal layout. Formatting runs after the transformation-stage dumps.

## Flattening continuation blocks

After loop reconstruction, standalone blocks in statement lists can flatten into
that same list. Loop bodies, branch arms, catch/finally bodies and labeled jump
targets retain their structural braces. This removes nested post-loop
continuation wrappers after the existing lifetime cleanup, then runs another
bounded copy/snapshot cleanup. Keeping the lifetime proofs in their original
scopes avoids losing storage-reuse opportunities when scopes are widened.

The pass replaces only brace bytes while checking the proposed result. Every
remaining AST node must retain its kind, span, and structural depth after block
nodes are ignored. Every identifier reference must resolve to the same original
declaration, and directive prologues and semantic validity must match. This
protects shadowing, TDZ, closures, duplicate declarations, and ASI boundaries;
`with` and `eval` disable the pass. Blocks directly declaring functions or
`using` resources retain their hoisting/disposal scope. If the combined lexical
lift fails, one bounded retry considers only blocks without direct lexical
bindings. Required scopes are retained without renaming any binding.

The final rounds also revisit literal, alias, snapshot, and local-value cleanup
so a removed wrapper no longer prevents an otherwise safe SSA/carry rewrite.
Numeric prefixes can commute with a single-use initializer when all inputs are
initialized, private, and stable across that initializer. Literal-number locals
whose only writes are `++`/`--` stay numeric; a `for` initializer dominates its
test, body and update. Unknown assignments, captured writes, RHS mutations,
BigInt and object coercions retain their snapshots. This lets an extracted
method/value pair become `receiver.method(i / 9, colors[i % 3])` without moving
a getter past another effect or changing the receiver. Method restoration runs
again on the newly adjacent shapes.

Numeric prefix facts also recognize stored results that are necessarily
Numbers after successful lexical initialization: unary `+`, unsigned shifts,
arithmetic/bitwise operations with a Number operand, and Number-valued
conditional/sequence expressions. Facts propagate through owned locals whose
only later writes are `++`/`--`. Hoisted `var` initializers are excluded because
their initializer may have been skipped. This proves the type of the stored
result, never that the initializer itself is safe to move or repeat.

Emitted `Number(value)` conversions share the existing native-Number guard
with numeric coercion cleanup. When that guard holds, looking up the native
constructor is an inert prefix, allowing an immediately preceding single-use
snapshot to move into its argument. The conversion itself still ends the safe
prefix: getters, coercion hooks and throws retain their order. Shadowed,
redefined, or accessor-backed Number lookups receive no native fact.

Single use alone does not permit moving a call past another call: in
`let saved = encode(); let out = target(values.join(""), array, saved);`,
`encode()` must still run before reading `target` and invoking `join`. The saved
binding remains unless those earlier evaluations can be proved safe to cross.

## Null-test channels and optional syntax

Some diamonds carry a value and a parallel boolean meaning `null == value`.
The per-thread pass symbolically follows aliases, stores and branch null facts
on every normal path. A flag can disappear only when that relation holds on
every path, its value channel has no later/captured writes, and no read observes
the flag before the handoff. The original value stores, getters and calls stay
in order. Enclosing exception handlers in the same execution owner, unsupported
jumps, dynamic scope, disposal, exposed cells and incomplete proofs decline.
The proof also runs after scope flattening, where some paired channels first
become visible. No replacement names are created.

Equivalent strict null-and-undefined guards can become `?.` property reads or
`??` defaults. Calls through a selected method remain detached, deletion does
not acquire a property reference, and optional keys/arguments remain conditional.
Loose `== null` checks are retained unless their operand is proved not to be
HTMLDDA: browser `document.all` compares equal to null but is not nullish for
optional chaining. Null-flag elimination itself preserves that loose comparison.

Discarded numeric argument reads are removed at the end of per-thread cleanup
under the existing ordinary-built-ins policy. An AST escape check follows each
function's own arguments object and inherited arrow uses. Child regular
functions, strings and comments no longer pin the parent's reads. Shadowing,
escaping, deletion, dynamic scope, or calls/tags that pass the arguments object
as a receiver retain the read.

## Remaining value storage

Removing an unread SSA binding may leave its initializer as an expression
statement: unknown property reads can invoke getters or throw, and calls still
execute even when their result is unused. A thread-only discarded-value pass
removes literal-only pure expressions and a finite set of known native reads
under the standard VM host/built-in policy. This includes `host.Object.keys`,
`host.Array.isArray`, and an empty literal's native `hasOwnProperty` lookup.
Live calls and feature checks remain. Visible intrinsic writes, descriptor or
prototype customization (including recognized aliases), dynamic scope, and
shadowed hosts prevent the corresponding native-read removal. Initialized
local `$KEY`/`$CARRY` value-only reads are also removed conservatively.

Inlining cannot erase every value: a getter used twice must execute once, and
loop state must survive the backedge. Such values retain their original `$SSA`
or `$CARRY` bindings. Cleanup never invents `$KEY` bindings to conceal remaining
temporaries. Reductions count actual expression/copy elimination.

For example, `$THREAD_7_56719` allocates `[null]`, passes that array to one
call, then passes the same array to another call. It has no private key for
that value. Repeating the allocation changes identity and loses any mutation
performed by the first call. One local binding remains necessary under the
current output model; changing its name would not eliminate storage.

An SSA result can also reuse an existing `$KEY` destination when the destination
is unobserved before its copy and stays unchanged through the final SSA read.
Branch-local copies require the old slot's reads to remain dominated by that
copy. A carry can reuse an incoming slot only when its original value is no
longer observed, including by closures. Mapped parameters are excluded.

## Local values and the byte-extraction audit

`local_values.rs` assigns expression identities using operators, literals,
resolved symbols, and the reaching version of each stored input. An existing
local result can replace the same pure expression while that result remains
available. Stores invalidate values, including writes inside initializers,
arguments and destructuring. Type facts describe successful evaluation and
never imply that the original initializer can be dropped or repeated.

Immutable local type facts survive calls and branches when the declaration
dominates the read. Single-use pure initializers can be substituted beyond the
next statement when their dependencies still have the captured store versions.
Mutable private dependencies qualify within a straight-line region; branches,
destructuring and direct stores invalidate their versions, and captured writes
prevent the proof. Every substituted dependency must still resolve to the same
binding. Shorthand property keys, binding deletion, lexical initialization and
directive boundaries remain protected. Computed results used more than once
remain cached rather than expanding repeated arithmetic.

When every observation of an immutable local applies the same pure consumer,
that consumer can be fused into the original producer. Each consumer must read
the producer once, and no other observations, captures, or writes may remain.
This applies to arithmetic, comparisons, masks, shifts, truth tests and other
classified expressions, without matching a particular coercion or generated
name. The producer's calls and allocations still execute once. Unknown getters,
coercion hooks, TDZ reads and throwing operations cannot be reused this way.

Owned local cells qualify for adjacent substitution one stored
version at a time. The version must have exactly one read before its next
unconditional store, with no captured reads, redeclarations, self-dependent
updates, or later observations outside the current statement list. Earlier reads
in an enclosing block do not observe a future store; loop backedges can, so an
earlier read in a loop prevents this substitution. Existing evaluation
order checks still apply. Hoisted declarations remain until all uses disappear;
incoming and exported slots remain protected. Harmless overwritten initializers
can be dropped only for uncaptured locals in functions without exception handlers.
These proofs use binding ownership and lifetime, regardless of whether a cell
is named `$SSA_*`, `$CARRY_*`, `$KEY_*`, or has an ordinary local name. Function
and class name inference, directive boundaries and lexical declarations remain
observable and are preserved. No new names are introduced.

Empty lexical declarations need not interrupt expression reconstruction when
all references occur after initialization in the same function and class region.
Any earlier reference or capture retains that boundary. Ordinary initialized
`this` reads can precede a substituted RHS; class contexts keep the barrier so
derived-constructor `this` errors still occur in their original order.

Initialized private declarations can also be crossed when their initializers
are nonthrowing and independent of the moved evaluation. The skipped cell must
be uncaptured and absent from the moved initializer, including TDZ reads. Its
inputs must stay unchanged throughout that evaluation. This handles staging
declarations without commuting getters, calls, coercions, or shared reads.
Constructor arguments use the same ordered-prefix proof as ordinary calls;
constructor lookups, argument effects, spreads, and constructor changes retain
their original order. Every selected destination is pinned for that rewrite
round to prevent intersecting substitutions across skipped declarations.

Duplicate lexical-input snapshots taken before any intervening effects can
share the first snapshot. Stores and control flow clear the available values,
as do getters, calls, unknown global reads, and other effectful initializers.
Incoming VM slots qualify as lexical reads through tracer metadata. This does
not assume that those slots remain unchanged after the snapshots are taken.

Literal-only primitive computations fold with ECMAScript numeric semantics
before substitution. Signed zero, infinities, and NaN use forms unaffected by
shadowed global names. BigInt computations, lone UTF-16 surrogates and template
strings remain outside this evaluator. Small literal-only values may propagate
at multiple reads; computations over stored inputs retain their caches.

Backward liveness starts from observable evaluations and follows dependencies
between private cells. A cycle of copies does not keep itself alive: loop
bookkeeping whose values never reach a live use can disappear together. Calls,
getters, possibly throwing reads and coercions remain evaluated even when their
destination is dead. Captured and shared cells remain roots. Empty branch
bodies left by removed stores retain their condition's evaluation.

Loop copy-in/copy-out cells can share the original cell when their live ranges
do not interfere. For `let next = value; ...; value = next;`, updates to `next`
can update `value` directly only if no subsequent read needs the old value.
The proof excludes captures, shadowing, unsupported writes, nested loops,
early exits before copy-back, and enclosing exception handlers that could
observe an early update. Multiple independent tail copies qualify separately.
This removes actual declarations and transfers while retaining the original
cell's name; it does not introduce replacement names.
The same proof handles branch-local construction followed by a result handoff:
`let next = expression; ...; value = next;` initializes `value` at the original
expression's evaluation point. The former value must be unobserved throughout
that interval, and `next` must have no uses after the copy. Anonymous function
and class initializers retain their binding because changing the destination
would change their inferred name.

A private join used only by a following return or copy into an existing
lexical destination can instead hand off from each branch. Every fallthrough
path must supply the value, and the removed stores must target an initialized,
mutable private cell. Captures, additional reads, shadowed destinations,
inferred function/class names, and resource-disposal scopes prevent this
rewrite. Already returning or throwing paths keep their abrupt completion.
Identical channel values can also move after the branch even when their stores
have different positions among the branch's other work. Every normal path must
supply the value; reads inside the branch, captures, later channel writes,
handlers, and unresolved child bodies block factoring. Inputs must be literals,
inert literal computations, initialized `this` in an ordinary function
declaration, or initialized local lexical bindings unchanged throughout the
branch. A later input write retains the snapshot when it is still needed.
This removes 14 further bindings and 38 lines in the 661-thread input: 5,447
SSA/carry bindings remain, with 24,492 lines and 1,455,178 bytes.
The return may also consume the join as the first evaluation of an expression,
such as a conditional test, unary operand, property receiver, or call callee.
Substitution preserves bare-call receivers and requires every remaining name
to resolve to the same binding inside each branch. Nested function/class
environments, earlier evaluations, and additional join reads retain the cell.
Branch facts can eliminate a repeated test over immutable lexical bindings,
including strict comparisons and null comparisons. They stop at new execution
environments and retain discarded branches containing hoisted declarations.
Substituted values preserve bare-call receivers and inferred function names.
For parallel channels, an earlier copy can disappear only when its RHS is a
literal-only value or an initialized immutable lexical snapshot. The handoff
stays after the branch's remaining work; later writes to the channel decline.
Adjacent expression reconstruction then consumes newly exposed single-use
values, without rerunning descriptor lowering over reused mutable storage.

Identical expression suffixes in both branch arms can execute once after the
branch. Each referenced binding must resolve identically outside the arms;
branch-local captures, dynamic scope, resource disposal and comments decline.
Labels and single-statement contexts retain a block around the branch and its
suffix so abrupt completion still skips the same work. A private uninitialized
local with exactly one later unconditional store can initialize at that store
instead, exposing its immutable value to existing inlining passes. Earlier
reads, captures, other writes and externally observed slots prevent promotion.
Thread branch-value folding also accepts expression prefixes, preserving their
order and lazy arm selection with comma expressions. Function/class RHS values
retain their assignment form when inferred names could be observed.

Copy-in lifetime coalescing handles the other end of a carry's lifetime: an
immutable snapshot copied into an existing private local can initialize that
local directly. The initializer stays at its original evaluation point. Every
snapshot read must finish before the first statement that may update the
destination; a read inside an updating loop therefore retains its snapshot.
Both declarations must belong to the same statement list. Earlier destination
observations, captures, shadowing, shorthand keys, shared slots and inferred
function/class names prevent the rewrite. Intervening effects can stay in
place because they neither move the initializer nor observe the private cell.
For example, `let snapshot = object.length; let state = snapshot; use(snapshot);`
can initialize `state` at the original property read and call `use(state)` when
all later snapshot reads finish before a state update. The property is still
read exactly once, and no new binding name is introduced.

Assignment diamonds can also normalize independently of the destination's
declaration: `if (test) { cell = yes; } else { cell = no; }` becomes
`cell = test ? yes : no`. This requires one resolved lexical destination and
expression-only arms (nested complete diamonds and expression prefixes are
allowed). Forming that lexical LHS reference before the test neither reads
nor writes the cell; the selected RHS and final store retain their order.
Globals, branch declarations, abrupt statements, comments, dynamic scope and
direct function/class RHS values decline. Existing short-circuit folding can
then consume repeated test snapshots without evaluating getters or coercions
twice. Old-value observations and captures still prevent promoting a mutable
destination to an immutable initializer.

Single-use array, plain object and regular-expression allocations may move to
their only use after expression/descriptor reconstruction has finished. Their
construction must be inert: literal data and initialized, immutable lexical
snapshots qualify; getters, calls, spreads, computed keys, methods, closures,
mutable slots and parameters do not. Dependency names must resolve to the same
bindings at the use. The allocation and use must have the same function/class
owner and innermost loop, preserving identity across iterations. Captures,
multiple uses, shorthand/delete contexts, dynamic scope and not-yet-linked child
thread placeholders retain storage. An empty statement preserves a directive
barrier when removing a declaration could expose a later string expression.

The program tracing path can provide a stronger, checked capture contract.
`ProgramFacts` lists inherited and exported numeric cells, and the linker scope
check rejects child free generated names outside those `$KEY` cells. Once those
facts are validated, fresh generated SSA/carry/array/object temporaries can be
inlined despite pending child markers. Shared slots, unstable dependencies,
multiple uses, captures already present in the AST and loop identity checks
remain protected. Standalone cleanup and its public options retain the broader
placeholder guard; arbitrary local names retain it in both paths.

Parenthesized extracted callees receive the same bare-call receiver protection
as direct identifier callees, including a parenthesized member initializer.

On the 661-thread input used here, adding backward liveness and final local
handoff coalescing reduced the linked output from 6,343 SSA/carry declarations
to 5,994 (4,538 SSA and 1,456 carry), and from 26,872 to 26,132 lines. These
are parsed declaration counts, not identifier occurrences or renamed bindings.
The subsequent ordered-expression, duplicate-snapshot and constant-value
passes reduced that output further to 5,680 declarations (4,230 SSA and 1,450
carry), 25,762 lines, and 1,516,082 bytes. The release trace took 1.12 seconds
in the measured run. This is a further reduction of 314 declarations and 370
lines, without claiming that every remaining binding has been proven minimal.
The final branch handoff phase brought the output to 5,673 declarations
(4,248 SSA and 1,425 carry), 25,732 lines and 1,513,800 bytes; the measured
release trace took 1.07 seconds. Relative to the start of this continuation,
that eliminates 321 declarations and 400 lines. Required snapshots retain
their existing names; a raw zero count is not the optimization criterion.
The next common-suffix and single-store normalization step reduced the linked
output by another 30 bindings and 366 lines: 5,643 declarations (4,218 SSA and
1,425 carry), 25,366 lines and 1,497,988 bytes. Its measured release trace took
1.13 seconds. This step also preserves assignment-inferred names and earlier
undefined observations when branch values cannot safely become initializers.
Copy-in lifetime coalescing then removed another 69 SSA/carry bindings and 71
lines, bringing the linked output to 5,574 declarations (4,150 SSA and 1,424
carry), 25,295 lines and 1,492,866 bytes. The measured release trace took 1.11
seconds. These are actual eliminated declarations; required source snapshots
remain separate when their lifetimes overlap destination updates.
Assignment-diamond normalization removed another 69 declarations (24 SSA and
45 carry) and 594 lines. The linked output now has 5,505 declarations (4,126
SSA and 1,379 carry), 24,701 lines and 1,470,031 bytes. The measured release
trace took 1.11 seconds. Line reduction includes collapsed branch scaffolding;
the declaration reduction is counted separately from formatting changes.
Single-use data-allocation inlining subsequently removed 11 SSA declarations
and 66 lines: 5,494 SSA/carry declarations (4,115 SSA and 1,379 carry), 24,635
lines and 1,465,163 bytes. The measured release trace took 1.09 seconds.
An initially broader allocation proof exposed a deferred-child capture in the
regression suite; the final pass retains storage in inputs with unlinked child
placeholders, including arbitrary local names outside numeric slot metadata.
Using the program emitter's checked capture contract removed another 24 SSA
bindings and 96 lines: 5,470 SSA/carry declarations (4,091 SSA and 1,379 carry),
24,539 lines and 1,458,391 bytes; the measured release trace took 1.23 seconds.
Return-expression handoffs remove two further carry bindings in the crypto
helpers, leaving 5,468 SSA/carry declarations (4,091 SSA and 1,377 carry),
24,537 lines and 1,458,387 bytes. The measured release trace took 1.12 seconds.
The next round removes seven more bindings through branch facts, commuting
lexical reads, and moving single-use inert computations over immutable values.
The same 661-thread baseline now contains 5,461 bindings (4,084 SSA and 1,377
carry), 24,530 lines and 1,457,729 bytes.
Primitive types are proved from successful initialization; unknown objects,
Symbols, BigInt arithmetic, getters, mutable inputs and parameter mappings
retain their snapshots. Moving a plain read across another plain read needs
no assumption that either cell stays unchanged across a later callback.
Incoming VM-cell reads rely on the tracer's lexical-cell contract, and a
possibly uninitialized incoming read cannot cross another possibly throwing
read. Ordinary unknown globals remain barriers.
Standalone source cleanup retains its
conservative pending-child behavior.

The real `3_73940_d124/04-receiver-this.js` dump already repeated its `| 0`
expressions before cleanup. All six original `$KEY_3` reads initialize SSA
snapshots with zero references; its child `4_39492_d70` does not read `$KEY_3`.
The dedicated read-only audit checks this semantic def-use chain and requires
the cleaned thread to contain one `| 0` expression. Differential tests cover
signed 32-bit boundaries, slot changes, captured writes, redeclaration writes,
shadowing, and observable object-coercion hooks.

## Validation

```sh
cargo test --test thread_cleanup

# Read-only replay of individual functions from a real stage dump:
SHAPE_THREAD_CLEANUP_CORPUS=/path/to/stages/functions \
  cargo test --release --test thread_cleanup real_emitted_threads \
  -- --ignored --nocapture

SHAPE_THREAD_CLEANUP_CORPUS=/path/to/stages/functions \
  cargo test --test thread_cleanup verify_real_byte -- --ignored --nocapture

# Replay the tracing path's checked generated-name capture contract:
SHAPE_THREAD_CLEANUP_CORPUS=/path/to/stages/functions \
  cargo test --lib real_emitted_threads_with_checked_captures \
  -- --ignored --nocapture
```

The measurements below come from one local development input and are
historical checkpoints, not results for either tracked canonical reference.
Re-run the replay on your own input to reproduce counts and timings.

Synthetic differential cases execute only their own small test programs in
Node and compare original/cleaned results, including snapshot changes,
branches, loop iterations, callbacks, coercion, getters, and exception order.
The real-thread replay cleans each `04-receiver-this.js` separately, reparses
it, and checks that no newly unbound names appear. It reports aggregate bytes,
rewrite counts, and sequential cleanup time. It does not execute those real
threads or claim runtime equivalence from syntax/scope checks alone. It also
replays cleanup on eight workers (or fewer when fewer CPUs are available) and
requires identical output to the sequential run.

The cleanup investigation input also exercises helper expansion: its replay
requires zero remaining helper declarations and identical output across
workers. The replay prints current counts and timings; timings exclude tracing,
linking, file reads, and scope checks.


The continuation-block and numeric-prefix follow-up on the same input removes
455 standalone blocks and 38 SSA declarations: 5,409 total
SSA/carry declarations (4,040 SSA and 1,369 carry), 23,544 lines and 1,267,755
bytes. Nineteen additional method calls are restored. The 21 retained standalone
blocks keep scopes the pass cannot flatten under its current proof. All 661
threads complete and `--check-reuse` agrees; captured JavaScript is never executed.

The dense-array follow-up reconstructs 238 additional array constructions on
the same seed and removes 184 additional SSA declarations, leaving 5,225
SSA/carry declarations (3,856 SSA and 1,369 carry). Final compact-array output
contains 21,331 lines and 1,219,376 bytes. All 661 threads complete with zero
stops and `--check-reuse` agrees. One release measurement reports 1.42 seconds
internally and 1.74 seconds wall time including the reuse check. Construction
fold counts are separate from the number of eliminated declarations.

Stored numeric-result facts and guarded Number argument forwarding remove
147 more SSA declarations on the same seed: 5,078 total declarations (3,709
SSA and 1,369 carry), 21,183 lines and 1,210,522 bytes. Three more method calls
are restored. The original conversions remain ordered and no binding names
are introduced. All 661 threads complete with zero stops, reuse checking
agrees, and debug/release output is byte-identical. One release measurement
reports 1.37 seconds internally and 1.70 seconds wall time with reuse checking.

The paired-null-channel, effectful-array and discarded-argument follow-up
removes 85 more declarations (75 SSA and 10 carry), leaving 4,993 total
(3,634 SSA and 1,359 carry). Array reconstruction grows from 797 to 888 folds;
these are rewrite counts, not eliminated bindings. Discarded argument-member
statements fall from 94 to 2; the two retained reads occur in a function with
deletion, which the arguments escape policy conservatively declines. Final
output has 20,724 lines and 1,193,500 bytes. All 661 threads complete with zero
stops, reuse checking agrees, and debug/release bytes match. One release
measurement reports 1.37 seconds internally and 1.69 seconds wall time including
reuse checking. The 582 focused tests include differential getter/coercion,
partial-construction exceptions, nested arguments owners, optional-call receiver
binding, conditional keys, captured writes and cached-predicate cases.

Private array-cell snapshots are another per-thread group. A dense array literal
or guarded native zero-argument `slice()` copy has own data cells at its initial
indices. If the binding never escapes, is never captured, and only accesses
those cells, snapshots of an unchanged cell can inline into multiple reads.
For example, two snapshots used in both halves of a rotate become reads of the
original private cells. Arithmetic and coercion expressions retain their
original positions and evaluation counts. The final assignment writes its cell
after evaluating its RHS, so reads on that RHS can still use the previous cell.

The proof rejects sparse/unknown arrays, structural writes, aliases, method/tag
receivers, captures, dynamic scope, cross-iteration uses and intervening writes
to the cached cell. Writes outside the initial own cells also decline: an
inherited setter could expose the array as `this`. Native slice copies share
the existing standard-intrinsic policy and mutation/escape guard used by dense
array construction; visible customized slice/species behavior disables it.
Only SSA/CARRY declarations are candidates, protected captures are excluded,
and replacements must shorten the output. Necessary names remain unchanged.

On the same seed this group eliminates 379 more SSA declarations, leaving
4,614 total SSA/CARRY declarations (3,255 SSA and 1,359 carry). The output has
20,345 lines and 1,181,212 bytes. All 661 functions trace with zero stops,
reuse checking agrees, and debug/release output is byte-identical. The 589
focused tests pass, including cell mutation, captured/aliased arrays, getters,
coercion counts, loop versions, inherited setters and customized slice/species.
One release run reports 1.98 seconds internally and 2.38 seconds wall time
including reuse checking; this adds analysis work compared with the preceding
1.37/1.69-second measurement and has not received a performance tuning pass.

The final expression cleanup also consumes snapshots of unchanged numeric
`arguments[index]` inputs. It shares the arguments-ownership analysis with the
discarded-read pass, but requires a stronger proof: no formal parameters (and
therefore no sloppy mapped-parameter writes), index writes/updates/destructuring,
deletion, shadowing, aliases, escaping arguments objects, or argument-method/tag
receivers. Arrows are inspected for mutation of the enclosing arguments object;
regular child functions own independent arguments objects. A substituted use
must stay in the same regular-function owner and lexical initialization range.

Input reads may cross loops and calls because that object remains unchanged;
property reads of the input values themselves are not duplicated. Extracted
function values keep detached-call/tag semantics. This uses the existing
ordinary-built-in policy for missing numeric indices, guarded against visible
prototype mutation/escape. Custom accessors, dynamic scope, captured snapshots,
comments, and unsupported cases retain their original bindings. No new names
are generated, replacement size must shrink, and directive barriers survive.

On the same seed this removes 136 further declarations (69 SSA and 67 carry),
leaving 4,478 total (3,186 SSA and 1,292 carry), 20,209 lines and 1,171,200 bytes.
It also exposes two more method calls to existing receiver reconstruction.
The 594 focused tests pass; all 661 functions trace with zero stops and reuse
checking agrees. Debug/release bytes match. One release measurement reports
2.13 seconds internally and 2.49 seconds wall time including reuse checking.

Primitive local cells can also keep type facts across branch joins and loop
backedges. The proof begins at lexical initialization (including an empty
`let`'s undefined value) and checks every store: the value must remain a
Number, Boolean, null or undefined. This category supports arithmetic without
user coercion hooks or BigInt/Symbol errors. Captured writes, shared slots,
strings/objects, unsupported stores, and skipped hoisted initializers decline.
Effectful initializers and conversions retain their original evaluations.

Value numbering seeds a fresh version after each control-flow join and on
entry to a nested block. Equal types never imply equal values: snapshots taken
before a store or join cannot reuse that new version. Immutable cells retain
their original version so existing shared computations remain cached. This
allows pure single-use calculations over joined cells to inline across
unrelated calls while retaining snapshots across actual cell writes.

This group has a small effect on the seed: nine fewer declarations (seven SSA
and two carry), leaving 4,469 total (3,179 SSA and 1,290 carry), 20,177 lines
and 1,170,721 bytes. The 601 focused tests pass, all 661 functions trace with
zero stops, reuse checking agrees, and debug/release bytes match. A release
run after the tests completed reports 1.51 seconds internally and 1.57 seconds
wall time including reuse checking; a run concurrent with tests took 7.88
seconds wall time, illustrating that these timings depend on machine load.

Private branch handoffs now also support an immediately following expression
statement or generated local initializer. Every falling-through branch must
provide the value; other consumer names must retain their binding identity in
the branch. Moving a value past an earlier consumer evaluation requires a
stable, inert RHS. Lifted destinations retain their existing SSA/CARRY names
and decline TDZ reads, captures, parameter redeclarations and shared cells.
Extracted callees, tags, and unresolved `typeof` operands preserve GetValue.

Fresh inert values may have multiple syntax references when each pair occurs
in opposite arms of a common `if` or conditional expression. At most one use
can execute. Same-owner, same-loop and same-class checks preserve allocation
identity; mutable and hoisted variable dependencies decline. This extends the
single-use proof without duplicating observable reads or computations.

Before string folding changes, these two groups remove 69 declarations net
from the previous 4,469 baseline, leaving 4,400 (3,191 SSA and 1,209 CARRY).
The focused suite passes 608 tests, with nine ignored inventory/slow tests.
All 661 seed functions trace with zero stops and reuse checking agrees.

The emitted-string evaluator also stops applying its cache-key length bound
to compute-only evaluation. Input, decoded-byte and output bounds still apply;
normal cached evaluations retain their cache-key bound. Regression tests cover
both modes and folding a long string through a compiled plan.

Thread emission retries declined string calls after helper expansion and
snapshot propagation, then cleans again only if that retry folds a call.
This removes the seed's last two decoders, whose literal arguments were hidden
until thread cleanup. Along with the cache-key fix, all six decoder calls and
all references to vmDecodeString, vmStringTable, vmStringCache and
vmDecodeBase64 disappear. Final output has 4,395 SSA/CARRY declarations
(3,186 SSA and 1,209 CARRY), 74 fewer than the 4,469 baseline; it has 20,079
lines and 1,156,344 bytes. The decoder changes expose four additional property
simplifications. Separate enumeration and worker helpers remain.

The emitter's three operation helpers now have a separate per-thread inliner:
`vmForInKeys`, `vmPostToWorker`, and `vmInheritOrObject`. It compares each
function's normalized syntax with the emitter's exact helper definition and
checks binding identity, strictness, shadowing, writes, redeclarations and
escaped references. Unsupported sites retain their helper. Arguments,
including unused extra arguments, evaluate left to right before helper work.
A destination declaration remains uninitialized until that work completes.

Enumeration expands to a separate array-building `for ... in` loop. It retains
inherited enumerable keys and snapshots the key list before caller iteration;
it does not substitute Object.keys or combine the two loops. Worker posting
expands to initialization followed by callback registration and postMessage,
with its original Boolean result and failure order. Prototype construction
retains its typeof checks and Object.create behavior.

On the seed, 19 helper call sites expand and 13 helper declarations disappear
before linking. Final linked output has no `function vm*` declarations or
references to the three operation helpers or the decoder/table/cache helpers.
The inline enumeration loops introduce necessary loop-key locals; total
SSA/CARRY declarations become 4,416 (3,207 SSA and 1,209 CARRY), still 53 fewer
than the starting 4,469. The focused suite passes 624 tests with nine ignored.
Original runtime dependencies remain: vmCreateThread dynamically dispatches
worker messages, vmEntry and vmNativeFunctions are exported runtime values,
and worker initialization/state still require their runtime implementations.
These are not erased or renamed to conceal the remaining dependency.

Snapshot fusion now crosses independent function declarations inside blocks,
not just hoisted declarations at function-body level. Strict block functions
initialize at block entry. Sloppy block functions can also assign an outer
Annex-B alias: moving a call/getter requires proving that every read of the
function name resolves to its lexical binding, so that outer assignment is
unobservable. If the outer alias is observed, only a proven initialized plain
lexical read with a different source/destination name can cross. Potentially
uninitialized incoming cells, function-name collisions, captured snapshots,
getters that observe the outer alias, and other dependency barriers remain.

This group removes 215 SSA declarations from the 4,416-binding output, leaving
4,201 (2,992 SSA and 1,209 CARRY). It exposes 93 more method calls to receiver
reconstruction. All 661 functions trace with zero stops and reuse checking
passes. The focused suite passes 627 tests, with nine ignored inventory/slow
tests; the added cases cover Annex-B observations, strict blocks, TDZ failures,
captures, getter order and detached receiver behavior.

Final debug/release output bytes agree: 19,809 lines, 1,141,121 bytes. The
release run reports 1.67 seconds internally and 1.99 seconds wall time,
including reuse checking. All 627 focused tests pass on the final source;
all-target/all-feature Clippy completes with repository warnings.

Discarded argument reads and empty else blocks also participate in the final
propagation fixed point. Removing a read at the end of cleanup can otherwise
leave a newly adjacent temporary/consumer pair behind. This scheduling fix
removes four more declarations on the seed (4,201 to 4,197); getter-bearing or
escaped arguments retain their observable reads. The dedicated behavior test
covers both the removable read and a getter that must still execute.

Private storage handoffs now consider nonterminal copy-out assignments in
function bodies as well as blocks. The destination must remain unobserved
between constructing the source and committing the copy; later source reads
can share its storage only until the next destination-update statement.
Implicit var initialization and Annex-B function writes count as writes too.
Control-flow hazards are checked within the moved interval rather than
rejecting unrelated loops or exits elsewhere in the body.

Exception observers are checked separately. A private lexical destination
inside the active try/catch arm can be reused. An outer destination requires
an exiting catch that cannot read it; a finally observer remains a barrier.
Nested exception regions, captures, source updates, TDZ and anonymous-function
name inference retain their guards. This removes 94 bindings net from the
4,197-binding seed output, leaving 4,103 before the next scheduling batch.

The final propagation fixed point also retries three existing proof groups:
private storage handoffs, single-use fresh values and private array-cell
reads. Block flattening and later snapshot removal can expose opportunities
after their original phase has finished. Each retry rebuilds semantic facts
and retains the same identity, scope, capture and cell-version requirements.
These three scheduling changes are verified together rather than rebuilding
and retracing after each individual change.

The scheduling batch removes four additional SSA declarations on this seed:
4,099 remain (2,889 SSA and 1,210 CARRY), 98 fewer net than the starting 4,197.
All 661 functions trace with zero stops and reuse checking passes. Debug and
release output bytes match: 19,671 lines and 1,134,867 bytes. The release run
takes 1.72 seconds wall time including reuse checking (a single run, not a
controlled speed comparison). The combined focused suite passes 784 tests,
with nine ignored; all-target/all-feature Clippy completes with existing
repository warnings.

The next batch broadens three per-thread proofs. Small identity-free inert
computations may be substituted at several reads, under the existing eight
reference and byte-growth limits. Allocations still require mutually
exclusive reads, preserving array/object/regular-expression identity. Calls,
getters and potentially observable coercions cannot be repeated.

Inputs to an inert expression may now be mutable lexical cells when their
version is unchanged between the original read and every replacement site.
Initialization, binding resolution and loop-instance checks still apply;
captured writers, intervening stores (including consumer-prefix stores),
shared slots and hoisted var initializers remain barriers. Arithmetic on a
mutable input additionally requires a primitive category preserved by every
successful store, rather than inferring its type from the first initializer.

Finally, the ordered adjacent-use proof handles copy assignments inside comma
expressions. It requires an initialized private cell with exactly one store
and one unconditional ordered read in the next operand. Loop-carried values,
captures, const/TDZ failures, inferred function names and conditional or
effectful prefixes retain their bindings. A discarded zero operand preserves
the comma expression's detached-callee semantics and original parentheses.

The three-pattern batch removes 52 bindings net on the seed: 4,099 to 4,047
(2,842 SSA and 1,205 CARRY), with 52 fewer lines and 1,839 fewer bytes. The
final program has 19,619 lines and 1,133,028 bytes. All 661 functions trace
with zero stops, reuse checking passes, and debug/release output bytes agree.
The combined focused suite passes 789 tests with nine ignored. Clippy
completes with existing repository warnings and no warnings in the changed
cleanup modules. One release run takes 1.75 seconds wall time including reuse
checking; this is not a controlled performance comparison.

The following three-pattern batch admits stable function-scoped var inputs,
computed expressions over unchanged numeric arguments reads, and literal
lengths of closed dense arrays. Var inputs require a unique declaration and
explicit checks for the initializer's implicit store, captured writes and
same-spelled block functions. The related local-value proof also excludes
possible Annex-B outer-var writes. Argument expressions reuse the existing
parameterless, unescaped, unmodified arguments proof and ordinary-prototype
guard; arithmetic over an unknown argument value remains effectful. Array
lengths require a literal origin and no escapes, length/structural writes or
out-of-bounds stores. Slice origins do not establish an exact length under
arbitrary species construction. Directive barriers remain in place.

This batch removes three more CARRY declarations: 4,047 to 4,044 (2,842 SSA
and 1,202 CARRY). The gain on this seed is small despite the broader proofs.

Sequential index reconstruction runs after propagation. It recognizes
successive left-associated `counter + 1` chains in indexed assignments and
their final return or handoff. The existing generated local becomes a prefix
counter; every read must be accounted for, numeric category must be proven,
and captures, dynamic scope, other stores and reads needing the old snapshot
decline the rewrite. Increments remain at the original key evaluation sites.
This retains repeated floating-point rounding and does not reassociate sums
to `counter + N`. The initial Number conversion is preserved.

The seed has 24 reconstructed prefix updates across nine private counter
bindings, including both 67,108,864 branches of thread 1/109539. A separate
unoutlined trace of PC 109539, state 1, descriptor 372 confirms the helper at
PC 166808 converts the old input with Number, adds one and returns the new
result used by the indexed store. This is prefix value flow; the VM helper
itself uses explicit conversion and addition.

Final output has 19,616 lines and 1,132,663 bytes; debug/release bytes match.
All 661 functions trace with zero stops and reuse checks pass. The focused
suite passes 796 tests, with nine ignored, including ordered proxy writes,
throwing callbacks, large-number rounding, nonnumeric inputs and captured or
otherwise observed snapshots. One release run takes 1.79 seconds wall time
including reuse checks; no controlled speed comparison is claimed.

Three subsequent proof extensions handle independent private stores,
branch-spanning aliases and adjacent var copy-in destinations. The ordered
producer proof can skip a private initialized mutable assignment only when
its RHS is inert, no captured observer or producer dependency mentions its
destination, and initialization/redeclaration checks pass. Property reads,
calls, coercion hooks, const/TDZ stores and shared cells remain barriers.
Throwing producers retain their order across stores within exception regions
so a catch/finally (or a continuation after a handled throw) cannot observe
an advanced value. This guard also protects skipped hoisted var initializers.

Private alias reads can cross nested if/try arms until the final read when
the source has no explicit or implicit intervening store, captured writer,
loop-instance change or binding-resolution change. Sloppy block-function
outer aliases are included in the write guard. Existing var destinations
can absorb an adjacent initializer handoff, preserving the actual existing
storage. Earlier observations, captures, repeated declarations, anonymous
definition name inference and intervening executable statements reject it.

The combined batch removes eight bindings net on the seed: 4,044 to 4,036
(2,830 SSA and 1,206 CARRY). There are twelve fewer SSA and four more retained
CARRY declarations; none are renamed to conceal a binding. The output loses
55 lines and 1,464 bytes, reaching 19,561 lines and 1,131,199 bytes. All 661
functions trace with zero stops and reuse checks pass. The combined focused
suite passes 800 tests with nine ignored; Clippy completes with existing
repository warnings and no warnings in the changed cleanup modules.

Debug/release output bytes agree for this batch. One release run takes 1.90
seconds wall time including reuse checking; it is a single-run observation.

The next batch proves stored primitive categories through mutually dependent
loop/join cells. It starts with candidate private cells and repeatedly removes
any cell with an unproved initializer or store, until dependencies converge.
This admits cyclic copies anchored in primitive values without assuming an
unknown incoming value is numeric. Captured/shared writers, redeclarations,
parameters and possible Annex-B var stores remain excluded. Hoisted var cells
may start as undefined; their explicit initializers still participate in the
proof even when execution can skip them.

A separate primitive category includes strings, typeof results and string
accumulators. Both rematerialization and ordered-prefix fusion consume it.
A successful primitive result does not make its producing expression inert:
ordered fusion checks every operand separately, preserving object coercions,
getters and calls. Existing scope, version and initialization guards still
apply to every moved read.

Repeated inert expressions now use an output-byte budget rather than an
eight-reference cutoff. Multi-use substitutions must be identity-free or
mutually exclusive; long expressions must shrink the output, and short ones
retain the existing bounded-growth allowance. Identity-free candidates skip
the quadratic branch-exclusivity comparison.

Implicit for-in/for-of iteration stores and destructuring declarations are
excluded from the stored-type candidate set. They need separate value proofs;
an absent declarator initializer does not imply that an iteration cell keeps
undefined. Differential cases check that object coercion runs once when a
snapshot is consumed twice, including var iteration and destructuring inputs.

The combined guarded batch takes the seed from 4,036 to 4,029 declarations
(2,823 SSA and 1,206 CARRY). This is a modest seven-binding reduction despite
the general proofs. Output has 19,549 lines and 1,130,625 bytes: twelve fewer
lines and 574 fewer bytes. All 661 functions trace with zero stops and reuse
checks pass. The focused suite passes 804 tests with nine ignored; Clippy
completes with existing repository warnings and none in the changed cleanup
modules. No bindings are renamed.

The guarded debug and release outputs are byte-identical.

### Path-proved forwarding and loop invariants

`stable_channels.rs` tracks whether each candidate contains its current input
on every reaching normal path. Copies establish equality; input stores end it
after evaluating their RHS. Branches merge the normal paths, and returning or
throwing paths do not constrain later reads. This handles parallel join cells,
readonly argument channels and initialized aliases without changing names.
A lexical declaration inside a loop is proved within its own block activation.

Loop-test copies can establish equality on both the body entry and the final
failing test. Unlabeled continues are supported when the for update cannot read
the alias; breaks and labeled control flow retain the snapshot. Other loops
require the source to remain stable throughout their entire repeated region.
Fresh-value rematerialization likewise checks the outermost newly entered loop,
including stores textually after the consumer that can run before its next
iteration. Identity-bearing allocations retain their original iteration rules.

Shared slots and closure-writable cells use a stricter effect window. Calls,
getters, property writes, possible coercions, spreads, destructuring, suspension
and unresolved global lookups end equality. Incoming lexical slots are never
assumed initialized: an initialized alias is eligible only when the immediately
following statement reads it unconditionally before observable evaluation.
That preserves the incoming slot's original TDZ/error order. `typeof`, direct
calls and tags keep GetValue semantics; no standalone slot-read guard is added.

The related comma fusion can cross independent inert private stores while
preserving exception-visible state. Loop comma stores can be rematerialized
only where their inputs retain the same versions. Discarded forwarding stores
leave no junk reads. Const stores keep their assignment errors, and captured
aliases, shadowed inputs and unsupported control-flow regions are retained.

The two guarded batches reduce the seed from 4,029 to 3,971 declarations:
2,797 SSA and 1,174 CARRY, a net reduction of 58. The first five patterns
accounted for 40; initialized/invariant/shared-source extensions and their
follow-on cleanup account for another 18. No bindings are renamed. The focused
suite passes 821 tests with nine ignored. The symbolic seed traces 661 functions
with zero stops and reuse validation passes. Clippy completes with existing
repository warnings and none in the changed cleanup modules.
The final debug and release outputs are byte-identical (19,549 lines,
1,124,831 bytes); the optimized seed run reports 2.22 seconds.

### Forward cell facts and existing-slot copy-outs

`cell_facts.rs` records facts about successful evaluation within each function.
An earlier definite GetValue proves that an incoming lexical slot is initialized;
`typeof` alone, a conditional read, a skipped loop, a default initializer, a case
test or a caught failing path cannot supply that proof. Facts are intersected
across normal branch exits. Each function starts a fresh analysis, including
nested callbacks; no creator's values are assumed at a child thread's entry.

Mutable value facts are forgotten at calls, getters, coercions, property stores,
spreads, destructuring and suspension. Exact value tokens recognize readonly
arguments from the same invocation, identically spelled primitive literals,
undefined and immutable lexical locals. They can prove that a slot restored
after another store again contains a snapshot's original value. Zero and
negative zero are not conflated; mutable locals, changing arguments and cached
object coercions decline. Primitive stores and explicit scalar type guards can
also prove that repeated scalar operations cannot invoke object coercion hooks.

An adjacent `let SSA = producer; KEY = SSA;` can place the producer directly in
that existing KEY when all remaining SSA reads pass the same value and effect
proof. The producer still runs once, the slot is stored after evaluating its
RHS, and no new name is created. Unknown mutations after the copy-out retain the
snapshot. Captured or reassigned temporaries, changed bindings, delete/shorthand
contexts and anonymous definition name inference remain protected. A failing
TDZ or const slot store retains the producer's prior effects and its error order.
The eliminated copy statement is deleted rather than left as an empty statement.

The five fact categories initially remove 18 bindings on the seed. Extending the
immutable-value proof to existing slot copy-outs and rerunning the per-thread
sweep removes another 85 net. The combined result is 3,971 to 3,868 bindings:
2,749 SSA and 1,119 CARRY, with no renaming. The combined focused suite passes
827 tests with nine ignored; all 661 seed functions trace with zero stops and
reuse validation passes. Captured seed JavaScript is not executed.
Final debug/release outputs are byte-identical: 19,468 lines and 1,117,067 bytes
(81 fewer lines and 7,764 fewer bytes). The optimized run reports 1.52 seconds.
Clippy completes with existing warnings and none in the changed cleanup modules.

### Declaration lifetimes and computed private commits

`declaration_lifetimes.rs` moves a private uninitialized generated declaration
up to its first actual use in the same statement list. A first direct store can
become its initializer, including when subsequent stores make the cell mutable.
There must be no earlier read, RHS self-read, capture, redeclaration or dynamic
scope observer. This exposes existing branch and alias rewrites without changing
names or assuming that a cell's initial undefined value was unobservable.

Existing-slot forwarding also handles delayed inert producers, existing `var`
slot initializers, and terminal branch results copied into an existing slot.
Delayed producers use only stable, initialized private inputs. Terminal branch
stores can target the existing destination only when every normal path supplies
a result and subsequent snapshot reads pass the path equality proof. Return and
throw paths, slot shadowing, inferred function names and failing slot stores keep
their original behavior.

Private handoff coalescing now accepts computed commits such as
`state = snapshot + 1`, rather than requiring `state = snapshot`. The snapshot's
entire live range must end inside that RHS, because the committed result can
differ from the snapshot. The destination must be private, initialized and
writable, and its old value must have no observer while the store is advanced.
Existing loop, abrupt completion, capture and catch/finally guards still apply.
Only one snapshot is selected per computed commit in a round: two independent
live values must never be assigned the same storage. Semantic facts are rebuilt
before selecting another candidate.

The ten-family follow-up audit covered the following groups in the intermediate
3,852-binding seed output. Counts classify initializers, not independent proof
obligations or promises that every member can be removed:

| Family | Bindings | Outcome |
| --- | ---: | --- |
| Binary expressions | 589 | Computed private commits remove finished snapshot live ranges; property reads and coercions inside other expressions remain ordered. |
| Identifier aliases | 283 | Forward only while slot equality holds; arithmetic can invoke a hook that changes a later slot read. |
| Uninitialized channels | 481 | Tighten declarations and materialize first stores; keep branches with multiple live results and initial undefined observations. |
| Calls | 873 | Computed commits can consume cached call results without moving calls; a later callee/property lookup remains a barrier to argument inlining. |
| Property reads | 820 | Preserve getter timing across calls, stores and receiver construction; single use alone is insufficient. |
| Arrays | 124 | Preserve identity for multiple consumers and per-iteration allocation; existing private-cell and dense construction proofs still apply. |
| Objects | 89 | Preserve field getter timing, escaping identity and creation before conditional worker setup. |
| Conditional expressions | 153 | Keep cached loop bounds and conditional effects; branch copy-outs require complete normal-path results. |
| Unary expressions | 38 | Keep exception-state sentinels and mutable channels observed by catch/finally. |
| Other initializers | 402 | Keep loop induction state and inferred anonymous definition names; use existing numeric counter proofs only where their prerequisites hold. |

The audit deliberately does not equate ten reviewed families with ten new
transforms. The broad computed-commit extension supplies most of this batch's
reduction; unsafe members of the other families retain their snapshots.

Before integrating upstream tracing changes, the combined cleanup produces
3,694 bindings (2,584 SSA and 1,110 CARRY), down 174 from 3,868. The output has
19,439 lines and 1,121,188 bytes. Storage coalescing can increase bytes when an
existing destination has a longer name; no names are shortened to improve this
metric. The focused suite passes 835 tests with nine ignored; the additional
ten-context differential test also passes. The seed symbolically traces 661
functions with zero stops and reuse validation passes. Captured JavaScript is
never executed.

After merging origin/main through 272cfdb, the final debug and release output
is byte-identical: 3,625 bindings (2,532 SSA, 1,093 CARRY), 19,220 lines and
1,103,664 bytes. The additional reduction relative to 3,694 includes upstream
tracing changes and is not attributed solely to the cleanup rules. The merged
focused suite passes 840 tests with nine ignored; Clippy completes with existing
repository warnings and none in the changed cleanup modules. All 661 seed
functions trace with zero stops and reuse validation passes. The optimized
seed run reports 1.47 seconds (one run, not a benchmark distribution).

The upstream `EmitOptions::allow_incomplete` addition also required updating an
existing symbolic-tracer test's explicit options literal to retain the default
(false) value. No tracer behavior is changed by that fixture repair.

### Ten consumer patterns and late construction lifetimes

The next batch extends existing proofs before running its combined checks:

1. A switch discriminant is evaluated once, so an adjacent snapshot may feed it.
2. A for initializer is evaluated once; the repeated test and update are excluded.
3. A for-in RHS is evaluated once before enumeration.
4. A for-of RHS is evaluated once before iterator acquisition.
5. A destructuring declaration's RHS can consume a snapshot before pattern work.
6. The first initializer of a grouped declaration is unconditional; later
   initializers still require their own ordering proof.
7. Comma-expression callees and tags retain GetValue via `(0, value)`, preserving
   detached receivers and indirect call behavior.
8. Comma-expression typeof operands retain GetValue so an unresolved producer
   still throws rather than becoming a nonthrowing typeof query.
9. Tail join elimination runs after late snapshot and array-cell propagation,
   when formerly separated branch consumers can become adjacent.
10. Finished private multi-store channels can reuse an existing destination when
    every source use finishes before its computed commit changes that value.

All newly admitted control inputs use the same ordered-prefix proof as ordinary
expressions. An insertion-scope check additionally prevents a for-let binding
from capturing a free name in the producer, including names in its closures.
Repeated loop tests, earlier effectful group initializers and captured snapshots
remain barriers. No fresh replacement names are created.

The multi-store proof distinguishes a carry's initial undefined value from a
value definitely assigned on every evaluated path. A read before assignment
keeps the undefined reset. Otherwise a path-sensitive assignment proof permits
reading the destination's old value before that path's first source store, but
rejects any old-value read after the store. Conditional and short-circuit paths
intersect their facts; complex exception/switch control declines. Existing
capture, loop, inferred-definition-name, shadowing, abrupt completion and
catch/finally guards remain in force. This pass runs after existing snapshot
storage choices, so short-lived channels cannot take storage needed by longer
snapshots.

The initial ten-pattern extensions alone remove nine net bindings on this seed;
most new control-consumer forms have little coverage here. A further general
construction improvement moves private, inert array/object literals to their
first use in the same statement list, where dense writes can be reconstructed.
It requires no earlier use, capture, reassigning write, redeclaration, initializer
effect or dynamic scope observer. Keeping relocation and reconstruction late
preserves producer provenance needed by earlier snapshot and method passes.
Runs of unrelated inert literals do not count as work to cross, avoiding
oscillating declaration order.

The new construction cases exposed and fixed a dense-array printer bug:
SequenceExpression spans can omit parentheses. Initial and assigned elements
must preserve those parentheses so `(effect(), value)` stays one array element.
Synthetic differential cases cover initial elements and later writes, nested
arrays, getters, captures, repeated calls, missing branch assignments, destination
old-value reads, short-circuiting, receiver binding and exceptions. Captured seed
JavaScript is never executed.

The final debug seed has 3,599 bindings (2,515 SSA and 1,084 CARRY), down 26 from
3,625. Its 18,581 lines and 1,086,485 bytes are 639 lines and 17,179 bytes smaller.
Dense arrays folded increase from 882 to 926; restored method calls remain 1,579.
All 661 functions trace with zero stops and reuse validation passes.
The combined focused suite passes 850 tests with nine ignored; the final frozen
library/thread subset was rerun and passes 624 tests with nine ignored. Clippy
completes with existing repository warnings and none in the changed cleanup
modules. Final debug/release output is byte-identical. The optimized seed run
reports 1.51 seconds (one run, not a performance distribution).

Final statement-list cleanup now removes empty-statement placeholders and empty
alternate arms, including arms containing only empty statements or empty blocks.
Empty loop/conditional bodies, directive barriers, comments, and dangling-else
binding are preserved. The descriptor-loop regression compares behavior with
and without descriptors. The combined focused suite passes 854 tests with nine
ignored; Clippy completes with existing warnings and none in these modules.
The regenerated seed has zero standalone-semicolon lines (previously 232),
18,343 lines and 1,083,119 bytes. All 661 functions trace with zero stops and
reuse validation passes.

### Point facts and branch accumulator storage

The next batch adds private-cell facts across effects, point-specific scalar
types, typeof branch refinements, short-circuit refinements, successful update
and compound-store types, unchanged loop inputs, inert templates, inert comma
expressions, and ordered numeric prefixes. Successful stored scalar values
exclude BigInt and Symbol; this never treats the original getter, call or
coercion as inert. Captured writers, exported slots, changed input versions and
backedge stores remain barriers. Loose null tests do not establish primitive
types because HTMLDDA values can satisfy those tests.

These ten extensions remove only two net bindings on the seed. A further
branch-sensitive accumulator proof removes 35 more: a snapshot may occupy its
existing private mutable copy-out cell when every snapshot read still reaches
that value, no early store exposes a changed old value, and every surviving path
restores storage equality before the proof interval ends. Captures, shared
slots, inferred anonymous function names, skipped optional-chain commits and
catch/finally observers stay barriers. This changes real storage assignments;
necessary snapshots retain their original names.

The seed now has 3,562 bindings (2,480 SSA and 1,082 CARRY), down 37 from 3,599.
Output is 18,280 lines / 1,080,236 bytes, down 63 lines / 2,883 bytes. All 661
functions trace with zero stops and reuse validation passes; debug and release
output is byte-identical. The combined focused suite passes 862 tests with nine
ignored. Clippy completes with existing warnings and none in these changed
modules. Release timings of 2.13, 2.96 and 2.75 seconds are individual runs,
not a controlled comparison with earlier measurements.

### Exception continuations, effect splitting and computed successors

The next ten control-flow extensions distinguish normal try/catch continuation,
catch entry from a partial throwing prefix, and finally entry for both normal
and pending abrupt completion. Finalizer point facts are collected from a
conservative state; a separate analysis computes normal continuation facts
without exporting those narrower assumptions back inside the finalizer.
Optional-chain, switch and labeled invariants, refreshed do-while tests, and
bounded non-fallthrough switch cases gain alias proofs. Complex catch bindings
remain barriers because computed keys/defaults can mutate an input. This batch
reduces the seed to 3,552 bindings (2,475 SSA / 1,077 CARRY).

A subsequent effect/value split preserves each side effect at the declaration
and rematerializes only identity-free, hook-free final values. It supports
sequences, stable conditional/logical tests, assignment results, void and other
safe unary operators, safe binary operators, templates, constant tails and
guarded readonly argument tails. Every moved value dependency still needs the
original version, scope and loop proof. Calls, getters, setters, throwing reads
and coercions execute once in order. It removes four more bindings; the extra
expression wrappers increase this intermediate output by 159 bytes.

The larger improvement recycles a private input cell after its old value's last
use computes a successor. The successor computation stays in place and executes
once. This recovers byte-position and bit-packing accumulator chains without
repeating getters/calls across intervening effects. A source const may become
let only with no original writes anywhere, so const-assignment errors cannot be
lost. Old-value reads, captures, exported slots, another loop activation, target
captures, scoped name conflicts and inferred anonymous function names decline.
Grouped uninitialized let inputs are supported without changing sibling cells.
This deletes 545 further bindings; it introduces no substitute KEY names.

The pasted `$THREAD_1_95914` and `$THREAD_1_139608` functions go from 116 to 75
and 129 to 68 own SSA/CARRY bindings respectively. The array-swap snapshot at
the start is retained: a later reread would see the overwritten element.
The two discarded host document/window reads were confirmed in the raw emitted
trace as unread declarations, not dropped consumers. Cleanup preserves their
getter/throw behavior, with an explicit differential regression.

That checkpoint had 3,003 bindings (2,036 SSA / 967 CARRY), 596 fewer than the
3,599 baseline. It has 18,259 lines / 1,069,179 bytes. All 661 functions trace
with zero stops and reuse checks pass. The focused suite passes 873 tests with
nine ignored. Clippy and targeted rustfmt checks complete, with no Clippy
warnings in the new/changed cleanup modules. Debug/release output is identical;
three optimized runs report 1.98, 2.33 and 2.45 seconds. The ongoing goal
remains active because
avoidable SSA/CARRY work is not finished.

The next batch extends dead-input storage reuse to enclosing statement-list
declarations and mutable successors. For-header declarations cannot lend cells:
their updates appear before the body in source but execute afterward. Explicit
successor stores/updates retain their original evaluation positions, and const
assignment failures and function/class name inference remain protected.

Uninitialized branch successors use a separate path proof. Every successor read
must follow its initialization on every reaching path, and no original input
read may follow reuse on that path. Return/throw paths leave the merge; skipped
logical/optional branches cannot manufacture initialization. Established cell
equivalence can cross structured regions only when they do not reference the
old input; regions preceding initialization must touch neither cell. This also
preserves loop updates, switch breaks and catch/finally completion behavior.

The new seed has 2,582 bindings (1,906 SSA / 676 CARRY), 421 fewer than the
3,003 checkpoint and 1,017 fewer than 3,599. It has 17,888 lines / 1,045,411
bytes. The pasted font-check thread goes from eight to six bindings, and the
two larger descriptor/byte-position threads go from 75 to 49 and 68 to 52.
The later `$THREAD_1_151005` example still has 25 bindings: its null-prototype
membership tables, cached lengths, indexed values and KEY snapshots cannot be
recomputed under the current per-thread proofs. No captured seed JavaScript was
executed; runtime differential checks use synthetic fixtures only.
The combined focused verification now covers 880 passing tests, with nine
ignored. Updated library/thread tests pass after the final control-flow
extension, and Clippy, targeted rustfmt and diff checks pass. Final debug and
release output is byte-identical. The optimized trace takes 1.64 seconds with
661 traced functions, zero stops and reuse validation enabled.

### Ended live ranges and bounded key lists

Grouped uninitialized let channels now remove only the selected declarator,
preserving sibling cells and their scopes. Copy-back stores track both cell
versions: copying a successor into the original cell restores equivalence;
updating either cell kills the other version until a proven copy restores it.
Logical stores merge their skipped/evaluated paths. Proof intervals include
every later original-cell observer, not only successor reads. These rules
remove 12 bindings from the prior checkpoint after a snapshot safety fix.

The new snapshot regression exposed an existing alias boundary error: a store
starting exactly at a preceding declaration's end was missed by a strict `>`
comparison. The boundary is now inclusive. Prefix/postfix updates, simple and
compound assignments, and form/event getters have differential regressions.
The original form value must stay cached when a later event getter or callback
can change a fresh property read.

Private dense literal key lists now allow reads through numeric counters with
proven in-bounds loop ranges. The counter starts at zero and has only its known
unit-step update; bounds come from constants or immutable own-length snapshots.
Pre/post increments, unit assignments, strict/loose inequality and inclusive or
reversed comparisons are covered. Holes, unknown indices, counter writes,
captured edits, receiver calls and structural array mutations remain barriers.
Fixed literal lengths can cross loop iterations, unlike mutable cell snapshots.
This removes four further bindings in the seed navigator example.

The larger change lets ended private live ranges lend storage to independent
later producers. It prefers existing computation-input chains, then the most
recently dead eligible cell. Every original observer must end before reuse,
and initialization, scope lookup, captures, loop activation, const errors and
function/class names retain their guards. Each producer stays at its original
position and executes once. The old cell's name survives; an actual later
declaration is replaced by its assignment, with no new KEY names. This runs
after reconstruction: an earlier trial lost 24 array folds, whereas final
scheduling keeps all 926 and removes more cells.

Current output has 1,780 bindings (1,270 SSA / 510 CARRY), 802 fewer than the
2,582 checkpoint and 1,819 fewer than 3,599. It has 17,882 lines / 1,038,829
bytes. The navigator thread goes 60→23, the form handler 25→18, and the earlier
membership-table thread 25→15 without repeating their property snapshots.
The two descriptor/byte-position examples go 49→27 and 52→27.

All 890 focused tests pass (nine ignored), including synthetic differential
fixtures; Clippy and targeted rustfmt/diff checks pass. Debug/release output is
identical. The optimized trace takes 1.62 seconds, with 661 traced functions,
zero stops and reuse checking enabled. The overall elimination goal is active.
There are four more retained defineProperty calls than at the 2,582 checkpoint;
their constant-key/descriptor provenance needs a follow-up audit rather than a
blind rewrite after mutable-cell reuse.

### Point literals and uninitialized storage

Ended private cells can now hold later uninitialized join channels. The original
declaration's undefined initialization stays at the same position; cells already
proven undefined can share that initial state. Grouped declarations preserve their
siblings. Structured regions invalidate the other cached version when either
cell is written, preventing stale copies after conditional stores or exceptions.

Point facts also replace exact literal values at proven reads. Property keys run
before construction cleanup; other literal reads run afterward so descriptor and
array provenance survives. This restores the four descriptor folds noted above.
Strict zero comparisons preserve the distinction between positive and negative
zero. Unknown global typeof/delete effects and implicit block-function stores
invalidate affected facts. Late constant/branch cleanup removes resulting inert
tails while preserving effects, scopes and directive boundaries.

The form/event attachment retains its first two cached reads: the event getter
or a later callback can change a fresh form read. A synthetic runtime regression
checks the getter order and saved object identity.

Current output has 1,751 bindings (1,263 SSA / 488 CARRY), 29 fewer than the
previous checkpoint, with 17,865 lines / 1,038,157 bytes. All 902 focused tests
pass (nine ignored); Clippy completes with existing warnings, and targeted
rustfmt/diff checks pass. Debug and release output is byte-identical. The release
command reports 2.10 seconds (2.47 seconds wall time), with 661 traced functions,
zero stops and reuse validation enabled. Necessary snapshot names remain intact;
the broader elimination goal remains active.

### Loop-carried version equality

Successor storage now follows while, for and do-while loops. Each header meets
the initial availability state with every backedge and rechecks the body when
either version becomes unavailable. Continue edges reach the appropriate update
or test; break edges join loop exits. Nested loops keep separate edge sets.
False tests participate in exit proofs, so a final successor cannot replace an
older snapshot still read after a failed test. Opaque exception/label regions
with unmodeled break/continue edges decline rather than skipping a copy-back.

Initialized snapshots use the same version proof when their input remains live:
an exact copy establishes equality, and an explicit copy-back can restore it.
Initializers and later producers stay in place and execute once. Unequal live
versions, conditional missing copy-backs, const write errors, inferred function
names, lexical TDZ, captured observers and scope lookup changes remain barriers.
This removes actual declarations, retaining the surviving cell's original name.

The loop extension removes 34 seed bindings, and initialized version sharing
removes six more. Current debug output has 1,711 bindings (1,231 SSA / 480 CARRY),
17,758 lines / 1,035,728 bytes. Descriptor/array construction totals stay at
3,320 writes / 926 arrays. The 909 focused tests pass (nine ignored), including
ten loop forms, ten initialized forms and runtime comparisons with coercion
effects, identities and abrupt exits. No captured seed JavaScript is executed.
Debug and release output is byte-identical. The optimized command reports 2.17
seconds (2.53 seconds wall time), with 661 traced functions, zero stops and reuse
checking enabled. Clippy completes with existing warnings; targeted rustfmt and
diff checks pass. The overall elimination goal remains active.

### Truth-specific continuation copies

Loop tests now retain separate true and false availability states. A copy-back
on the right of `test && (copy, true)` is guaranteed on the backedge even when
the false exit retains an unequal old value. The old snapshot must still stay
separate if an exit observer needs it. Logical OR, negation, parentheses,
sequences, conditional expressions and explicit Boolean results use the same
edge proof; unknown values conservatively permit both outcomes. Nullish
coalescing keeps its separate skip/evaluate merge rather than treating falsy
values as nullish. The proof never reassociates arithmetic or duplicates reads.

If/conditional/logical consumers also use those states, so a restored copy is
available only on the branch that actually executed it. While/for bodies receive
the true test state; do-while backedges receive the true post-body state. Breaks
and false-test exits still merge before subsequent observers. Tests cover ten
continuation forms, ten conditional consumers, missing-copy and false-exit
declines, and runtime comparisons with getter/coercion effects and zero-entry
cases. All 914 focused tests pass (nine ignored).

The existing carry self-store remover now requires an initialized mutable cell
in the current function, rather than textual declaration order. Const stores
retain their TypeError, and closures called before an outer lexical initializer
retain their ReferenceError. Removed standalone stores leave an empty-statement
barrier until the directive-aware empty cleanup runs. A no-candidate fast path
avoids building local facts for functions without eligible self-stores. Ten
additional runtime forms and a unit regression raise the focused total to 916.

The seed has 1,688 bindings (1,210 SSA / 478 CARRY), 23 fewer than the previous
checkpoint. The largest thread drops from 75 to 54. Construction totals remain
3,320 simplified property writes / 926 folded arrays; 661 functions trace with
zero stops and reuse checking enabled. The safety fix retains eight additional
lines of stores whose initialization was not proven, rather than dropping an
error or a shared-cell write.

Final output has 17,739 lines / 1,035,348 bytes, and debug/release output is
byte-identical. Clippy completes with existing warnings; targeted rustfmt/diff
checks pass. Optimized traces report 3.58 and 3.65 seconds (4.05 / 3.74 seconds
wall time) while separate corpus work is active, so these are not controlled
before/after timing comparisons. The overall goal remains active.

### Once-per-activation for headers

Late ended-cell reuse now handles single initialized let/const declarations in
ordinary for headers. The initializer stays in the header and executes once;
its later reads and explicit updates use an existing finished generated cell.
Header storage is separate from ordinary body storage: the outermost repeated
region entered from the source declaration must be precisely this for loop.
This prevents an enclosing loop from restarting against an input overwritten
by a previous counter run. Locals initialized earlier in the same parent-loop
activation remain eligible.

All original source observations must finish by the initializer. Captured
iteration bindings, changed name resolution, source const writes, target const
writes, target TDZ reads, implicit named-function stores, inferred anonymous
function/class names, grouped/destructured headers and comments decline.
Original source names survive; this removes storage without new KEY aliases.
The original statement-list source rule remains intact, so a header counter
does not lend its live update cell to a body temporary.

Ten control forms and nearby declines cover increments/decrements, arbitrary
steps, effectful initializers, arrays, parent activations, continue, break,
conditional bodies, nested loops and catches. Synthetic runtime comparisons
also cover per-iteration captures, const/TDZ failures, coercion counts, labeled
breaks and finalizer observers. All 920 focused tests pass (nine ignored).
Debug output has 1,664 bindings (1,212 SSA / 452 CARRY), 24 fewer than before.
Construction counts remain 3,320 property writes / 926 arrays; 661 functions
trace with zero stops and reuse checking enabled.

Final debug/release output is byte-identical, with 17,739 lines / 1,035,183
bytes. The optimized command reports 2.19 seconds (2.55 seconds wall time).
Clippy completes with existing warnings; targeted rustfmt/diff checks pass.
The overall goal remains active.

### Discarded cell values and branch-local stored versions

Discarded-value cleanup now handles initialized generated local reads and exact
mutable self-stores in expression statements, comma prefixes and unused for
fields. Calls, getters, unknown reads, const/resource/name-binding errors and
concise-arrow return values remain observable. Used comma expressions retain
GetValue, including detached calls, delete and typeof; directive barriers remain.
An exact AST inventory found 70 self-stores in the seed, all removed. Regex
matches also included real updates and were not used as the inventory.

Ended header storage can also cross a newly entered enclosing loop when every
old observation finishes before that enclosing activation starts. Inputs still
observed inside that parent cannot use this relaxation. The initializer stays
once per activation, and no live header counter lends storage to body locals.

Adjacent forwarding now asks whether the particular stored version is live
after its consumer. A backward analysis joins branches, solves loop backedges,
and conservatively includes enclosing break/continue destinations. Only simple
unconditional root stores kill a prior value; unknown control forms are opaque.
Exception-bearing owners decline this new proof. Captures, shared/exported
cells, scope changes and initializer self-reads retain the existing guards.
This permits a private cell's branch-local final read even if other branches
use different stored versions. Numeric reads from a proven readonly, unescaped
parameterless arguments object can precede the inserted producer under the
existing ordinary-prototype contract. Arbitrary getters and calls remain
barriers; computed-key coercion still follows the RHS.

The encoder `$THREAD_5_39657` now directly stores two byte results that formerly
passed through its surrogate carry. The surrogate/code-point binding remains
needed. `$THREAD_0_136734` retains its old-key snapshots: shared source writes
and possible coercion hooks prevent blind substitution. Synthetic comparisons
cover numeric operation and hooks that mutate the shared keys during coercion.
The attached hook/descriptor code retains reads that cannot commute safely;
non-enumerable property descriptors still require defineProperty.

The focused suite passes 934 tests with nine ignored, plus the exact rotating-key
fixture passes separately (935 total). Debug seed output has 1,663 bindings:
1,211 SSA and 452 CARRY. This final stored-version change removes one binding
and 40 lines from the preceding unused-value checkpoint; the combined changes
since the 1,664-binding header checkpoint remove 99 lines. Construction remains
3,320 simplified property writes / 926 folded arrays, with 1,580 method calls
restored, 276 reconstructed loops and 437 flattened blocks. All 661 functions
trace with zero stops and reuse checking enabled. The capture is never executed.
