# Signal schema format

`shape signals` reads one deobfuscated program (`shape trace program`
output) and reports the signal layout of that build: which slot carries which
signal, how it is routed, its mask, its field order and the wire type of every
field. Signal names are the canonical names of the [signals index](README.md);
field names are the ones on each signal's page.

```sh
shape signals <program.js> [--json] [--layouts-only]
```

Everything in the report is per build. Slot addresses, positional indexes,
routing names, masks, field order and flag bit positions all change between
builds; never carry one build's values to another.

The descriptive fields remain a layout view. The additive `wire` member
retains the source contract and versioned native execution metadata. The
schema contains no captured per-message seed or host observation.

## Report layout

```json
{
  "seed": "AAA0OXehAQAAotsvPJ9iw_-jyr_R3mWfS6zMA7rIyqJD1P5z81AOWeaoIt1I",
  "source": {"sha256": "9578…", "bytes": 636188},
  "signals": {
    "BrowserApiOwnPropertyFlags": {
      "address": "wave:14:1",
      "type": "positional",
      "positional": 9,
      "mask": 2689089531,
      "order": ["ApplePaySession", "ServiceWorker", "WebkitOverflowScrolling", "clipboard"],
      "fields": {
        "ApplePaySession": {"type": "boolean", "bit": 3, "fieldKey": "f1"},
        "ServiceWorker": {"type": "boolean", "bit": 2, "fieldKey": "f1"},
        "WebkitOverflowScrolling": {"type": "boolean", "bit": 0, "fieldKey": "f1"},
        "clipboard": {"type": "boolean", "bit": 1, "fieldKey": "f1"}
      },
      "physicalFlags": {"sourceName": "ownPropertyFlags", "fieldKey": "f1",
        "schema": {"type": "flags", "mask": null,
          "bits": {"WebkitOverflowScrolling": 0, "clipboard": 1, "ServiceWorker": 2, "ApplePaySession": 3}}}
    }
  },
  "diagnostics": {"unidentified": [], "ambiguous": [], "refused": [], "no_emission": [], "flags_fallback": []}
}
```

### Build header

- `seed` — the seed ID of the program, taken from the program file name
  (`<seed>.js`, the file stem). It is not read from the program text.
- `source` — `sha256` and `bytes` of the program file.

### Signal entry

| Key | Meaning |
| --- | --- |
| `address` | The slot in this build: `wave:<group>:<position>` for a signal produced by a grouped (wave) collector, `direct:<descriptor>` for one registered directly. |
| `type` | Routing class: `positional` or `name`; `no_data` for a [signal with no data](#no-data-signals) (its routing member stays). |
| `positional` | Only for `type: "positional"`: the signal's positional index (it has no descriptor name). |
| `name` | Only for `type: "name"`: the routing name. A string name routes to the named signals, a numeric name to the keyed signals. |
| `mask` | The signal's mask constant: the salt of the stream its fields are XOR-masked with. `null` for a name-routed signal, and for a signal whose messages use different masks. |
| `order` | Logical field order in this build. Original physical write order remains in executable naming paths. |
| `fields` | One entry per field name, giving its wire type (below). |
| `messages` | Instead of `order`/`fields` when the signal has more than one emission shape (see [messages](#messages)). |

If two slots of one build resolve to the same signal name, the later ones get
a `_2`, `_3`, … suffix so no slot is dropped.

### Name-routed signals

A few signals hand one plain object (or a bare value) to the shared generic
encoder instead of writing their fields one by one. Their entry has
`type: "name"`, the build's routing `name`, signal `mask: null`, and every
field typed `"value"` with its wire `"key"` (the object key in the program).
Each signal page says when a signal is sent this way.

### No-data signals

A slot whose producer completes without ever calling `next` is declared but
never emitted: it never collects a value, so the payload only carries the
not-collected marker for it. When a signal recognizer identifies such a slot,
it is listed under `signals` with `type: "no_data"`, its routing member
(`name` or `positional`), `fields: {}`, no `order` or `mask`, and an `effect`
object naming what the producer does on the page in this build:

```json
"SensorReadyEvent": {
  "address": "direct:83",
  "type": "no_data",
  "name": "qsnbepcestpp",
  "fields": {},
  "effect": {"windowFlag": "f5cd836ea16e4a77", "event": "f5cd836ea16e4a77"}
}
```

A no-data entry has no wire value: decoders and the wire consumer skip it
(the consumer only checks its address and routing against the `wire` slot).
An unrecognized declared-not-emitted slot stays under
`diagnostics.no_emission`.

## Field types

Every physical field and choice arm has a `type` and a `mask`. Synthetic arrays
and flattened logical booleans use the projection types below. The historical nullable
`mask` is retained. Leaf fields additionally carry `maskMode`: `inherit`,
`none`, `nested`, or `unresolved`. In particular, historical `mask: null`
alone cannot distinguish inheritance from an unmasked/static-XOR write.
A numeric mask with mode `nested` identifies a different salt; it does not
by itself establish a stream instance or reset location.

| `type` | Wire value | Extra keys |
| --- | --- | --- |
| `number` | A masked number. | |
| `string` | A masked string. | |
| `byte` | One masked byte. | `byte: <n>` when the value is a literal, for example the 130+ null markers. |
| `flags` | One byte packed from up to eight booleans. | `bits` (see [flags](#flags)). |
| `fixed_array` | Synthetic heterogeneous packed-byte tuple; no repetition on wire. | `length`, `items`: per-index flag schemas, original source names and keys. |
| `boolean` | One logical check from a flattened whole-byte signal. | `bit`, `fieldKey`; full physical schema under `physicalFlags`. |
| `array` | A count-driven repetition. | `item`: the element's type. An item that writes more than one value is a `record`. |
| `record` | Several values written together (an array item or a choice arm). | `order` and `fields`, like a signal. |
| `choice` | One of several alternatives chosen at run time. | `arms`: a list of typed arms, each with a `label` (`then`/`else`, …). |
| `empty` | A choice arm that writes nothing. | `label`. |
| `value` | A value of a name-routed signal, typed by the generic encoder. | `key`: its wire key. |

Any number, string, byte or flags field whose value is also XORed with a
static literal carries `"xor": <literal>`.

A byte written as `(c ? A : B) ^ stream()` is a `choice` of two `byte` arms.
A choice arm that writes more than one value has the `record` shape plus its
`label`.

Source-derived tag meanings add `logicalType` (`boolean` or `null`) and
`valueMappings` (emission, concrete byte cases, condition and source span) to
mapped fields; a choice with a null arm also carries `nullable: true`. The
executable copy lives in each wire leaf's optional `byte_mapping`. The named
decoder returns logical JSON values and verifies their inverse through the
wire encoder. Byte values without semantic evidence remain numbers. A numeric
payload equal to a null tag remains numeric on the value arm.

```json
"records": {"type": "array", "item": {"type": "record", "order": ["blue", "green", "alpha", "red"],
  "fields": {"red": {"type": "number", "mask": null}, "…": {}}}},
"status": {"type": "choice", "mask": null, "arms": [
  {"label": "then", "type": "byte", "byte": 130},
  {"label": "else", "type": "string"}]}
```

## Flags

A byte built from up to eight true/false values (`a | b << 1 | …`, or a
packing loop `bits |= x[j] << n` flushed every eight values) is

```json
{"type": "flags", "mask": null, "bits": {"<flag>": <bit>, "…": 0}}
```

- Bit 0 is the least significant bit. Bits not listed are always 0. `bits`
  keys print in name order; there is no separate order list.
- Bit positions are per build. They are read from the write site: the `|`
  terms and their shift amounts, or the loop's order over its probed list.
- Bit names are the flag names on the signal's page, matched to each bit by
  what its boolean reads (the probed string, host member or global, the
  record label the collector stored, or for a whole-list probe the list item
  itself). A bit set by a folded constant or a state set only in a `catch` is
  named only when it is the one flag no other bit reads.

Shapes:

- One byte: `bits` maps each flag to 0–7. A boolean written on its own byte
  is a one-bit `flags` (`{"isKeyup": 0}`).
- An `array` of in-loop flush bytes (for example `ownPropertyBits` of
  GlobalOwnPropertyBits): the item is `flags` and positions count across the
  bytes of the array (byte `p / 8`, bit `p % 8`). An empty `bits` means the
  loop never fills a whole byte.
- The trailing partial byte (`…Tail`, `…Flush` fields): positions count from
  the last full byte on.

A packed field whose bits cannot all be derived and named stays
`{"type": "byte"}` and is listed under `diagnostics.flags_fallback`; the
signal is still emitted.

## Messages

A signal with more than one distinct emission shape (different lifecycle
phases writing different layouts, or a top-level choice whose arms each start
their own prefix) has

```json
"messages": [{"when": "init", "mask": 1234, "order": ["…"], "fields": {"…": {}}}]
```

instead of top-level `order`/`fields`. `when` lists the lifecycle phases
sharing that layout: `init`, `collection`, `last_chance`, `body`, or
`<phase>:<arm>`. The signal `mask` is the common salt, or `null` when the
messages differ.

## Diagnostics

Slots that are not emitted under `signals` are listed under `diagnostics`.
Nothing is guessed:

| Key | Meaning |
| --- | --- |
| `unidentified` | No signal recognizer claimed the slot. |
| `ambiguous` | More than one recognizer claimed it. |
| `refused` | A field could not be bound to a field name of the signal (placeholder names such as `field_1` are never emitted). |
| `no_emission` | The contract pass reconstructed no emission for the slot. The `reason` says `declared, not emitted: …` when the producer provably completes without ever calling `next` and no no-data signal claimed it. |
| `flags_fallback` | Emitted signals with a packed field left as `byte` (`address`, `signal`, `field`, `reason`). |

`--layouts-only` adds each unidentified or refused slot's projected `layout`
with positional keys (`f1`, `f2`, …), for checking layouts before the signal
is identified.

## Versioned wire metadata and library consumer

The existing generation command emits both views:

```sh
shape signals program.js --json --layouts-only > wire-schema.json
```

`wire.version: 1` contains the exact `source` identity, source-bound
`algorithms`, the typed `encoding` evidence, and a `slots` array. Every slot
retains its descriptor, address, explicit routing, complete original contract
and per-emission `programs` support result, including unidentified descriptive
slots. The parallel `source_plan` array retains control-flow certificates even
when a separate codec or cursor prerequisite prevents execution. Routing uses
`type: positional` and its integer `positional`; names and
JSON object order never assign indices.

The contract grammar is the shared wire IR. Its ordered `raw_prefix`,
`number`, `string`, `byte`, `sequence`, `repeat`, and `choice` nodes retain
source spans, value expressions, mask forms, static XORs, loop count/header/
direction, branch conditions and unresolved facts. Expressions are evidence;
the consumer does not evaluate arbitrary JavaScript expressions. Prefix and
write records also retain seed binding, concrete helper binding, stream factory
call and string writer factory call identities. Equal salts do not imply a
shared stream cursor or intern table.

A supported program records BE32 captured-prefix input, an emission-local
stream cursor, the salt and ordered native operations. Algorithm identifiers
are granted by whole-body, scope-aware equivalence against reviewed generic
reference functions, plus dependency and public-token provenance checks.
Leading local declaration normalization and identifier renaming do not erase
operators, constants, declaration kinds, parameter identity or control flow. The integer path has
its own certificate; it does not certify the floating-point path. A separate
certificate proves the float prefix marker without certifying its body.

Float execution additionally requires source-bound `wire.number_float` writer
and native scratch/dependency proofs, plus external runtime configuration:
`Decoder::load_with_options(&schema, &source, &pin, TypedDecoderOptions {
float_byte_order: FloatByteOrder::Little })` (or `Big`). `load` defaults to
`Unknown`; `with_options` changes configuration on an already validated decoder.
Unknown refuses float marker 128 while preserving integer support, including
negative zero. Selecting an order cannot certify an unknown source primitive.
The legacy codec number API remains little endian. Named document metadata
retains the selected mode under `decoderOptions`. See the
[float API and proof prerequisites](../../src/signals/wire/README.md) for the
conditional native proof, bit-preserving NaN policy and unsupported fallback.

`shape::signals::wire::Decoder` consumes the saved JSON directly:

```rust,no_run
use shape::signals::{payload::{Report, SeedLayout}, wire::{Decoder, Outcome}};
# fn run() -> Result<(), Box<dyn std::error::Error>> {
let schema = std::fs::read("wire-schema.json")?;
let source = std::fs::read_to_string("program.js")?;
let pin = SeedLayout::from_json(&std::fs::read("layout.json")?)?;
let report: Report = shape::signals::payload::typed_json::parse_typed_json(
    &std::fs::read("report.json")?)?;
let decoder = Decoder::load(&schema, &source, &pin)?;
let slots = decoder.decode_report(&report)?;
for slot in slots {
    if let Outcome::Decoded { value, .. } = &slot.outcome {
        let emission = slot.selection.as_ref().and_then(|s| s.emissions.first())
            .copied().expect("decoded signals retain source emission selection");
        let bytes = decoder.encode_slot_for_emission(slot.positional, emission, value,
            slot.captured_seed.expect("decoded signals have a captured seed"))?;
        assert_eq!(Some(bytes), slot.raw);
    }
}
# Ok(()) }
```

Supply the pin generated from these exact source bytes and verified initializer
literals. Loading checks the source digest/length, statically regenerates pin
facts, checks descriptor routing and bounds, and compares saved control plans,
evidence and
programs with static extraction. It then executes the saved programs. It never
substitutes a caller-authored `codecLayout` or a separate contracts file.

Regenerate older wire evidence when adding control certificates: public loading
compares the saved plan and operations exactly with the current source compiler.

A successful signal decode consumes the entire body, validates literals/types,
reencodes using that signal's captured four-byte seed, and compares the whole
blob including its prefix. The signal seed is independent of the report's two
outer payload seeds. No API obtains fresh randomness. Exact reencoding is
necessary in addition to source/algorithm verification.

`WireValue` preserves byte values, number bits (including negative zero),
UTF-16 string units (including lone surrogates), ordered sequences, repetitions
in source iteration order, and selected arms. Neither fixed nor referenced
repeats acquire an extra length prefix. `CountExpr::PriorNumber` refers to an
already consumed scalar by its original grammar child path and source write.
The certificate checks resolved bindings, reaching definitions, zero start,
unit update, bound, direction, mutation and accounted calls. It covers cached
lengths, ascending `for`/`while` loops, descending bounds and the explicit
decrement-and-negative-exit form. Referenced counts must be finite exact
nonnegative integers within the recorded bound; known Array lengths also
reject negative zero. Encoding requires exactly that many repeated values.
Unknown external bounds require input; remaining byte length never supplies a
count. Values remain in physical source iteration order, including reverse
loops, empty iterations and trailers.

`DomainChoice` records disjoint first-byte domains, common stream/mask mode and
arm-owned consumption. Literal bytes, certified NUMBER envelopes and certified
modified UTF-8/interner string domains can discriminate an optional value with
one peek. The selected arm consumes the byte once. Integer certification alone
does not certify the general NUMBER envelope. The envelope needs a whole
reference match or a separately checked marker/eight-write shell, with no
buffer aliases, cursor resets, extra stream draws or counter mutations in its
opaque computation. This shell proof does not interpret float arithmetic or
select float byte order. Collisions, unknown
domains and internal choices requiring speculative continuation parses are
refused. A marker remains a physical byte; optional `Nullish`/`NullOnly`
metadata requires a source guard on the same resolved value binding.

`CounterChoice` instead uses a source-proven induction predicate or packing
counter recurrence. The supported packing recurrence starts at zero,
increments once per iteration and resets at eight under its exact flush guard;
a final nonzero-residue guard uses the referenced loop count. The repeat tree
keeps every iteration and chosen/empty arm. Its length is an iteration count,
not an emitted-byte count, and bytes are never reversed to model bit order.
The counter's exact predicate read is retained: writes before that read are
refused, while reuse of the binding inside the selected arm is accounted for.

`RootChoice` retains each source alternative's own RAW4, stream initialization,
salt, support result and conservative whole-message length bounds. It selects
only when the supplied length lies in exactly one proved bound, before creating
a reader or using any branch salt. Overlapping ranges are ambiguous. A selected
case still needs its codec and cursor certificates. RAW4 has no value leaf;
the named adapter skips only the compiler-certified root-prefix paths.

For distinct explicit source emissions, the consumer requires a unique full,
canonical parse across the closed set of candidates. Every other candidate
must be proved invalid, possibly by a source length bound. Any unsupported
candidate, unresolved float path or exhausted budget remains unknown and blocks
selection. Multiple valid candidates are ambiguous. No keys, offsets, seeds or
cursor histories are searched. Identical verified programs retain all matching
emission provenance. `DecodedSlot.selection` and named metadata retain the
selection proof and emission/part/message relationships. `encode_slot` retains
its unambiguous behavior; use `encode_slot_for_emission` with explicit source
selection when messages differ.
Selection also retains the selected root-case certificate, including its arm,
prefix and stream identities. Structural child/arm/body paths disambiguate
different helper occurrences that share a source span.

Literal NUMBER tags and separately certified float prefixes retain their source domains; float execution additionally requires the native dependency proof and configured runtime byte order.

`decode_report` returns positional physical trees. `decode_named_report` uses
source-verified bindings to attach card fields and packed flag booleans; named
and generic report channels remain retained in the complete original Report.
Execution is bounded to
65,536 operations and 16,777,216 decoded UTF-16 units per blob, with bounded
grammar depth, repeat counts and JSON input size.

`Outcome::Missing` preserves null slots. `Unsupported` and `Invalid` retain the
original raw blob and captured seed, with structured errors and no fabricated
values. Unsupported program metadata includes a concrete reason and, where
applicable, the minimum missing branch/count/prior-state input. Current limits
include unresolved or nested stream resets, external repeat counts, unproved
branch selectors, shared prior cursor/intern history, uncertified helper
variants, uncertified float dependencies, and unknown runtime float byte order. Static scope checks reject
unaccounted stream reads and escaped state. A separate cursor proof requires
contiguous physical writes from the prefix, including nested writer returns;
unproved gaps, overwrites and cursor expressions remain unsupported.
Source literals containing lone
surrogates currently require additional literal-validation support, even though
dynamic UTF-16 values are lossless.

This is a public library stage, reusable by future CLI stages. It adds no
`analyze` command and does not collect signals, execute recovered JavaScript,
or change report/payload/header encoding.

### Named decoded document

```rust,ignore
let decoder = Decoder::load(&saved_schema_json, source, &pin)?;
let named = decoder.decode_named_report(&report)?;
let saved = serde_json::to_vec_pretty(&named)?;
let loaded = NamedDocument::from_json(&saved)?;
assert_eq!(loaded.restore_original_report(), report);
```

The returned document has `formatVersion: 1`, a `signals` object, an
`unresolvedSlots` array and `metadata.originalReport` containing the exact
six-field Report. The legacy Report API is unchanged. Each signal carries
`fields`, `status` (`decoded`, `missing`, `unsupported`, `invalid`) and
`metadata`. Missing/unsupported fields are explicitly unavailable, rather than
decoded nulls. Literal bytes remain bytes unless a semantic interpretation is
proved by the schema. Selected no-write fields are omitted; choice and absence
diagnostics stay in metadata. Written null markers remain explicit null.

Analysis retains the post-flags FieldBinding in the additive
`wire.slots[].naming` envelope (`version: 2` for fresh schemas; version 1 remains loadable). It records the unique emitted key,
canonical name, emission/part/message edges, part leaf ordinals, structural
grammar paths, FieldKeys, container projections, masks, XOR operands and named
bit maps. A source-scoped leaf ordinal covers every static arm and one loop
template; runtime repetition indexes instantiate it. JSON map order has no
role in joining values to names. Shared tails retain an edge into every split
part. Duplicate canonical names get collision-safe emitted keys; canonical
spellings are reserved before suffix allocation.

The public loader regenerates naming evidence from the exact source whenever
saved naming is present, including the descriptive fields. Names never change
wire execution. Old schemas without naming remain executable through the
physical API; their named view explicitly reports unavailable mappings.
`SchemaDocument::from_json`, `Decoder::load` and `NamedDocument::from_json`
reject duplicate JSON members before typed maps can discard them, at every
object depth, including descriptive fields, bit maps and naming metadata.

Decoded metadata distinguishes the captured prefix seed, stream salt, each
field's mask mode, static XOR operand and applied per-byte keystream. Byte
offsets refer to the original blob including its four-byte prefix. Metadata
collection uses a separate reader and never advances the authoritative
decoder's cursor. It preserves original bytes and raw number bits. Finite
numbers other than negative zero render as JSON numbers; negative zero and nonfinite
values render explicit `numberBits`/`special` objects. Valid UTF-16 renders as a
string, and isolated surrogates as `utf16Units`. These presentation rules do
not enable an uncertified floating-point decoder.

Multiple indistinguishable emissions retain all `projectionCandidates` and
their provenance. Conflicting field interpretations are unsupported; event
order is never inferred. Unknown or refused identities remain unresolved.
String-named and numeric-named channel execution is currently unsupported, and
their original data remains in the retained Report. Unsupported mappings stay
explicit instead of attaching guessed names to physical values.

`restore_original_report` restores retained metadata only, including errors,
generic values, missing sentinels, entry boundaries, both outer seeds and
keysets. It deliberately ignores display edits. There is no named-field edit
encoder. `Decoder::encode_slot` applies physical `WireValue` edits with the
caller's captured seed; callers can then update the restored Report explicitly.
Header context remains in the original header-report sidecar.

Source-only wire programs can carry `status: "requires_initializer"` with their
verified structural IR and typed initialization-epoch prerequisite. This keeps
source identity and grammar evidence separate from runtime session readiness.
For callback lifecycles use the existing `shape signals program.js --json
--layouts-only` output with `SchemaDocument::bind_initializer(source, pin,
trusted_initializer)`; serialize that same document, then pass it to
`Decoder::load_with_initializer(schema_json, source, pin, trusted_initializer)`.
The [staged API example](../../src/signals/wire/README.md) details this flow.
`wire.initialization.emissions` records every concrete emission's readiness or
refusal. The loader regenerates all witnesses, checks initializer-token equality
and enables only authorized instances, preserving all independent wire checks.
Named metadata retains this census and the complete original Report. Float byte
order remains Unknown until explicitly configured for the collecting runtime.

## Readable projection version 2

Fresh schemas use the same grouping rules as executable wire naming. The
positional payload remains encoded bytes; these JSON names are projections.
`order` describes logical field order after projection. The executable naming
leaves retain physical grammar paths, ordinals, masks, XORs, stream salts,
message/emission identities and occurrence contexts; grouping does not reorder
writes. A synthetic group may span interleaved sibling writes.

| Signal | Original source fields | Readable array |
| --- | --- | --- |
| FingerprintApiIntegrity | integrityFlags1–11 | integrityChecks[0–10] |
| NativeApiIntegrityFlags | integrityFlags1–3 | integrityChecks[0–2] |
| BrowserApiProxyFlags | proxyFlags1–3 | proxyChecks[0–2] |
| GlobalPropertyChecks | presenceBits1–9 | presenceBits[0–8] |
| AnchorIdnaUnicodeVersion | resultBits1–2 | resultBits[0–1] |
| GrecaptchaAccessorHookFlags | hookFlagsA–B | hookChecks[0–1] |

These are `type: "fixed_array"` with `length` and heterogeneous `items`,
not repeated wire fields. Each item retains `sourceName`, `fieldKey` and the
complete original flag schema (`bits`, mask and XOR). Numeric suffixes determine
indices: suffix 10 is index 9. Every element is the original named boolean
object. Grecaptcha's second chunk contains four checks; it is retained without
padding. Unused bits remain in physical values and `logicalBytes` metadata.
The catalog audit found no other A/B/C-suffixed packed-byte family.

A whole message consisting of one packed flag byte exposes its booleans at the
signal root, including ConsoleStackAccessFlags and CrossOriginIsolationFlags.
Its descriptive `physicalFlags` retains the original source name, field key and
byte schema; each logical boolean references its bit. Mixed-data messages,
repeated flag streams and multi-byte families keep their structure. This rule
is structural and applies consistently to analogous messages in each build.

A field with a no-write alternative carries `optional: true`. Its selected
empty alternative omits the readable key; a written null marker remains JSON
null. Choice selection and absence diagnostics remain in metadata. Empty arrays
and records retain their values, and missing entire signals retain the existing
missing-signal behavior. Tuple positions never compact when a subfield is absent.

A scoped catalog `nullable_pair` certifies scalar null alternatives represented
by two marker leaves. Their descriptive arm has `logicalType: "null"` while
retaining both component schemas and inverses. Readable output is scalar null.
Similarly named genuine records remain records; number/string arms stay scalars.

Version 1 naming artifacts remain loadable with their original flat flag fields
and component records (and their existing scalar mappings, if present). No-write
fields now omit their keys even for these artifacts. Fresh source compilation
emits version 2 naming. Strict source validation requires the current generated
schema, so regenerate a legacy schema for that loader. Artifact-only loading
validates tuple membership/order, unique leaves/keys and source-local leaf paths,
and revalidates saved scalar meanings against retained source expressions and
scoped catalog evidence. Retained values support exact mapped-value inversion;
this does not provide an arbitrary edited-readable-JSON encoder.

Single-byte wrapper paths observed in the verified capture are
`ConsoleStackAccessFlags.consoleStackFlags`,
`CleanConsoleStackAccessFlags.consoleStackFlags`,
`CrossOriginIsolationFlags.isolationFlags`,
`SeleniumIdeArtifactFlags.automationFlags`, `ProxyTamperFlags.proxyFlags`,
`WebAuthnInterfacePresence.flags`,
`BrowserApiOwnPropertyFlags.ownPropertyFlags`, and
`StorageAccessApiPresence.apiFlags`. The analogous catalog message
`BraveDuckDuckGoNavigatorFlags.browserFlags` also follows this structural rule
when present and resolved to one byte. All meaningful boolean keys move intact
to their signal root.

Readable finite whole-valued numbers within the signed/unsigned 64-bit integer
ranges serialize as exact integer JSON tokens (`1`, `32`, `0`). Fractions keep
their f64 representation. Conversion uses strict upper bounds: `i64::MAX as f64`
rounds to 2^63 and `u64::MAX as f64` to 2^64, so neither is treated as an inclusive
cast boundary. Values beyond these ranges remain f64 JSON numbers. Negative zero
and nonfinite values retain the `numberBits`/`special` representation. Exact IEEE
bits remain in field metadata for every number; source/artifact/codec metadata
is not numerically normalized. Integers above 2^53, including raw bit patterns,
never pass through JavaScript Number serialization.

## Concise public capture schema

Use `shape decode --artifact artifact.json --headers headers.json
--out decoded.json --schema-out schema.json` to export the public contract.
The same option works with `--seed` and `--save-artifact`. Outputs are published
only after successful signal and header roundtrips; existing files require
`--force`, and outputs must be distinct from each other and from inputs.
The library API `DecodeArtifact::compact_schema()` returns `CompactSeedSchema`,
versioned as `shape-seed-schema/1`. `SchemaDocument::compact_schema()` remains
collector-only and returns the unchanged signal-name map (`CompactSchema`).
The artifact loader is unchanged: neither public export is an executable codec.

```rust,ignore
let artifact = DecodeArtifact::from_json(&std::fs::read("artifact.json")?)?;
let seed_schema = artifact.compact_schema()?;
let collector_only = &seed_schema.signals;
std::fs::write("schema.json", serde_json::to_vec_pretty(&seed_schema)?)?;
```

The document contains `format`, `seedId`, `encoding`, `keysets`, `signals`,
`reportRoutes`, and `unroutedSignals`. `seedId` is the source literal emitted in
the `-c` header, found by the shared [source-only finder](../generator/pin.md#source-only-seed-id-finder).
Caller-supplied `siteId` stays in decode inputs/artifacts and is excluded from
this source-only public document.
`encoding.cipher` and `encoding.header`
retain source-pinned cipher constants and header policy/literals losslessly.
To avoid duplicating the long seed ID, `encoding.header.c` is exactly
`{"$ref":"#/seedId"}`: resolve this local JSON Pointer against the document root
and substitute the resulting string to recover the original header layout.
The other header literals remain strings. This is a documented reference in the
descriptive export, not an artifact-loader representation. Old artifacts continue
to supply `seedId` from their existing `layout.header.c`; no session field or
artifact migration is required.
`encoding.serializer` describes the outer signals serializer (ordered omitted
keysets, depth bound 100 and missing sentinel extension) and the default serializer
for errors and entry values. `encoding.float.callerByteOrder` is a saved caller
choice, not a source fact; `number` retains available float geometry and runtime
prerequisites, or null when unavailable. Native-path prerequisites and an
uncertified software fallback remain explicit. Session alphabet, cipher keys,
transaction token, shuffle key and nonce values are excluded. Their input names
appear under `externalInputs`; payload seeds, collector entropy, observations and
optional header overrides also remain external. No capture defaults are copied.

`keysets` is an ordered array of ordered string arrays used only by the outer
`signals` serializer. Empty/repeated rows and key order are preserved verbatim;
a row index is never interpreted as a keyed-signal ID. No per-route keyset
association is asserted without source proof.

`reportRoutes.positionalSignals`, `.signals`, and `.keyedSignals` preserve every
pinned route in channel order, including repeated descriptors/keys and routes
without a certified logical contract. Each route carries its original `descriptor`
and a `contracts` list referencing `signals`; an empty list leaves the value opaque.
Positional routes use `positionalSignal` and retain `ctor`/`priority`; named routes
use string `key`; keyed routes use numeric u32 `key`. Thus string `"17"`, numeric
`17`, and positional slot 17 cannot collide. A named contract is associated only
when both its routing key and direct descriptor address match. `unroutedSignals`
explicitly lists contracts without a corresponding pinned route rather than
inventing routes from collectors. Empty route arrays mean no such routes in the
layout, independently of which entries a capture happens to contain.

Positional/keyed entries have the raw shape `[values, extra]`: a generic superpack
value array and opaque byte array. Runtime null is the missing-entry sentinel
(two zero wire bytes); `[[], []]` is ambiguous and rejected. Keyed reports preserve
ordered `[u32 key, entry-or-null]` pairs, including unknown valid keys. Named
reports preserve `[string key, raw value]` pairs in the generic superpack channel,
including its missing sentinel extension. `errors` is a generic superpack array,
not a per-seed descriptor table. An opaque route gains no guessed fields, XOR or
tag semantics. The route tables describe source layout, never captured presence.
The CLI still retains all original report bytes/values in decoded metadata.

For example, the source fixture test exports
`reportRoutes.signals: [{"descriptor":2,"key":"17","contracts":[]}, …]`
and `reportRoutes.keyedSignals: [{"descriptor":3,"key":17,"contracts":[]}, …]`.
These demonstrate typed opaque routes; a seed without these routes exports empty
arrays rather than fabricated examples.

`signals` maps signal names to `positionalSignal` or `key` (the original
string or numeric source key), `order`, and `fields`. `messages` retains distinct
layouts. Signal/message `xor` initializes the keystream; field `xor` is static
XOR, including explicit zero. Field stream masking inherits by default;
`maskMode: none` disables it, while `maskMode: nested` and `mask` select a nested
stream salt. Missing message `xor` inherits the signal salt; missing signal
`xor` means no common salt. An unresolved mask is explicitly marked unresolved.
Static field XOR never inherits from signal `xor`.

Certified scalar tags use direct logical types and `values`. For example, a
build may export `{"type":"boolean","values":{"true":79,"false":78}}`.
Those bytes come from that build's validated mapping. A marker with a distinct
static XOR or stream-mask mode uses an object such as
`{"byte":79,"xor":3,"maskMode":"none"}` instead of a bare byte.
Different emission tables remain in `variants`, each with `emissions` and
`values`; identical tables share one map. No version or status meaning is inferred.

Multi-byte nulls retain every marker in physical order, for example
`{"type":"null","values":{"null":[{"byte":162},{"byte":1}]}}`.
Distinct layouts remain under `cases`, including different null marker sequences.
`optional: true` describes a no-write alternative; explicit null and absence
remain different. The compact format never exposes compiler `arms`, conditions,
spans, source execution contracts, or duplicate logical/tag tables. Route
`contracts` lists contain only public signal names.

Data arrays whose source-certified count is an invertible length carry `count`:
the original NUMBER descriptor, `position` in the original containing record's
physical order (zero based), and its validated `maximum`. Its value is derived
from the logical array length before the original writer runs. `order` omits the
hidden numeric field. To recover the original field positions, insert each count
at its `position` among the remaining fields. A count+array record with no other
fields becomes the array itself; its count position remains relative to that
original record. Signal roots remain objects. See the [migration inventory](counted-arrays.md).

`bits` and `bit` are zero-based bit positions. `physicalFlags` describes the one
byte behind flattened root booleans. Fixed arrays have per-index `items`, including
partial flag chunks. When grouping changes physical order, `writeOrder` lists
public field paths in physical write order. Strings name fields or the repeat
template `item`; integers select tuple items or alternatives. `emissions` lists
`[emission, branch-part]` pairs within each message only when different from the
implicit `[[0,0]]`. Message and variant selection information remains scoped.

A traced source can also be exported before a capture exists:

```sh
shape decode --schema-only --seed program.js --input-kind program \
  --schema-out schema.json
```

The library equivalent is `CompactSeedSchema::from_source(source)`.
It reuses source analysis and the pinned consumed descriptor registry, without
requiring a session or executing runtime recipes. Source header defaults and seedId are pinned without a session; caller float
byte order remains null until a runtime artifact supplies it.
A descriptive export does not certify that every positional codec can execute.

Named generic fields retain `type: "value"`, their source `key`, and `order`.
For example, the viewport contract contains eleven ordered generic fields; the
cookie contract retains both its `{value: …}` and empty-object emission shapes.
Their values are runtime typed; they are not silently converted to byte/number
recipes. The readable report keeps the ordered raw `signals` and `keyedSignals`
channels, including unknown and duplicate keys, with the original report in
metadata for exact inverse encoding.

A proven completion-only registration exports `behavior: "completion_only"`
and `payload: "none"`, plus its source event/window flag when available. It has
no measurement `fields` member. This requires a registered callback whose
observer is used only by an unconditional `complete()` call; generic unsupported
`no_emission` cases do not acquire that behavior. `SensorReadyEvent` is a
historical descriptive label for the host event, with no physical sensor meaning
asserted. Its registered named route remains present even though it emits no
payload. The source store's MissingSentinel is distinct from literal null,
empty object/array, and an unregistered key. Errors remain an open generic array;
`ReportLayout.errors: []` is a legacy layout placeholder, not an empty-error claim.

The opt-in `SHAPE_FROZEN_INPUTS=reports/frozen-inputs` regression verifies ten
frozen sources (`programs/<seedId>.js`) against the independent saved routing
census and field inventory, and writes examples under `reports/frozen-keyed`.
Its constructed reports prove generic codec inversion, not live capture byte
equivalence.
When different source encoders share a logical message but prove different count
recipes, compact export keeps the message's explicit source count fields and
order. It does not choose one encoder's normalization as a common runtime recipe.
