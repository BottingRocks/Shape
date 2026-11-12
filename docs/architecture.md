# Architecture

Shape turns captured JavaScript into inspectable VM facts and, where the source
supports it, traced JavaScript. The architecture separates source preparation,
static proof, bounded execution, and code emission. Each stage carries the
evidence needed by the next; readable source or a successful inspection alone
does not authorize execution.

This guide explains those handoffs and the responsibilities behind the CLI and
Rust API. Use [source modules](reference/source-modules.md) to locate implementations,
[deobfuscation](deob/deobfuscation.md) for source preparation, and the
[trace CLI guide](tracing/trace-cli.md) for runnable workflows.

## Current shape

The root crate exposes the `shape` executable and a Rust library. Its default
build uses Rust and Oxc. Acquisition is a separate input step:
the acquisition commands save capture bytes without executing their JavaScript.
Analysis and tracing then operate on supplied source or a compiled artifact.

```text
captured JavaScript
       │
       ▼
source preparation ───────────────► standardized JavaScript
       │
       ▼
PipelineReport
       ├──► kernel facts and properties ──► inspection output
       ├──► operation catalogue, recipes, operand proofs
       │                  │
       ├──────────────────┴──► dispatch preparation
       │                            ├──► complete-dispatch invocation
       │                            └──► PrefixPreparation
       │                                      ▲
       └──► kernel artifact compilation       │
                     │                        │
                     ▼                        │
               KernelArtifact ── load/validate/prepare
                                              │
                               thread / corpus / program drivers
                                              │
                                              ▼
                              symbolic sessions and trace reports
                                              │
                                    validated Oxc AST emission
                                              │
                            function output or linked program cleanup
```

There are two useful reuse points. A `PipelineReport` shares source analysis
among static consumers. A `KernelArtifact` carries an admitted prefix
preparation across process boundaries so later walks can run without parsing
the original JavaScript. They solve different problems and are not
interchangeable reports.

The command surface follows these boundaries. `shape deob` produces readable
source; `shape kernel show` and `shape ops` inspect facts and handlers;
`shape report` compiles an execution artifact; `shape trace` selects a tracing
workflow. In particular, `shape deob --report` also writes a kernel artifact,
not a serialized `PipelineReport`.

## Implemented boundaries

The main ownership boundaries are stable even when a large module is split
into smaller files:

| Area | Owner | Responsibility |
| --- | --- | --- |
| Source preparation | [`transforms/deob`](../src/transforms/deob/README.md) | Normalize source, establish binding roles, extract facts, and retain diagnostics |
| Shared syntax services | [`oxc`](../src/oxc.rs) | Parsing, semantic identity, source edits, and JavaScript generation |
| Static kernel analysis | [`kernel`](../src/kernel/README.md) | Properties, layout, runtime facts, decoder proofs, and JavaScript/WebAssembly relationships |
| Handler semantics | [`operations`](../src/operations/README.md) | Inventory, recipe extraction and admission, operand separation, and bounded concrete execution |
| Dispatch | [`dispatch`](../src/dispatch/README.md) | Prove interpreter structure, bind invocations, decode instructions, and drive bounded walks |
| Symbolic execution | [`tracing/symbolic`](../src/tracing/symbolic/README.md) | Track values and effects, execute recipes transactionally, and emit validated JavaScript |
| Command orchestration | [`cli/trace`](../cli/trace/README.md) (binary only) | Native commands, JSON output, reachability, shared-cell analysis, comparisons, and program assembly |
| Output refinement | [`vm_string`](../src/vm_string/README.md) and [`transforms/emitted_js`](../src/transforms/emitted_js/README.md) | Fold proven string computations and simplify emitted code |
| Signal schema | [`signals`](../src/signals/README.md) | Read the signals of a traced program: slot extraction, one recognizer/binder/card per signal, flag bits, wire codec and payload |
| Function naming | [`naming`](../src/naming/README.md) | Name the functions and bindings of a traced program from structural proofs and rewrite the program |

These are analysis and transformation components, not a general JavaScript
runtime. Host-facing calls can be represented symbolically without executing
them. A handler's presence in an operation table says nothing by itself about
whether its behavior is admitted for tracing.

## Source and report responsibilities

Source preparation runs CLEAN, RENAME, FIND, and EXTRACT, followed by
presentation passes. CLEAN removes or simplifies proven boilerplate. RENAME
uses semantic binding and receiver identity to assign readable names. FIND
recognizes VM structure, while EXTRACT records typed facts and their evidence.
Presentation makes the proven tables and declarations easier to read.

`deobfuscate` returns standardized source. `deobfuscate_with_report` also returns
mappings, extraction results, operation-table identity, and diagnostics in a
`PipelineReport`; its `output` field is the standardized source used by
subsequent analyses. Consumers should retain that report instead of treating
printed names as a replacement for its evidence.

**Note:** `deobfuscate` returns the standardized `String` only. Report-backed
proofs in this tree take a `&PipelineReport`, for example
`runtime_config_from_report`, `handler_templates_from_report`,
`js_operand_separation_from_report`, `classify_report`,
`analyze_wasm_boundary_report`, and `KernelArtifact::compile_from_raw`. A caller
that will analyze the capture should call `deobfuscate_with_report` *once* and
retain that value; calling `deobfuscate` and later rebuilding a report repeats
the pipeline, and every later proof then has to use the second report.

Reparsing creates a new syntax tree. Oxc symbol identities belong to one tree,
so later passes re-establish the relevant binding using retained declaration
positions, initializer structure, and source evidence. Likewise, raw-source
spans and standardized-source spans refer to different byte sequences. A
source span without its source basis is insufficient to identify a handler.

### Data contracts

Several outputs describe the same capture from different perspectives:

| Representation | What it carries | What it does not establish |
| --- | --- | --- |
| `PipelineReport` | Standardized source and the pipeline's structural evidence | That every handler or dispatch path can execute |
| `RuntimeConfig` | Static runtime facts with extraction status, provenance, and limits | A prepared execution image |
| Properties bundle | The concise values shown by `kernel show` | Recipe admission or a loadable tracing artifact |
| Handler templates and `HandlerIR` | Ordered semantics and per-handler value/effect relationships | The behavior of every instruction occurrence |
| `KernelArtifact` | Compiled prefix data, recipes, operands, descriptors, and identity metadata | Authenticity of the original capture or completion of a future walk |
| `TraceReport` / `PrefixTraceReport` | Observed symbolic work, completion or stop state, and diagnostics | Coverage of unrelated entries or all possible environments |

Static facts retain unknown and unsupported outcomes. Partial extraction can
still be useful for inspection without becoming execution authority. The
[runtime configuration guide](deob/runtime-config.md) describes that distinction;
[ops inspection](operations/ops-inspection.md) explains the handler-facing projections.

## From handler source to executable recipes

Operations analysis keeps inventory, recipe extraction, operand lifting, and
admission separate. The catalogue describes a proven table's entries. Template
extraction translates recognized behavior into ordered recipes. Operand
analysis proves inline byte reads and their effects on the program counter.
`admit_recipe` decides whether a consumer can use the resulting recipe.

Those answers need not match. An inventoried handler may have a partial
unlifted template but become executable after operand analysis proves its
inline reads. Another may remain visible in inspection while execution rejects
its unsupported effects. Combining these results into a single support flag
would hide the reason a handler can or cannot run.

`HandlerIR` describes relationships within a handler. The dispatch driver adds
the instruction occurrence: image position, dispatch state, selected handler,
and decoded operands. A sparse operation-table key is also distinct from a
handler's row index. Keeping these coordinates separate prevents a display
order or a repeated handler body from becoming an accidental execution rule.

The concrete executor and symbolic executor consume admitted semantics for
different purposes. Concrete execution works on supplied values within bounds;
symbolic execution preserves unknown inputs and observable effects in a trace.
Neither gains permission to run arbitrary source merely because extraction
recognized part of a handler.

## Execution variants and terminology

A **JavaScript VM** places its interpreter's dispatch, control flow, and operand
decoding in JavaScript. A **WebAssembly VM** places those responsibilities in a
proven WebAssembly interpreter. A WebAssembly VM can still call JavaScript
handlers or helpers; both variants share the semantic tracing machinery.

A module's presence is insufficient to establish interpreter placement. Static
WebAssembly inspection records module, import, export, and boundary facts. The
separate interpreter lift must prove the module/table/helper/wrapper
relationships needed to prepare execution. Its implementation lives under
[`kernel/wasm`](../src/kernel/wasm/README.md), with the dispatch adapter under
[`dispatch/prefix`](../src/dispatch/prefix/README.md).

For admitted captures, JavaScript proofs or the WebAssembly lift feed the same
`PrefixPreparation` and prefix walker. This is a bounded lifted execution path,
not an embedded WebAssembly runtime. Admission depends on source structure;
a familiar capture name, matching handler text, or module bytes alone cannot
substitute for those relationships.

### Complete dispatch and prefix execution

The complete-dispatch API requires a proven image, an admitted handler table,
and an explicit `CompletionContract`, such as a terminal selector and the
location of its result. Its completion claim is relative to that contract and
invocation. It does not establish whole-capture coverage.

The prefix engine supports a different contract, including a proven run loop,
completion register, and supported image-patching behavior. It advances through
admitted instructions and stops at a boundary it cannot justify. Despite its
name, a prefix walk can finish: a `Completed` result can proceed to emission.
An unsupported instruction or exhausted budget leaves an inspectable stopped
prefix instead.

Each invocation starts with its own context and image copy. A committed image
patch changes subsequent decoding in that invocation, while another invocation
starts from the preparation's original image. Dispatch records distinguish
patches from recipe executions, so instruction counts and handler counts need
not match. See the [prefix execution contract](tracing/javascript-vm-prefix.md).

## Compiled artifacts and reuse

Artifact compilation performs the source analysis and preparation proofs once.
The artifact stores the bytecode, dispatch cells, admitted handler data, operand
descriptors, startup information, and any supported WebAssembly machine or
string-folding plan needed by subsequent tracing.

Loading follows a different path: bounded JSON and compressed-payload decoding,
format and compiler compatibility checks, integrity verification, structural
validation, and reconstruction of runtime indexes. It does not reparse
JavaScript or rerun source recognition. Both paths produce the same preparation
representation, keeping execution independent of how that preparation arrived.

Identity metadata distinguishes compilation that observed raw source from
compilation based on a declared identity or standardized source. A matching
integrity digest detects artifact inconsistency; it does not authenticate who
created the capture. When a command accepts both raw source and a cached
artifact, the source identity must also agree.

Prepared data can be reused across many entries, but invocation state cannot
be shared implicitly. Changing the entry, arguments, captures, limits, or
available shared-cell facts may change the trace. An artifact cache avoids
repeating compilation; it does not make a previous execution result valid for
a different invocation.

## Invocation and symbolic sessions

Thread startup combines four distinct inputs: a dispatch entry, a
`ThreadDescriptor`, captured values, and invocation arguments. The descriptor
assigns slot roles; the shared startup builder applies initialization and
capture/invocation ordering. Caller-supplied descriptors remain caller input
even when dispatch preparation succeeds. A VM thread here is an invocation
model, not an operating-system thread.

A `TraceSession` owns the symbolic context, value and statement arenas,
diagnostics, and execution progress. It records supported computation while
preserving unknown values as expressions. Stable references and ordered effects
allow later emission to retain storage behavior and evaluation order rather
than reconstructing them from debug strings.

Handler execution is transactional. The session checkpoints before attempting
a recipe; unsupported behavior or a resource stop rolls back that attempt's
state, nodes, and statements. Earlier committed work remains inspectable.
Speculative branches also use checkpoints, with the driver controlling which
paths to walk, join, or revisit.

The session provides branch merges, loop and exception machinery, and facts
used during retracing. The dispatch driver additionally owns VM position,
image state, and successor selection. A valid symbolic merge alone cannot
justify joining incompatible machine states. Ordinary handler returns and VM
function returns also differ: continuing sessions can consume the former
without ending the enclosing walk.

Limits exist at several layers: artifact decoding, preparation, dispatch steps,
recipe execution, symbolic storage, retracing, and emission. Increasing one
limit does not remove another layer's boundary. The [symbolic execution
reference](tracing/symbolic-execution.md) explains values and emission; the
[session reference](tracing/symbolic-session.md) covers transactions and control flow.

## Tracing workflows and program assembly

The command drivers share preparation and walking, but choose different scopes
and policies:

| Workflow | Scope and result |
| --- | --- |
| `trace thread` | One supplied entry and invocation; inspect its result and emit when eligible |
| `trace corpus` | Discover reachable functions, report coverage and stops, and optionally compare a baseline |
| `trace program` | Trace reachable functions from one capture and assemble a linked JavaScript program |

Every trace command supports `--json` for structured results using its normal flags.

Reachability follows functions discovered during tracing, including VM closure
creation. Function identity includes the relevant descriptor and VM entry
coordinates. The orchestration layer schedules these walks and retains results;
the symbolic session does not independently scan the capture for every possible
function.

Shared cells complicate this process because one function can write a value
another function uses as control-flow information. Full corpus runs gather
holder information and converge cell contents within bounds. Limited runs have
a different information policy; their results should be compared with matching
options. Baseline comparison distinguishes regressions from newly discovered
behavior rather than treating every changed fingerprint as failure.

Program tracing also discovers return shapes and can revisit dependent walks.
When functions remain stopped, it attempts shared-cell convergence and another
program walk. The revised result is adopted only when it reduces the number of
stops and still forms a valid program. This policy belongs to the program
driver, not every invocation of the shared traversal helper.

Program assembly nests functions according to their creation relationships so
shared slots have the intended lexical ownership. A function that could not be
traced becomes a throwing stub carrying its stop reason; the command returns
status `3` when stopped functions remain. A written JavaScript file therefore
does not by itself mean every reachable function completed. See
[corpus comparison](tracing/corpus-comparison.md) and the
[trace JSON format](tracing/trace-json.md) for their separate contracts.

## Emission and output cleanup

Emission consumes structured traces and builds JavaScript through Oxc. It first
checks completion eligibility, arena references, storage ownership, expression
structure, and permitted external names. Completed, returned, or thrown traces
are candidates; partial and budget-stopped traces are refused. Passing the
completion check does not bypass the remaining validation.

Standalone function emission exposes its required inputs through the selected
emission options. Program-mode emission additionally supplies facts used to
link created functions and shared cells. The program builder checks names and
scope while assembling the function tree, before whole-program cleanup.

Output cleanup is separate from source preparation. It simplifies emitted
initializers, temporary bindings, branch joins, property writes, and related
patterns using their supported proofs. String folding uses a proven decoder
and tables through `vm_string`; unknown computations remain represented instead
of being evaluated by a general JavaScript engine.

`trace program --no-cleanup` skips the linked-program cleanup passes, and
`--dump-stages` makes intermediate output available for diagnosis. These options
help distinguish tracing, linking, and cleanup failures. The
[emitted JavaScript cleanup guide](transforms/emitted-js-cleanup.md) owns the pass details.

## Signals and naming over a traced program

Two consumers read the linked `program.js` that `trace program` emits; neither
executes it.

`shape signals <program.js> --json` (`signals::analysis`) builds one slot view
per registered signal slot from the extracted contracts (`signals::extract`),
runs every recognizer in `signals::catalog` over it, binds the winning
signal's fields to the names on its card and renders the per-seed schema
([schema format](signals/schema.md)). Each signal is one catalog file holding
its recognizer, binder and card; the [per-signal pages](signals/README.md) are
rendered from those cards. Recognition reads structure only — salts, slot
addresses, descriptor indices and generated spellings are never match rules,
and a structural test enforces that over every catalog and flag source.

`shape naming …` (`naming`) proposes readable names for functions and bindings
from whole-body structural proofs (thread rules in a frozen stage order,
class proofs, collector positions, the semantic naming plan) and applies them
with the fail-closed rewrite engine in `naming::program`.

## Diagnostics and observation text

Trace reports retain completion, diagnostics, provenance, and committed work.
Dispatch reports add instruction and machine-level evidence. Opt-in
`TraceDebugOptions` output helps inspect execution as it happens, but is not the
serialized trace model or an input to emission.

The separate `tracing::text` module validates and renders supplied observations.
Its typed observation model is not currently connected to a live producer; the
renderer neither executes handlers nor infers missing events. Keep that API
separate from debug output when adding instrumentation. Its contract is in the
[trace text renderer guide](tracing/trace-text-renderer.md).

## Evidence and failure policy

Preparation rejection and execution stopping have different meanings. Invalid
artifacts, ambiguous structural proofs, or inconsistent startup inputs reject
the request before a usable walk. Unsupported behavior encountered later
returns a stop with committed evidence. An actual VM throw is represented
program behavior, not automatically an analysis failure.

Coverage claims stay local to their evidence: one extracted fact, one admitted
recipe, one completed invocation, or one reachable program under its supplied
inputs. No stage silently upgrades those claims to unrestricted execution.
Tests exercise these boundaries through near-miss source rejection, artifact
validation, rollback, dispatch behavior, emission checks, and command-level
results. When extending support, establish the missing proof and its refusal
cases in the owning layer before relying on it downstream.
