# Typed plain-text trace renderer

Status: **implemented renderer; no runtime producer is connected.**
A trail states its scope and evidence mode, then follows reads, decisions,
calls, changes, and completion in execution order. Branches and calls retain
indentation and explicit resume points. Unknown results remain unknown; the
renderer never invents names, values, or provenance to make a trail look complete.
The detailed record carries handler keys, micro-op ordinals, source spans, and
IR coordinates only when the producer supplies them.

[`src/tracing/text/mod.rs`](../../src/tracing/text/mod.rs) exposes a small typed observation API. A caller
supplies scopes, ordered events, values, evidence labels, and optional provenance;
`Trace::render()` returns readable plain text, a companion details report, and an exact rendered
line map. The renderer never evaluates an expression, invokes a getter or coercion, executes
JavaScript/WASM, solves a branch, lifts bytecode, or builds a second CFG.

This renderer is distinct from `tracing::symbolic`: the symbolic component executes
admitted handler recipes and can emit live `TraceDebugOptions` diagnostics, while
this component only validates and renders observations supplied by another
producer. It does not define a persistence format or replay protocol.

The [module README](../../src/tracing/text/README.md) keeps the essential producer
contract and source map. This guide is the detailed reference for constructing
observations, understanding validation failures, and presenting the results.

| Need | Section |
| --- | --- |
| Call the renderer and use its outputs | [Producer handoff](#producer-to-renderer-handoff) |
| See a complete result | [Rendered trail](#a-complete-rendered-trail) |
| Construct values, scopes, evidence, and events | [Observation model](#the-observation-model) |
| Understand accepted control-flow shapes | [Validation](#validation) |
| Present text, details, and navigation | [Rendering](#rendering) |
| Configure bounded work and handle failures | [Limits](#limits) and [errors](#errors) |
| Connect a new producer | [Integration](#integrating-a-producer) |
| Find a type or debug a rejected observation | [Public API](#the-public-api) and [troubleshooting](#where-a-bug-belongs) |

<a id="api-shape"></a>

## Producer-to-renderer handoff

The public path is `shape::tracing::text` (also `shape::trace_text`). A producer
supplies a `TraceHeader`, the invocation `Scope` records, and ordered
`TraceEvent` records. Every event has a unique ID, a scope, typed evidence,
optional provenance, and an `EventKind`. Use the supplied IDs to connect calls,
resumes, and control-flow boundaries.

```rust
let rendered = shape::tracing::text::Trace::new(header, scopes, events)
    .with_limits(limits)
    .render()?;
```

This fragment assumes the producer has constructed those typed inputs and
returns a compatible `Result`. `render` validates first; `validate` is also
available when no output is needed. A `RenderedTrace` contains:

- `text`: the readable trail;
- `details`: evidence and supplied source/IR coordinates;
- `line_map`: inclusive rendered line ranges tied to event IDs and provenance.

One event can occupy several lines. Use the returned map rather than assuming
that event number N corresponds to line N. Events that render no text
(`CallEnd`, `IterationEnd`, `CatchEnd`, and a `BranchEnd` that only closes a
returned branch) contribute no map range at all.

### Contracts the producer owns

| Input | Contract |
| --- | --- |
| Event IDs | Unique and stable for the run; references (`Assumed`, `ProvenDead` evidence) point at them |
| Scope IDs | Unique; scope `0` exists and is the root; every other scope names an existing parent |
| Event order | Execution order within the active invocation; a child's events sit between its `CallStart` and `CallEnd` |
| Evidence | `Observed`, `StaticDerived`, or `Assumed`; an assumed branch must cite the branch that opened the assumption |
| Values | Typed, not stringified by the producer; unknown outcomes stay unknown |
| Provenance | Optional; when supplied, it is displayed as supplied and never invented |

The producer also owns the claim behind a proof. A `ProvenDead` branch needs a
nonempty `proof` and `evidence_ref`; the renderer checks presence and prints the
premises with a "not re-certified" note. It does not check logical
reachability. Likewise, a `Suspected` target stays heuristic: runtime not-taken
is not proof of unreachability.

## A complete rendered trail

The first exact snapshot in `tests/trace_text.rs`
constructs one runtime-observed scope and six events: input, read, test,
branch start, branch end, and change. `Trace::render()` produces:

```text
Trace observation
sample: test
mode: runtime observed
scope: thread 0, call 1
capture/exploration: selected invocation captured

input: slot = 1
read slot -> value = 1
if slot > 0 -> true (known)
Take true branch (observed; false branch)
Continue after branch (proven CFG join)
Change slot: 1 -> 2
```

The line map ties events 1–6 to rendered lines 7–12 respectively. It does not
claim the test condition was computed by this renderer: the producer supplied
both the `true` result and the reason `known`. `Continue after branch` is legal
only because the producer supplied `BranchExit::ContinueAfterProvenJoin`; the
renderer rejects malformed nesting rather than guessing a join from adjacent
text. A caller that needs to explain one line can use `RenderedTrace.line_map`
to recover its event ID and then read the companion details for supplied
provenance. This is why the details remain separate from the human trail.

The test constructs events directly through `Trace::new(header, scopes,
events).render()`. There is no command that accepts a kernel and produces this
trail. Synthetic test records exercise rendering independently of capture
analysis.

## The observation model

### Header and mode

`TraceHeader` declares the sample, the evidence `Mode`, the selected scope, the
capture/exploration boundary, and whether the record is illustrative.

| Mode | Displayed as | Meaning |
| --- | --- | --- |
| `RuntimeObserved` | `runtime observed` | A producer observed execution |
| `StaticDerived` | `static derived` | A producer derived the record statically |
| `Symbolic` | `symbolic` | A symbolic analysis supplied it |

`illustrative: true` changes the trail title to `Illustrative — not recorded
execution`, which is the header's only behavioral effect. Mode is a claim, like
every other evidence field; the renderer neither upgrades nor downgrades it.

### Scopes

A `Scope` identifies one VM-thread invocation: `id`, `thread`, `call`, and an
optional parent call scope. Scope `0` is mandatory and has no parent. Any other
scope must have an existing parent. Parent links are validated for existence
and cycles, and the scope graph is walked in key order so the reported error is
deterministic.

A non-root scope becomes active through `CallStart`, whose `child_scope` must
name a declared scope with `parent == caller scope` and `call == invocation`.
Each child scope may be entered once per trace; a second `CallStart` for it is
an error. This is what makes the event sequence a depth-first call tree rather
than an arbitrary interleaving.

`thread` and `call` are producer identities, not ordering keys: they connect a
scope to its VM thread and call site for display and cross-referencing. The
renderer uses `parent` for the scope graph and the event sequence for control
flow. Two scopes with the same thread but different calls are different
invocations, and a child scope must also declare the `call` that its
`CallStart.invocation` names. Producers should allocate scope 0 once, create
child scopes only for calls they will actually emit, and leave the root parent
as `None`.

### Events, evidence, and provenance

`TraceEvent` carries `id`, `scope`, `evidence`, `provenance`, and `kind`.

| Evidence | Meaning | Extra rule |
| --- | --- | --- |
| `Observed` | Directly recorded | Required for `BranchStatus::Observed` |
| `StaticDerived` | Derived without execution | Treated as evidence, not as an observation |
| `Assumed { branch, label }` | A hypothesis opened at a branch | `branch` must be a `BranchStart`; at the opening event it may be itself, inside it must be an ancestor |

Only events inside an assumed path may carry `Assumed` evidence, and they must
cite an assumed ancestor. Observed or derived evidence inside an assumed path
is rejected: a trail may not silently promote a hypothesis to an observation.

`Provenance` is five optional fields: `handler_key`, `micro_op_ordinal`,
`source` (`artifact`, `start`, `end`), `ir_revision`, and `ir_op`. They are for
the companion details, not the main trail. An absent field prints as
`unavailable`; a source span with `start > end` is invalid. Nothing is derived
from a source map, and no coordinate is guessed.

### Values

`Value` preserves the distinctions a deobfuscation trail needs:

| Variant | Trail rendering |
| --- | --- |
| `Undefined`, `Null`, `Uninitialized` | `undefined`, `null`, `uninitialized` |
| `Unavailable(reason)` | `unavailable (reason: …)` |
| `Bool`, `I32`, `I64` | Their numeric/boolean text |
| `Number` | Decimal text, with `NaN`, `Infinity`, `-Infinity` |
| `String` | Quoted and escaped, truncated at the preview limit |
| `Reference(name)` | `ref(name)` |
| `Preview { display, full_ref, size_bytes }` | Quoted display plus `[truncated; size N bytes; …]` |

Numbers are not coerced. Structural comparisons of values use exact semantics:
floating-point values compare by bit pattern, so identical NaN payloads match
while `-0` and `+0` do not; previews compare display, full reference, and size.
That exactness matters when a `CallEnd` completion must agree with the child's
terminal `Return`, or a `FinallyEnd` completion with the pending value.

### Event kinds

The event vocabulary is structural. Each kind opens, closes, or annotates a
state; indentation is derived from those boundaries, not from producer text.

| Group | Kinds |
| --- | --- |
| Data | `Input`, `Read`, `Compute`, `Test`, `Change`, `Capture` |
| Branches | `BranchStart` (with `BranchStatus`), `BranchEnd` (with `BranchExit`) |
| Calls | `CallStart`, `CallEnd`, `Resume` |
| Completion | `Return` (with `ReturnDisposition`), `Throw`, `CatchStart`, `CatchEnd`, `FinallyStart`, `FinallyEnd` |
| Loops | `LoopStart`, `IterationStart`, `IterationEnd`, `LoopExit`, `LoopEnd` |
| Boundaries | `OpaqueStop`, `NoUnconditionalResult` |

#### Branch status and exit

| Status | Meaning |
| --- | --- |
| `Observed { outcome, alternate }` | The branch was taken; the other side is named but not explored |
| `Assumed { outcome, assumption }` | An exploration hypothesis, not an observation |
| `ProvenDead { dead_edge, proof, evidence_ref }` | A producer claim that one edge is unreachable, with retained premises |
| `Suspected { outcome, target, reason }` | A heuristic guess; rendered as unconfirmed |

| Exit | Meaning |
| --- | --- |
| `ContinueAfterProvenJoin` | The path rejoins; legal only when the branch has no terminal or pending completion |
| `Returned` | The branch ended by a recorded return/throw/stop |
| `Stopped(reason)` | The branch exploration stopped without a normal join |

A `ProvenDead` start is rendered as a selected/dead-side claim in the trail and
as proof premises plus a disclaimer in details. A `Suspected` start renders
three lines: the assumption, the target, and the reason. `ContinueAfterProvenJoin`
is the producer's claim that a join exists; the renderer does not compute one.

#### Calls, resumes, and captures

`CallStart { callee, invocation, child_scope, capture }` enters a child
invocation. `CaptureClaim::CapturedCell { cell, identity_evidence }` records a
shared-cell capture and requires nonempty identity evidence; equal values never
establish aliasing, and the claim is displayed as supplied.

`CallEnd { completion }` closes the child and must agree exactly with the
child's terminal record: `Returned(value)`, `Threw(error)`, or
`Stopped { reason }`. A stopped child deliberately produces no normal `Resume`.
A returned child puts the caller in an awaiting state; `Resume` with the
caller's scope and the same value clears it. A thrown child must be caught or
propagated with the same error; it cannot be resumed normally. `Resume` before a
child completes is an error, as is a second resume.

#### Return, throw, catch, finally

`Return` is either `Complete` (terminal) or `PendingFinally` (records a pending
completion for a following `FinallyStart`). `Throw` records an error and a
destination string; inside a finally the producer must use `FinallyEnd`
replacement instead.

`FinallyStart` consumes a pending return or a terminal throw and compares it
with the supplied `PendingCompletion`. `FinallyEnd` either confirms that same
completion (`Completed`) or replaces it with a return/throw. `CatchStart`
consumes a pending throw and opens a catch frame; `CatchEnd` closes it. Nested
finally frames, a catch inside a finally, and a `Return`/`Throw` inside a
finally are rejected rather than narrated into a fabricated completion.

#### Loops

`LoopStart` opens a labeled loop. `IterationStart(ordinal)` must supply the next
ordinal, starting at 1, with no iteration open; `IterationEnd` closes it.
`LoopExit { result, .. }` records the false exit test and must occur outside an
iteration, once. `LoopEnd` requires no open iteration and a recorded `false`
exit, unless the path already has an explicit abrupt completion (return, throw,
or stop). This is how the model refuses a loop that simply disappears.

<a id="validation-boundary"></a>

## Validation

Validation is ordered so the first error is deterministic: header text limits,
scope/event counts, duplicate scope IDs, the root scope and scope graph, then
event IDs, scope references, per-field limits, provenance, cross-event
references, and finally the stateful structure pass. Hash-backed lookups are
iterated in key order wherever the choice of "first" error would otherwise
depend on map order.

### What the reference layer checks

- An assumed branch's evidence points at a `BranchStart`; the opening event may
  cite itself, later events must cite an assumed ancestor.
- `ProvenDead` has nonempty proof text and a nonempty evidence reference.
- A `CapturedCell` claim (on a call or a capture event) has nonempty identity
  evidence.
- An observed branch status does not arrive with assumed evidence, and assumed
  branch events do not claim observed evidence.

These are presence and consistency checks, not proof checks. Structural
validation cannot re-certify a producer's dead-branch proof.

### The structure machine

The validator keeps one active invocation stack and, per invocation, a stack of
control frames:

| Frame | Opened by | Closed by |
| --- | --- | --- |
| `Branch { .. }` | `BranchStart` | `BranchEnd` |
| `Loop { .. }` | `LoopStart` | `LoopEnd` |
| `Catch` | `CatchStart` | `CatchEnd` |
| `Finally { pending }` | `FinallyStart` | `FinallyEnd` |
| Invocation | `CallStart` | `CallEnd` |

Each event must belong to the active invocation's scope. Ordinary actions are
rejected while the frame is awaiting a child completion, terminal, or holding a
pending finally completion. Depth is maintained incrementally on every
push/pop, so nesting validation is O(1) per event; a debug assertion cross-checks
it against a recomputation.

At the end, exactly the root invocation may remain, with no open control
frames, no awaiting child, and no unresolved finally completion. A trace that
stops mid-call or mid-branch is rejected, not truncated into a tidy ending.

### Validation summary

A valid trace can still say "unknown" everywhere: `TestResult::Unknown`,
`OpaqueStop`, `BranchExit::Stopped`, `CallCompletion::Stopped`,
`Value::Unavailable`, and `NoUnconditionalResult` exist precisely so a producer
does not have to fabricate an outcome. What validation enforces is that the
records agree with each other and with the allowed control-flow shapes.

### Claim versus check

| Producer claim | The renderer checks | It does not establish |
| --- | --- | --- |
| `BranchStatus::Observed` | Observed evidence and a structurally valid branch | That the runtime actually took it |
| `ProvenDead` | Nonempty proof and evidence reference, correct nesting | Logical reachability |
| `Suspected` | Evidence and branch shape | Anything; it is rendered as unconfirmed |
| `CapturedCell` | Nonempty identity evidence | Object identity or aliasing |
| `TestResult::Known` | A stated reason | That the reason is sound |
| `Provenance` | Field lengths and a sane source span | Source-map accuracy or attribution |
| `CallCompletion` | Exact agreement with the child terminal record | That the callee really behaved so |

Keeping this table in mind prevents two opposite mistakes: treating a validated
trail as a proof, and rejecting a producer's honest uncertainty because it
"looks incomplete".

## Rendering

Rendering happens only after validation succeeds, so the renderer can assume
well-formed frames and never has to repair a trail. The trail is deterministic:
the same observations produce the same bytes.

### Trail layout

The header is six lines:

```text
Trace observation
sample: test
mode: runtime observed
scope: thread 0, call 1
capture/exploration: selected invocation captured

```

The first line becomes `Illustrative — not recorded execution` when the header
says so. The sixth line is blank, and event text starts on line 7. Indentation
is two spaces per open call or control frame; `BranchEnd`, `FinallyEnd`, and
`LoopEnd` print their closing line at the parent depth, while `CatchEnd` only
dedents. `CallEnd`, `IterationEnd`, and a `BranchEnd` that only closes a
returned branch print nothing; their effect is carried by the surrounding
lines.

The event portion of a complete small trail, from the public snapshot in
`trace_text.rs`:

```text
input: slot = 1
read slot -> value = 1
if slot > 0 -> true (known)
Take true branch (observed; false branch)
Continue after branch (proven CFG join)
Change slot: 1 -> 2
```

The producer supplied the test result and the reason, the observed branch
status, and the proven-join exit. The renderer only validates and lays them out.

### Details

`details` is a separate report, not a second trail:

```text
rendered line map (line numbers are 1-based; ranges are inclusive)
  line 7 -> events [1]
    handler key: <supplied or unavailable>
    micro-op ordinal: <supplied or unavailable>
    source: artifact:start-end
    module revision: <supplied or unavailable>
    IR op ref: <supplied or unavailable>
```

For a `ProvenDead` event in that range, the details also include the proof
claim, premises, and evidence reference with an explicit
"producer supplied; not re-certified" note. Repeated event references are kept
when one event renders several lines (an opaque stop is two).

### The line map

`LineDetail` is `{ line_start, line_end, event_ids, provenance }`, with
one-based and inclusive ranges. Adjacent physical lines from one event with the
same provenance are consolidated into one range; non-adjacent repeated
references stay separate. A single event can therefore map to a multi-line
range, and a line range can contain one event only in the current renderer. Use
the map to recover the event for a line and then read details for that event's
provenance. Do not index the map by event ID.

### Events with several lines

Two shapes recur. An opaque stop renders its boundary and reason as two lines
for the same event (indentation omitted here):

```text
opaque operation: result not described
reason: <supplied reason>; trace stops at this scope
```

A suspected branch renders its assumption, target, and reason as three lines.
Both are still one `LineDetail` range, because the lines are adjacent and share
provenance.

### Which output to show

`text` is the human trail; `details` is the evidence annex; `line_map` is the
navigation index. Keep them separate when presenting a result: pasting details
under each trail line would mix producer provenance into a narrative that is
supposed to remain readable. For an audit, show the trail and then the details
range for the event in question, with its `unavailable` fields intact. The
details header states its own line-number convention, so a reader never has to
infer whether ranges are inclusive.

### Escaping and previews

Every producer-controlled display field is escaped: backslash, quote, newline,
carriage return, tab, the Unicode line separators U+2028/U+2029, bidi controls
(U+061C, U+200E/U+200F, U+202A–U+202E, U+2066–U+2069, and U+206A–U+206F), and other
control characters (as `\xNN`). Embedded newlines therefore cannot create new
trail lines or spoof structure.

Strings and previews are truncated at `max_value_preview_bytes`; truncated
strings say `[truncated; size N bytes; full reference unavailable]`, while a
`Preview` carries the producer's `full_ref` when one exists. Truncation changes
the display only, never the recorded value.

## Determinism and stability

The output is a pure function of the observations and limits: no time, hash
iteration order, locale, or environment participates. The scope and reference
layers report the lowest-ID error first, the structure layer reports in event
order, and value formatting is fixed. The deterministic golden corpus in `render.rs`
exists to keep that stable; when a legitimate format change lands, update the
snapshots rather than loosening the assertions.

## Limits

`Limits` bounds cost, not meaning. Defaults:

| Field | Default | Bounds |
| --- | --- | --- |
| `max_events` | 4,096 | Number of events |
| `max_scopes` | 1,024 | Number of scopes |
| `max_nesting` | 64 | Call + control-frame depth |
| `max_field_bytes` | 16,384 | Each producer text field (UTF-8 bytes) |
| `max_value_preview_bytes` | 256 | Rendered string/preview text |
| `max_output_bytes` | 524,288 | Trail + details combined |

The main text budget is charged line by line as rendering proceeds, and the
combined budget is checked again before returning. Exceeding a limit is a
`TraceError`, never a truncated-but-successful result.

## Errors

`TraceError` is part of the producer contract. It should be reported or fixed
at the producer, not replaced with a partial narrative.

| Variant | Typical cause |
| --- | --- |
| `LimitExceeded { what, limit }` | Scope/event count or nesting depth over the configured bound |
| `OutputLimitExceeded { limit }` | Trail plus details exceed the byte budget |
| `FieldTooLong { field, limit }` | One producer text field is over `max_field_bytes` |
| `DuplicateEventId` / `DuplicateScopeId` | Two records claim the same ID |
| `UnknownScope` | An event references a scope that was not supplied, or scope 0 is missing |
| `InvalidScopeParent` | A parent is absent, or root scope 0 has a parent |
| `ScopeCycle` | Parent links form a cycle |
| `UnknownEventReference` | Assumed evidence or a reference points at a missing/non-branch event |
| `InvalidBranchStructure` | Join/return/stop exit disagrees with the branch's recorded state |
| `InvalidCallStructure` | Child scope/call mismatch, completion mismatch, or a resume without a completion |
| `InvalidFinallyStructure` | Pending completion mismatch, nested finally, or return/throw inside finally |
| `InvalidLoopStructure` | Ordinal, iteration, exit, or end ordering is wrong |
| `MissingProofEvidence` | `ProvenDead` has empty proof or evidence reference |
| `UnsupportedEvidence` | Observed/assumed evidence conflict, empty capture identity, or a bad source span |

`EventAfterOpaqueStop` remains in the public error surface; current structure
validation rejects post-stop events as ordinary actions after a terminal
completion.

## Integrating a producer

Keep `Observed`, `StaticDerived`, and `Assumed` evidence distinct. Structural
validation can reject an impossible call/resume sequence or a missing proof
reference; it cannot re-certify a producer's claim that a branch is dead.
Represent an uncaptured outcome explicitly instead of filling in a plausible
value. Equal values do not establish shared-cell identity.

A producer should also:

- Assign stable event IDs and build scopes before emitting events; a child
  scope's `call` field must match the `CallStart.invocation`.
- Emit child events between `CallStart` and `CallEnd`, and end every child with
  a terminal record before its `CallEnd`.
- Pair `Return { PendingFinally }` with a matching `FinallyStart` and
  `FinallyEnd`; use the replacement variants when a finally changes the result.
- Give `ProvenDead` real premises; if the producer only suspects an edge, use
  `Suspected` and let the trail show it as unconfirmed.
- Choose `OpaqueStop`, `Stopped`, `Unknown`, or `Unavailable` at the real
  boundary rather than letting the renderer infer a completion.

`Limits` bounds events, scopes, nesting, fields, previews, and combined output;
set them from the expected trail size. Validation and output-limit errors are
part of the producer contract and should be reported, not replaced with a
partial narrative presented as complete.

## The public API

| Area | Items |
| --- | --- |
| Entry | `Trace::{new, with_limits, validate, render}` |
| Identity | `EventId`, `ScopeId` |
| Inputs | `TraceHeader`, `Mode`, `Scope`, `TraceEvent`, `EventKind`, `Evidence`, `Provenance`, `SourceSpan` |
| Semantics | `Value`, `TestResult`, `BranchStatus`, `BranchExit`, `CaptureClaim`, `CallCompletion`, `ReturnDisposition`, `PendingCompletion`, `FinallyCompletion` |
| Output | `RenderedTrace`, `LineDetail`, `TraceError` |
| Bounds | `Limits` |

`Trace::render` returns `Result<RenderedTrace, TraceError>`; `validate` returns
`Result<(), TraceError>`. `RenderedTrace` is `{ text, details, line_map }` and
is comparable and cloneable.

## Where a bug belongs

| Symptom | Inspect |
| --- | --- |
| A valid-looking event sequence is rejected | The active-invocation and control-frame ordering; events must be depth-first and scoped to the active call |
| A call is rejected for its child scope | `CallStart` requires `child.parent == caller` and `child.call == invocation`, and each child scope may be entered once |
| A `Resume` fails | It must follow a `CallEnd` whose child returned, use the caller scope, and carry the exact value |
| An assumed branch is rejected | Its evidence must be self-referencing at the start and cite an assumed ancestor inside |
| A proven-dead branch is rejected | Empty `proof` or `evidence_ref`; the renderer does not synthesize either |
| A finally fails | The pending return/throw must match `FinallyStart`; replacements must use `FinallyEnd`; nesting is unsupported |
| A loop fails to close | Ordinal must be sequential; `LoopExit` must be outside an iteration; normal end needs a false exit |
| Text is rejected for size | A producer field exceeds `max_field_bytes`, or trail+details exceed `max_output_bytes` |
| The line map looks off | An event may render zero or several lines; ranges are inclusive and merge only adjacent same-event lines |
| Display text contains unexpected escapes | Producer text is escaped by design; check the field rather than the renderer |
| A value renders as a quote-wrapped preview | It is a `String` or `Preview`; `Reference` renders as `ref(…)` and lacks content |
| An error names a different event than expected | Reference and scope errors report in ID key order to be deterministic |

## Tests

```sh
cargo test --test trace_text --test trace_text_review --test trace_text_structure
```

The focused suites cover
exact multi-step output and ranges, value origin, call/resume handoff, finally
replacement, stopped assumed siblings, proof-vs-suspicion evidence, Unicode
escaping, malformed structures, and bounds. The renderer also carries a
deterministic golden corpus in `render.rs` so structural results stay stable
across changes.

An invalid observation produces an error rather than a plausible-looking trail.
The [module README](../../src/tracing/text/README.md) contains the implementation
map and essential producer invariants; the [tracing overview](../../src/tracing/README.md)
places this renderer alongside symbolic execution. The `shape trace` commands do
not currently produce this observation model from a capture; their live debug
text is a separate surface.
