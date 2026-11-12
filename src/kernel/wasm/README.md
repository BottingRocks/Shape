# Kernel WASM analysis

These modules inspect captured WebAssembly and its JavaScript boundary without
executing either. Static inventory, the fixture-scoped dispatch adapter, and the
structural interpreter lift have distinct admission rules. A proven import name
does not automatically identify an operation-table handler.

The [WASM boundary reference](../../../docs/reference/wasm-boundary.md) owns inventory,
module parsing, and adapter detail. [WASM prefix preparation](../../../docs/tracing/wasm-prefix.md)
owns the interpreter-to-recipe handoff.

## Choose the analysis by the question

| Question | Entry point or consumer | Result |
| --- | --- | --- |
| Which imports, exports, functions, and tables are linked to this source? | `kernel::wasm::analyze_report(&report)` | `WasmBoundaryReport`, including unsupported findings |
| I only have raw source; where do I start? | `kernel::wasm::analyze_source(source)` | Runs source preparation, then the same boundary analysis |
| How do I format an existing report? | `boundary_json`, `boundary_markdown` | Versioned JSON or a compact inventory; no execution |
| Which dispatch cell does the pinned WebAssembly fixture select? | `kernel::wasm_dispatch::WasmDispatchProgram::prepare_s3`, then `resolve` | One bounded lookup; a fixture adapter |
| Can this interpreter participate in a prefix trace? | Prefix preparation through [`dispatch/prefix`](../../dispatch/prefix/README.md) | A structural lift and shared walker, subject to preparation and per-walk stops |

## Files

| File | Responsibility |
| --- | --- |
| [`mod.rs`](mod.rs) | Source-linked import/export, table, and function inventory |
| [`module.rs`](module.rs) | Bounded base64 decoding, module validation, and ABI summaries |
| [`dispatch.rs`](dispatch.rs) | The fixture-scoped, pinned WASM dispatch-cell adapter |
| [`lift.rs`](lift.rs) | Admitted WASM handler operations for bounded tracing |

`wasm_dispatch` is public; `wasm_module` and `wasm_lift` are crate-private
aliases. The boundary API uses the report's extracted module facts only, without
rerunning deobfuscation or rewriting the module.

## Evidence boundaries

Follow the extracted module identity, import namespace/name and WASM index,
JavaScript instance binding, and finally the optional operation-table key.
Numeric import names and table indices are not substitutes for that chain.
Copied presentation adapters can preserve import facts without proving a link
back to an original handler.

`module.rs` shares decoding and ABI inspection while retaining each consumer's
validation policy. The pinned adapter resolves one cell for an admitted
module/image pair. The structural lift names selector effects for the prefix
walker; it does not run a WebAssembly engine.

A valid module may have unresolved JavaScript links. A successful lift may still
produce a walk that stops for an unsupported handler, machine bound, or budget.
Raw-source prefix preparation can use the lift; saved-pipeline-report
preparation is JavaScript-only. A compiled kernel artifact supports subsequent
source-free WebAssembly tracing.

## Focused checks

- Boundary inventory and binding presentation
- Pinned adapter
- Interpreter preparation and tracing

See [boundary declines](../../../docs/reference/wasm-boundary.md#explicit-declines),
[WASM prefix admission](../../../docs/tracing/wasm-prefix.md#admit-what-must-line-up-before-lifting),
and [host exceptions](../../../docs/reference/wasm-exception-boundary.md) when narrowing a failure.
