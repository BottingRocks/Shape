# Static WASM/JavaScript boundary inventory

The WASM boundary analysis is an opt-in, library-only inventory of the relation
between the standardized Oxc output and embedded WebAssembly. It is a
locator/proof report, not a VM, tracer, evaluator, or WebAssembly engine. It
uses only Rust/Oxc and `wasmparser`; it does not invoke Node, Babel, `eval`, a
JavaScript runtime, or WASM execution, and it does not change the default
pipeline output.

## Library API

The analysis is exposed from the `shape` crate. There is no
`shape-wasm-boundary` binary and no `shape` CLI subcommand for it in the accepted
CLI contract.

Callers that already own a `PipelineReport` should use the report directly:

```rust
let report = shape::deobfuscate_with_report(raw_source)?;
let boundary = shape::analyze_wasm_boundary_report(&report);
```

`analyze_wasm_boundary_report` parses `PipelineReport.output` once with Oxc and
uses only the report's extracted `WasmModule` facts to select immutable bytes.
It does not rerun deobfuscation and does not rewrite the raw input or module
bytes. The convenience wrapper `shape::analyze_wasm_boundary(source)`
runs `deobfuscate` exactly once and then performs the same analysis.

Serialization lives in `shape::wasm_boundary`:

- `boundary_json(&report)` emits the versioned JSON document;
- `boundary_markdown(&report)` emits a compact inventory with per-module
  imports, table proof counts, and unsupported findings;
- `SCHEMA_VERSION` is `shape-wasm-js-boundary/v1` and `ANALYSIS_VERSION` is
  `shape-oxc-deob/wasm-js-boundary-v1`.

A report used against a raw capture therefore links both the immutable extraction
artifact and the standardized deobfuscated source. Names remain display hints:
module identity comes from the extraction fact and source span, while JS aliases
are accepted only when Oxc's symbol/dataflow walk resolves the object and
property.

The two report hashes (`pipeline_input_hash` and `deob_output_hash`) use the
same FNV-1a 64-bit `stable_hash` as the pipeline's `input_hash`. They are stable
content identifiers for this report, not cryptographic digests and not
substitutes for SHA-256.

Read a report in identity order: select the extracted module, then its import
namespace/name and actual WASM index, then the JS instance-object binding,
then the optional original handler key. A numeric import field such as `"7"`
is a validated import name; it becomes handler key 7 only when the imported
function is joined to the proven operation-table binding. The test fixture
`wasm_boundary.rs` exercises both outcomes: a
direct proven table import carries `original_handler_key: Some(7)` at table
index 0, while an adapter-shaped `vmHostBindings` copy keeps the import fact
but reports zero proven handler mappings. Do not substitute table index 0 or
the import spelling for a handler identity.

## What is proven

For each validated module the report records:

- module hash, byte length, extraction/deobfuscated source links, and validation
  status;
- every import's namespace, name, external kind, signature, external-kind index,
  function index when applicable, JS instance-object binding/property, source
  span, proof status, and evidence;
- exports and their actual function/table indices and function signatures;
- imported versus defined function index space;
- table type/limits, exported table names, default `ref.null` entries, active
  element offsets, function references, and null entries;
- direct, return, indirect, and return-indirect call sites with caller function
  index, WASM byte offset, signature/type/table indices, operand ordering, and a
  bounded immediate-index provenance note when one is directly adjacent;
- table-entry-to-import mappings. A numeric `<namespace>.<key>` import is linked to the
  original handler key only when the JS import object resolves to a proven
  operation-table binding. Table index, element index, branch/index operands,
  import field, and original handler key remain separate fields.

Direct host imports remain direct host imports even when their names or bodies
look operation-like. The report only calls a numeric import a handler when its
instance namespace resolves to the exact proven operation-table binding and
property; unrelated numeric namespaces and shadowed/reassigned aliases remain
unknown. Cross-module imported tables are resolved only through a validated
`WebAssembly.Instance` symbol and exported table name, then joined to the
producer's active element table. The report never turns a branch index into a
handler index.

### Generated adapter boundary

Current post-rename output no longer keeps the original operation table
as the instance import object. It emits a generated adapter with computed keys:

- `vmWasmAbi` carries the semantic ledger of host namespace/import/export names;
- `vmHostBindings` copies the original operation-table function expressions
  under keys such as `hostCallback0`;
- `vmHostImports` builds the computed-key namespace object passed to
  `new WebAssembly.Instance(...)`;
- the interpreter receives `{ v: vmHostImports, u: handlerTableInstance.exports }`
  inline.

Both shapes below are synthetic contract fixtures from `tests/wasm_boundary.rs`,
not captures. `module` is defined earlier in the fixtures and is elided here.

```js
// Direct namespace: the import resolves to the proven operation-table binding.
var vmOperations = { 7: function (context) { return context; } };
var opsAlias = vmOperations;
var instance = new WebAssembly.Instance(module, {
  f: { directHost: function (context) { return context; } },
  s: opsAlias,
});

// Generated adapter: the import is proven, but the copied closure has no
// operation-table provenance.
var vmWasmAbi = {
  namespace: { host: "q" },
  imports: { hostCallback0: "7" },
  exports: { wasmExport0table0: "Q" },
};
var vmHostBindings = {
  ["hostCallback0"]: function (context) { return context; },
};
var vmHostImports = {
  [vmWasmAbi.namespace.host]: {
    [vmWasmAbi.imports.hostCallback0]: vmHostBindings["hostCallback0"],
  },
};
var instance = new WebAssembly.Instance(module, vmHostImports);
```

**A proven import is not a proven handler.** The first shape can produce
`original_handler_key: Some(7)`; the second keeps `operation_table` absent and
records no proven handler mappings (`proof_counts.handler_mappings == 0`) even
when the function body is identical. Non-proven mapping rows may still be
recorded; their count is not a proven-handler count.

The boundary analysis resolves the adapter's computed keys to inventory the
validated WASM namespace/name relation, but it does not claim that
`vmHostBindings` values came from the selected operation table. The current
`PipelineReport` carries exact table identity and shape, not generated-adapter
binding/dataflow provenance. Computed-key shape, matching key sets/table size,
or equal function source text cannot establish that missing relation: unrelated
objects, shadowed closures, duplicated expressions, and non-function values can
all look identical. Consequently a copied-adapter numeric import remains a
proven JS/WASM import fact but has no `operation_table` link and contributes no
`handler_mappings` proof. Direct `namespace: vmOperations` bindings retain the
strict exact-symbol/table/property proof.

## Explicit declines

The report uses `unknown`, `partial`, or `unsupported` findings for dynamic or
unresolved relations. In particular, it declines incomplete instance import
objects, passive/dynamic element initialization, oversized tables, imported
table contents whose producer cannot be proven, table writes/growth/copy/fill,
unresolved mutable table writes, and unbounded indirect indices. Null/default
slots remain visible rather than being filled by a guessed function. An input
with no extracted WebAssembly module has `wasm_present: false`, `js_only: true`,
no modules, and a top-level
`wasm_boundary` unsupported finding.

The interpreter module is intentionally reported as `partial` on the
WebAssembly VM reference. Its imported table contents are proven through the
producer instance and exported table, but its function and memory imports remain
`unknown`: their namespace object statically resolves to the later adapter object
keyed by the handler namespace, and the analysis does not guess which
`vmHostImports` declaration supplies them. The handler-table module's numeric imports remain
validated namespace/name facts, but copied-adapter handler links are unknown
because the report has no provenance for the generated closures. The previous
adapter-shaped handler counts are therefore not discovery regressions; they
were unsupported identity claims removed at this boundary.

The report is not a generic WASM interpreter. It does not decode a linear-memory
program image, infer operand widths, execute `br_table`, or claim a dispatch
branch-to-handler mapping. Those are separate open questions. It also does not
expand `workerSource`, alter tracing, or introduce a second analysis
IR.

## Canonical reference inventory

The checked-in references supply both boundary shapes. The tracked WebAssembly
VM reference has two modules, a handler table and an interpreter; the tracked
JavaScript VM reference has no extracted WebAssembly module. The optional local
input test reads a directory only when `SHAPE_WASM_BOUNDARY_SEEDS_DIR` is set;
that directory is test input, not a checked-in reference.

| Reference | Modules | JS/WASM imports linked | Table entries proven | Handler mappings | Module status |
|---|---:|---:|---:|---:|---|
| WebAssembly VM reference | 2 | 202 / 215 | 201 / 201 | 0 copied-adapter links | handler table `proven`; interpreter `partial` |
| JavaScript VM reference | 0 | 0 / 0 | 0 / 0 | 0 | `js_only: true` |

The WebAssembly reference exposes a handler-table module and an interpreter
module. The import-link counts are validated JS/WASM namespace/name facts, not
claims that the adapter functions are the original operation handlers. The
table-entry count remains proven through the producer table and
instance/export relation; the copied-adapter handler-link count is zero.

The WebAssembly reference records 71 call sites. Those are call-site inventory
counts, not dispatch handler counts. A module's presence alone does not prove
WebAssembly interpreter placement; that classification requires the source
evidence described in [architecture.md](../architecture.md).

The counts above are proof counts, and imported-table entries are counted once at
their producer while the consumer report still shows the resolved imported
entries and evidence. In particular, the roughly 234 dispatch branches seen in
the interpreter image are not 201 handlers; the branch-to-handler join remains
unproven.

## Reproducing the checks

The checked-in tests synthesize reports and adapter-shaped sources, including
negative cases for unrelated computed objects, shadowed closures, and
non-function values, so they run without any local capture:

```text
cargo test --test wasm_boundary
```

The optional local-input check is environment-gated and must use a release
build because it deobfuscates full inputs. It is test evidence only; its inputs
are not checked-in references:

```text
CARGO_BUILD_JOBS=2 \
SHAPE_WASM_BOUNDARY_SEEDS_DIR=/absolute/path/to/inputs \
cargo test --release --test wasm_boundary -- --nocapture
```

Without the environment variable the test prints a skip notice and returns
success; it does not silently substitute synthetic data. Local inputs stay
outside the repository and are not part of the canonical reference set.

## Two imports that look the same but prove different things

The synthetic direct-import test in
`tests/wasm_boundary.rs` has a numeric WASM
import `s."7"` at function index 1. Oxc resolves its JavaScript import object
to the proven operation-table binding (`opsAlias` in the test). The active
element at table index 0 references function index 1; table index 1 is null.
That chain allows `original_handler_key: Some(7)` for table index 0. The
other imported function, `f.directHost`, is function index 0 and is never
promoted to a handler merely because it is callable. The defined function is
index 2, after both imports; `call_indirect` identifies its table separately.
These are four different coordinates: handler key 7, imported function index 1,
table index 0, and the indirect call's caller/offset.

The adapter-shaped test uses a generated `vmWasmAbi` and
`vmHostBindings["hostCallback0"]`. It also resolves a validated import with
namespace `q`, name `"7"`, and JS binding
`vmHostBindings["hostCallback0"]`. The import status is `proven`, but
`operation_table` is absent and `handler_mappings` remains zero. An identical
function body would not change that outcome; separate tests use an unrelated
computed namespace, a shadowed capture, and non-function values to guard this
boundary. The missing evidence is a binding/dataflow chain from the adapter's
copied closure to the specific proven operation-table row. Matching the key
string or comparing printed function text would make an unsupported identity
claim.

For a quick source-backed check without local captures, run the focused suite:

```sh
cargo test --test wasm_boundary resolves_function_index_space_elements_and_js_aliases
cargo test --test wasm_boundary adapter_shaped_computed_namespace_stays_unknown_without_binding_provenance
```

The first command exercises the direct proof; the second exercises the
adapter unknown. The library's `boundary_json` and `boundary_markdown` render
the same inventory for a `BoundaryReport`. They preserve unknowns in the
output; neither renderer performs a new join or invokes WASM.

## What an unresolved item would need

“Unknown” in this report names a particular missing relation. A copied
adapter mapping needs source-level closure provenance tied to the exact table
symbol and property. An imported table needs a validated producer instance,
exported table identity, and active element contents. A dynamic table write
needs its target and value proved over the relevant control flow; static
initial elements alone cannot settle later contents. A `br_table` selector
needs a separate interpreter/image analysis to join its case to a handler.
The boundary report intentionally does not fill these fields from counts,
spelling, or historical similarity. A caller should carry the status and
evidence forward, then use the stricter WASM prefix preparation when it needs
an executable dispatch claim.

## Implementation reference

The [module guide](../../src/kernel/wasm/README.md) maps these responsibilities to
files. Interpreter-to-recipe details live in [WASM prefix preparation](../tracing/wasm-prefix.md).

## What the boundary report contains

`WasmBoundaryReport` carries the schema and analysis versions, the report's
`pipeline_input_hash` and `deob_output_hash` (FNV-1a-64 content identifiers),
`wasm_present`/`js_only`, the modules, `proof_counts`, and unsupported findings.
Each `WasmModuleBoundary` records:

| Fact | Fields |
| --- | --- |
| Module identity and source | source binding, extraction/deobfuscated source links, module hash, byte length, instance id, validation status |
| Imports | index, namespace/name, kind, signature, JS instance binding/property, optional operation-table link, source link, status, evidence |
| Exports and functions | export name/kind/index/signature; function index space, signature, and import origin |
| Tables and elements | table type/limits, exported names, entries, active element segments, offsets, null/default entries, mutability |
| Call sites | caller function, WASM byte offset, kind, target/table/type indices, signature, operand order, bounded provenance |
| Handler mappings | table/element/function indices, import name, host wrapper, original handler key when proved, status, evidence |

`ProofCounts` summarizes imports, linked imports, functions, table entries,
proven table entries, call sites, and handler mappings. Counts are evidence
coverage, not coverage promises: a module can be validated while many JS links
remain unknown.

## Module decoding and validation

`module.rs` centralizes the low-level WASM work so the four consumers do not
grow independent decoders. It owns two historical base64 policies
(`UrlSafe`, and `Standard { limit }`), strict padding/length checks, bounded
decoded output, module parsing/validation via `wasmparser`, and a neutral ABI
summary of imports, exports, and the imported/defined memory booleans.

Policies deliberately differ by caller: runtime-config and layout require
validation, while actual-properties keeps its historical parse-only acceptance.
The shared module preserves both outcomes rather than forcing one. A detailed
parse error must never erase the simple ABI result that older callers used.
`ModuleLimits` bounds module bytes (8 MiB by default) and per-table
materialization (100,000 entries); the entry bound is a projection threshold,
not a global rejection budget.

Decoding policy also belongs to the consumer. Runtime payload extraction uses
the alphabet established by the [decoder proof](../deob/runtime-decoder.md);
the transport-oriented helper is not evidence that a source decoder accepts
the same bytes.

## The pinned dispatch adapter

`wasm/dispatch.rs` is deliberately narrower than the JavaScript dispatch
driver. It admits immutable, source-derived module and memory-image facts,
resolves one `(pc, state)` cell, and reports the host-import boundary. It does
not execute WASM, invoke copied JavaScript handlers, or claim a table element
is an original operation.

Admission is fixture-scoped. `prepare_s3(source, report)` checks the checked-in module/image
fingerprint: base `170845`, stride `468`, 234 opcodes, 201 handlers. `prepare`
accepts limits (`max_image_bytes` 8 MiB, `max_opcodes` 4096, `max_states` 256,
`max_table_entries` 4096) but is still pinned to the checked-in module/image
pair by SHA-256; there is no name-only or image-tail fallback. It rebuilds the
canonical report to authenticate the supplied hand-off before reading bytes.

`resolve(pc, state)` validates the entry PC, dispatch region, state, opcode,
and cell bounds, then reads `(selector, next_state)` from the image. The result
includes the cell address/span, selector, next state, and an optional host
import. A host import is a required stop until a host model and original-handler
provenance are separately proven; the adapter never calls it. Errors are
`MissingFact`, `AmbiguousFact`, `InvalidFact`, `Unsupported`, `Limit`, and the
four bounds variants.

This adapter is not general WASM dispatch. The prefix path admits interpreters
by structure instead; do not promote one fixture's geometry into a rule.

## The interpreter lift

`wasm/lift.rs` lifts an admitted interpreter's run loop into VM operations. It
evaluates every selector path once over abstract values (the context, operand
loads relative to `pc`, call results), for every try and record depth the
locals can hold, and names what the path does as a `SelectorOp`:

- JavaScript operation calls with operand descriptors and length;
- literal/number/table-number pushes, image writes, jumps and conditional
  jumps, pop-jumps, and returns;
- `try` enter/leave, `finally` enter/leave, and catch drop/bind;
- VM subroutine call and return;
- `Trap` for `unreachable` or an unmapped selector.

The model fixes the admitted interpreter's locals: at most three try frames and
two error records, with defined PC/state/context/selector and call-return
banks. `lift` checks every path against that layout; a path it cannot name is an
error, never a guess. Selectors past the branch table trap. Prefix preparation
turns the lifted selectors into the walker's dispatch table, recipes, and
bookkeeping operations, so the same walker and emitter handle WASM-dispatched
kernels. That is a structural lift, not WASM execution.
