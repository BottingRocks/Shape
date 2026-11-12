# Documentation index

This index covers the current tools, contracts, and source references. The root
[README](../README.md) is the short project entrypoint. Start with a workflow
guide for commands, a contract page for supported behavior, or a source-module
README for module ownership and entry points. Keep one canonical home for each contract.

## Documentation map

Each area has a short folder index that lists every page in that area.

| Area | Index | Scope |
| --- | --- | --- |
| Capture facts and deobfuscation | [docs/deob/](deob/README.md) | Capture acquisition, kernel layout and facts, artifacts, runtime configuration and decoder proofs, source deobfuscation, and VM string evaluation. |
| Handler operations | [docs/operations/](operations/README.md) | Operation catalogue, table and classification, handler IR and templates, bounded execution, and operand separation. |
| Tracing | [docs/tracing/](tracing/README.md) | Trace CLI and JSON output, dispatch, prefixes, symbolic tracing and sessions, rendering, corpus comparison, and branch triage. |
| Emitted JavaScript transforms | [docs/transforms/](transforms/README.md) | Thread and linked-program cleanup, helper and property rewriting, and loop simplification. |
| Reference | [docs/reference/](reference/README.md) | Source-module map, end-to-end flow, operation taxonomy, and WASM boundary evidence. |
| Signals | [docs/signals/](signals/README.md) | What each of the 93 signals collects and why, its fields, an annotated JavaScript sample, and the `shape signals` schema format (pages rendered from the cards in `src/signals/catalog/`), with the [positional signal registry](signals/positional-signals.md) and [signal contracts](signals/contracts.md) guides. |
| Naming and program inputs | [docs/naming/](naming/README.md) | Structural generator-input extraction, per-signal wires, role renames, the `shape signals` / `shape naming` inspection commands, and the durable naming corpus baseline. |
| Generator | [docs/generator/](generator/README.md) | Pinned seed layouts, report and payload framing, session headers, and generic typed serialization. |

[Architecture](architecture.md) describes the current module boundaries and
execution variants.

If you have a specific question, start here:

| Question | Read |
| --- | --- |
| What does one capture contain? | [Kernel show](deob/kernel-layout.md) for compact data; [runtime configuration](deob/runtime-config.md) for source-linked library facts. |
| Where do the seed, session and headers come from? | [Inputs](../README.md#inputs) for the four files and the `x-` header prefix; [capture acquisition](deob/seeds.md#what-a-capture-produces) for `shape seeds fetch/parse/import`. |
| What is this handler and what are its operands? | [Ops inspection](operations/ops-inspection.md), then [templates](operations/handler-templates.md) and [operand separation](operations/js-operand-separation.md) for the independent proofs. |
| Why did tracing stop or differ between runs? | [Trace CLI](tracing/trace-cli.md), [corpus comparison](tracing/corpus-comparison.md), and [fake-branch triage](tracing/fake-branch-triage.md). |
| Where does emitted JavaScript get smaller? | [Thread cleanup](transforms/thread-cleanup.md), [program cleanup](transforms/emitted-program-cleanup.md), [helper and tuple cleanup](transforms/emitted-helper-cleanup.md), and [property writes](transforms/emitted-property-writes.md). |
| Which code owns an execution step? | [VM flow](reference/vm-flow.md), then the [dispatch](../src/dispatch/README.md) or [prefix](../src/dispatch/prefix/README.md) module guide. |
| How does a driver manage symbolic state across paths? | [Symbolic sessions](tracing/symbolic-session.md) for transactions, rollback, branch merges, loops, and shared cells. |
| Why do fake branches need stack holes, caches, and retraces? | [Design rationale](tracing/fake-branch-triage.md#why-the-extra-state-exists), [stack alignment](tracing/symbolic-session.md#why-joins-have-stack-holes), and [cache lifetimes](tracing/symbolic-session.md#why-facts-and-caches-have-different-lifetimes). |
| Why does a WASM import have a name but no handler key? | [WASM boundary](reference/wasm-boundary.md#two-imports-that-look-the-same-but-prove-different-things), then [VM flow](reference/vm-flow.md#where-javascript-and-webassembly-vm-paths-diverge). |
| Which JSON can I pass to tracing? | [Trace input modes](tracing/trace-cli.md#input-modes); [runtime configuration](deob/runtime-config.md) distinguishes the bundle, facts report, and compiled artifact. |
| Where do I change a module and find its focused checks? | [Source-module map](reference/source-modules.md#areas-and-practical-guides), then that folder's README. |

Each example names the fixture or test that establishes its behavior. Handler
keys, offsets, hashes, and spans belong to that input and build; use the
current report before carrying a coordinate into another capture. A reported
`unknown` or `partial` result identifies a missing proof, not a value to fill
from a similar capture.

## Current usage and contracts

- [Architecture](architecture.md) — current module boundaries and execution
  variants.
- [From capture to executed handler](reference/vm-flow.md) — conceptual walkthrough of
  extraction, `HandlerIR`, templates, operands, dispatch, and tracing; start
  here if you are new to the analysis flow.
- [Deobfuscation workflow](deob/deobfuscation.md) — ordered passes, naming, evidence,
  and supported input boundaries.
- [Capture acquisition](deob/seeds.md) — the in-tree `shape seeds fetch`, `parse`,
  `import`, and `sites` commands, and the ignored capture directory.
- [Tracked VM kernel references](../tests/fixtures/kernels/README.md) — the
  immutable JavaScript VM and WebAssembly VM source snapshots and manifest.
- [Static kernel layout](deob/kernel-layout.md) — source-indexed layout facts.
- [Runtime configuration](deob/runtime-config.md) — extracted, source-linked values.
- [Operation catalogue](operations/operations.md) — handler inventory and versioned schema.
- [Operation table](operations/operation-table.md) — source-linked operation-table identity
  and handler entries.
- [Handler templates](operations/handler-templates.md) — recipes and admission status.
- [Bounded handler executor](operations/handler-executor.md) — one-handler execution.
- [Symbolic session driver guide](tracing/symbolic-session.md) — session lifecycle,
  checkpoints, control-flow operations, shared cells, and driver troubleshooting.
- [JavaScript operand separation](operations/js-operand-separation.md) — inline operand
  reads and program-counter effects.
- [Ops inspection](operations/ops-inspection.md) — the `ops show` join, four separate
  statuses, additive JSON contract, display-only annotation, and the
  occurrence/decode library projection.
- [JavaScript VM bytecode prefix](tracing/javascript-vm-prefix.md) — the bounded
  proof-supported prefix contract.
- [WASM/JS boundary inventory](reference/wasm-boundary.md) — parsed module relationships.
- [Fake-branch triage](tracing/fake-branch-triage.md) — how to inspect a slow, stopped,
  or divergent walk against branch evidence across captures.
- [WASM-dispatched prefix preparation](tracing/wasm-prefix.md#wasm-dispatched-kernels-prefixwasmrs)
  — structural admission, lifted interpreter operations, machine limits,
  and source-free artifact behavior.
- [Decoder proof](deob/runtime-decoder.md) — standard and URL-safe runtime decoder
  proofs, with the string evaluator's narrower standard-only boundary.
- [SDK integration](../sdk/rust/README.md) — public library and binding workflows.

## Live tracing and rendered artifacts

- [Symbolic tracer](tracing/symbolic-tracer.md) — current bounded dispatch/thread APIs,
  initialization order, completion, and live debug options.
- [Symbolic execution and emission](tracing/symbolic-execution.md) — value and graph
  models, recipe evaluation, emitter validation, and linked-program contracts.
- [JSON trace output](tracing/trace-json.md) — native `--json` results and explicit input snapshots.
- [Trace CLI](tracing/trace-cli.md) — the `trace` front ends (`thread`, `dispatch`, `corpus`, and `program`) plus
  `cleanup-thread`: input modes, shared-cell policy, streams, exit codes, and
  diagnostics.
- [VM string evaluator](deob/vm-string-evaluator.md) — the proof-backed, non-executing
  `vmDecodeString` evaluator and its exact admitted domain.
- [Emitted JavaScript cleanup](transforms/emitted-js-cleanup.md) — the opt-in, non-executing
  pass that removes proven-dead pure declarations and regenerates provenance
  maps.
- [Remaining-loop audit](transforms/remaining-loop-audit.md) — reproducible procedure for
  finding and checking labeled infinite loops that structured-loop recovery did
  not rewrite.
- [Emitted property-write cleanup](transforms/emitted-property-writes.md) — the
  independent, fail-closed `Object.defineProperty` -> assignment pass, its
  prepopulation proof, and CLI integration contract.
- [Generated-binding renamer plan](emitted-renamer.md) — the not-yet-implemented
  final naming pass that gives linked temporaries per-function names, how it
  stays capture-safe, and where it runs.
- [Trace renderer](tracing/trace-text-renderer.md) — typed observations, producer
  contracts, validation, readable output, line maps, and integration guidance.

## Corpus comparison

- [Corpus comparison](tracing/corpus-comparison.md) — a direct first run, an optional
  baseline workflow, mode differences, and how to read changes.
- [Program naming corpus](naming/corpus.md) — the durable replacement for target-list
  reports: trace -> plan -> rename -> annotate per seed, a reviewed versioned
  fingerprint baseline, and reference/vendor comparison matrices.

## Implementation references

These guides hold the detailed material behind the short source-folder READMEs.
Choose the boundary you are investigating rather than reading a module manual
from top to bottom.

| Need | Reference |
| --- | --- |
| Typed kernel facts, property extraction, and source coordinates | [Kernel analysis](deob/kernel-analysis.md) |
| Complete dispatch, shared step mechanics, and thread startup | [Dispatch](tracing/dispatch.md) |
| JavaScript run-loop, image-write, and constructor proofs | [Prefix preparation](tracing/prefix-preparation.md) |
| Forks, loops, exceptions, calls, patches, and budgets | [Prefix execution](tracing/prefix-execution.md) |
| Stop families, retraces, and discarded work | [Prefix diagnostics](tracing/prefix-diagnostics.md) |
| Interpreter admission and selector-to-recipe mapping | [WASM prefix preparation](tracing/wasm-prefix.md) |
| Artifact fields, integrity, validation, and reuse | [Kernel artifacts](deob/kernel-artifacts.md) |
| Reachability, shared cells, return shapes, and nesting | [Trace orchestration](tracing/trace-orchestration.md) |
| String proof, evaluator semantics, plans, and emitted folding | [VM string evaluator](deob/vm-string-evaluator.md) |
| Static handler occurrences, effects, validation, and projection | [Handler IR](operations/handler-ir.md) |
| Abstract stack behavior and grouping across captures | [Operation classification](operations/operation-classification.md) |
| JavaScript cleanup order, scalar bindings, carries, and branches | [Emitted program cleanup](transforms/emitted-program-cleanup.md) |
| Linked helper sharing, inlining, tuples, and generated names | [Helper and tuple cleanup](transforms/emitted-helper-cleanup.md) |

## Source-module guides

Each source folder keeps its purpose, practical API choices, file map, essential
contracts, and focused checks. Detailed proofs, field references, and worked
walks live in the guides above.

- [Source/module map](reference/source-modules.md) — cross-module flow, ownership, and
  reusable primitive pointers.
- [`src/transforms/README.md`](../src/transforms/README.md) — source preparation (`deob/`) and
  emitted-JS cleanup (`emitted_js/`); see [architecture](architecture.md) for
  the cross-module flow.
- [`src/kernel/README.md`](../src/kernel/README.md) — kernel facts,
  runtime configuration, layout, WASM boundary APIs, and the interpreter lift.
- [`src/operations/README.md`](../src/operations/README.md) — catalogues,
  recipes, graphs, operands, and bounded handler execution.
- [`src/dispatch/README.md`](../src/dispatch/README.md) — complete dispatch,
  JavaScript VM prefix, thread startup, and stop contracts.
- [`src/tracing/README.md`](../src/tracing/README.md) — symbolic execution,
  `TraceDebugOptions`, and observation rendering.
- [`cli/trace/README.md`](../cli/trace/README.md) — dispatch,
  thread, corpus, and program command policies.
- [`src/vm_string/README.md`](../src/vm_string/README.md) — proof-backed string
  evaluation after emission.
- [`src/seeds/README.md`](../src/seeds/README.md) — acquisition and stored
  capture boundaries.

The [source-module map](reference/source-modules.md#areas-and-practical-guides) links
the focused READMEs for the split modules. Use those for entry points, data
flow, file ownership, failure diagnosis, and focused checks; use the contract
pages above for behavior and output.

## Source reference

- [Operation reference](reference/operation-reference.md) — primitive operation
  taxonomy and compositional vocabulary; the matching fixture is
  [`tests/fixtures/operations/primitive-reference.json`](../tests/fixtures/operations/primitive-reference.json).
- [JS/WASM operand cases](reference/js-wasm-operand-cases.md) — static paired
  operand-decoder evidence for the two tracked references.
- [VM control-flow operations](reference/vm-control-flow-ops.md) — source-shaped jump,
  call, branch, loop, and exception cases used by the operation reference.
- [WASM host exception boundary](reference/wasm-exception-boundary.md) — source evidence
  for catch, rethrow, and host-flag behavior in the tracked WebAssembly VM
  reference.

Canonical full captures are exactly two tracked, immutable references:
[`tests/fixtures/kernels/javascript/kernel.raw.js`](../tests/fixtures/kernels/javascript/kernel.raw.js)
for the JavaScript VM and
[`tests/fixtures/kernels/webassembly/kernel.raw.js`](../tests/fixtures/kernels/webassembly/kernel.raw.js)
for the WebAssembly VM.
[`manifest.json`](../tests/fixtures/kernels/manifest.json) records their byte
lengths and SHA-256 digests. Focused synthetic fixtures and bounded dispatch
examples are regression or contract evidence for specific behavior; they are
not additional representative captures. The source-reference pages do not
grant execution support for an operation or claim that handler indices match
across captures. Current classification is available through
`shape ops classify`.

## Open capability boundaries

Use the specific status and missing evidence when a guide says a result is
unknown. These are the current cross-module gaps most likely to be confused
with implemented behavior:

| Boundary | What exists | What is still needed |
| --- | --- | --- |
| [WASM adapter to handler key](reference/wasm-boundary.md#two-imports-that-look-the-same-but-prove-different-things) | Module/import/table inventory | A source-proven binding chain from each copied adapter closure to its operation-table row. |
| [Instruction occurrences in `ops show`](operations/ops-inspection.md#missing-joins-and-command-support) | Recorded/prospective library classifiers | Command inputs for image, PC/state, and completion; WASM also needs selector-to-handler proof. |
| [Undecided prefix branches](tracing/fake-branch-triage.md#what-remains-open) | Test/context fake-side proofs and real-branch records | A complete value/cell or VM-invariant proof for the specific side; a stop or timing outlier alone is insufficient. |
| [Capture-specific encoder/decoder build](reference/vm-flow.md#what-this-path-does-not-claim) | A traced serializer path and static inputs | Fixed-input signal-stage and payload-stage reference bytes, then a proven path through the capture's encryption and final encoding. |
| [Rendered live trace](tracing/trace-text-renderer.md) | Validated typed renderer | A producer that supplies ordered observations and provenance without inventing runtime facts. |

These rows describe missing proofs or connections, not a schedule. Corpus
differences continue to be recorded as evidence; neither an unknown mapping
nor a changed branch order is repaired by copying an answer from another
capture.
