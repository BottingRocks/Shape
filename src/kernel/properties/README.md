# Actual kernel properties

This module reads values and structure from standardized JavaScript using Oxc
bindings and extraction limits. It never executes a function or turns retained
source text into a guessed runtime value. Detailed field, slot, and presentation
contracts live in [kernel analysis](../../../docs/deob/kernel-analysis.md).

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

## Files

| File | Responsibility |
| --- | --- |
| [`mod.rs`](mod.rs) | Public property types, extraction entry points, and limits |
| [`scanner.rs`](scanner.rs) | AST and binding walk that collects entries, slots, and operation tables |
| [`json.rs`](json.rs) | Compact JSON presentation of extracted values |
| [`tests.rs`](tests.rs) | Extraction and WASM-summary boundary cases |

## Essential distinctions

`PropertyValue::Expression` is unresolved source; `Function` preserves a body.
Neither is executed. Entry accessors return `None` when the literal shape is
not established. Do not parse an expression string to manufacture a slot index.

Constructor initializers, prototype defaults, entry startup slots, and descriptor
slots are separate facts. Helper write templates describe a possible effect,
not an observed invocation. Compact presentation selects handler bodies by local
table key; the flat bundle includes all handlers by default.

## Debugging and tests

Inspect the typed field and `unresolved` first. `scanner.rs` owns recognition,
`json.rs` owns compact presentation, and [`bundle.rs`](../bundle.rs) owns the flat
bundle. `ActualPropertiesError` separates invalid input, parsing, and limits.

```sh
cargo test --test actual_properties --test actual_properties_review --test tracer_bundle
```

See [kernel analysis](../../../docs/deob/kernel-analysis.md#following-a-missing-value)
for the symptom table and [kernel layout](../../../docs/deob/kernel-layout.md) for the
bundle contract. Start at the [parent guide](../README.md) for adjacent analyses.
