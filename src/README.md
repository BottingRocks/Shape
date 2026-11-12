# Source map

The Rust library turns a raw Shape-style JavaScript kernel into source-backed
facts, admitted handler recipes, bounded traces, and generated JavaScript. Oxc
supplies parsing, binding identity, and code generation. Captured JavaScript
and WebAssembly are inspected as data; neither is executed.

Start with the [repository README](../README.md) for commands or [the analysis walkthrough](../docs/reference/vm-flow.md) for the concepts. This
page points to the owner of each part of the implementation.

## Flow

```text
raw capture → transforms → PipelineReport
                         ├─ kernel facts and proofs
                         ├─ operations inventory, recipes, and operands
                         └─ dispatch + tracing → emitted JavaScript
                                                ├─ vm_string
                                                └─ emitted_cleanup
                                                     ▼
                                     traced program.js ─┬─ signals (shape signals)
                                                        └─ naming  (shape naming)
```

`seeds` acquires raw captures before this flow. The binary-only
[`cli/`](../cli/mod.rs) folder (declared from `shape.rs`, not part of the
library) holds every command, including the trace commands and their JSON output. A source-free kernel artifact
can preserve proven prefix preparation; compiling it still requires source.
[Architecture](../docs/architecture.md) and [source modules](../docs/reference/source-modules.md) cover the cross-module contracts.

## Folders

| Folder | Owns | Start here |
| --- | --- | --- |
| [`transforms/`](transforms/README.md) | Source preparation and emitted-JS cleanup entry points | Transform overview |
| [`transforms/deob/`](transforms/deob/README.md) | Source passes and `PipelineReport` | Source preparation |
| [`transforms/emitted_js/`](transforms/emitted_js/README.md) | Oxc passes over emitted JavaScript (public path `emitted_cleanup`) | Post-emission cleanup |
| [`kernel/`](kernel/README.md) | Kernel facts, layout, properties, and WASM proof/lift | Static inspection |
| [`operations/`](operations/README.md) | Handler catalogue, recipes, operands, and bounded execution | `shape ops show` |
| [`dispatch/`](dispatch/README.md) | Complete dispatch from a supplied entry | Dispatch contract |
| [`dispatch/prefix/`](dispatch/prefix/README.md) | JS/WASM prefix walking and source-free artifacts | Prefix boundaries |
| [`tracing/`](tracing/README.md) | Symbolic recipe execution, emission, and text rendering | Trace values and stops |
| [`vm_string/`](vm_string/README.md) | Proof-backed string evaluation and folding | Post-emission strings |
| [`seeds/`](seeds/README.md) | Fetch, parse, import, and storage | Acquisition |
| [`signals/`](signals/README.md) | Signal schema of a traced program (`shape signals`): extraction, recognizers/binders and cards per signal (`catalog/`), flag decoding, wire codec and payload | Signals |
| [`naming/`](naming/README.md) | Structural function naming of a traced program (`shape naming`): thread rules, classes, collectors, naming plan, inventory, rewrite engine | Function naming |

## Large-module subgroups

The largest areas are split into focused folders. The public module paths are
unchanged; each folder's README maps its files.

| Area | Subgroups |
| --- | --- |
| `transforms/` | `deob/` (source passes), `emitted_js/` (post-emission passes), and `diagnostics.rs` |
| `kernel/` | `wasm/` (`mod`/`module`/`dispatch`/`lift`), `properties/` (`mod`/`scanner`/`json`); `config.rs` and `decoder_proof.rs` stay whole |
| `operations/` | `templates/` (`mod`/`extract`/`build`), `graph/` (`mod`/`types`/`build`), `operands/` (`mod`/`resolve`/`lift`), `view/` (`mod`/`show`/`json`/`occurrence`) |
| `tracing/` | `symbolic/` (`mod`/`model`/`session/`/`exec.rs`/`emit.rs`), `text/` (`mod`/`validate`/`render`) |
| `dispatch/prefix/` | `execute/` (`mod`/`state`/`walk`/`loops`/`converge`/`patches`) |
| `vm_string/` | `proof.rs`, `evaluator.rs`, `value.rs`, `integration.rs` |

## Root files

[`lib.rs`](lib.rs) defines public exports and aliases. [`oxc.rs`](oxc.rs) shares Oxc parsing,
semantic analysis, code generation, and exact source edits; the crate re-exports that module as
`analysis` so the established `shape::analysis` path keeps working. [`vm_patterns.rs`](vm_patterns.rs)
owns the shared nested-dispatch recognizer used by the kernel config, dispatch proof, and transform
pipeline. [`source.rs`](source.rs) reads exact source bytes and computes SHA-256 capture identity. The
binary dispatcher is [`shape.rs`](../shape.rs); every command lives in the binary-only [`cli/`](../cli/mod.rs) folder. [`transforms/diagnostics.rs`](transforms/diagnostics.rs) defines typed findings, [`js_number.rs`](js_number.rs) owns shared
JavaScript numeric semantics, [`profile.rs`](profile.rs) owns opt-in pipeline timing, and
[`util.rs`](util.rs) owns small serialization/hash helpers.

## Boundaries worth keeping

- Static facts require structural and binding proof. A readable name is never
authority to run a handler.
- Recipe admission belongs to `operations::templates::admit_recipe`. Execution and symbolic tracing consume
that decision rather than making their own.
- Traces are bounded. Unknown, unsupported, and budget-limited paths remain
explicit partial results.
- Source-backed reports and source-free kernel artifacts are different types
with different validation. Only the prefix workflow has a source-free
preparation path today.

For the terminology behind table key, handler index, instruction occurrence,
operand position, captured slot, and artifact identity, use the [glossary](../docs/reference/source-modules.md#glossary).
