# Prefix tracing

This module prepares bounded VM execution and binds invocations for the shared
prefix walker. It supports proven JavaScript dispatch and a structurally admitted
WebAssembly interpreter lift. A walk can complete and become eligible for
emission, or retain an inspectable prefix with an explicit stop.

Use the [execution contract](../../../docs/tracing/javascript-vm-prefix.md) for supported
behavior. Detailed proof, execution, artifact, and diagnostic references are
linked below; this guide maps the API and code.

## Files

| File | Responsibility |
| --- | --- |
| [`prepare.rs`](prepare.rs) | Source-backed preparation of the prefix program; dispatches to WASM first |
| [`proof.rs`](proof.rs) | SymbolId proofs for the run loop, the bytecode-write handler, and the completion protocol |
| [`fresh_stack.rs`](fresh_stack.rs) | Standalone proof for a helper that returns a fresh empty array |
| [`execute/`](execute/README.md) | The bounded walk: forks, joins, loops, `try`, subroutines, shared cells, patches (split below) |
| [`wasm.rs`](wasm.rs) | Structurally admitted WASM interpreter preparation (no WASM engine) |
| [`artifact.rs`](artifact.rs) | Compile, validate, and load the source-free `KernelArtifact` |
| [`types.rs`](types.rs) | Records, limits, stops, retraces, and the public data types |
| [`mod.rs`](mod.rs) | `PrefixPreparation`, the thread invocation, the public facade, patch-layout constants |

The walk is split by responsibility under [`execute/`](execute/mod.rs):

| File | Owns |
| --- | --- |
| [`execute/mod.rs`](execute/mod.rs) | `run_prefix`, the retrace loop, and `CellSharing` |
| [`execute/state.rs`](execute/state.rs) | `Walk`, `Cursor`, `Visited`, `Positions`, `Log`, `Learned`, `Side`, and the fork/join state |
| [`execute/walk.rs`](execute/walk.rs) | `run`, `advance`, `step_once`, `call`, and single-step dispatch |
| [`execute/loops.rs`](execute/loops.rs) | `run_loop`, `run_body`, `run_try`, and `resolve_try` |
| [`execute/converge.rs`](execute/converge.rs) | fork convergence, fake-branch audits, and cut-back |
| [`execute/patches.rs`](execute/patches.rs) | image patch decode/apply, halt mapping, and the WASM bound |

## Input paths

| You have | API | Work performed |
| --- | --- | --- |
| Raw source and its report | `prepare_prefix_from_report` | Tries the WASM path, then JavaScript preparation |
| A saved `PipelineReport` | `prepare_prefix_from_saved_report` | Validates the report and rebuilds JavaScript proofs only |
| A compiled `KernelArtifact` | `from_json` → `prepare_with_limits` | Checks integrity, bounds, and cross-references; rebuilds runtime indexes |

Saved-report preparation cannot admit the WebAssembly path. Compile a WASM
artifact with raw source available, then load it for source-free execution.
The properties bundle and pipeline-report JSON are not kernel artifacts.

## Invocation lifecycle

```rust
let prepared = shape::prepare_prefix_from_report(
    source, &report, shape::PrefixLimits::default(),
)?;
let invocation = prepared.thread(entry, descriptor, inputs)?;
let result = invocation.invoke(arguments)?;
```

This fragment assumes caller-supplied entry, descriptor, captures, and arguments.
The preparation owns immutable facts; each invocation owns a fresh context and
image. Reusing an invocation plan does not retain the previous walk's patches.
Thread startup is shared with [complete dispatch](../README.md).

## Results and limits

Preparation and startup return typed errors before a walk. A walk returns a
`PrefixTraceReport` with records, patches, branch evidence, retraces, image
identity, and a stop. `stop_detail()` returns the last diagnostic message.
`Completed` permits the emitter to validate the trace; it does not assert
full-capture coverage. Other stops withhold completed-function emission.

`PrefixLimits` separates preparation dimensions from dispatch work and symbolic
`TraceLimits`. Dispatch steps include both branch sides and region retraces
within an attempt; a whole-walk restart resets that attempt's step counter.
The independent apart-step guard is 100,000. Consult the
[budget reference](../../../docs/tracing/prefix-execution.md#limit-and-budget-accounting)
before interpreting `records.len()` as total work.

## Contracts to preserve

Proof uses binding identity and source structure. Table keys, handler rows,
selectors, instruction coordinates, and source spans remain distinct until a
proved relationship connects them. Unsupported coverage remains visible.

A handler or patch commits only after all step checks succeed. A failed side
is not automatically fake, and a matching PC/state is insufficient to merge
paths with different call stacks, cells, exception state, or image history.
The walker and symbolic session jointly enforce those boundaries.

## References and focused checks

- [JavaScript preparation proofs](../../../docs/tracing/prefix-preparation.md)
- [WebAssembly preparation](../../../docs/tracing/wasm-prefix.md)
- [Execution and control flow](../../../docs/tracing/prefix-execution.md)
- [Stops and retrace diagnostics](../../../docs/tracing/prefix-diagnostics.md)
- [Compiled artifacts](../../../docs/deob/kernel-artifacts.md)
- [Execution source guide](execute/README.md)

Start with prefix integration,
fresh-stack proof,
stop codes, and
WASM kernels. Internal step checks live in
[`execute/tests.rs`](execute/tests.rs).
