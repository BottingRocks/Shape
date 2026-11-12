# Derived value merging

Merges a value the VM derived from a local back into that local:
`let r = f(L);` → `L = f(L);` and every read of `r` reads `L`, when `L`'s old
value is dead. `split_single_reads` does the reverse for a value only the next
store reads, so forwarding can place it. Per-function stages only, in
`forwarding`: `merge_derived_values` (up to 24 rounds via
`merge_until_settled`) at `11m-merged-derived-values.js`, after the
`split_single_reads` loop at `11o-split-single-reads.js`, and right after
the `11p1-forwarded-remaining-helpers.js` dump (its result first shows in
`11p-late-helpers.js`). `--thread-cleanup` does not use it.

## Files

| File | What it does |
| --- | --- |
| [`mod.rs`](mod.rs) | `merge_derived_values` (one merge per binding per call, adjacent edits wait a round) and `split_single_reads` (`v = E; …; v = f(v);` → `let $SSA_sN = E; …; v = f($SSA_sN);`) |
| [`merger.rs`](merger.rs) | Candidates (including `let r;` stored on every path before its reads), source locals, the local's fit (own function, or an outer scratch slot), `local_keeps_derived`, name reach, and the rename edits that drop `L = L;` copies |
| [`statements.rs`](statements.rs) | The forward walk: whether the old value is live after a statement, through blocks, branches, loop back edges, `break`/`continue`, `try` handlers; `definitely_stores` |
| [`tests.rs`](tests.rs) | Countdowns, chains, copy-backs, joins and scratch slots merged; reads of the old value, loops, closures, `try` handlers and objects kept. Merged code is run with Node and compared to the source |

## Contracts

- `r` is a generated `let`/`const`, never written, read only in its own
  function and never as a shorthand property.
- `L` is a single `let`, `var` or parameter (not `const`) only its function
  references, so no call can read or write it; a parameter only when the
  function never mentions `arguments`.
- Nothing writes `L` between the declaration and a read of `r`, and no read of
  `r` sits in a loop after the declaration that writes `L`.
- The old value is dead: no read of `L` is reachable before a plain store
  kills it. `return`/`throw` leave the function; inside a `try` block the
  handler must not read the old value; a finalizer declines.
- `with`/`eval` decline. Overlapping edits are an error; output is reparsed.
- `forward_values_and_property_reads` runs before merging: merging would
  change an object under a copied property read.

[Emitted JavaScript cleanup](../../README.md) ·
[Pipeline table](../../../../../docs/transforms/emitted-program-cleanup.md#pipeline) ·
[SSA folder](../README.md)
