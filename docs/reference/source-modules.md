# Source modules

The downstream analysis tree has four responsibility areas: `kernel`,
`operations`, `dispatch`, and `tracing`. Two consumers read the traced
program they produce: `signals` (the per-seed signal schema) and `naming`
(structural function names). The `transforms`
directory is the pipeline
front end that produces their shared report input. `src/seeds/` is a separate
acquisition front end for saved or fetched inputs; it does not analyze kernel
semantics. Each area with a practical guide is linked below. This page is the
cross-module map and ownership guide.

## Data flow

The diagrams below show the implemented investigation/compilation path and
the shared semantic helpers used by its consumers.

### Investigation and compilation (today)

~~~text
capture (raw JavaScript source)
  │  Oxc passes: CLEAN → RENAME → FIND → EXTRACT
  │  transforms::deob::pipeline
  ▼
PipelineReport (standardized source + extraction facts)
  ├─ kernel facts / layout / operation inspection    kernel, operations::view
  ├─ source-backed complete dispatch preparation     dispatch::prepare_*
  └─ source-proof compilation → source-free
       KernelArtifact / prefix workflow              dispatch::prefix
  ▼
thread / corpus / program tracing                    cli/trace (binary)
  ▼
trace facts + provenance                             tracing::symbolic
  ├─ emit_javascript (Oxc AST → codegen)
  │     └─ emitted-JS Oxc passes                     vm_string, emitted_cleanup
  └─ live debug sink / text rendering                TraceDebugOptions, tracing::text
~~~

Complete dispatch preparation is source-backed today. The prefix workflow has
a source-free path (`KernelArtifact`) for admitted JavaScript and WASM kernels;
compilation still requires source. Do not describe every command
as running solely from a compiled artifact.

The report path owns metadata and standardized source. The source-only path is
for callers that only need the resulting JavaScript. Downstream modules should
consume the report when one is available so they retain extraction facts and
binding identity.

### Shared semantics

~~~text
js_number · operations::templates::admit_recipe        today
        ├─► tracing::symbolic evaluation               today
        └─► operations::executor                       today
~~~

These helpers are plain Rust over values. The symbolic tracer and bounded
executor consume shared numeric semantics and recipe admission. The emitter
builds an Oxc AST and prints it with Oxc codegen; `vm_string` and
`emitted_cleanup` operate after emission.

The post-emission Oxc passes own the `$SSA`, `$CARRY`, helper, tuple, and
property-write cleanup described in
[emitted program cleanup](../transforms/emitted-program-cleanup.md).

For a beginner-oriented walkthrough of this flow, including `HandlerIR`,
recipes, operand descriptors, and bounded dispatch,
see [From capture to executed handler](vm-flow.md).

## Areas and practical guides

| Area | Question | Guide |
|---|---|---|
| `src/transforms/` | How is raw source standardized, then emitted JavaScript cleaned? | [Transform modules](../../src/transforms/README.md) and [deobfuscation](../deob/deobfuscation.md) |
| `src/oxc.rs` and `src/vm_patterns.rs` | Which parsing, binding, codegen, and nested-dispatch helpers are shared? | [Shared Oxc helpers](../../src/oxc.rs) and [source map](../../src/README.md) |
| `src/kernel/` | What static facts, roles, payloads, and boundaries are present? | [`src/kernel/README.md`](../../src/kernel/README.md) |
| `src/operations/` | What handlers and proven recipes are available? | [`src/operations/README.md`](../../src/operations/README.md) |
| `src/dispatch/` | Which proven handlers run from a supplied entry? | [`src/dispatch/README.md`](../../src/dispatch/README.md) |
| `src/tracing/` | How are recipes executed symbolically or observations rendered? | [`src/tracing/README.md`](../../src/tracing/README.md) |
| `src/seeds/` | How are saved/fetched launchers and kernels classified and stored? | [Capture acquisition](../deob/seeds.md) and [capture module guide](../../src/seeds/README.md) |
| `src/signals/` | Which signals does a traced program collect, with which fields, masks and flag bits? | [`src/signals/README.md`](../../src/signals/README.md) and [per-signal pages](../signals/README.md) |
| `src/naming/` | Which readable names do the functions of a traced program get, and from which proofs? | [`src/naming/README.md`](../../src/naming/README.md) and [rewrite engine](../../src/naming/program/README.md) |
| `cli/trace/` | Which trace command policy applies? | [Trace CLI and program assembly](../../cli/trace/README.md) and [trace CLI](../tracing/trace-cli.md) |

For a specific file, use the focused module guides:

| Area | File maps |
| --- | --- |
| Kernel | [Actual properties](../../src/kernel/properties/README.md), [WASM](../../src/kernel/wasm/README.md) |
| Operations | [Templates](../../src/operations/templates/README.md), [operands](../../src/operations/operands/README.md), [Handler IR](../../src/operations/graph/README.md), [inspection views](../../src/operations/view/README.md) |
| Dispatch | [Prefix execution](../../src/dispatch/prefix/execute/README.md) |
| Tracing | [Symbolic session](../../src/tracing/symbolic/session/README.md), [text rendering](../../src/tracing/text/README.md) |
| Trace front ends | [Commands](../../cli/trace/commands/README.md), [corpus internals](../../src/trace/corpus/README.md) |
| Signals | [Signal modules](../../src/signals/README.md), [schema format](../signals/schema.md), [per-signal pages](../signals/README.md) |
| Naming | [Function naming](../../src/naming/README.md), [rewrite engine](../../src/naming/program/README.md) |
| Emitted JavaScript | [Linked-program property writes](../../src/transforms/emitted_js/literals/property_writes/README.md) |

## Find the owner of a change

Start with the first incorrect fact or output, then use that module's focused
tests. A later failure can be the consequence of an earlier missing proof.

| Symptom or intended change | Start here | Check the boundary with |
| --- | --- | --- |
| A source binding or VM field has the wrong readable name | [Source preparation](../../src/transforms/deob/README.md) | `deobfuscate_api`, `native_readable`, and the affected transform suite |
| A literal exists but is absent from `kernel show` | [Properties](../../src/kernel/properties/README.md), then `kernel/bundle.rs` | `actual_properties`, `tracer_bundle` |
| An encoded payload remains unknown | [Decoder proof](../deob/runtime-decoder.md) and `kernel/config.rs` | `runtime_decoder`, `runtime_config` |
| A visible handler is unavailable for execution | [Templates](../../src/operations/templates/README.md) and [operands](../../src/operations/operands/README.md) | `handler_templates`, `recipe_admission`, `js_operands` |
| `ops show` attaches the wrong analysis or span | [Inspection views](../../src/operations/view/README.md) | `ops_inspection` |
| A walk loses a branch, loops, or stops at a merge | [Prefix execution](../../src/dispatch/prefix/execute/README.md) and [session](../../src/tracing/symbolic/session/README.md) | `vm_branches`, `vm_control_flow`, `trace_stop_codes` |
| Emission is valid but a cleanup stage changes behavior | [Emitted JavaScript](../../src/transforms/emitted_js/README.md) | Stage snapshots and the affected `emitted_*` suite |
| JSON trace results or corpus comparisons change | [Commands](../../cli/trace/commands/README.md) and [corpus](../../src/trace/corpus/README.md) | `trace_*` suites |

Names in the last column are Cargo integration-test targets: run
`cargo test --test TARGET` from the repository root.

`src/dispatch/` and `src/tracing/` are separate areas even though dispatch uses
the symbolic tracer for each committed handler. `tracing::symbolic` produces
execution reports and supports opt-in live `TraceDebugOptions`; `tracing::text`
accepts producer-supplied observations and renders them without execution.

## Ownership and canonical paths

The area list above is the taxonomy. This table adds the owning module and the
public path a contributor should call. `lib.rs` keeps aliases (`tracer`,
`operation_catalogue`, `runtime_config`, and so on) that preserve exact type
identity; prefer the canonical path in new code and documentation.

| Area | Owning module | Canonical public path |
|---|---|---|
| `src/transforms/` | `transforms::deob::pipeline` (crate-private) | `deobfuscate`, `deobfuscate_with_report`, `PipelineReport` (crate root) |
| `src/kernel/` | `kernel::{config, layout, properties, bundle, wasm, wasm_dispatch, wasm_lift, decoder_proof, roles}` | `kernel::config::extract`, `kernel::layout::static_kernel_layout_with_options`, `kernel::wasm::analyze_report`; `wasm_lift` is crate-internal |
| `src/operations/` | `operations::{catalogue, templates, operands, graph, view, executor}` | `operations::catalogue::catalogue_from_report`, `operations::templates::handler_templates_from_report`, `operations::operands::js_operand_separation_from_report`, `operations::graph::HandlerIR` |
| `src/dispatch/` | `dispatch::{prepare, execute, thread}` | `dispatch::prepare_dispatch_from_report`, `dispatch::DispatchPreparation::execute` |
| `src/dispatch/prefix/` | `dispatch::prefix` (private, re-exported), including structural `prefix::wasm` preparation | `prepare_prefix_from_report`, `KernelArtifact` (crate root) |
| `src/tracing/` | `tracing::symbolic`, `tracing::text` | `tracing::symbolic::{trace_handler, TraceSession, emit_javascript}`, `tracing::text::Trace` |
| `src/transforms/emitted_js/` | `transforms::emitted_js` (crate-private; public path `emitted_cleanup`) | `emitted_cleanup::cleanup_emitted_javascript`, `emitted_cleanup::simplify_property_writes` |
| `src/vm_string/` | `vm_string::integration` (crate-private) | `fold_emitted_vm_strings_from_report` (crate root) |
| `cli/` | binary-only command line (`shape.rs` declares it; not part of the library). `cli/trace/` holds the trace front ends (snapshots in `inputs.rs`, routing and JSON output in `commands/mod.rs`) | run the `shape` binary |
| `src/seeds/` | `seeds::{collect, script, store}` | `seeds::collect` |
| `src/signals/` | `signals::{extract, analysis, catalog, flags, schema, docs, codec, payload}`; one file per signal under `catalog/` holds its recognizer, binder and card | `signals::analysis::analyze_program_signals`, `signals::extract::extract_program_properties`, `signals::catalog::{card, cards, aliases}` |
| `src/naming/` | `naming::{functions, classes, stages, collectors, semantic_names, function_index, role_graph, program}` | `naming::name_threads`, `naming::function_index::extract_function_index`, `naming::program::transform_program` |
| root helpers | `oxc` (re-exported as `analysis`), `source`, `profile`, `vm_patterns` | `analysis::parse`, `analysis::codegen`, `source::read_source`, `source::sha256_hex` |

Not every module is public. `transforms` and its `deob` and `emitted_js`
pipelines, `js_number`, `util`, `operations::table_scan`, `kernel::wasm_module`,
are crate-internal; their behavior is reached through the public re-exports
above. The command line is the binary-only `cli/` folder. The root `oxc` module is re-exported as
`analysis`, and `transforms::emitted_js` is re-exported as `emitted_cleanup`.

## Facts, execution, inspection and code generation

Keep these concerns separate. A module that produces one does not imply
another, and a rendered report is never execution authority.

| Concern | Produced by | Not to be confused with |
|---|---|---|
| Source-proved facts | `transforms::deob::pipeline` (standardized source, extraction facts), `kernel` (config, layout, properties, WASM boundary, decoder proof), `operations` (catalogue, recipes, operand descriptors, `HandlerIR`) | A run. A catalogued, classified or admitted handler is not an executed one. |
| Execution | `tracing::symbolic` (`TraceSession`, `trace_handler`), `dispatch` (complete and prefix drivers), `operations::executor` (bounded concrete recipe executor) | Execution of captured JavaScript/WASM. Nothing in this tree runs the input. |
| Inspection | `operations::view` (`ops list`, `ops show`), `kernel::bundle` (`tracer_json`, `tracer_markdown`), `kernel::layout` renderers, `tracing::text` | The evidence itself. Rendering projects facts; it does not prove them. |
| Code generation | `analysis::codegen`, `tracing::symbolic::emit_javascript`, `emitted_cleanup`, `vm_string` | Emitted JavaScript is an output, not an input to tracing. |

## Shared rules

- Use Oxc SymbolIds, exact AST structure, and operation-table declaration
  identity for proof decisions. Names are display aliases.
- Preserve unknown, partial, unsupported, and ambiguous findings with source
  evidence. Do not infer execution from a pretty-printed report.
- Keep limits explicit for analysis, decoding, execution, and rendering.
- Parse and validate embedded WASM; do not execute it. An admitted interpreter
  can be lifted once into the shared prefix walker. See the
  [prefix guide](../tracing/wasm-prefix.md#wasm-dispatched-kernels-prefixwasmrs)
  for its proof and limits.
- Treat versioned JSON and markdown outputs as contracts. The owning module is
  the source of truth for schema fields and serializers.

## Shared primitives and source pointers

Keep these reusable decisions in their existing implementation homes:

| Concern | Source of truth |
|---|---|
| Operation-table scanning | `operations::table_scan` |
| Recipe admission (may this recipe run?) | `operations::templates::admit_recipe` |
| JavaScript numeric conversions | `js_number` (`parse_numeric_literal`, `number_to_js_string`) |
| AST identity and parsing | `oxc` (re-exported as `analysis`) (`identifier_symbol`, `parse_program`) |
| Nested-dispatch recognition | `vm_patterns` (`recognize_nested_dispatch`, `recognize_nested_dispatch_member`) |
| Stage status and findings | `transforms::diagnostics` (`StageStatus`, `Diagnostic`) |
| FNV hashing and JSON escaping | `util::{fnv1a64_hex, json_string}` |
| WASM decode/parse/validate | `kernel::wasm_module` |
| WASM interpreter lift and prefix preparation | `kernel::wasm_lift`, `dispatch::prefix::wasm` |
| Dispatch step mechanics | `dispatch::step` |
| Trace stop codes | `tracing::symbolic::TraceStopCode` |

## Contract ownership

| Contract | Owner |
|---|---|
| `shape-kernel-operation-catalogue/v1` | `operations::catalogue` |
| `shape-kernel-handler-templates/v1` | `operations::templates` |
| `shape-js-operand-separation/v1` | `operations::operands` |
| `shape-handler-graph/v1` | `operations::graph` |
| `shape-runtime-config/v1` | `kernel::config` |
| `shape-kernel-layout` (unversioned locator) | `kernel::layout` |
| `shape-wasm-js-boundary/v1` | `kernel::wasm` |

Cross-module usage and CLI behavior belong in docs/; practical implementation
details and public item examples belong in the module READMEs.

## Glossary

| Term | Meaning here |
|---|---|
| Handler key / table key | The capture-local numeric selector a dispatch cell stores (`DispatchCell::selector`); it selects a handler in that capture's table and is serialized as `OperationRecord::table_key`. Not a semantic identity; not comparable across captures. |
| Handler index | The handler's own numeric index (`OperationRecord::handler_index`, `HandlerTemplate::handler_index`). The extractor currently gives it the same value as `table_key`, and dispatch joins a cell selector to `HandlerTemplate::handler_index`; the fields stay separate because sparse table keys and handler indices are distinct. |
| Instruction occurrence | One concrete dispatch step: `DispatchExecutionRecord` (`handler_key`) or `PrefixExecutionRecord` (`selector`), carrying PC, operand-entry PC, state, opcode, operands and actual next PC. An `operations::graph::Occurrence` is a node inside one handler graph, not a dispatch step. The `ops show` occurrence projection is a library API, not a CLI output. |
| HandlerIR / recipe | `HandlerIR` (alias of `HandlerGraph`) is a per-handler value/effect graph recovered from standardized source. A recipe (`HandlerTemplate`) is its ordered projection, run only after `admit_recipe`. Both are static facts: no PC, image or run. |
| Operand-entry coordinate | Where operand decoding starts in the image: `OperandEntryCoordinate::AfterOpcode` (offset zero is the first operand byte, because the dispatch method consumed the opcode) or `AtOpcode` (offset zero is the opcode byte, because the binding handoff says the handler starts there). `operand_entry_pc` is that PC; `OperandSpan { start, end }` is the byte range. |
| Slot | A numbered VM storage position (`TraceCell::Memory`, `TraceCell::Stack`, a `ThreadDescriptor` slot list, a `SlotAssignment`). A coordinate, not a value. |
| Shared cell | A memory slot's boxed value shared between a VM function and the closures it creates (`open_cells`; emitted as `$CELL_<slot>`). Corpus `converge_cells` converges what each cell can hold. Two slots may alias one cell. |
| Raw vs standardized source | Raw source is the exact input bytes read once by `cli`; it is the basis of the SHA-256 capture identity. Standardized source is `PipelineReport::output` after CLEAN → RENAME → FIND → EXTRACT; handler and operand spans are offsets into it, never into raw source. |
| Hash | Always give algorithm and byte basis. FNV-1a-64 (`util::fnv1a64_hex`) over exact raw input bytes is `PipelineReport::input_hash`; it also fingerprints normalized handler bodies and decoded images. SHA-256 is used over raw capture bytes (`cli::sha256_hex`), standardized source bytes (`operations::graph`), the source a `VmStringProof` is proved over, and encoded `KernelArtifact` bytes. |
| `PipelineReport` | Standardized source plus extraction facts and source-span maps from one pipeline run. In memory; its legacy JSON is a debug projection, not a runtime format. |
| `KernelArtifact` | A versioned, source-free, SHA-256-integrity-checked encoding of a prepared prefix program, compiled while source proofs are in hand. It prepares tracing; it is not a codec. |

"IR" is an umbrella word, not a shared representation. These representations
are not interchangeable; any conversion between them is a named lowering step,
not an identity:

| Representation | What it is | Status |
|---|---|---|
| Handler IR | `operations::graph::HandlerIR`, a per-handler value/effect graph | implemented |
| Trace IR | Trace facts (`TraceReport`, `TraceArena`, `TraceProvenance`): recorded execution | implemented; not a lowering IR |
| Oxc AST | `oxc_ast` programs used by `analysis` and `emit_javascript` | implemented |

The beginner glossary in [From capture to executed handler](vm-flow.md#glossary)
covers the same terms in narrative form.

## Common entry points

Build the release `shape` executable once as described in
[the root README](../../README.md#build-once). These commands
use the tracked JavaScript VM capture directly:

~~~text
shape ops show tests/fixtures/kernels/javascript/kernel.raw.js --op 0 --format markdown
shape trace corpus tests/fixtures/kernels/javascript/kernel.raw.js --fast
~~~

See the [trace CLI](../tracing/trace-cli.md) for other source-level commands. The
[SDK guide](../../sdk/rust/README.md) describes public integrations.

## Detailed implementation references

The module READMEs keep ownership, entry points, and focused checks. For deeper
contracts, use [kernel analysis](../deob/kernel-analysis.md), [dispatch](../tracing/dispatch.md),
[prefix preparation](../tracing/prefix-preparation.md), [prefix execution](../tracing/prefix-execution.md),
[prefix diagnostics](../tracing/prefix-diagnostics.md), [WASM preparation](../tracing/wasm-prefix.md),
[kernel artifacts](../deob/kernel-artifacts.md), and [trace orchestration](../tracing/trace-orchestration.md).
String evaluation and integration are documented in the
[VM string reference](../deob/vm-string-evaluator.md).

Handler graphs have a dedicated [IR reference](../operations/handler-ir.md); abstract
cross-capture grouping is documented in [operation classification](../operations/operation-classification.md).
For output refinement, use [program cleanup](../transforms/emitted-program-cleanup.md),
[helper and tuple cleanup](../transforms/emitted-helper-cleanup.md), and
[thread cleanup](../transforms/thread-cleanup.md).
