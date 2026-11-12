# Runtime configuration extraction

Terminology follows the canonical [architecture and VM variants](../architecture.md).

`shape::runtime_config` is the library API for source-linked runtime facts:
status, evidence, spans, hashes, and proven values or bytes. Use it when a
consumer needs to distinguish an extracted fact from an unknown or unsupported
one. The `kernel show` CLI uses a separate flat properties bundle.

For normal per-sample inspection use
`shape::extract_actual_properties` on the deobfuscated JavaScript, or
use `shape kernel show`. These related surfaces have different contracts:

| Surface | Contents | Consumer |
| --- | --- | --- |
| `shape kernel show` / `ActualProperties::tracer_json` | Flat entry/table/role/payload data; handler bodies included by default | Human/static inspection; [bundle contract](kernel-layout.md) |
| `ActualProperties::compact_json` | The older compact typed-properties projection, with explicit handler selection | Library callers retaining that view |
| `RuntimeConfig::stable_json` | `shape-runtime-config/v1` facts with status and evidence | Provenance-aware library integration |
| `shape report` / `KernelArtifact` | Compiled, source-free prefix preparation | Tracing artifact loader; [trace input modes](../tracing/trace-cli.md#input-modes) |

None of the first three JSON forms can be substituted for a `KernelArtifact`
at a trace command's `--report` boundary.

## CLI

Build the release executable once as described in
[Build once](../../README.md#build-once); these examples use `shape` directly:

```text
shape kernel show tests/fixtures/kernels/javascript/kernel.raw.js
shape kernel show /absolute/path/capture.js --input-kind deobfuscated
shape kernel show tests/fixtures/kernels/javascript/kernel.raw.js --format markdown
shape kernel show tests/fixtures/kernels/javascript/kernel.raw.js --format json --pretty
shape kernel show tests/fixtures/kernels/javascript/kernel.raw.js --op 7
```

The default parses raw JavaScript, runs the existing pipeline once to obtain
its standardized source, and emits the deterministic flat tracer-input bundle.
`--input-kind deobfuscated` is an explicit opt-in for an
already transformed source. `--format json|markdown` (`md` is an alias) and
`--pretty` are presentation-only; `--pretty` applies only to JSON and is
rejected with Markdown. Flags can appear before or after the source. The CLI never
prints deobfuscated JavaScript and never executes JavaScript or WebAssembly.

`shape kernel show` is the current CLI for this flat bundle. It has no
output-file option, accepts repeatable local numeric `--op KEY` selections, and
uses status 0 for successful inspection (including a closed pipe), 1 for an
operational failure, and 2 for invalid options, modes, keys, or positionals.
The bundle is not `shape-runtime-config/v1`; that
versioned, provenance-heavy format remains a library/report contract.

The bundle is a flat name-to-value map with actual binding names as keys.
Operation-table values contain exact handler source strings, all included by
default. `--op` filters those maps by proven local key and reports a helpful
error for a missing key. The `entry` object keeps known `pc`, `state`, and
`initialSlots`; it does not contain `entry.arguments`. Descriptor `initialSlots`
remain separate from the startup vector. Artifact IDs and evidence/span
envelopes belong to `RuntimeConfig`, not this display. See the
[checked-in bundle](../../tests/fixtures/runtime-config/actual-properties.json) for a complete
CLI response.

## Library hand-off from a raw capture

When a Rust caller needs evidence and statuses instead of the flat CLI
view, reuse one pipeline report:

```rust
use shape::{deobfuscate_with_report, runtime_config_from_report,
            RuntimeConfigOptions};

let source = std::fs::read_to_string(
    "tests/fixtures/kernels/javascript/kernel.raw.js"
)?;
let report = deobfuscate_with_report(&source)?;
let config = runtime_config_from_report(
    &source, &report, RuntimeConfigOptions::default()
)?;
println!("{}", config.stable_json());
```

This example is a function-body fragment returning an error-compatible result.
`from_report` checks that the report's input hash belongs to the **exact**
supplied source bytes, then scans `report.output` for artifact facts. Passing
a different capture with the same-looking VM structure fails identity
validation. Each `RuntimeArtifact` exposes its kind, status, value/bytes when
proved, metadata, and evidence; read `config.diagnostics` as well. An
`Unknown` fact is a result, not a missing JSON field that may be filled from
another capture. `extract_runtime_config(&source)` is the convenience path when
the caller does not already own a pipeline report.

## Schema and proof boundary

`RuntimeConfig` is a collection of static facts about one capture. Each fact
states what was established, where its evidence came from, and what remains
unknown. An `extracted` table or byte image is useful input to further analysis;
it does not certify a complete executable VM.

The implementation and serializer live in
[`src/kernel/config.rs`](../../src/kernel/config.rs). The schema version is
`shape-runtime-config/v1`; the analysis version is
`shape-oxc-deob/runtime-config-v1`. `json()` and `stable_json()` currently
produce the same deterministic serialization.

### Envelope and source identity

The top-level fields describe the analysis before any individual artifact is
interpreted:

| Field | Meaning |
| --- | --- |
| `schema_version` / `analysis_version` | Wire contract and extractor revision |
| `input_mode` | `raw-javascript`, `deobfuscated-javascript`, or `saved-report` |
| `source` | Capture identity, or the declared capture identity in saved-report mode |
| `artifact_source` | Fresh identity of the JavaScript actually scanned for runtime facts |
| `coordinate_system` | Human-readable description of the span coordinate space |
| `artifacts` | Ordered records with individual status and evidence |
| `diagnostics` | Analysis-wide findings, including declined certification and truncation |

Both identity objects contain `algorithm: "fnv1a64"` and a hexadecimal
`digest`. These are byte-identity checks for correlating inputs and artifacts,
not cryptographic authentication of a report or its producer.

The entry point determines which bytes are scanned and which identity can be
checked:

| Library entry point | Scanned JavaScript | `source.digest` | Artifact `span_source` |
| --- | --- | --- | --- |
| `extract(source)` | Newly produced `report.output` | Computed from raw `source` | `artifact` |
| `from_report(source, report, options)` | Supplied `report.output` | Computed from `source`; must match `report.input_hash` | `artifact` |
| `extract_deobfuscated(source, limits)` | Supplied `source` | Computed from the same supplied bytes | `input` |
| `from_saved_report(report, options)` | Supplied `report.output` | Declared by `report.input_hash`; original bytes are absent | `artifact` |

In all four cases, `artifact_source.digest` is computed from the scanned
JavaScript. With `extract_deobfuscated`, both identities therefore describe
the same bytes. That path has no pipeline report to supply operation-catalogue
or thread-descriptor facts.

`from_report` always scans `report.output`. Changing `options.input_mode`
does not switch the parsed source or move spans into the raw capture. Use the
entry point, `coordinate_system`, and `span_source` together when resolving a
location.

### Spans and saved reports

Artifact spans and optional evidence spans are half-open UTF-8 byte ranges,
`[start, end)`, into the source named by `span_source`. They are neither
character indices nor offsets into decoded bytecode or WASM memory. For a raw
capture, an artifact span must be sliced from `report.output`, not from the
original capture.

Records without a specific location can use `0..0`; category-level unknowns
also have evidence with no span. Treat these as unlocated findings rather than
source text at byte zero. The ordinal IDs (`artifact-00000`, and so on) identify
records within this result. They are deterministic for the same analysis, but
are not persistent keys across changed captures or extractor revisions.

`from_saved_report` accepts a complete typed `PipelineReport`. It validates
such envelope details as the hash format, non-reversed spans, and agreement
between declared extraction lengths and retained decoded bytes, then scans
the standardized output again. Runtime consumer relationships are checked
against that output; report-backed thread literals must match the scanned
binding name and declarator span.

This validation does not authenticate the absent raw capture or make every
report claim authoritative. Pipeline binding/property mappings can describe
the original source, so their coordinates are not automatically constrained
to `report.output`. Further consumers still have their own admission checks.
A saved `PipelineReport` is also distinct from the source-free `KernelArtifact`
accepted by tracing commands.

### Reading an artifact record

Every serialized artifact includes the following fields, with `null` for
absent optional data:

| Field | How to read it |
| --- | --- |
| `id`, `kind`, `status` | Local record identity, fact category, and proof outcome |
| `binding`, `consumer` | Optional source labels; `consumer` is currently left `null` by this extractor |
| `span`, `span_source` | Location and coordinate space of the finding |
| `byte_length`, `bytes_base64` | Exact retained byte count and serialized bytes, when present |
| `value` | A bounded literal snapshot, when available |
| `metadata` | String-to-string details specific to the kind and proof rule |
| `evidence` | Rules and explanations, each with an optional span and its source label |

Consumer relationships are described in metadata and evidence. A null
`consumer` field does not mean that the artifact has no consumer, and the
record is not a complete typed dataflow graph. Metadata values such as
`width_bytes: "1"` are strings, even when their contents look numeric.

Read `status` before using a value:

| Status | What it permits |
| --- | --- |
| `extracted` | Rely on the specific fact described by this record's rule and evidence |
| `unknown` | Inspect a candidate or snapshot; the required relationship was not proved |
| `unsupported` | Inspect the reason a recognized candidate could not be accepted, such as invalid WASM module bytes |

An unknown record may still retain a literal or decoded bytes. For example,
a mutated bytecode binding can retain its initial decoded snapshot without
certifying those bytes as the runtime image. Non-null data alone is not an
admission signal.

When a category has no bounded proof, the scanner normally adds an unknown
placeholder with `category-not-proven` evidence. This is not a successful
empty table or empty byte array. Artifact-count truncation can suppress these
placeholders too, so consumers must also read `diagnostics`; there is no
whole-configuration completeness flag.

### Literal values and byte encoding

The Rust `RuntimeValue` model contains strings, numbers, booleans, null,
arrays, and objects. It records bounded literals, not arbitrary JavaScript
objects, closures, or live state. In JSON, both an absent optional `value`
and a literal `RuntimeValue::Null` serialize as `null`; the field alone
cannot distinguish them. Use the typed value and accompanying status/evidence
when that distinction matters.

Rust callers receive retained bytes through `RuntimeArtifact::bytes`. JSON
serializes those bytes as **standard, padded base64** in `bytes_base64`,
regardless of the original decoder's alphabet or padding policy.

For example, a certified source payload `AQIDBA` decodes to the four bytes
`[1, 2, 3, 4]`. Its JSON representation is `bytes_base64: "AQIDBA=="` with
`byte_length: 4`. The added padding is a serialization choice; it does not
mean the source decoder accepts padded input. Where retained,
`metadata.encoded_literal` describes the original source payload instead.

### What the current proof rules establish

Each kind has a deliberately limited meaning. The JavaScript-side facts are:

| Kind | Established fact and remaining boundary |
| --- | --- |
| `decoder-parameters` | A symbol-resolved decoder matches the certified standard or URL-safe template; metadata records the alphabet, proof rule, and destination parameter |
| `dispatch-table` | A bounded nested lookup reads `table[state][bytecode[pc++]]`, writes `record[0]` to the same state field, and returns `record[1]`; runner/handler linkage remains separate |
| `js-bytecode-image` | A certified decoder result is linked to that dispatch read; mutation or a destination-backed returned view prevents certification as an exact runtime image |
| `operand-format` | The proven fetch reads one byte and advances the PC by one; this is not a schema for the selected handler's operands |
| `string-table` | A bounded all-string literal array is retained as an initial snapshot; the current candidate rule selects arrays with more than 32 entries |

Candidate discovery and semantic proof are separate. The string-table size
threshold selects literals to inspect; it does not establish their VM role.
Mutation can downgrade a string-table snapshot to unknown. Even an extracted
snapshot does not prove a lazy string decoder's XOR key, cache behavior, or
runtime immutability.

A proven dispatch record contains a **selector**. The runtime-config layer
leaves its handler mapping unknown until a downstream consumer proves the
join to an operation table. Likewise, `operand-format` records
`completeness: "byte-fetch-observation-only"`; interpreting this as every
handler having a single one-byte operand would be incorrect.

WASM bytes and report-backed facts have their own boundaries:

| Kind | Established fact and remaining boundary |
| --- | --- |
| `wasm-module` | Certified decoded bytes reach a recognized `WebAssembly.Module` consumer and pass bounded module validation; this does not establish an interpreter role |
| `wasm-memory-image` | The decoder writes an exact byte prefix into a proven, bounded `Uint8Array` view of `WebAssembly.Memory.buffer`; metadata records the view and capacity |
| `thread-descriptor` | A report-identified descriptor matches the scanned binding and declarator span and retains a literal snapshot; unmatched or nonliteral candidates become unknown |
| `initial-memory-slots` | A candidate entry argument is retained when literal, but its slot role remains unknown; argument position alone is insufficient proof |
| `operation-table` | The report-backed catalogue supplies table identity and handler count |
| `operation-recipe-reference` | A catalogue fingerprint and handler index identify a recipe candidate; the record neither embeds an executable template nor grants recipe admission |

A memory image is the decoded prefix written to its destination, not the
entire returned typed-array view or a complete memory snapshot. A
destination-backed decode therefore cannot be reused as whole module bytes
without proving the missing view contents. Foreign, mutated, or unbounded
destinations remain unknown.

### Decoder certification and downstream admission

The exact decoder rule is documented in
[runtime-decoder.md](runtime-decoder.md). The runtime scanner's feature probe
only discovers candidates. Certification checks the complete symbol-resolved
AST template, declaration identity, and relevant mutation/native constraints
before decoded bytes become evidence.

The source payload must match the proved alphabet: standard `+/` or URL-safe
`-_`. This custom consumer proof rejects padding, whitespace, malformed
lengths, and characters outside that alphabet. The transport decoder's broader
capabilities do not relax this contract. Declined candidates retain available
source evidence and remain unknown. Direct `eval` or `with` causes decoder
certification to be declined and adds a diagnostic.

Runtime-config does not prove WASM dispatch geometry, a branch-to-JavaScript
handler map, or per-branch operand widths and signedness. Those require
additional operator/dataflow analysis. Nearby arrays, image-tail arithmetic,
large constants, and matching permutations do not supply that proof.

Downstream preparation already has separate contracts for
[JavaScript dispatch](../tracing/symbolic-tracer.md#bounded-js-dispatch) and
[structurally admitted WASM prefix tracing](../tracing/wasm-prefix.md#wasm-dispatched-kernels-prefixwasmrs).
Those consumers establish the relationships needed for their own execution
paths. Keep that distinction when integrating: a runtime fact may be useful
and correct while still being insufficient to prepare a trace.

### Rotating captures and structural recognition

Every capture supplies its own entry coordinates, states, image/table
offsets, handler indices, descriptor keys, dimensions, and decoder constants.
A value observed in one capture must not silently fill an unknown in another.
Source offsets locate evidence within that capture; they do not establish
semantic identity across captures.

Reusable rules describe relationships: the opcode lookup, state update,
selector use, operand assembly, slot initialization, and decoder dataflow.
A renamed binding or changed coordinate can preserve those relationships.
A changed consumer must satisfy the rule again, even when the surrounding
source resembles a previously supported family.

Regression fixtures should pair equivalent structural variants with near
misses: changed assignment order, mutated bindings, shadowed natives, wrong
alphabets, and unrelated destination views. Successful discovery must never
promote an unproved candidate merely because its shape looks familiar.

### Limits and failure handling

`ExtractionLimits` bounds input size and candidate processing. Defaults are:

| Limit | Default | Scope |
| --- | --- | --- |
| `max_source_bytes` | 16 MiB | Raw input and standardized source size checks |
| `max_decoded_bytes` | 8 MiB | Decoded payload and memory-image certification bounds |
| `max_literal_elements` | 100,000 | Bounded literal aggregate traversal |
| `max_literal_depth` | 64 | Literal recursion and the lexical preparse nesting guard |
| `max_artifacts` | 10,000 | Number of records retained in the result |

Failure handling depends on the boundary:

- Invalid input, identity mismatch, parse/pipeline failure, or a top-level
  source/nesting limit returns a typed `RuntimeConfigError`.
- A candidate that exceeds literal or decoding bounds can lose its proof and
  produce an unknown finding. An `Ok(config)` result does not certify every
  candidate.
- Reaching `max_artifacts` drops additional records and adds an
  `artifact limit ... reached` diagnostic. It does not return a typed error
  for the whole extraction.

Base64 output capacity is checked before allocation. The nesting guard runs
before Oxc parsing; these controls do not claim to bound every allocation
inside Oxc. Consumers should handle both the outer `Result` and the statuses
and diagnostics within a successful configuration.

Focused regressions for these contracts live in
`tests/runtime_config.rs`,
`tests/runtime_decoder.rs`, and the config
module's unit tests:

```sh
cargo test --lib kernel::config::tests
cargo test --test runtime_config --test runtime_decoder
```
