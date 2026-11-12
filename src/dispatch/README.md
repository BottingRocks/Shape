# Bounded dispatch

This folder connects source-proven images, dispatch tables, and admitted handler
recipes to symbolic execution. It selects and decodes instructions, constructs
invocations, and records completed work. It does not execute captured JavaScript
or WebAssembly.

The [dispatch reference](../../docs/tracing/dispatch.md) covers preparation, shared step
mechanics, thread construction, and completion. Use the
[trace CLI](../../docs/tracing/trace-cli.md) for runnable workflows.

## Module map

| File | Responsibility |
| --- | --- |
| [`proof.rs`](proof.rs) | SymbolId proofs for the dispatch/operation join, image immutability, and the selector join |
| [`prepare.rs`](prepare.rs) | Source-backed complete-dispatch preparation (`prepare_dispatch_from_report`) |
| [`step.rs`](step.rs) | Shared per-step mechanics: selector index, fetch, decode, completion coordinates, step faults |
| [`execute.rs`](execute.rs) | Complete-dispatch execution and committed records |
| [`thread.rs`](thread.rs) | Caller entry, descriptor, inputs, and the canonical context-construction order |
| [`types.rs`](types.rs) | Limits, completion, records, stops, and errors for complete dispatch |
| [`prefix/`](prefix/README.md) | Prefix proof, walk, WASM lift hand-off, and the source-free artifact |
| [`mod.rs`](mod.rs) | Public facade and re-exports |

The two drivers share `step.rs` and `thread.rs`: the prefix driver imports
`build_captured_memory`, `build_context_from_spec`, `collect_invocation_arguments`,
`validate_descriptor` and `ThreadContextSpec` from `thread.rs`, so both paths
initialize a context in exactly the same order.

## Choose the execution path

| Input and purpose | Entry point | Boundary |
| --- | --- | --- |
| Immutable image and terminal-selector completion | `prepare_dispatch_from_report` | Every non-sentinel dispatch selector must have lifted operands |
| Source-backed bounded VM walk | `prepare_prefix_from_report` | JavaScript proof or admitted WebAssembly lift; unsupported paths stop |
| Saved `PipelineReport` | `prepare_prefix_from_saved_report` | Rebuilds JavaScript proofs only |
| Compiled source-free artifact | `KernelArtifact::from_json`, then `prepare_with_limits` | Prefix mode; loading validates without rerunning Oxc |

Both drivers can finish an invocation. Complete dispatch uses its declared
selector contract; a prefix walk finishes when its explored control flow
completes. Neither result proves whole-capture coverage. Kernel artifacts
support prefix execution; complete dispatch remains source-backed.

## Prepare, bind, invoke

Preparation is reusable. Bind a new entry or descriptor to it instead of
rebuilding analysis for every invocation:

```rust
let prepared = shape::prepare_prefix_from_report(
    source, &report, shape::PrefixLimits::default(),
)?;
let thread = prepared.thread(entry, descriptor, inputs)?;
let result = thread.invoke(arguments)?;
```

This is a function-body fragment; the caller supplies the entry, descriptor,
inputs, and arguments. `ThreadDescriptor` assigns slot roles, while `ThreadInputs`
supplies explicit values. These inputs do not replace source proof. Captures are
snapshots; the startup API rejects `CapturePolicy::LiveShared`.

Complete-dispatch callers can use `execute` for the prepared entry or
`with_entry` for another validated coordinate. The original completion contract
travels with the preparation. `is_complete_completion()` checks that both the
terminal record and returned value were committed.

## Shared rules

A visit includes PC, state, and the VM call stack. `step.rs` owns fetch/decode
mechanics; each driver maps faults into its own stops and diagnostics.

The thread builder applies one initialization order for both drivers. Initial
slots overwrite captured cells; positional argument slots must already exist.
Foreign-arena `Computed` inputs and inconsistent captures are rejected before
execution. Failed handler attempts commit no record; prior committed work stays
inspectable.

Prefix invocations clone their image. Patches affect that invocation's later
reads without mutating the preparation or another invocation. Branch merging
must account for machine state as well as symbolic values.

## Debugging and tests

Identify the phase first: preparation proof, thread startup, instruction decode,
symbolic execution, or emission. A budget change cannot repair a missing proof.
For prefix stops, inspect the public stop and last diagnostic together.

Focused suites: dispatch, thread,
prefix, and stop codes.

## Detailed references

- [Dispatch APIs, proofs, and startup](../../docs/tracing/dispatch.md)
- [Prefix preparation](../../docs/tracing/prefix-preparation.md) and [execution](../../docs/tracing/prefix-execution.md)
- [Prefix diagnostics and retraces](../../docs/tracing/prefix-diagnostics.md)
- [Kernel artifact format and lifecycle](../../docs/deob/kernel-artifacts.md)
- [WebAssembly interpreter preparation](../../docs/tracing/wasm-prefix.md)
- [Symbolic sessions](../../docs/tracing/symbolic-session.md)

The [prefix module guide](prefix/README.md) maps the second driver to its files.
