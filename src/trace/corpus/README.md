# Corpus walk and comparison

This folder implements reachable-function walking, shared-cell convergence,
emission checks, and baseline comparison for `shape trace corpus`. Its command
parser lives in [`commands/corpus.rs`](../../../cli/trace/commands/corpus.rs).

Use [corpus comparison](../../../docs/tracing/corpus-comparison.md) for a first run and
baseline interpretation. [Trace orchestration](../../../docs/tracing/trace-orchestration.md)
explains the traversal phases, function identity, and shared-cell policies.

## Files

| File | Responsibility |
| --- | --- |
| [`mod.rs`](mod.rs) | Internal wiring and per-capture stack bound |
| [`walk.rs`](walk.rs) | Reachability, capture walking, shared cells, and scheduling |
| [`emission.rs`](emission.rs) | Emitted-code checks and created-function extraction |
| [`compare.rs`](compare.rs) | Fingerprints and baseline regression/change comparison |

## Interpreting failures and changes

| Result | Meaning | Next place to inspect |
| --- | --- | --- |
| Function no longer reached or previously complete function now stops | Baseline regression | Reachability, stop reason, and the function's dispatch evidence |
| New reachability, a newly completed function, or a different partial stop | Recorded change | Compare positions, effects, created functions, and branch records |
| New branch decided by context | Baseline regression | The discarded side's evidence and rollback reason |
| Different reversed-side behavior | Order-dependence observation | Fork/join evidence; a digest difference alone does not prove which walk is correct |
| Emission, undeclared-name, or self-check failure | Run can fail without a baseline | `emission.rs`, generated code, and the applicable comparison |

`fingerprint` retains semantic and diagnostic fields rather than using elapsed
time as correctness evidence. `compare` separates regressions from changes;
the command front end decides how to print them and set its exit status.
`--update-baseline` writes the current result, so keep it out of the run meant
to evaluate a regression.

Use a limited run to investigate a specific small walk, then reproduce with
matching flags before comparing results. Limited and full runs have different
shared-cell knowledge. The command-level checks are in
`trace_corpus_cli.rs`; branch semantics
are exercised by `vm_branches.rs`.

`--fast` skips self-checks; it does not skip reachability or convergence.
`--limit` changes the shared-cell policy, and `--seed-by-seed` changes
scheduling. Reverse-side and branch-order differences are observations to
inspect, not automatic failures.

## Contracts to preserve

Function identity is `(descriptor, pc, state)`. The same coordinates with a
different descriptor can describe another invocation. Created functions extend
the reachable set; comparison does not discover them.

Full runs converge shared-cell contents within bounds. Limited runs omit that
analysis and walk a deterministic queue. `--fast` skips self-checks without
changing reachability or convergence. Program mode reuses traversal but owns
its conditional convergence retry separately.

Compare results under matching inputs, flags, artifact mode, and convergence
policy. Timing is diagnostic, not correctness evidence. Keep baseline updates
separate from the comparison being evaluated.

## Focused checks

Corpus CLI tests cover command behavior;
branch tests cover branch semantics. Prefix
execution tests cover fork and retrace mechanics. Use the
[orchestration troubleshooting table](../../../docs/tracing/trace-orchestration.md#where-a-bug-belongs)
to distinguish traversal, emission, and comparison failures.
