# Fresh values

`inline_fresh_values` moves a data allocation or inert computation held in a
binding to its read: `let result = [n]; effect(); consume(result);` →
`effect(); consume(([n]));`. An allocation needs one dynamic use (several
references are allowed only in mutually exclusive branches); a small
identity-free computation may be rematerialized at several reads. Runs only in
the thread cleanup driver (`cleanup_thread_impl` in
[`pipeline/thread.rs`](../../pipeline/thread.rs), used by
`trace program --thread-cleanup`): `storage_sweep`, `late_propagation` and
`literal_relocation`.

## Files

| File | What it does |
| --- | --- |
| [`mod.rs`](mod.rs) | Entry point: runs the scan, filters candidates (pending children, class regions, exclusivity, byte budget), builds the edits; `InputFacts` |
| [`scan.rs`](scan.rs) | The scan for fresh values and the reads they can move to: `split_value` (effects stay, the result is recomputed), the `primitive` and `inert` proofs, version checks (`stable_iterations`, `stable_var_initializer`), `loop_values` for while-test stores |
| [`tests.rs`](tests.rs) | `var` inputs, readonly `arguments` computations, structural declines |
| [`flow_pattern_tests.rs`](flow_pattern_tests.rs) | Nine rematerialization patterns that need the current stored type of a cell |
| [`split_value_tests.rs`](split_value_tests.rs) | Effects stay once while ten result forms rematerialize; unstable inputs, coercions, identity and TDZ keep the cache |
| [`exceptional_type_tests.rs`](exceptional_type_tests.rs) | Normal continuations keep stored types without exporting them to abrupt finalizers |

## Contracts

- Identity: the read must be in the same execution owner, innermost loop and
  class region as the declaration, so an allocation is never built twice or in
  another iteration.
- Inputs: literal fields may read initialized lexical inputs only while their
  version is unchanged; a `var` initializer counts as a write. Properties,
  parameters, shared slots, globals, calls, spreads and closures are barriers;
  readonly `arguments` reads pass under the existing ordinary-prototype and
  non-escape proofs.
- Rematerialization uses output cost, not a reference count: with more than
  one read, the rewrite declines when it adds more than 128 bytes, or the
  value is over 256 bytes and the output grows.
- Primitive-type proofs exclude BigInt and Symbol so repeated arithmetic
  cannot throw or call coercion hooks; a mutable cell needs a category every
  successful store preserves.
- A split value keeps its effects once at the declaration (`(0, value)` at the
  reads keeps GetValue); only the stable result is recomputed.
- `with`/`eval` returns the input unchanged; comments in a removed span
  decline. With an empty `$THREAD_` child marker only private temporaries move,
  and only under the tracer's checked-capture contract.

[Emitted JavaScript cleanup](../../README.md) ·
[thread cleanup](../../../../../docs/transforms/thread-cleanup.md)
