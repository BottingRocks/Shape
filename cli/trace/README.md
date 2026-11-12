# Trace CLI and program assembly

This folder owns native `shape trace` commands. Every command supports `--json` for structured output using its normal flags.
There is no separate automation protocol. Rust callers use the public dispatch, prefix, tracing, and emitter APIs.

| File | Responsibility |
| --- | --- |
| [commands/](commands/README.md) | Native argument parsing, output and exit policies |
| [artifact.rs](../../src/trace/artifact.rs) | Bounded source-free artifact loading |
| [inputs.rs](../../src/trace/inputs.rs) | Explicit input snapshots and tagged values |
| [corpus/](../../src/trace/corpus/README.md) | Reachability, scheduling, emission checks, and comparisons |
| [program_nest.rs](../../src/trace/program_nest.rs) | Linked functions, lexical placement, and shared cells |
| [cell_contents.rs](../../src/trace/cell_contents.rs) | Shared-cell holders and converged contents |
| [return_shapes.rs](../../src/trace/return_shapes.rs) | Return-shape discovery for outlined calls |

See the [trace CLI](../../docs/tracing/trace-cli.md), [JSON format](../../docs/tracing/trace-json.md), and
[orchestration guide](../../docs/tracing/trace-orchestration.md). Tests cover JSON output,
report-only tracing, and corpus behavior.
