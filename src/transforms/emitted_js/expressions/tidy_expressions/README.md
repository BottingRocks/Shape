# Tidy expressions

Parse once, rewrite the AST, and reprint with Oxc codegen (tab indent, only
the parentheses precedence needs). Per function, in `clean_function`:
`tidy_expressions` at `11h5` and `11p1`; `resugar_expressions` at `11h0`,
`11i2`, in the `11k3` forward loop and at `13b`; `tidy_final_expressions` at
`13b` (twice when `hoist_branch_tails` moved something). Not used by the
`--thread-cleanup` driver.

## Files

| File | What it does |
| --- | --- |
| [`mod.rs`](mod.rs) | `tidy_expressions` and `tidy_final_expressions` (one walk, `counter_steps` off/on), the `Unwrap` state, and `own_names` (names only their declaring function references). |
| [`unwrap.rs`](unwrap.rs) | The walk: `(0, e)` → `e`; `(0, o.m)(x)` → `o.m.call(void 0, x)`; drops bare `;` and empty `else`; `if (t) {} else { X }` → `if (!t) { X }`. Final tidy also drops discarded reads, swaps `if (!t) A else B`, writes `else { if … }` as `else if`, splits `return t ? a : b` with busy arms into an `if`, and splices bare blocks whose names are bound nowhere else. |
| [`resugar.rs`](resugar.rs) | `resugar_expressions`: `o[k].call(o, …)` → `o[k](…)`, `c ? c : b` → `c || b`, `c ? b : c` → `c && b`, `c == null ? b : c` → `c ?? b`, `c == null ? void 0 : c.p` → `c?.p`; shared predicates. |
| [`loops.rs`](loops.rs) | Final tidy only: moves a loop's exit path after the loop; `for (;;) { if (c) break; … }` → `while (!c)`; `x = Number(x) + 1` → `++x`; folds counter steps into indexed stores (`++i; a[i] = v;` → `a[++i] = v;`); jump helpers. |
| [`chains.rs`](chains.rs) | Final tidy only: optional chains spelled through a flag and temporary → `A?.p?.q`. |
| [`tests.rs`](tests.rs) | Unit cases and a Node comparison. |

## Contracts

- `(0, e)` stays where the comma is observable: a tag callee, a `delete`
  operand, `typeof` over an identifier. `(0, name)(…)` → `name(…)` only when
  the program has no `with`. The member form assumes the standard
  `Function.prototype.call`.
- Resugaring reads `c`/`o` once instead of twice, so they must be plain
  names. `??` treats `document.all` as present where `== null` does not.
- Loop restructuring (`restructure_loop`) rewrites
  `L: for (;;) { S; if (t) { A; continue L; } B; break L; }` to
  `for (;;) { S; if (!t) break; A } B`. The exit path `B` must be the loop's
  only way out: it declines when another `break` leaves the loop before the
  split or on the stay path (`breaks_from`), and when the exit path jumps back
  to the loop other than by its own trailing `break` (`exit_jumps_back`). A
  `let` of `S` that `B` reads moves before the loop only when no closure sees
  it.
- Counter-step folding needs a local no call can read, with the array
  expression and value not mentioning it. `x + 1` → `++x` relies on the VM
  stepping numbers.
- `tidy_final_expressions` writes `++`/`--`, which earlier passes read as an
  opaque update, so it stays in `13b`.

[Expressions](../README.md) · [Emitted JavaScript cleanup](../../README.md) ·
[pipeline table](../../../../../docs/transforms/emitted-program-cleanup.md#pipeline)
