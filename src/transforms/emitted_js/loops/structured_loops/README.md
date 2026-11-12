# Structured loops (parts)

The parts of [`../structured_loops.rs`](../structured_loops.rs)
(`reconstruct_loops`, stage `11d2-reconstructed-loops` and the end of the
thread driver's `expression_sweep`). The parent file holds the basic
rewrite: a `vm_loop_*` whose body branch continues on one arm and leaves on
the other becomes `while`/`for`.

## Files

| File | What it does |
| --- | --- |
| [`extended.rs`](extended.rs) | The extended scan: prefix work ahead of the branch, shared exit paths after it, and process-before-test loops written as `do … while`. |
| [`tests.rs`](tests.rs) | Rewrite and decline cases, Node comparisons, and an ignored inventory test that needs `SHAPE_LOOP_AUDIT_SOURCE` and `SHAPE_LOOP_AUDIT_OUT`. |

## Contracts

- `reconstruct_loops` runs the extended scan only in a round where the basic
  scan found nothing, and works inner loops first; overlapping edits in one
  round are an error, not a silent skip.
- Both scans share the parent's proofs: no invented names, no lifted
  captured per-iteration binding, `protected` names stay put, `with`/`eval`
  stops the pass.
- Loops whose body breaks must skip the false-condition continuation stay as
  they are; the pass does not guess a condition for arbitrary control flow.

[Loops](../README.md) · [Emitted JavaScript cleanup](../../README.md) ·
[thread cleanup: structured loops](../../../../../docs/transforms/thread-cleanup.md#structured-loops)
