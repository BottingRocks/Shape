# Tracing

This folder covers the tracing commands and their contracts: the top-level
trace CLI and JSON output, dispatch and prefix execution,
symbolic tracing and session state, text rendering, corpus comparison, and
branch triage. The
[documentation index](../README.md) covers the other areas.

Start with the [trace CLI](trace-cli.md) for commands and [dispatch](dispatch.md)
for the execution path. Symbolic work starts with the
[symbolic tracer](symbolic-tracer.md), and the
[`tracing` module guide](../../src/tracing/README.md) maps the implementation.

## Pages

- [Trace CLI](trace-cli.md) — the `shape trace thread --json` automation entry, the
  `thread`, `corpus`, `program`, and `cleanup-thread` trace commands, and their
  inputs, streams, exit codes, and diagnostics.
- [JSON output](trace-json.md) — structured results and explicit input snapshots.
- [Dispatch preparation and invocation](dispatch.md) — complete dispatch,
  shared step mechanics, and thread startup.
- [Trace orchestration](trace-orchestration.md) — reachability, shared cells,
  return shapes, and program nesting.
- [Corpus comparison](corpus-comparison.md) — a first run, baseline workflow,
  and change interpretation.
- [Symbolic tracer](symbolic-tracer.md) — bounded dispatch/thread APIs and live
  debug options.
- [Symbolic execution and emission](symbolic-execution.md) — values, graphs,
  recipe evaluation, and emitter validation.
- [Symbolic session driver](symbolic-session.md) — lifecycle, checkpoints,
  branches, loops, exceptions, and shared cells.
- [Typed trace renderer](trace-text-renderer.md) — observations, producer
  contracts, validation, and line maps.
- [JavaScript VM bytecode prefix](javascript-vm-prefix.md) — the bounded
  proof-supported prefix contract.
- [Prefix preparation](prefix-preparation.md) — JavaScript run-loop,
  image-write, and constructor proofs.
- [Prefix execution](prefix-execution.md) — forks, loops, exceptions, calls,
  patches, and budgets.
- [Prefix diagnostics](prefix-diagnostics.md) — stops, retraces, and discarded
  work.
- [WASM prefix preparation](wasm-prefix.md) — interpreter admission and
  selector-to-recipe mapping.
- [Fake-branch triage](fake-branch-triage.md) — branch evidence and
  troubleshooting across captures.
