# Saved signal schema execution

[mod.rs](mod.rs) adds `wire.version: 1` to the existing per-seed signal schema.
It embeds the original typed contract grammar and encoding evidence, preserving
source and descriptor provenance, mask distinctions, ordered prefixes, repeat
bounds/direction, branch conditions, concrete helper bindings and factory-call
identities. It does not replace the descriptive fields.

- [algorithms.rs](algorithms.rs) checks full generic helper bodies and their
  dependencies before granting narrowly scoped native codec identifiers.
- [program.rs](program.rs) is the single grammar compiler and native read/write
  engine. Source expressions remain evidence. Unsupported semantics produce
  explicit reasons; they are never guessed from successful decoding.
- [position.rs](position.rs) proves contiguous physical writes from the captured
  prefix, independently of source evaluation order.
- [consumer.rs](consumer.rs) loads actual saved schema JSON, verifies exact
  source/pin/evidence/routing, preserves nulls and raw bytes, and requires exact
  consumption and deterministic whole-blob reencoding.
- [consumer_tests.rs](consumer_tests.rs) builds synthetic source programs,
  extracts their contracts, renders and serializes schema JSON, then tests the
  typed loader and execution independently of outer header recognition.
- [named.rs](named.rs) projects physical trees through source-derived field
  keys, static leaves and runtime occurrences. The saved `wire.slots[].naming`
  metadata is versioned separately. [named_tests.rs](named_tests.rs) covers
  projection, duplicate-key rejection and the successful public loader path.

`Decoder::decode_named_report` returns a separate `NamedDocument`. Its `signals`
object contains schema keys, readable field values, flag booleans and explicit
statuses. Unknown/refused names appear in `unresolvedSlots`. Every channel of
the six-field Report survives under `metadata.originalReport`.

[semantic.rs](semantic.rs) maps source-proven byte tags to booleans or null.
Cards distinguish fixed null slots, nullable number/string fields, and explicitly
scoped two-byte null components (`nullable_pair`). A single-list card applies
to every message layout; multi-message and nested cards retain their scopes.
Byte values or field-name patterns alone never authorize a mapping. The physical
choice and occurrence retain each concrete tag for inversion, including distinct
missing and null alternatives. Actual numbers, numeric startup sentinels, packed
flags, record discriminators and event "unchanged" tags retain their meanings.
Only projections with mapped leaves perform the additional semantic inverse
encoding check; physical and full-report correctness checks remain independent.

Use `NamedDocument::from_json` to reject duplicate members at every depth and
`restore_original_report` to restore that original Report after serialization.
Restoration deliberately ignores display-field edits. There is no named-edit
encoder: use `Decoder::encode_slot` with a physical `WireValue` and the captured
seed to apply validated physical edits. No CLI command was added.

Use the existing `shape signals program.js --json --layouts-only` command.
The API, current supported representations and stage boundaries are documented
in the [schema reference](../../../docs/signals/schema.md#versioned-wire-metadata-and-library-consumer).
There is no additional analysis CLI command.

Float byte order is external configuration. `Decoder::load` defaults to
`TypedDecoderOptions::default()` (`FloatByteOrder::Unknown`). Select a confirmed
collecting runtime order through `Decoder::load_with_options(schema, source,
pin, TypedDecoderOptions { float_byte_order: FloatByteOrder::Big })`, or apply
`with_options` to a validated decoder. This does not infer a host's byte order,
select a fallback by plausibility, or weaken source/pin checks. Integers,
including negative zero, remain supported with Unknown. Float marker 128 needs
both certified source dependencies and a selected order. The legacy
`codec::SignalReader::number` / `SignalWriter::number` stay little endian;
`number_with_float_order` is their explicit-order low-level counterpart.

Additive `wire.number_float` metadata records the marker, eight-byte width,
whole writer identity, native dependency identity, fallback limitations and
runtime prerequisites. Loading recomputes this metadata against exact source.
Older schemas without it remain integer-capable but cannot enable floats.
The integer certificate ignores the alternate branch and never certifies floats.
The relational proof in `float_proof.rs` reuses canonical whole-body comparison
against `reference/number.js`, excluding only the native guard's software
alternate. It retains the complete integer/native branches, marker, exact
scratch store, same-buffer byte view and ascending byte iteration. The original
whole writer span and digest are retained; this conditional certificate makes
no claim about the excluded software fallback. A different software fallback
therefore does not mislabel a compatible native path as an incompatible body.

Scope-resolved references bind the selected extracted number function to its
own captured one-element Float64Array scratch and Uint8Array constructor.
Single-origin constructor/scratch aliases, separate module assignments,
typeof availability guards, direct/aliased writer calls and exports, and
simple successful-allocation/null-on-exception try/catch origins are checked.
Unrelated enclosing statements are allowed. No enclosing-module text match or
capture/name allowlist is used. Mutated/overwritten aliases, scratch escapes,
wrong buffers/types/widths and unresolved class/coercion paths refuse. Signed
byte views remain unsupported without a separate proof of low-eight-bit
narrowing at the output boundary.

`number_float.provenance` records roles, exact spans and SHA-256 digests for
the selected writer, binding linkage, native guard/view and constructor/scratch
origins. The saved per-seed schema loader recomputes this evidence against the
source. Generic public-loader tests now emit, reload, decode and exactly
reencode a float using the actual extracted direct writer; no injected IR
linkage or forced certificate is used in that test.

The conditional native proof requires unmodified typed-array intrinsics,
successful scratch allocation/native branch execution, no concurrent or
reentrant mutation of scratch/backing views, and the configured
order matching the collecting runtime. Source text cannot establish those
runtime facts. The reviewed software fallback emits least-significant bytes first,
but its arithmetic and NaN binding are **not certified**; selecting Little does
not authorize that path. Missing primitive proof and unknown runtime order
produce distinct unsupported reasons with required inputs in metadata.

IEEE bits are retained in `WireValue::NumberBits`, including subnormals,
infinities and NaN payloads. Reencoding replays those bits without arithmetic
or NaN normalization; it does not promise that a future JavaScript NaN store
will choose identical payload bits. A float tag encoding any value in the
writer's int32 domain (including either zero sign) is invalid. Body bytes each
consume one mask when enabled; the original four-byte prefix is unchanged.
The consumer requires full consumption and compares the entire reencoded blob
with the same captured seed. Named views retain the selected options under
`metadata.decoderOptions`, finite display values and lossless per-leaf bits;
special values use the existing lossless objects. Original-report restoration
continues to preserve all bytes. Capture-specific float configuration is supplied
explicitly by the caller.

Source-only generation retains structurally verified `requires_initializer`
programs when global initialization order is unknown. That status contains the
actual source-verified Program, its stream body proof and a typed initialization
epoch prerequisite; it does not authorize execution. All cursor, intern-table,
prefix, count and branch checks still apply. Global certification remains
separate from readiness of one concrete factory invocation.

For a callback lifecycle, stage the existing command and public API as follows:

```text
shape signals program.js --json --layouts-only > schema.json
```

```rust,no_run
# fn staged(schema_json: &[u8], source: &str, pin: &shape::signals::payload::SeedLayout, trusted_initializer: &str) -> Result<(), shape::signals::wire::WireError> {
use shape::signals::wire::{SchemaDocument, Decoder};
let mut saved = SchemaDocument::from_json(schema_json)?;
saved.bind_initializer(source, pin, trusted_initializer)?;
let bound_json = serde_json::to_vec(&saved).unwrap(); // persist as schema.json
let decoder = Decoder::load_with_initializer(
    &bound_json, source, pin, trusted_initializer,
)?;
# let _ = decoder;
# Ok(()) }
```

The same per-seed JSON is the execution input. `wire.initialization.emissions`
contains a result for every emission: source-bound readiness witness or explicit
refusal. Loading verifies the exact source, pin, routing, contracts, naming and
structural programs, then regenerates and compares the entire census and each
successful witness against the trusted artifact and pinned token. An initializer
for a different token is refused. `load_with_initializer` can also stage readiness
from a source-only document directly. Plain `load` never enables conditional IR
and requires the artifact when saved witnesses are present.

The caller associates this Report with the supplied public initializer, before
any intervening initialization of shared session state. A witness authorizes only
its concrete instance's initial state. Named output retains the initializer
digest, epoch, source and activation evidence under initialization metadata,
without executing the artifact. Source mutation, extra state writes, tampered
witnesses and initializer-token mismatches remain refusals. Ordinary globally
ready schemas continue using `Decoder::load`.

If the collecting runtime's native float byte order is known, use
`Decoder::load_with_initializer_and_options` with explicit
`TypedDecoderOptions { float_byte_order: FloatByteOrder::Little }` (or `Big`).
The default remains `Unknown`. This setting is retained in named document
metadata; it cannot replace the source writer and native scratch proofs.

Packed flag projection retains every physical loop iteration. It groups bytes
only across compiler-certified packing flush choices, including their empty
arms, while metadata keeps the original occurrence indices. Other empty or
unknown values cannot be silently removed to manufacture a flag group.

Separately certified emissions with the same executable prior-count references
can share a physical decode even when their source proof spans differ. Selection
metadata records `equivalent_count_programs` and every emission index; exact
saved source proofs remain distinct and are all revalidated. Count paths, limits,
direction, masks, salt and other operations must still match. Differing or
unproven candidate languages continue to require an explicit selector.

Cursor proofs account for assignment commit after an exact writer-call RHS and
fixed affine byte positions relative to the last accounted cursor. They still
refuse gaps, overwrites, stale bases across variable-length writers and
unproved loop invariants.

A retained stream draw is one scalar value. Reusing it at another write is
refused when the byte IR cannot preserve that shared mask identity; exact byte
reencoding alone cannot justify a second logical field. A separately assigned
new draw is distinguished using its actual reaching definition.

Readiness also follows immutable named callback aliases, post-registration
snapshots of stored callbacks, and every direct caller of nested emission
helpers. Snapshot capture must follow registration and precede any overwrite;
unknown consumers and mutable aliases remain refusals. An inline function
argument allocated inside an already proven initialized activation cannot be
invoked before that allocation. This source creation bound does not choose an
async ordering or certify the consumer, and retains the same no-intervening-
initialization epoch. The census retains the artifact digest and epoch even
when every emission is refused.

Physical cursor proofs version a holder through its actual position reads.
Recycling it after its last position read does not change already written bytes;
an alias retains the cursor value captured before that overwrite. Conditional or
loop-dependent reaching definitions cannot supply an unconditional cursor,
including updates before prefix construction. The original gap/overwrite/const
and nested-return checks remain independent. For a source-proven inline single-
store helper, `WriteNode.position_span` binds the exact caller index argument;
loading regenerates it, and the cursor proof evaluates that argument's real
increment. It does not choose an argument by a matching value or source spelling.

The [explicit initializer example](../../../docs/signals/decode-checkpoint.md)
packages the existing direct source-format adapter separately from these strict
loader APIs. Its successful direct decode is not a strict lifecycle certificate.

[readable.rs](readable.rs) owns the six packed-byte grouping rules shared by
executable naming and descriptive schemas. Naming version 2 uses `FixedArray`
for per-index heterogeneous flag maps; it never substitutes wire repetition or
changes leaf traversal. Whole single-byte flag messages flatten at the root.
`ScalarNull` certifies scoped multi-component scalar nulls. Selected `Empty`
fields omit their readable keys and retain absence/choice diagnostics; genuine
empty records, flag objects and arrays remain present. Legacy version 1 artifacts
keep their flat flags and component records. See the schema migration reference
for validation guarantees and compatibility limits.
