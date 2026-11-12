# Prefix execution

This folder walks a `PrefixPreparation` over an isolated image. It owns
control-flow policy: forks, joins, loops, calls, exceptions, patches, and stops.
The symbolic session owns values and state operations; shared dispatch helpers
own bounded fetch and operand decoding.

[Prefix execution and control flow](../../../../docs/tracing/prefix-execution.md) is the
full reference. Use [diagnostics](../../../../docs/tracing/prefix-diagnostics.md) for
stop families, retrace counters, and discarded work.

## Files

| File | Responsibility |
| --- | --- |
| [`mod.rs`](mod.rs) | `run_prefix`, step limits, and the execution entry |
| [`state.rs`](state.rs) | Cursors, sides, learned facts, visit history, and walk state |
| [`walk.rs`](walk.rs) | Advancing a side through dispatch and handler execution |
| [`converge.rs`](converge.rs) | Fork meetings, rollback, retracing, and fake-side audits |
| [`loops.rs`](loops.rs) | Loop headers, widening, and loop exits |
| [`patches.rs`](patches.rs) | Bounded image patch reads/writes and patch-related stops |
| [`tests.rs`](tests.rs) | Prefix execution and stop-code checks |

A fork's sides are traced in turns and merged only when their positions and
state permit it. A failed side does not automatically prove a fake branch;
the walk records the reason and can audit the discarded side. Loop and fork
retraces roll back speculative work while retaining the facts needed for the
next attempt. These rules affect reachability and branch-order observations,
so keep them intact when editing this folder.

## The execution loop

```text
prepared image + entry + context
  → fetch and decode → admitted recipe or patch
  → commit record and effects
  → advance, fork, join, loop, call, complete, or stop
```

`run_prefix` is internal. Public callers bind and invoke through the
[parent module](../README.md). Each invocation receives a fresh context and
private image; paths share image storage until a write requires a copy.

Whole-walk restarts retain learned facts while dropping the abandoned attempt's
records and patches. Region retraces can instead rewind a fork, loop, or `try`.
`retraces` and `wasted_steps` explain repeated work; committed records alone do
not measure its cost.

## Rules that matter when editing

- A position includes PC, state, and VM call frames.
- A step commits only after fetch, decode, admission, and execution succeed.
- A cut-back restores cursor, image effects, visit history, and symbolic logs.
- A join requires compatible cells, stack, exception state, and image history;
  reaching the same position is insufficient.
- Context pruning uses explicit bad-state rules. A catch-stack uncertainty,
  ordinary program throw, or tracer limit is not blanket fake-branch evidence.
- Loop and exception widening are bounded. A failed convergence becomes a stop.
- Patches never write another invocation's image.

## Diagnose and verify

Start at `mod.rs` for attempts and limits, `walk.rs` for instruction handling,
`converge.rs` for fork meetings, `loops.rs` for widening, and `patches.rs` for
image effects. Check both the public stop and the final diagnostic before
raising a budget.

The [budget reference](../../../../docs/tracing/prefix-execution.md#limit-and-budget-accounting)
explains dispatch steps versus symbolic work and the 100,000-step apart bound.
The [branch triage guide](../../../../docs/tracing/fake-branch-triage.md) explains how to
read pruning evidence.

Tests: [`tests.rs`](tests.rs), prefix,
stop codes,
branches, and
WASM kernels.
