# Kernel facts and property extraction

Kernel analysis reads standardized source and reports what is present without
executing JavaScript or WebAssembly. Use this reference for the typed library
surfaces and their identity boundaries. The [kernel module guide](../../src/kernel/README.md)
and [property extractor guide](../../src/kernel/properties/README.md) map the code.

The [kernel layout guide](kernel-layout.md) owns the command's flat bundle;
[runtime configuration](runtime-config.md) owns evidence-bearing facts; the
[WASM boundary guide](../reference/wasm-boundary.md) owns module relationships. A
[kernel artifact](kernel-artifacts.md) is a separate compiled tracing input.

## Runtime facts

`config.rs` models each extracted thing as a `RuntimeArtifact`: `id`,
`ArtifactKind`, `FactStatus`, optional binding and consumer, a span with its
coordinate space, optional bytes and typed value, metadata, and evidence.
Kinds cover WASM modules and memory images, JS bytecode, dispatch tables,
operand formats, string tables, decoder parameters, thread descriptors,
initial slots, operation tables, and recipe references.

Status is the contract: `Extracted` was proved, `Unknown` was not established,
`Unsupported` was declined. A missing artifact is not an empty value and must
not be filled from another capture. `RuntimeConfig` also carries diagnostics,
versions, input mode, coordinate system, and two identities: `source` (caller
input) and `artifact_source` (standardized source scanned).

`InputMode` is `RawJavaScript`, `DeobfuscatedJavaScript`, or `SavedReport`.
`from_report` checks that the report's `input_hash` belongs to the exact
supplied source; `from_saved_report` validates the envelope and computes a
fresh artifact identity for `output` while keeping the declared capture
identity separate.

## Layout

`layout.rs` answers "what is here and how is it connected?". A `KernelLayout`
holds components (kind, status, confidence, count, raw/deobfuscated source
links, pass lineage, evidence, unknown reason), relationships, optional WASM
modules, an operation inventory, readiness and pending facts, and the extracted
`ActualProperties`.

The inventory is diagnostic: classification counts, sparse entries, an optional
dispatch-branch count, and a dispatch link stated as fact or unknown. Optional
per-handler records are recognizer summaries, not execution admission.

## Source identity and coordinates

Raw-source and standardized-output spans use different coordinate systems.
Facts record which one they use; a readable field name alone never proves a
role. A `PipelineReport` carries source and proof context, while the CLI's
`--report` file is a source-free `KernelArtifact` with its own integrity rules.

| Identity | Names | Used for |
| --- | --- | --- |
| `RuntimeConfig.source` | Caller input bytes (FNV-1a-64) | Capture correlation |
| `RuntimeConfig.artifact_source` | Standardized source scanned | Artifact spans |
| `PipelineReport.input_hash` | Raw input identity | Report preparation |
| `report.output` | Standardized JavaScript | Deobfuscated spans |
| `KernelArtifact` | Compiled source-free prefix input | Trace `--report` |

FNV hashes are content identifiers, not cryptographic digests. A SHA-256
elsewhere is a different algorithm over different bytes.

| Need | Entry point using an existing report |
| --- | --- |
| Actual values and the flat bundle | `extract_actual_properties(&report.output)`, then `tracer_json` |
| Runtime facts with evidence and status | `runtime_config_from_report(raw_source, &report, options)` |
| JS/WASM relationships | `analyze_wasm_boundary_report(&report)` |
| Structural inventory | `static_kernel_layout(&report.output)` |

Keep raw input bytes for APIs that authenticate `report.input_hash`; handler
snippets and spans are read against `report.output`.

## Roles, decoder proof, and WASM

`roles.rs` is the closed VM role vocabulary (context, memory, stack,
dispatcher, operands, call frames, exceptions, descriptors, and related
concepts), with canonical strings, Rust names, parameter roles, and labels.
Binding identity, not spelling, decides a role. `decoder_proof.rs` proves one
narrow thing: a function implements the standard or URL-safe base64 decoder
under a specific alphabet and evidence. It is not a general decoding claim;
consumers choose their own policy. See [runtime decoder](runtime-decoder.md).

The static `WasmDispatchProgram` adapter is pinned to one checked-in
module/image pair and resolves only a dispatch cell. The separate [WASM prefix
path](../tracing/wasm-prefix.md#wasm-dispatched-kernels-prefixwasmrs) admits
interpreters by structure and lifts selector effects without executing WASM.
The boundary report inventories imports, exports, functions, tables, elements,
call sites, and handler mappings with proof counts and explicit declines; a
numeric import name becomes a handler key only when its namespace resolves to
the proven operation-table binding. See [WASM boundary](../reference/wasm-boundary.md).

## Using the extractor

Start from standardized source. If the input is a raw capture, run the pipeline
once and pass `PipelineReport::output`:

```rust
let report = shape::deobfuscate_with_report(&source)?;
let properties = shape::extract_actual_properties(&report.output)?;
let selected = vec!["0".to_owned()];
properties.validate_selected_operations(&selected)?;
println!("{}", properties.compact_json(&selected));
```

This is a function-body fragment for a caller returning a compatible `Result`.
Selection is by the handler's actual table key; validate before formatting so
an absent key becomes an error instead of an empty-looking view.
`extract_actual_properties_with_limits` accepts explicit `ExtractionLimits`.

| Surface | Use it for |
| --- | --- |
| `ActualProperties` fields | Typed entry arguments, operation sets, string tables, slots, and unresolved source |
| `compact_json` / `compact_markdown` | The small inspection view, with handler bodies selected explicitly |
| `tracer_json` / `tracer_markdown` | The flat `kernel show` bundle: all handlers by default, tables, field roles, payloads, and selected constants; helper bodies and aliases are omitted |
| `entry_coordinates` / `first_entry_initial_slots` | Concrete entry data only; `None` means the necessary literal shape was not extracted |

Neither serialized view is a `PipelineReport` or the compiled `KernelArtifact`
accepted by tracing.

## What it keeps

`ActualProperties` is a flat collection of typed top-level facts, not a generic
binding dump:

| Field | Contents |
| --- | --- |
| `entry` | Entry function name/parameters, call arguments, and initial memory slots |
| `thread_descriptors`, `operations`, `strings`, `bytecode`, `dispatch`, `decoders`, `constants`, `wasm` | Named tables and payloads found in the copy |
| `unproven_decoder_calls` | Decoder calls without a base64 proof, with exact literal and destination |
| `slots` | Context/memory/stack constructors, prototypes, assignments, templates |
| `properties`, `additional`, `unresolved` | Entry-owned bindings, other bindings, and values the walk could not resolve |

`PropertyValue` preserves what was seen: `String`, `Number`, `Boolean`, `Null`,
`Undefined`, `NaN`, `PositiveInfinity`, `NegativeInfinity`, `Hole`, `Array`,
`Object`, `Function(String)`, and `Expression(String)`. `Expression` is retained
source that static evaluation could not reduce; `Function` is a body kept as
source. Neither is executed or parsed into a guessed slot index.

## Entry and slot facts

Concrete entry data is what tracing consumes. `entry_coordinates()` returns
integer PC/state from arguments 2 and 3, `first_entry_initial_slots()` returns
the fourth argument only when it is an array of non-negative integers, and
`first_thread_descriptor_rows()` returns the first descriptor array. Each
returns `None` rather than an `Expression` fallback or an invented value.

`SlotProperties` separates facts that look similar:

- constructor `this.<field>` values from prototype descriptors;
- proven role constructor names and parameters from field-only maps;
- the entry call's fourth argument (special slot indices) from descriptor
  `initialSlots`;
- writes by the selected entry function from helper-body write templates. A
  template is not a claim that the helper ran.

A stack constructor's literal initial array stays distinct from prototype
defaults, and unnamed role constructors keep their exact maps in
`constructor_properties`; data is not discarded because a role is unnamed.

## Extraction and limits

`extract_actual_properties` parses standardized source once with Oxc, builds
scoping, and walks the AST following resolved lexical bindings. Empty input is
an error, unparseable input is a parse error, and `ExtractionLimits` bound
source bytes, decoded bytes, literal elements, literal depth, and artifact
count (16 MiB / 8 MiB / 100,000 / 64 / 10,000 by default).

The scanner also produces derived summaries through shared `wasm_module`
helpers: bounded base64 decoding, module validity, and the ABI import/export
summary it reports. Decoder calls are recognized only through
`decoder_proof`; a call without the proof stays in `unproven_decoder_calls`.
`ActualPropertiesError` is `InvalidInput`, `Parse`, or `Limit`: a limit error
means a bound was exceeded, not that the kernel is unsupported, and a missing
value belongs in `unresolved`, not an error.

## Two presentations

`compact_json`/`compact_markdown` are the inspection view: operation bodies are
opt-in by table key, and large internal payloads are not duplicated.
`tracer_json`/`tracer_markdown` produce the flat `kernel show` bundle, whose
components are top-level keys using the capture's actual names; empty optional
components are omitted. It carries every operation-table handler by default,
exact string/descriptor tables, context/memory/stack field roles and descriptor
flags, bytecode/module payloads, WASM import/export names, and selected
constants. Alias definitions, decoder bodies, helper sources, caches,
native-function tables, and external-global lists are intentionally absent
as a presentation policy. Handler bodies stay byte-exact,
including aliases they still reference.

The bundle's `entry` has only known positions (`pc`, `state`, `initialSlots`);
it has no parameter names or `entry.arguments`. Descriptor `initialSlots` stay
separate from the startup vector. If two components claim the same top-level
name, the first wins; a consumer needing the full binding inventory must use
the typed facts.

## Following a missing value

Check `unresolved` and the property's `PropertyValue` variant first.
`Expression` is retained JavaScript text, while a literal array or number is
extracted data. Do not parse an expression string into a guessed slot index or
entry coordinate; the accessors above intentionally refuse that fallback. A
missing extracted value and a value omitted by presentation have different
places to investigate: `scanner.rs` owns recognition and binding dependencies,
`json.rs` owns compact display, and [`bundle.rs`](../../src/kernel/bundle.rs) owns the bundle.

| Symptom | Inspect |
| --- | --- |
| Entry PC/state/slots are `None` | Whether the arguments are literal numbers/arrays; `Expression` is not converted |
| A value is in JSON but not in `ActualProperties` | Presentation policy vs extraction; check the typed field and `unresolved` |
| A decoder call has no decoded payload | `unproven_decoder_calls` and the decoder proof |
| A constructor field looks initialized but is not | Prototype descriptor vs constructor initializer |
| A helper write appears as if it ran | `function_write_templates` is a template, not an execution claim |
| Selection fails | `validate_selected_operations`; keys are local to the capture |

Tests: `cargo test --test actual_properties --test actual_properties_review
--test tracer_bundle`. Suites: `actual_properties.rs`,
`actual_properties_review.rs`,
and `tracer_bundle.rs`.

Start with the [kernel module guide](../../src/kernel/README.md). The [property bundle
contract](kernel-layout.md) describes the output and its
coordinate space.
