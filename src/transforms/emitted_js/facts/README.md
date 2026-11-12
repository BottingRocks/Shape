# Facts

Analyses the `--thread-cleanup` passes consult, plus one rewrite. None of these
modules is a `clean_function` or `finish_program` stage. `simplify_branch_facts`
runs in the `expression_sweep`, `storage_sweep` and `recycling` phases of
`cleanup_thread_impl` ([`pipeline/thread.rs`](../pipeline/thread.rs)).
`cell_facts` is collected by `fresh_values`, `stable_channels`,
`point_literals` and the thread variant of adjacent SSA
(`inline_thread_adjacent_ssa`); `cell_liveness` is collected by adjacent SSA
and consulted only in that thread variant.

## Files

| File | What it does |
| --- | --- |
| [`mod.rs`](mod.rs) | Declares the three modules |
| [`branch_facts.rs`](branch_facts.rs) | `simplify_branch_facts`: an `if` or `?:` whose test is already decided (a boolean literal, or a test an enclosing arm assumed) keeps only the taken arm; e.g. `const v = x; if (v) { return v ? 1 : 2; }` → `… return (1); …`, `if (!v) {…} else { good(); }` → `{ good(); }` |
| [`cell_facts.rs`](cell_facts.rs) | `collect` → `Facts`: per read span, whether the cell is definitely initialized (`ready`) and what it is known to hold (`values`: an exact token — literal, `undefined`, a readonly `arguments[n]`, an immutable local — and whether it is primitive or scalar) |
| [`cell_liveness.rs`](cell_liveness.rs) | `collect` → `Facts::dead_after(statement, symbol)`: backward statement liveness, so adjacent forwarding can consume one stored version of a private cell |

## Contracts

- `branch_facts` keys tests only over block-scoped bindings with no writes and
  no redeclarations, and over literals. Only `!`, `typeof`, `void`, strict
  (in)equality, and `==`/`!=` against `null` are combined: none can call user
  code. `eval` or `with` anywhere leaves the source unchanged. Text holding a
  comment is not replaced. An `if` whose dropped arm declares a `var` or a
  function, or whose kept arm is a function declaration, stays. A kept member,
  function or class expression is written `(0, (…))` so it loses its receiver
  and name inference. Facts do not enter nested functions, arrows or classes.
- `cell_facts` justifies forwarding; it never removes an evaluation itself.
  Unknown effects (calls, getters, coercions, property stores, spreads,
  suspension) forget mutable values but cannot undo lexical initialization;
  control-flow boundaries forget everything. Branches intersect; loops keep
  only private cells with no store in the loop; a catch never sees end-of-try
  values; switch fallthrough cannot establish facts for a case body. Strict
  equality does not prove `+0` versus `-0`. Each function, including nested
  callbacks, starts fresh. BigInt and Symbol are never scalar.
- `cell_liveness` treats unknown control flow as opaque and skips any function
  body containing `try`. Loop bodies iterate to a fixed point, and `break` and
  `continue` reach every loop entry, exit and update. A store kills liveness
  only as an unconditional simple `x = …` at statement root or as a declaration
  with an initializer (`var x;` does not).

[Emitted JavaScript cleanup](../README.md) ·
[thread cleanup](../../../../docs/transforms/thread-cleanup.md)
