# Source preparation

This folder is the source-preparation pipeline. It turns raw JavaScript into
standardized source and a `PipelineReport`. The report carries extraction
facts, diagnostics, and proven operation-table identity for downstream modules.
The public entry points are re-exported from [`lib.rs`](../../lib.rs) as
`deobfuscate` and `deobfuscate_with_report`; the owning implementation module is
`transforms::deob::pipeline`. This is source preparation only: it does not
execute a handler or decide how a particular bytecode instruction behaves at
runtime. Post-emission cleanup lives separately in
[`../emitted_js/`](../emitted_js/README.md).

From the repository root, this prints standardized source from a tracked
fixture:

```sh
shape deob tests/fixtures/kernel.raw.js
```

[The deobfuscation guide](../../../docs/deob/deobfuscation.md) covers command
options, the output contract, and the evidence handed to tracing.

## Pass order

```text
raw source → CLEAN → RENAME → FIND → EXTRACT → presentation passes
```

CLEAN removes proven native catalogue and decoder boilerplate. RENAME plans
binding and property names from Oxc semantic identity. FIND recognizes VM
roles; EXTRACT records typed facts. Presentation then rewrites the proven
operation table, WASM bindings, prototype aliases, and declaration layout.
Each rewrite can produce a new syntax tree. Oxc `SymbolId` values belong to
one tree, so a later pass must re-establish identity from retained declaration
position, initializer shape, and source evidence. A readable name alone is
never proof that a new binding is the old one.

## What each stage changes

**CLEAN** runs native alias cleanup before string-decoder collapse. The native
pass checks binding identity, writes, and uses before replacing catalogue
aliases or uncurry wrappers. The decoder pass replaces a complete repeated
base64/XOR region with a call to a shared `vmDecodeString` helper. It leaves
near misses in place and does not evaluate unknown strings in Rust. A helper
call remaining in the output is therefore expected; folding a particular
call needs separate table, cache, and call-site proof.

**RENAME** uses Oxc's scope and reference graph to give proven VM bindings and
typed receiver properties readable names. It checks lexical collisions and
does not infer a role from a minified spelling. An unrelated object's property
that happens to have the same key is not renamed. `with` or direct `eval`
prevents safe binding renames, so the stage records a diagnostic instead of
guessing what dynamic scope refers to.

**FIND** reparses the renamed source and recognizes structural roles such as
entry, dispatch, tables, and thread construction. **EXTRACT** then records
typed facts for static values, including string and number tables, bytecode,
dispatch data, and WASM modules. A module's bytes are not automatically VM
bytecode. Each extraction fact has an `extracted`, `unknown`, or `unsupported`
status and evidence; one successful fact does not certify the rest of a
capture.

**Presentation** makes proven structures easier to read without supplying
missing semantics. `operation_table.rs` turns a proven function array into a
numeric-keyed operation object only after re-identifying its binding and
checking its shape. `wasm_bindings.rs` preserves raw import/export ABI names
while presenting readable JavaScript names through a validated adapter.
`prototype_aliases.rs` removes a read-only alias only when every use is a
proven `Object.defineProperty` receiver, for example:

```js
var p = vmMemory.prototype;
Object.defineProperty(p, "slots", descriptor);
// becomes: Object.defineProperty(vmMemory.prototype, "slots", descriptor);
```

`variable_declarations.rs` splits multi-declarator statements in statement
lists, preserving each initializer and its order. It does not split `for`
headers or single-statement control-flow bodies. These last two rewrites
improve readability; neither proves a new VM role.

## Files

| File | Responsibility |
| --- | --- |
| [`pipeline.rs`](pipeline.rs) | Stage sequencing, `PipelineReport`, entry recognition, extraction, and public API implementation |
| [`naming.rs`](naming.rs) | Role proof, collision-safe naming, and binding/property mappings |
| [`native_cleanup.rs`](native_cleanup.rs) | Proven native-function catalogue and wrapper cleanup |
| [`string_decode.rs`](string_decode.rs) | Repeated base64/XOR decoder recognition |
| [`operation_table.rs`](operation_table.rs) | Presentation of a proven operation array |
| [`wasm_bindings.rs`](wasm_bindings.rs) | Raw WASM ABI names and their JS adapter |
| [`prototype_aliases.rs`](prototype_aliases.rs) | Read-only constructor-prototype alias inlining |
| [`variable_declarations.rs`](variable_declarations.rs) | Statement-list declaration splitting |
| [`mod.rs`](mod.rs) | Module wiring |

## Reading the report

`PipelineReport.output` is the standardized JavaScript. `input_hash` records
FNV-1a-64 over the raw input bytes; it is not a SHA-256 source digest.
`stages` and `diagnostics` record what the current run
found or declined; they are in-memory fields and are omitted from the legacy
JSON projection. Binding and property mappings link original source spans to
the chosen readable names. `binding_residue` lists kernel-owned bindings that
kept their old spelling because a coherent replacement was not proved.

`proven_operation_tables` is more than a display name. It carries declaration
ordinal and initializer counts so later consumers can locate and validate the
same table in generated source. The presentation passes account for removed
declarators when carrying that ordinal forward. `ExtractionFact` preserves
kind, status, source span, and evidence, including unknown or unsupported
outcomes. These fields let a consumer reject an uncertain handoff instead of
silently treating formatted JavaScript as execution authority.

## Handoff and proof limits

A `PipelineReport` is source-bearing and in-memory. Its legacy `shape-pipeline-report/1` JSON is a
debug projection; tracing loads a separately compiled, source-free
`KernelArtifact`. `shape deob --report` writes that kernel artifact, not the
pipeline report JSON. Inspecting `output` or a role name cannot substitute
for a downstream admission check. A pass declines ambiguous bindings,
dynamic scope, unsafe effects, and near-miss syntax rather than guessing.

The [module flow](../README.md) shows the consumers. Use
`tests/deobfuscate_api.rs` and the focused
transform tests when changing a pass. For command options and artifact input
modes, use the [deobfuscation guide](../../../docs/deob/deobfuscation.md).
