# Program generator inputs

This area recovers the **inputs a generator needs** from one emitted
`program.js` capture, and can optionally rewrite that same original source with
canonical role names and explanatory comments. It is an extractor and inspection
surface, not a shipped header encoder/decoder and not a codec claim.

## Commands

```sh
shape naming threads seed1.wave.js --rename --out reviewed.js --map-out mapping.json
shape naming collectors program.js --map-out collectors.json
shape signals show program.js            # compact fact summary
shape signals show program.js --json     # typed report (serde JSON)
shape signals contracts program.js [--json] [--signal N] [--pair other.js]   # signal contracts (../signals/contracts.md)
shape naming corpus <source>... [--baseline FILE] [--update-baseline] [--reference LABEL|PATH]
```

- `threads` names remaining generated function bindings and assigns proven grouped
  collector registrations their positional `waveCollectorN` names. Other readable
  KEY/main/local/function names remain intact. It uses small static evaluators;
  see [threads.md](threads.md). Verified seed counts are recorded in
  [thread-baseline.md](thread-baseline.md).
- `collectors` provides the focused identity/routing map and optional collector
  renames. These two focused commands always require new output files.
- `show` prints the descriptor registry, wave/direct routing, per-signal ordered
  emission layouts, session/context contract, header/envelope/hash/serializer
  facts and external runtime inputs. `--json` prints the full
  `ProgramProperties` report.
- The focused commands use create-new outputs; proven grouped registry positions
  take precedence over readable purpose labels, which remain available as metadata.
- The former broad annotation and rename CLI entry points have been removed; use
  `threads --rename` for focused function edits.
- `corpus` runs the whole chain per seed (raw capture -> linked program ->
  naming plan -> renamed/annotated source), fingerprints it, and compares a
  reviewed baseline, a reference seed, or vendor groups. It is the durable
  replacement for target-list coverage reports; see [corpus.md](corpus.md).
- [`shape::signals`](../../src/signals/mod.rs) turns those facts into the
  addressable per-collector registry: collector -> `positionalSignals` position
  -> descriptor -> ordered wire and per-emission XOR; see
  [positional signals](../signals/positional-signals.md).
- Recovered JavaScript is **never executed**. The commands only parse, analyse
  and edit source text in Rust with Oxc 0.120.

## Library API

```rust
use shape::signals::extract::extract_program_properties;

let properties = extract_program_properties(&source)?;
println!("{} descriptors, {} signals", properties.coverage.descriptor_count, properties.coverage.signal_count);
for signal in &properties.signals {
    for emission in &signal.emission_layouts {
        // ordered STRING / NUMBER / BYTE wire tree, prefix seed and mask
    }
}
```

`extract_program_properties` returns a serde-serializable `ProgramProperties`:

| Fact | Where |
| --- | --- |
| Exact source identity (SHA-256, bytes) | `source` |
| Main scope, top-level call, host event bridge | `program_scope` |
| Main-owned bindings with a proven `BindingRole` and exact identifier span | `bindings` |
| Number/string/PRNG/observer/snapshot/hash helper bodies | `helpers` |
| Descriptor registry (base literal + indexed population writes) | `descriptors` |
| Wave function array, index rows, routing selector, direct slots | `waves` |
| Per-signal route, collector, raw emissions and reconstructed layouts | `signals[].emissions`, `signals[].emission_layouts` |
| Session constructor, context fields, argument mapping, stores | `session` |
| Envelope fields and dispatch header prefix/chunk size | `session.envelope` |
| Serializer factory, default encoder and alphabet | `session.serializer` |
| Hash chain from the per-session token state to the runtime token | `session.hash_chain` |
| Numeric tag table | `constants` |
| Per-emission static XOR captures with exact source literals | `static_captures` |
| Runtime-only inputs (session arguments, context fields, host globals) | `external_inputs` |
| Host side-effect probes that complete an observer without a value | `host_probes` |
| Explicit gaps | `unresolved`, `diagnostics` |

`host_probes` records a collector that is a probe rather than a value
producer: it hands the observer to a scheduler, calls `complete()`, and records
its result as a host side effect. The detected shape is a
`new CustomEvent(<string literal>)` inside a callback whose subtree calls
`.complete()` and `.dispatchEvent` and never mentions a `.next` property. The
event literal is data, not identity; when the probe belongs to a direct
descriptor task, `signal` carries that descriptor index. Such a signal has no
wire to reconstruct, so it is reported here instead of as an emission.


The high-level identification adapter lives in the transform layer:

```rust
use shape::naming::program::{identify_and_transform, IdentifyOptions};

let identified = identify_and_transform(&source, IdentifyOptions::full())?;
println!("{}", identified.transform.transformed_source);
println!("{} rename(s), {} comment(s)", identified.applied_renames(), identified.applied_annotations());
```

`IdentifiedProgram` carries the original `properties`, the unified naming plan
and the rename plans derived from it (see [naming.md](naming.md)), the comment
plans, the transformed source and both source hashes.

## Function index

`extract_function_index` is a second, exhaustive view of one exact capture. It
lists every callable once (named, anonymous, object/class method,
getter/setter/constructor and arrow) under a source-derived
`fn@<byte-start>..<byte-end>` id, wires the proven lexical call graph, and records
the direct bindings of the broadest reachable main scope.

```rust
use shape::naming::function_index::extract_function_index;

let index = extract_function_index(&source)?;
```

| Fact | Where |
| --- | --- |
| Source identity (SHA-256, bytes) | `source` |
| Main scope selection basis and top-level call | `main` |
| Every function with kind, spans, parent, params and own bindings | `functions` |
| Proven lexical call edges | `call_edges` |
| Unresolved calls with the exact callee source and a reason | `unresolved_calls` |
| Main-scope owned bindings with assignment classes, evidence and alternatives | `main_variables` |
| Counters | `coverage` |

Honesty rules (the full contract is in
[`src/signals/README.md`](../../src/signals/README.md)):

- `is_top_level_entry` is call reachability at the program top level, not a rank.
  An ordinary top-level call, a wrapper entry and an IIFE are entries; a
  `main(host)` nested inside a wrapper is reachable but is not itself an entry.
- A call edge requires a single-identity proof. `let x; x = A; x = B; x()` stays
  `ambiguous_assignment` and `let x = A; x = unknown(); x()` stays
  `possible_assignment` — neither names `A` as the callee. Every declaration
  origin of a SymbolId is inventoried, so sloppy duplicate function declarations
  and repeated `var` declarators are separate definitions, not a first candidate.
- `held.proven_static` is true only for a single unconditional definition. A
  conditional, nested, compound/update or repeated literal write is
  `mutable_state` with every definition listed in `alternatives` — never a single
  guessed current literal. Each definition's scope class is its own AST location,
  so a condition/block/loop-nested initializer is not unconditional.
- `caller_count == 0` does not prove a function dead: dynamic callbacks stay
  unresolved on purpose.

Observed on the three immutable Target captures (structural assertions, not
count gates), seeds 1/2/3:

| Quantity | seed 1 | seed 2 | seed 3 |
| --- | --- | --- | --- |
| functions (named + anonymous) | 636 (634 + 2) | 641 (639 + 2) | 653 (651 + 2) |
| main-owned bindings | 411 | 414 | 415 |
| proven call edges | 994 | 1033 | 1049 |
| unresolved calls | 3367 | 3443 | 3584 |
| functions with a proven caller | 120 | 121 | 124 |
| functions without a proven caller | 516 | 520 | 529 |
| top-level entries / main is an entry | 1 / no | 1 / no | 1 / no |
| main variables `RuntimeState` / `proven_static` | 58 / 362 | 59 / 364 | 59 / 365 |

The counts are capture observations, never recognition gates. In each capture the
single top-level entry is an outer declaration (depth 0) and the exported `main`
is a distinct function nested inside it — reachable, but not itself a top-level
entry. Unresolved and ambiguous edges, `mutable_state` values and the main-scope
selection each carry explicit uncertainty rather than a guessed value.

## Signal model

- `STRING`, `NUMBER` and `BYTE` are the only leaves. `WireOp::Sequence`,
  `WireOp::Repeat` and `WireOp::Choice` are the only containers.
- `NUMBER` is not asserted to fit `i64`. `BYTE` includes plaintext prefix bytes
  and masked direct stores.
- The four plaintext prefix bytes are four individual `BYTE` leaves (or a
  `REPEAT 4` of one `BYTE` when the array elements are not readable), never one
  wide `NUMBER`.
- A mask is `PRNG(seed_word ^ C)`; `C` is the per-emission static literal kept
  in `MaskSeed::xor_literal` / `xor_source` and repeated in `static_captures`.
- A capture that traces several threads repeats each encoder helper once per
  thread. Every proven copy is listed for its role, and a write is typed by the
  callee's own lexical binding, so one copy never types another's writes.
- The seed word, session token state, session arguments, context fields and host
  globals are **runtime** inputs. Values are not invented.
- A raw named object (for example the descriptor-named product identity object)
  is a `GenericObject` emission with typed logical fields and no per-signal
  PRNG, prefix or mask.

Every signal also has one addressable entry in
[`shape::signals`](../../src/signals/mod.rs): the wave collector and
`positionalSignals` position that own it, the descriptor it reports, the ordered
`PREFIX`/`NUMBER`/`TEXT`/`BYTE` wire of each emission, and that emission's
static XOR literal. The checked-in `ssx` capture keeps the mapping as data with
the exact program hash and regeneration commands; see [positional signals](../signals/positional-signals.md).

## Identity rules

- Every signal is found through its registry entry, wave row or direct
  constructor, observer wiring, emitted buffer, ordered writes, encoder and
  PRNG/mask — never through a generated spelling, signal index, wave count, XOR
  literal or capture string.
- A direct registry slot is scanned at the **task callback** passed to the
  constructor wrapper, not at the wrapper factory.
- Signal indices are capture-local. The same index has different semantics in
  the three captures.
- Counts (66 descriptors, 13/13/14 waves, 2/3/2 direct slots) are test
  expectations, never recognition gates.

## Boundaries

- Facts outside the accepted corpus are not guessed: unresolved writes stay
  `WireOp::Unknown`, unproven fields stay untyped, and required gaps are listed
  in `unresolved`/`diagnostics`. A missing value emission, prefix, mask or
  payload is a required `UnresolvedFact`; `required_missing_facts()` is not
  cleared just because a signal has a partial layout.
- The generic token encoder's byte-exact grammar and the state cipher are not
  claimed here.
- The host call of the exported initializer is a boundary: only the in-file
  constructor call and session arguments are proven.
- Annotations are comments at parsed statement boundaries; they are escaped,
  never treated as values, and idempotent.

## Tests

```sh
cargo test
```

The test suite is synthetic and self-contained: naming, renaming, local names
and the corpus command are exercised on small sources and fixtures. The former
external-capture acceptance suites and target lists have been removed; run
`shape naming corpus --baseline` against reviewed captures when you want that
check. Production never reads a report JSON or embeds per-seed expected values.
