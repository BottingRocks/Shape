# Focused function and class naming, reusable labels and dead empty declarations

```sh
shape naming threads readable.js --rename --out review/output.js --map-out review/map.json
```

Input is UTF-8 recovered JavaScript. The command never executes it or retraces a
capture. Report-only mode (omit `--rename --out`) returns byte-identical source;
without `--map-out`, JSON goes to stdout. Every output must be new: existing
files, input/output/map aliases, symlinks/hard links and `..` output traversal
are refused. There is no force-overwrite mode. The former broad annotation and
rename CLI commands are removed and reject immediately.

Generated `$THREAD_<id>_<pc>` and producer collision forms `$THREAD_<id>_<pc>_<n>`
(canonical decimal `n >= 2`) **function bindings** and supported **native class
bindings** are renamed together with their resolved references. Existing
meaningful KEY/main/local names, parameters and human names stay intact.
Checked lexical renaming handles recursion and shadowing,
expands shorthands while preserving their keys, and refuses capture, collisions,
dynamic scope and invalid rewrites. Native method and constructor property keys
keep their names; recognized method roles remain metadata.

Thirteen standalone finder packages and the purpose finders run on the existing
scoped parse. No full property/layout extraction, naming plan, role graph,
anonymous naming or local-variable renaming runs. Individual finder timings are
reported. The graphics/canvas package is excluded from this interactive path:
it alone took 15.6 seconds in the diagnostic, while every other package took
20–305 ms. Its existing internal inspection API is retained. Small local
summaries and optional structural labels supplement the independent finders.
Function claims come with confidence and evidence; conflicting names remain
explicitly ambiguous. Generic aliases are not purpose evidence.

The bounded `broadcastSignalNotification` rule checks the complete local
notification path: four immutable fields copied from one input, the exact
record passed to each listener, a distinct definition-table length, and two
filter counts from the same captured state table using `hasCollected` and
`hasErrored` predicates. Callback parameters, record writes and captured tables
are compared by scoped identity. Changed APIs, arguments, returns, fields,
redeclared locals or substituted objects refuse the rule. The proof uses the
existing scope index and never generated names or inherited reference labels.

With `--rename`, the process also removes unreferenced, empty **direct scoped
function declarations**. Empty means no statements and no directives. References,
duplicate/redeclared bindings, top-level declarations, sloppy block/Annex B
uncertainty and any eval/with uncertainty prevent removal. Referenced no-op
callbacks and live throwing helpers survive. This is not general DCE or clone
deduplication. `removals` reports actual original-coordinate deletion spans and
reasons separately from naming. When a declaration occupies otherwise whitespace
only lines, its indentation and line ending belong to that deletion span; no
blanket formatting or trimming occurs. Every other byte is preserved.

## Native class bindings

The class route recognizes `Observable`, `Subscription`,
`SubscriptionObserver`, `ValueEncoder`, `BatchScheduler`, `RingBuffer`,
`CallbackNotifier`, `LiveCallbackNotifier` and `LazyHookRegistry` through their own state, method
behavior and scoped alias relationships. Factory-based proofs identify the
actual returned class. Subscription proofs also recognize the source's separate
`closed` getter installation when its descriptor, getter and prototype receiver
belong to that same class. Unproven property mutations and ambiguous owners
remain explicit refusals.

`CallbackNotifier` traverses a reverse snapshot; `LiveCallbackNotifier` traverses
the live list forwards. Their proofs and labels remain distinct. Lazy registries
require a verified notifier base, ordered reachable installation, and an owned
callback protocol. Ordinary constructor/prototype bases and unsupported callback
object-holder forms remain unnamed. A seven-seed focused checkpoint verified
this bounded change; it does not replace a full baseline.

Class declarations and their references use the checked binding renamer. Existing
function proposals are reserved first. Competing class identities or a collision
with a reserved proposal refuse the class rename. Class expressions with
unproved inner/outer binding relationships receive an explicit refusal reason.

The JSON report adds `class_outcomes`, `generated_class_count`,
`applied_class_count` and `unresolved_class_count`. Each class outcome records a
`class@start..end` identity in original UTF-8 byte coordinates, its binding,
proposed name, evaluator, evidence, application result and refusal reason.
Function counters and `outcomes` retain their existing denominator. Class names,
class aliases and method-role metadata are counted separately from function
rename gains. Reusable reference signatures continue to describe functions;
class identities are proved from the current source.

## Reusable labels

Build a reference once from an **unpruned**, readable source and its equal-build
original trace, before removing dead stubs:

```sh
shape naming threads unpruned-readable.js --original original.js --save-labels labels.json
shape naming threads another-seed.js --labels labels.json --rename \
  --out review/another.js --map-out review/another.json
```

Reference creation is a separate report-only mode. A differently pruned source
fails the equal-build proof; function order must never join unrelated builds.
The proof compares the full canonical AST and scoped reference identities,
retaining properties/literals/control flow and ignoring comments and unused
inserted expression names. This permits a same-build provenance join only.

Across builds, signatures normalize binding spellings while retaining lexical
reference relationships, API/property names, constants, operators, parameter
relationships and control flow. Distinct captured symbols keep distinct first-use
ordinals and a shared capture-origin graph, even when their initializers match.
Mutable captured functions and other ambiguous/multiple-origin captures are
refused. Capture expansion is bounded. Empty bodies carry no transferable
semantic identity. WaveCollectorN positional labels are not transferred across
builds. Unmatched/different literals and ambiguous identical wrappers remain
unknown rather than being made equivalent. Whole-program function order, THREAD
IDs, PCs and per-seed manual maps are never cross-build recognition keys.

The reference persists source hashes, equal-build proof, labels, confidence,
evidence and structural signatures. Reports distinguish inherited structural
labels from independently recognized purposes. Existing readable names are kept
except for proven grouped collector registrations, whose positional names are
authoritative. `outcomes.applied` records binding renames;
`collectors.renames` also records anonymous registry identities. Proposals and
metadata alone are not coverage. `function_roles` includes noneditable and
existing-method roles separately.

Public APIs are `shape::naming::{name_threads,name_threads_with_labels}`
and `shape::naming::labels::build_label_reference`.
The report contains source/output hashes and a validated `edits` ledger. Replay
edits in descending byte offset to reproduce the output. The ledger contains
identifier edits (with shorthand expansion), anonymous identity-holder insertions,
and separately reported empty-declaration removals. All spans refer to the exact
original UTF-8 input.

The embedded collector map keeps descriptor indices, wave row slots and final
payload positions separate. Positions are conditional on a source-proven static
default filter. Custom or externally mutated filters can reorder/repeat/omit
signals. Unsupported partition shapes are explicit refusals. The separate
`naming collectors` command still provides mapping/collector-only naming.
Readable direct-collector names are kept; grouped registrations use positional
names even when their previous label described a semantic purpose.

See [internal naming APIs](naming.md) and [program inspection](README.md).

Each proven grouped registry slot uses `waveCollector{slot + 1}`: slot zero is
`waveCollector1`. Signal membership, permutation and declaration order do not
determine that name. `collectors.registered_groups` records registry identity
independently of the stronger inner-emitter/routing proof, so incomplete routing
does not prevent positional naming. Naming combines those facts with existing
positions proved by `collectors.wiring.groups`; matching claims count once and
conflicting positions are refused. Independent registry discovery rejects
ambiguous or mutable origins. Anonymous arrows receive a capture-safe private
identity holder that
preserves their lexical `this` and `arguments`; these identities are counted
separately from generated function-name gains.

Finer package, prior readable and inherited purpose claims are retained under
`aggregate_collector_claims` as metadata. Wave positions are not cross-seed
semantic-role identities. The separate platform-hook registry and direct
descriptor callbacks do not receive positional wave names.

The exact live hook that only throws global `Error("serialisation is hard-coded
into the encoder")` is named `throwHardcodedSerializationError`. The constructor
must be unshadowed, the argument must be that literal, and no other body statement
or directive may be present. The body and live references are preserved. When
building a reference from an earlier readable snapshot, replay its checked
identifier edits while omitting deletion edits, then pass that unpruned source to
`--save-labels`; the equal-build proof rejects a pruned reference.

Small shared rules also compose independently proven function roles through
scoped, single-origin function bindings and aliases. They recognize warmed
keystream factories, interned string writer factories, signal observer factories
(with methods in the literal or assigned once), and captured array comparison
wrappers. Exact AST rules cover RGBA channel concatenation and conditional
zero-argument `empty` calls. Different arguments, receivers, returns, literals,
mutable helper bindings and unsupported shapes do not inherit these claims.
The rules run before reference matching; their `shared_scoped_role` evidence is
independent of the optional readable reference. Their cost is reported as
`shared_scoped_rules` within the static-evaluator phase.

A late post-host rule extends the seeded field emitter to the verified
WebRTC-candidate form: one null-guarded formal, one `Math.random() * 2^32` seed
packed most-significant-byte first into a four-byte header, one `seed ^ <number>`
reseed and derived writer, the candidate-list length and every candidate
`ip`/`candidate` string mixed through that writer, then `next([[], header])` and
`complete()` on one captured observer. It reads the seed and header through the
same `seed`/`seed | 0` symbol; the running state may be the seed itself
(`seed = normalize(seed ^ k)`) or a fresh normalized local, and the writer is
derived from that exact running-state symbol (never a different argument or
generated spelling). It accepts the forward and descending counter forms and
either field order; the descending length mix may forward the cursor local or the
same formal `length` it was initialized from, and its exit accepts the actual
unbraced `if (!(count >= 0)) break;` or the equivalent single-statement braced
spelling, while a labeled break or any other exit stays unclaimed. It has no
outer-container fallback and other media fields, wrong nonces/writers,
non-captured or disconnected observers, shadowed `Math` and nested callback
copies stay unclaimed.

Concrete spelling remains owned by the checked renamer and, for simultaneous
same-role duplicates, by the duplicate-role allocator described below. This rule
only proves the canonical `emitSeededHashEncoding` role identity on the exact
binding; a later refusal to allocate a concrete name is reported as the
pre-existing policy, never as a missing proof.

Four explicit body-template groups disambiguate queue runners, once-callback factories,
class descriptor installers and their factories. Reconciliation requires exactly
one name proven by a current-source evaluator, and every other claim must belong
to that same enumerated group. The current evaluator's name is retained and the
weaker inherited body-template claims are recorded in its explanation. Queue
context roles are not universal synonyms. Reference-only
ambiguity, unrelated labels and independent disagreements remain unresolved.
Lexical edit safety checks still run after reconciliation.

The seeded-emitter shape has two independently proved labels over the same
straight-line owned body: `signals_seeded_signal_frame` proves the signal frame
(`emitSeededSignalFrame`) and `encoding_seeded_hash_encoding` proves the seeded
hash encoding (`emitSeededHashEncoding`). When both named proofs are present and
every independent claim on the body is already one of those two labels, they
are one emitter, so the conflicted pair collapses onto the canonical governed
label `emitSeededHashEncoding`. Solo claims, foreign evaluators, inherited
labels, genuine third disagreements, and unrelated families remain untouched.

## Capability layout and report schema 2

The rules are private modules under `src/naming/`, grouped
by capability: `events`, `signals`, `encoding`, `graphics`, `scheduling`,
`browser`, `runtime` and `network`. `threads.rs` remains the public
`properties::threads` entry point and keeps the report types; `stages.rs` owns
the frozen stage order, `shared.rs` the shared consequence and reconciliation
proofs and `resolve.rs` the ordered callable resolver. Per-capability micro-AST
helpers live under `threads/support/`. See `threads/README.md` for the full
layout and stage rules.

`ThreadNamingReport.schema_version` is `2`. Evaluator ids now name the rule
capability instead of a capture family, and a source that proves several roles
reports a separate id per role (for example the six roles formerly reported as
one console/vector family now report `events_console_error_statement`,
`scheduling_sine_prng_refill`, `graphics_bounded_pixel_vector_history`,
`browser_submit_instrumentation_teardown`, `encoding_seeded_hash_encoding` and
`events_visibility_change_sample`). Migrating a schema-1 consumer means mapping
the old family id to the id of the role it actually names; role names, claim
order, confidence, evidence, explanations, ambiguity outcomes, edit ledgers and
the emitted JavaScript are unchanged. Shared ids that were already capability
names (`shared_anchor_console_error`, `shared_elapsed_timer`,
`shared_serializer_error`, …) did not change.

Single-assignment alias resolution requires an actual plain variable declarator.
An initializer with no later writes or an initless variable with exactly one
assignment may supply its function origin. Parameters, catch bindings,
destructuring and for-in/of bindings cannot qualify as initless aliases: each
has a distinct runtime origin. Function declarations require zero writes and no redeclarations. Alias variables
also refuse redeclarations; equal spellings in separate lexical scopes remain
independent bindings.

### Additional shared variants

Observer factories accept a local plain object with `next` in its literal and
`error`/`complete` assigned afterwards. The returned binding must be that exact
object, with no escape, redeclaration, reassignment, dynamic property, duplicate
method or overwrite of a real method. Only generated `void 0` placeholders may
be filled. All three nested callbacks require independent scoped role proofs.

The asynchronous string/view hash dispatcher requires the exact string test,
guarded global `ArrayBuffer.isView` branch, unchanged argument slots 0–3 to
independently proven string/view hash functions, and error slot 3 receiving
`new TypeError()`. Reference labels and generated spellings cannot prove its
callees; mutable or ambiguous origins remain refused.

Collision suffix metadata records `thread_id`, `pc` and optional `disambiguator`
while the complete original binding and spans remain its identity. No stem join
is performed. Unsupported THREAD-prefixed function names remain in the actual
generated count and unresolved outcomes, with null numeric identity metadata.

Distinct functions that independent evaluators positively identify with the
same semantic role are allocated deterministic, scope-safe concrete names. The
bindings that the older policy already applied keep the meaningful unsuffixed
base; a suffixed clone or a simultaneous duplicate in the same or an
ancestor/descendant scope (including Annex B hoisting of sloppy block
declarations) receives the first free ordinal suffix (`name2`, `name3`, ...) in
stable declaration order. Sibling scopes the exact renamer proves independent
may still share the base. Every existing readable/human binding and every
unresolved free name is reserved, so a duplicate never reuses one of those
spellings: if the unsuffixed base is already owned by such a name, the group
receives the first free ordinal instead and the existing name is left
untouched. `ThreadName.role_name` keeps the canonical semantic role while
`proposed_name` is the concrete unique binding name; `function_roles` continues
to report the canonical role. The exact reference validator still decides
whether each concrete name is applied, so dynamic scope, reassignment, capture,
Annex B and binding-site refusals are never forced. Unsupported generated names
retain even empty bodies rather than disappearing through the empty-declaration
removal pass.
