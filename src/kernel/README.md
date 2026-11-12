# Kernel facts

This folder extracts and displays entry coordinates, operation and string
tables, bytecode, layout, and JavaScript/WebAssembly relationships. It does not
run a trace. The interpreter lift in `wasm/` supplies a bounded plan to prefix
preparation.

```sh
shape kernel show tests/fixtures/kernel.raw.js --format markdown --op 0
```

For the typed APIs and identity rules, read [kernel analysis](../../docs/deob/kernel-analysis.md).
The [kernel layout guide](../../docs/deob/kernel-layout.md) owns the command output.

## Files

| File | Responsibility |
| --- | --- |
| [`config.rs`](config.rs) | Source-linked runtime facts and explicit extracted/unknown/unsupported status |
| [`bundle.rs`](bundle.rs) | Flat `kernel show` projection of the actual values |
| [`layout.rs`](layout.rs) | Structural inventory |
| [`roles.rs`](roles.rs) | Canonical VM role vocabulary |
| [`properties/`](properties/README.md) | Actual values: types and public API in [`mod.rs`](properties/mod.rs), the AST reader in [`scanner.rs`](properties/scanner.rs), and the JSON projection in [`properties/json.rs`](properties/json.rs) |
| [`decoder_proof.rs`](decoder_proof.rs) | Narrow standard/URL-safe base64 decoder proof (with its characterization test inline) |
| [`wasm/`](wasm/README.md) | The JS/WASM boundary and its helpers, grouped below |
| [`mod.rs`](mod.rs) | Module wiring and the `kernel::wasm_*` compatibility re-exports |

`wasm/` groups the boundary with its helpers, preserving their paths and
visibilities. `wasm/mod.rs` (`kernel::wasm`) owns imports/exports/handlers;
`wasm/module.rs` (`kernel::wasm_module`, crate-private) owns bounded
decode/parse/validation; `wasm/dispatch.rs` (`kernel::wasm_dispatch`, public)
is the fixture-scoped one-cell adapter; `wasm/lift.rs` (`kernel::wasm_lift`,
crate-private) is the structural interpreter lift.

## Choose a surface

| Need | API or module |
| --- | --- |
| Typed actual values | `extract_actual_properties(&report.output)` in `properties/` |
| Flat `kernel show` data | `ActualProperties::tracer_json` / `tracer_markdown`, implemented in `bundle.rs` |
| Facts with status and evidence | `runtime_config_from_report(raw_source, &report, options)` |
| Structural inventory | `static_kernel_layout(&report.output)` |
| Import/export and table relationships | `analyze_wasm_boundary_report(&report)` |
| A tracing input | Compile a `KernelArtifact` through dispatch; inspection JSON is not loadable there |

## Contracts to preserve

Unknown, unsupported, and extracted facts remain distinct. Retained expressions
are source text, not evaluated values. Bindings and AST structure establish
roles; canonical names are presentation.

Keep raw input bytes for APIs that check `report.input_hash`. Read standardized
spans against `report.output`, and preserve the coordinate system on every fact.
A module inventory, a dispatch adapter, and an interpreter lift prove different
things; none automatically supplies the others' authority.

## Diagnose and verify

Start with `FactStatus`, diagnostics, and `unresolved` for missing data. If a value
appears in one view only, check projection policy before changing extraction.
For a bad span, check its source basis before changing coordinates.

Focused suites: actual properties,
property review,
bundle, and layout.
The [properties](properties/README.md) and [WASM](wasm/README.md) guides identify
more specific checks.

Detailed contracts: [kernel analysis](../../docs/deob/kernel-analysis.md),
[runtime configuration](../../docs/deob/runtime-config.md),
[decoder proof](../../docs/deob/runtime-decoder.md), and
[WASM boundary](../../docs/reference/wasm-boundary.md).
