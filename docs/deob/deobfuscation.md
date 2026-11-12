# Deobfuscation contract

## Status and scope

The Rust/Oxc deobfuscation pipeline is implemented for supported captures. Its
output and recognizer coverage remain inspectable as kernels are added. The
shared VM semantics and JS/WASM execution variants are documented in
[architecture](../architecture.md); pass correctness and per-capture extraction
coverage are tracked separately from that shared semantic model.
Support is declared per kernel family and evidence set; this workflow does not
promise to handle every arbitrary capture.

Production implementation is Rust with Oxc parsing, semantic analysis, and code
generation. The reference JavaScript transformations are mined for behavior and
evidence, not copied as a production runtime.

Transformation implementations live together under `src/transforms/`. The
source-preparation pipeline and its passes (the pipeline, naming, operation
presentation, native cleanup, string decoding, and WASM ABI passes) sit in
`src/transforms/deob/`; the shared per-stage findings are in
`src/transforms/diagnostics.rs`. Post-emission cleanup is a separate pipeline
in `src/transforms/emitted_js/` (the public path stays `shape::emitted_cleanup`).
Extractors and tracing remain separate top-level modules. The established
root imports for `native_cleanup`, `string_decode`, and `wasm_bindings` remain
available as re-exports.

## Four ordered stages

### 1. Clean

Parse the raw kernel with Oxc, preserve source identity and spans, and remove only
structural noise justified by a deterministic rule. Restricted decoding is typed,
bounded, and safe; unsupported expressions remain unknown rather than reaching
`eval`. The pass ledger records before/after identity and diagnostics.

### 2. Rename

Apply the current human naming contract through stable Oxc bindings and lexical
collision checks. Operation entries remain anonymous; their parameters are
`context`, `operand1`, `operand2`, ... by position, and locals are `value1`,
`value2`, ... by declaration order per handler. Unknown bindings retain distinct
source spellings with an explicit unresolved diagnostic; they are never merged
onto invented placeholders. Original spellings remain authoritative residue when
the role cannot be proven.

### 3. Find

Recognize VM roles, dispatch tables, thread configurations, constants, exception
boundaries, and operation/action structure from structural evidence. Keep static
definitions distinct from execution occurrences. A recognizer can report
`supported`, `abstracted`, `unknown`, `conflict`, or `unsupported`; a plausible
name is not proof of a role.

### 4. Extract

Materialize typed kernel facts: bytecode, dispatch operations/tables, constants
and thread configurations, with an internal mapping/provenance record sufficient
to inspect every supported binding and role before tracer construction. Code
generation emits the readable deobfuscated JavaScript. The mapping may be printed
on request or to an optional console
view; it is not a requirement for a forest of sidecars, manifests, dashboards, or
per-capture reports.

The two immutable tracked kernel references are deliberately simple:

```text
tests/fixtures/kernels/javascript/kernel.raw.js
tests/fixtures/kernels/webassembly/kernel.raw.js
```

The raw files are never replaced. The JavaScript path is the current JavaScript
VM reference; the WebAssembly path is the tracked WebAssembly VM
reference.
The CLI writes generated JavaScript to stdout by default; saving the second file
is explicit, through redirection or `--output`. Diagnostics and pass summaries
are available on stderr with `--verbose`. There are no `SeedBundle` objects or
per-capture storage trees in this contract.

The current entry point is the build-once `shape` executable (`shape deob
<source>`; see [Build once](../../README.md#build-once)). It accepts
`-o`/`--output`, `-v`/`--verbose`, `--report`, and `--force`; `--force` is valid only with
an output path and replaces that destination atomically after the input/output
identity check.
There is no filename-suffix validation or mode inference. Exit status is 0 for
success, including a closed stdout pipe, 1 for I/O or analysis failure, and 2
for usage errors.

## One source, two useful outputs

To retain both the readable source and a source-free artifact from one raw
capture, run:

```sh
./target/debug/shape deob tests/fixtures/kernels/javascript/kernel.raw.js \
  --output /tmp/shape-deob.js --report /tmp/shape-kernel.json
```

The first file is JavaScript for inspection; the second is a
`shape-kernel-artifact/v1` report that a report-only `trace thread` invocation
can load. With `--output`, deobfuscation does not print the generated program
to stdout. Running the same command again refuses to replace an existing
destination unless `--force` is supplied. The report is a compiled tracer
input, not a copy of the deobfuscated source or a trace result. If only the
source is wanted, omit `--report`; if only the artifact is wanted, use
`shape report` instead. See [trace CLI input modes](../tracing/trace-cli.md#input-modes)
for the artifact consumer.

For a quick semantic check of the readable file, look for the numeric-keyed
`vmOperations` table and compare a selected handler through `shape ops show`
on the **raw** fixture. The table key is local to this capture. Remaining
`vmDecodeString` calls do not by themselves mean deobfuscation failed: folding
emitted calls needs the later table/cache/call-site proof described in
[VM string evaluation](vm-string-evaluator.md).

Keep the worker-source concatenation compact. Resolving a worker's entry and
initial slots belongs to extraction; do not expand another copy of `vmEntry`
inside the generated kernel. Recognizers cover both the JavaScript VM and
WebAssembly VM execution variants when interpreter placement is source-proven,
and retain the mapping to their shared semantic roles. A module-presence signal
alone does not select a variant.

## Canonical VM names

These names describe demonstrated roles. They are output vocabulary, not identity;
stable binding IDs and source spans remain authoritative.

| Role | Canonical JavaScript binding | Rust vocabulary |
|---|---|---|
| VM entry | `vmEntry` | `VmEntry` |
| Execution runner | `vmRun` | `run` |
| Thread construction | `vmCreateThread` | `create_thread` |
| Thread capture | `vmCaptureThread` | `capture_thread` |
| Thread invocation build | `vmBuildThreadInvocation` | `build_thread_invocation` |
| Thread invocation | `vmInvokeThread` | `invoke_thread` |
| Thread body | `vmExecuteThread` | `execute_thread` |
| Execution context | `vmContext` | `VmContext` |
| VM memory | `vmMemory` | `VmMemory` |
| Bytecode image | `vmBytecode` | `BytecodeImage` |
| Dispatch mapping | `vmDispatchTable` | `DispatchTable` |
| Operation definitions | `vmOperations` | `DispatchOperation` |
| Thread descriptors | `vmThreadDescriptors` | `ThreadDescriptor` |
| String constants/cache | `vmStringTable`, `vmStringCache` | `StringTable`, `StringCache` |
| Number constants | `vmNumberTable` | `NumberTable` |
| Prototype values | `vmPrototypeTable` | `PrototypeTable` |
| Proven base64 decoder | `vmDecodeBase64` | `decode_base64` |
| Guarded operation boundary | `vmInvokeOperation` | `invoke_guarded` |
| Exception routing | `vmHandleException` | `handle_exception` |

The rename is positional, not semantic guessing. This abbreviated pair comes from
the synthetic fixture in `tests/combined_contract.rs`; the outer `vmEntry`
wrapper is omitted.

```js
// Synthetic raw input shape from tests/combined_contract.rs.
var operations = [function (first, second, third) {
  var firstValue = second;
  var secondValue = firstValue;
  var firstValue = secondValue;
  return first && third && secondValue;
}];
```

```js
// Standardized output: same bindings, role names by position.
var operations = [function(context, operand1, operand2) {
  var value1 = operand1;
  var value2 = value1;
  var value1 = value2;
  return context && operand2 && value2;
}];
```

**Note:** the function itself stays *anonymous*. There is no inferred operation
name to search for; select a handler by its proven table key and source span, and
use the binding mapping when you need the original spelling.

`dispatchRow` and `byteOffset` remain separate coordinates. VM bytecode is not VM
memory. Legacy aliases such as `$KEY_42`, `$THREAD_3_67266`, or `GlobalChecks59`
may be retained for lookup and comparison, but never as the canonical output
contract. Do not turn `ReferenceError` helpers into return-state names, and do not
infer meaning from duplicate parameters or familiar binding labels.

## Mapping and safety requirements

For every supported binding, retain an internal record containing input digest,
stable binding ID, lexical scope, original name/span, canonical name, recognition
evidence/status, reference spans, affected properties, and pass version. Mapping
and provenance are inspectable before tracing but remain internal or optional
console output. They do not become sidecar sprawl.

Structural roles use collision guards and deterministic ordering independent of
worker arrival. Unknown meanings stay unknown. Structural recognition is not a
runtime observation; concrete and symbolic evidence are recorded separately.

WASM import/export identity is derived from the validated module's ABI edges:
namespace/member for imports, and export ordinal plus entity kind/index for
exports. Export aliases therefore get distinct ledger keys while retaining
their shared entity correspondence. Raw names are preserved as ABI data, not
used as a whitelist for semantic roles; if the relation is ambiguous, the
adapter declines instead of guessing.

## Trace hand-off

The dispatch and prefix drivers validate the source-backed facts they require
when preparing a run. An unknown role, ambiguous binding, unsupported table
shape, or incomplete decoder proof remains a diagnostic or stop; presentation
names do not authorize execution. `shape trace thread` can prepare from raw
source or load a compiled artifact, while `trace corpus` and `trace program`
apply their own traversal policies. No manual approval record is required by
these APIs. The specific admission and stop rules are in the
[dispatch](../../src/dispatch/README.md) and
[prefix](../../src/dispatch/prefix/README.md) guides.
