# Function naming

`shape::naming` names the functions and bindings of one recovered
`program.js` from structure alone: small static function evaluators
(`name_threads`), collector identities, the semantic naming plan, the
function index / role graph inventory, class proofs and the program rewrite
engine. Readable bindings are retained except for authoritative proven
grouped-registry positions (`waveCollectorN`). Nothing here executes the
program.

```sh
shape naming threads|collectors|plan|inventory|annotate|corpus <program.js> …
```

## Layout

| Path | Responsibility |
| --- | --- |
| `mod.rs` | Public entry points (`name_threads`, `name_threads_with_labels`), report types, proposal/resolution helpers |
| `stages.rs` | The frozen fallback order of the thread rules (order decides which name wins) |
| `shared.rs`, `resolve.rs`, `support.rs`, `support/`, `operations.rs`, `operations/` | Shared consequence proofs, ordered-callable resolution, per-capability micro-AST helpers, operation-level rules |
| `functions/` | Thread rules by capability: `browser/`, `encoding/`, `events/`, `graphics/`, `network/`, `observables/`, `runtime/`, `scheduling/` |
| `classes/` | Whole-class proofs (batch scheduler, callback notifier, encoder, lazy registry (+transport), pre-register forwarder, ring buffer, subscriptions) with their tests |
| `native_classes.rs`, `labels.rs` | Native class names and the reusable label references |
| `collectors.rs`, `collector_payload.rs` | Collector identities and positional `waveCollectorN` names |
| `semantic_names.rs` | Source-stamped descriptive naming plan for proven functions and main-owned data (claims from `signals::analysis::roles`) |
| `function_index.rs`, `role_graph.rs` | The function/class inventory and the role graph (below) |
| `output.rs`, `inputs_context.rs` | Output properties and the context joins of `shape signals inputs` |
| `program/` | The rewrite engine: explicit renames, anonymous/local names, opt-in annotations ([`program/README.md`](program/README.md)) |

## Function index

`function_index.rs` is a deliberately exhaustive view of one exact
capture. It is independent of the signal generator-input roles and answers three
plain questions: which callable functions exist (named, anonymous, object/class
method, getter/setter/constructor and arrow), each listed once under a
source-derived `fn@<start>..<end>` id; how they relate through proven call
edges; and which direct bindings the broadest reachable main scope owns.

```rust
let index = shape::naming::function_index::extract_function_index(&source)?;
let index = shape::naming::function_index::extract_function_index_with_limits(&source, limits)?;
```

Soundness rules:

- `FunctionRecord::is_top_level_entry` is true only when a function is invoked
  directly at the program top level — an ordinary top-level call, a wrapper
  entry or an IIFE. A `main(host)` nested inside a wrapper is reachable but is
  **not** itself a top-level entry.
- A call edge is emitted only on a sound single-identity proof: a function
  declaration, or a lone function-valued definition with no other write. Every
  other case is an explicit `UnresolvedCall` with the exact callee source and a
  reason: one known candidate that other writes could change is
  `possible_assignment`; two or more distinct candidates is
  `ambiguous_assignment`; writes the inventory cannot see through with no known
  candidate is `dynamic_assignment`; an unbound name is `external_identifier`.
  Every declaration origin of a SymbolId is inventoried, so sloppy duplicate
  function declarations and repeated `var` declarators count as separate
  definitions. The candidate and assignment inventories are kept; they are never
  collapsed to a first candidate by source order.
- `caller_count == 0` is not a deadness claim. Callbacks reached only through a
  member call, an alias or an external stay unresolved on purpose and keep their
  exact callee source and reason. `functions_without_callers` is therefore an
  "unproven caller" count, not a dead-code set.
- `MainVariableRecord.held.proven_static` is true only for a single
  unconditional definition. A conditional, nested, compound/update or repeated
  write is `mutable_state`: the value fields are cleared and every definition is
  listed in `alternatives` with its `AssignmentClass` scope and, where static,
  its closed literal/array/object shape. Each definition's scope class comes from
  its own AST location, so a condition/block/loop-nested initializer is not
  unconditional. A representative `source` is labelled as evidence, never as the
  current value.
- Main variables are keyed by `SymbolId` (`mv@<start>..<end>`), so one spelling
  reused across scopes is several records rather than one multi-purpose slot.

### Functions, bindings and classes

A function and the bindings that hold it are separate identities
(`FUNCTION_INDEX_SCHEMA_VERSION` 2):

| Form | `FunctionRecord.name_binding` | `FunctionRecord.holders` |
| --- | --- | --- |
| `function f(){}` | `f` — the same symbol as the MAIN record `f` | `f`, `declaration` |
| `let a = function b(){}` | `b`, the inner self-name (a different symbol) | `a`, `initializer` |
| `let f = () => …` / `let g = function(){}` | none | `f` / `g`, `initializer` |
| `let x; x = function(){}` | none | `x`, `assignment` |
| `let a = f` | — | `a`, `alias` (also listed on `f`'s function) |
| methods, getters, setters, constructors | — | none; `class_id` names the owning class |

Each holder carries a `HolderProof`: `proven` only when the binding's
resolution is a single definition with no other write **and** every definition
runs unconditionally in the declaring scope. One candidate that a conditional,
compound or update write could change is `possible`; repeated distinct values
are `ambiguous`; unmodelled writes are `dynamic`. Destructuring and `for…of`
writes never create a holder link. No holder is chosen by last assignment.

Classes are `ClassRecord`s (`class@<start>..<end>`, `FunctionIndex::classes`)
with their kind, own name binding, holders, constructor/method/getter/setter/
field function ids, superclass preview and parent function. They are values,
not callables: `coverage.function_count` stays the number of Function/Arrow
nodes and `coverage.class_count` is separate. A class declaration's MAIN
binding is `declaration_kind = class`, `classification = Class`, initialized by
the declaration (`unwritten = false`); `let k = class {}` is `Class` with
`held.class_id`. A function declaration's binding is likewise initialized by
its declaration, so its `assignment_class` is the declaration's location rather
than `Unwritten`.

## Role graph

`role_graph.rs` joins the function index with the encoding and wiring passes
into one node per function, MAIN binding and class (plus explicit unresolved
call targets). Every node has a source-derived role or an explicit unknown.

- **Value identity before role uniqueness.** Each node records a canonical
  `value_identity`: a binding that provably holds a function or class takes that
  id, and an immutable alias follows `alias_of` while each hop is a proven single
  unconditional definition whose target is initialized first. Unique-role checks
  compare value identities, so a holder and its own alias are not a conflict, but
  two different values with one unique role are. Holder and alias keep separate
  nodes; merged groups are listed as `role_identity_canonicalized` diagnostics.
- **Edge coverage.** `reads_main_binding` / `writes_main_binding` come from the
  per-function MAIN captures, which exclude the MAIN function's own body. They
  cover accesses from nested functions only and are not an assignment
  inventory: zero write edges does not mean a binding is never initialized. Use
  `MainVariableRecord.writes` and `MainVariableRecord.assignments` for total
  assignment questions.
- Classes are `class` nodes linked by `holds_class` (MAIN binding → class) and
  `class_member` (class → constructor/method/getter/setter/field function)
  edges. Function node counts are unchanged.

## Thread rules

`mod.rs` is the public entry point (`name_threads`,
`name_threads_with_labels`, report types, the shared `Facts`/`Proposal`
helpers and the `proposal` constructor). Everything in this directory is
private rule implementation grouped by capability rather than by capture
family.

### Rule layout

| Path | Responsibility |
| --- | --- |
| `mod.rs` | Public API, report types, proposal/resolution helpers |
| `stages.rs` | The frozen fallback order: every stage must decline before the next one is consulted |
| `shared.rs` | Shared consequence proofs and cross-capability reconciliation |
| `resolve.rs` | Ordered-callable resolution shared by every stage |
| `support.rs` | Namespace for the small per-capability micro-AST helper bags in `support/` |
| `functions/events.rs` | Event, visibility, selector, touch, storage and console roles |
| `functions/observables.rs` | Signal frame, observer, serializer and relay roles |
| `functions/encoding.rs` | Seeded hash/entropy, packing, code-unit and writer roles |
| `functions/graphics.rs` | Canvas, font, pixel/vector and animation-frame roles |
| `functions/scheduling.rs` | Timers, task queues and PRNG refill roles |
| `functions/browser.rs` | Navigation, frame, proxy and injected-global roles |
| `functions/runtime.rs` | Runtime/collection carriers, worker and class-installer roles |
| `functions/network.rs` | WebRTC/ICE emitter roles |

Each domain module is a thin shell:

```rust
//! `events` thread rules.
use super::*;
use super::shared::*;

pub(super) mod visibility_change_sample;
```

and every leaf starts with `use super::*;` so it inherits the domain's globs.
Leaves that need one capability family's helper bag add an explicit
`use super::super::support::<family>::*;`. A helper that the permissive shared
version cannot express (for example the `var`/`let`/`const`-only declarator
proof) stays defined in the leaf that needs it, which shadows the glob import.

### Stage boundaries

`stages.rs` preserves the original pipeline exactly:

1. `evaluate_additions` runs every earlier pass in source order and returns
   their claims unchanged when any exist.
2. Only when all of them decline does the rolling frontier run.
3. `evaluate_worker_environment` keeps its own stage order and grouping.

Do not reorder, merge or short-circuit these calls. Stage membership, budgets,
confidence, evidence, explanations and emitted JavaScript are part of the
behavioral contract.

### Evaluator ids and report schema 2

`ThreadNamingReport.schema_version` is `2`. Evaluator ids describe the rule
capability, not a capture family:

- shared capabilities keep their established generic ids
  (`shared_anchor_console_error`, `shared_elapsed_timer`, `shared_serializer_error`, …);
- single-role capabilities use a capability id such as
  `events_visibility_method_alias` or `encoding_chacha_like_stream_cipher`;
- a source that proves several roles reports one capability id per role, for
  example `events_console_error_statement`, `scheduling_sine_prng_refill`,
  `graphics_bounded_pixel_vector_history`, `browser_submit_instrumentation_teardown`,
  `encoding_seeded_hash_encoding` and `events_visibility_change_sample` from the
  former six-role console/vector source.

Migrating from schema 1 means mapping the old family id to the id of the role
the report actually names; nothing else changes. Role names, proposal order,
confidence, evidence spans, explanations, ambiguity outcomes, edit ledgers and
the emitted JavaScript are unaffected by the schema bump. The one exception is
the historical `shared_anchor_case1` / `shared_anchor_case00005` agreement:
that reconciliation now compares `signals_seeded_signal_frame` with
`encoding_seeded_hash_encoding`, as documented in `docs/naming/threads.md`.
