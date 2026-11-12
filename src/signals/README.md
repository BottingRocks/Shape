# Signals

`shape::signals` is everything about the signals a Shape program collects:
reading them out of one recovered `program.js` (`shape trace program`
output), naming every slot with its canonical signal and every field from the
signal's approved card, and the wire codec and report payload around them.
Recognition is structural and scope-aware: every identity is an Oxc
`SymbolId` plus exact source spans; generated names, salts, slot addresses
and build constants are never match rules.

```sh
shape signals <program.js> --json      # the per-seed signal schema (docs/signals/schema.md)
shape signals show|inputs|contracts <program.js> [--json]
shape signals docs <dir>               # hidden: render docs/signals/*.md from the cards
```

```rust
let analysis = shape::signals::analysis::analyze_program_signals(&source, &Default::default())?;
let properties = shape::signals::extract::extract_program_properties(&source)?;
let card = shape::signals::catalog::card("DeviceMemory");
```

## Layout

| Path | Responsibility |
| --- | --- |
| `extract/` | Structural readers: `bindings` (scope/function indexes), `scanner` (+`grouped_registry`, `host_probes`: registry, waves, sessions, emissions, adapters), `wiring`, `encoding`, `layout` (per-emission ordered wire, prefix seed, PRNG/mask), `reaching`, `inputs`, `generator_inputs`, `contracts` (generator-ready typed grammar per emission; see `docs/signals/contracts.md`) |
| `analysis/` | `analyze_program_signals`: one `SlotView` per contract slot, every registered recognizer over it, the winner's binder, the report projection. `slot_view`, `write_sites` and one helper module per technique (`shape_text`, `leaf_checks`, `slot_closures`, `property_census`, `field_solver`, `value_facts`, `described_fields`, `buffer_writes`, …) shared by two or more signals |
| `analysis/roles/` | Finder roles (structure-proven function roles one recognizer can anchor on): `entry_runtime`, `vm_exec`, `collector_wiring`, `encoding_framing`, `serializer`, `request_envelope`, `carriers`, `interception_dom`, `browser_env`, `platform_gpu`, `interaction_timing`, `graphics_canvas`, `persistence_telemetry`, `purposes`, `function_interceptor` |
| `analysis/catalog_tests.rs` | Catalog-wide tests: registry, structural rules of every recognition source, vendor-seed corpus runs |
| `catalog/` | One flat file per canonical signal (`<signal_snake>.rs`): its `recognize_*`, `bind_*`, the helpers only it uses, and its card `CARD`. `mod.rs` registers all 93 (`SIGNALS`; `cards()`, `card(name)`); `card.rs` holds the card types |
| `flags/` | Packed flag bytes: bit positions from the written value, bit names from the card's `flags` lists (`mod.rs`, `loops.rs`, `flow.rs`, `rules.rs`) |
| `schema.rs` | The shared report types (spans, bindings, confidence, the schema report and its projection) |
| `docs.rs` | The `docs/signals/` pages rendered from the cards (`pages_match_cards` keeps them in sync) |
| `codec/` | The wire writers of one signal (keystream, number/string/byte writers, token hash) |
| `wire/` | Versioned per-seed contract metadata and the typed saved-schema consumer; validates source/pin evidence and exactly reencodes supported signal blobs |
| `payload/` | The report payload around the signals (framing, cipher, encode/decode); its CLI is `cli/payload.rs` |
| `timing.rs` | `SHAPE_SIGNAL_TIMES=1` phase timings on stderr |

## Cards

A card is the approved description of one signal: what it collects, why,
and its fields with their wire types, repeats and packed flag names. Each
catalog file ends with its card as Rust data:

```rust
pub(crate) const CARD: Card = Card {
    signal: "DeviceMemory",
    summary: "Reads navigator.deviceMemory once and sends it. …",
    why: "Device memory is a coarse hardware trait …",
    sample_payload: Some(r#"{"deviceMemory": 32}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number("deviceMemory", "Approximate RAM in gigabytes …"),
    ]),
};
```

`CardBody::Messages` lists one field list per message shape (`when`);
`CardField::group(name, &[…])` nests a record, `.repeat("each font")` marks a
repeated field and `.flags(&[…])` names the booleans of a packed byte in card
order (their bits are per build). Binders may only emit card field names; the
flag decoders take bit names only from `flags`. The pages under
[`docs/signals/`](../../docs/signals/README.md) are rendered from the cards
(`shape signals docs docs/signals`); the annotated `<signal>.js` samples and
`schema.md` are hand-maintained.

`catalog/mod.rs` is the source of canonical identities and cards.
Tests validate the verified answer key directly against this registry.

## Tests and truth data

- `cargo test --profile iter --lib signals::` runs the unit tests, the
  registry check, `catalog_sources_stay_structural` (recognition sources
  never read salts, addresses, descriptors, generated spellings or truth
  files) and `pages_match_cards`.
- Corpus runs are `#[ignore]`d and read the frozen vendor-ten traces
  (`SHAPE_FROZEN_ROOT`, default `/tmp/shape-corpus-ten-fresh`):
  `cargo test --profile iter --lib catalog_tests -- --ignored --nocapture`;
  `SHAPE_SIGNAL=<Name>[,<Name>]`, `SHAPE_VENDORS=<profile>` and
  `SHAPE_SEEDS=baseline`, `SHAPE_SEEDS=all`, or comma-separated layout names narrow them.
- The verified answer key is `tests/fixtures/signals/verified/<profile>/`:
  `slot-map.json` (slot address -> canonical identity per seed) and
  `schemas/<label>.json` (the seed and, for the proof-keyword signals, the
  first emission's verified grammar items). `emoji-channel-sample-halves-match-media-layouts/proofs/` keeps the
  emoji-channel sample-halves layout proofs `tests/program_signal_projection.rs` reads, and
  `tests/fixtures/signals/field-orders.json` the analysts' recorded field
  orders of the namespace hash tables and the console nativeness pair.
- `SHAPE_SIGNAL_DUMP=<address>[,<address>]` prints the structural view of
  those slots during any run.

The full analysis evidence behind the cards (analysis notes, per-build
variants, validations, the complete verified alignment) is not in the
repository.

## Extraction: signal layouts

`extract/scanner.rs` builds one `SignalLayoutRequest` per collector:

- a wave collector uses its own function span and the row-derived
  `slot -> signal` mapping;
- a direct registry slot uses the **task callback** argument passed to the
  constructor wrapper (the body that owns the emissions), never the wrapper
  factory itself.

`extract/layout.rs` reparses the exact source, resolves helper identities through
whole-program value aliases, replays each collector in JavaScript evaluation
order, snapshots the ordered writes on the path that reaches each emission, and
returns `Emission` records. A reused buffer therefore reports the
first emission as `A`, a later in-branch emission as `A + B` (without the later
`C`), and the emission after the branch as `A + CHOICE(B) + C`. The adapter
merges those into `SignalRecord::emission_layouts`, converts mask literals into
`StaticCapture` records with their exact literal spans, and turns every
remaining layout gap (unknown write, missing prefix/mask/payload, missing value
emission) into a required `UnresolvedFact` rather than a diagnostic-only note.

The wire grammar is `STRING` / `NUMBER` / `BYTE` leaves inside `SEQUENCE` /
`REPEAT` / `CHOICE` containers. Values and seed words are exact source slices;
anything not provable stays `WireOp::Unknown` and is reported.

## Extraction: adapter rules

- Direct collector identity: descriptor `ctor` expression -> constructor wrapper
  -> last function argument (the task callback).
- `[[], bytes]` and `[aux, bytes]` both name their byte buffer in the second
  element; a raw named object is a `GenericObject` emission.
- `.next(...)`, `.next.call(...)` and `.next.apply(...)` are accepted.
- `try`/`catch` is a `CHOICE`, `finally` follows as a `SEQUENCE`; short-circuit
  operands are conditional `CHOICE` branches.
- The four prefix bytes are four individual `BYTE` leaves, each keeping its own
  `seed >> n & 255` source expression. They are never one wide leaf.
- A payload array built with an emitted store helper
  (`payload[1] = store(payload, 0, [], buffer)`) resolves to the helper's
  returned argument only when the body is exactly one plain indexed store
  followed by `return` of a distinct plain formal parameter, a scope-resolved
  scan proves that parameter unchanged (no direct/compound/update write, no
  nested captured write, no mapped `arguments` use, no `eval`/`with`, and no
  unmodeled call or mutation of the returned buffer), and the helper *binding*
  is single-origin and never written. A `SymbolId` identifies the binding, not
  the function it currently holds, so a later `store = other` /
  `store = factory()` / `store++`, a write captured by a nested closure, or a
  duplicate/conflicting declaration of the same binding fails closed instead of
  reusing the declaration's proof.
  Anything else stays an explicit unresolved payload rather than attaching a
  buffer whose bytes may be missing.
- A STRING-writer closure keeps the mask value captured when the closure was
  created, including an inline `PRNG(seed ^ C)` argument; reassigning the mask
  binding later does not retype or re-mask the earlier write.
- The per-emission mask follows the emitted buffer and observer wiring, not the
  enclosing function, sibling literal or signal index. Sibling buffers in one
  container keep their own static XOR literal.
- A generic named-object field is typed from its source expression (literal,
  same-kind branches, the hash helper, and a small set of structural member-call
  result shapes). A field that depends on page state through an ordinary
  identifier stays unknown.

## Boundaries

- No recovered JavaScript is executed; no runtime value is synthesized.
- Names, indices, wave counts, XOR literals and capture strings are never match
  rules.
- `max_source_bytes`, AST node, function, binding, array-element, emission and
  retained-body budgets are enforced.
- Reports are facts, not a byte-exact codec claim. In particular the generic
  named-object signal keeps its source-proven fields but the generic outer token
  encoder stays an explicit required boundary.

Fixture paths describe behavior or source layout. Independent controls remain
separate, using baseline/alternate labels when their protocol is equivalent.
Verified profiles use their baseline opening signal sequence; schema labels use
each layout's opening signal sequence with profile context when shared, and proof
filenames use the signal name. Slot maps, field orders and proof metadata share
those layout identities. External frozen programs are located by their exact
encoded seed, independent of corpus directory names. Fixture source bytes,
cryptographic pins and source spans remain unchanged during path cleanup.
