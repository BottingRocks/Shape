# Expression tidying

Expression-level rewrites, in three groups that run in different places:

- **Per function** (`clean_function`): `tidy_expressions/` (stages `11h0`,
  `11h5`, `11i2`, the `11k3` loop, `11p1`, `13b`), `optional_chains.rs` and
  `trailing_returns.rs` (`13b`).
- **Thread driver only** (`--thread-cleanup`, `cleanup_thread_impl`):
  `numeric_coercions.rs`, `method_calls.rs`, `null_flags.rs`,
  `nullish_syntax.rs`, `discarded_values.rs`.
- **Late whole-program passes** (`finish_program`, after linking):
  `constant_slots.rs` (dump `13d-`), then `prototype_methods.rs`,
  `path_temporaries.rs`, `bit_accumulators.rs`, `negated_comparisons.rs`,
  `empty_starts.rs`, `let_runs.rs` (dump `13e-`), in that order with other
  folders' passes between.

## Files

| File | What it does |
| --- | --- |
| [`mod.rs`](mod.rs) | Declares the modules. |
| [`tidy_expressions/`](tidy_expressions/README.md) | `(0, e)` → `e` where the comma means nothing, reprint with minimal parentheses; resugaring (`o[k].call(o, …)` → `o[k](…)`, `c == null ? b : c` → `c ?? b`); the final tidy's loop and counter rewrites. |
| [`optional_chains.rs`](optional_chains.rs) | `x == null ? void 0 : x["a"]["b"]` → `x?.["a"]["b"]`, and the flag-plus-temporary two-link form → `p?.["prototype"]?.["m"]`. `13b`. |
| [`trailing_returns.rs`](trailing_returns.rs) | Drops a body's final `return void 0;`/`return;`. `13b`. |
| [`numeric_coercions.rs`](numeric_coercions.rs) | `Number(x)` → `x` once `x` is proved a Number. Thread `expression_sweep` and phase ends. `number_argument`/`ordinary_number` are shared with `ssa/adjacent_ssa/` and `facts/cell_facts.rs`. |
| [`method_calls.rs`](method_calls.rs) | `o.m.call(o, …)`/`.apply` → `o.m(…)` when the receiver is the same local binding. Thread `expression_sweep` and `late_propagation`; `ordinary_call_lookups` is shared with `ssa/adjacent_ssa/`. |
| [`null_flags.rs`](null_flags.rs) | Removes a private flag channel that always equals `null == value`. Thread `expression_sweep` and `late_propagation`. |
| [`nullish_syntax.rs`](nullish_syntax.rs) | Equivalent guards → `?.` reads and `??` defaults. Thread `late_propagation`. |
| [`discarded_values.rs`](discarded_values.rs) | Removes unused values under the standard-intrinsics policy. Thread `expression_sweep` and `recycling`. |
| [`constant_slots.rs`](constant_slots.rs) | `$KEY_237 = Array.prototype.push;` read as `Array.prototype.push`; literal `RegExp(…)` without `g`/`y` → a regex literal; the slot goes. `regexp_literal` is shared with `literals/array_fills.rs`. |
| [`prototype_methods.rs`](prototype_methods.rs) | `[].push` → `Array.prototype.push`, `{}.hasOwnProperty` → `Object.prototype.hasOwnProperty`, `"".trim` → `String.prototype.trim`. |
| [`path_temporaries.rs`](path_temporaries.rs) | `let t = a.b?.c; use(t, t);` → the path at each read in the next statement. |
| [`bit_accumulators.rs`](bit_accumulators.rs) | `let a = x | 0; a = a | y << 1; … use(a)` → one `|` expression at the use. |
| [`negated_comparisons.rs`](negated_comparisons.rs) | `!(a === b)` → `a !== b`; `!(i < n)` → `i >= n` over never-NaN numbers; De Morgan over `&&`/`||` groups. |
| [`empty_starts.rs`](empty_starts.rs) | `let x = void 0;` → `let x;`; `let x; x = e;` → `let x = e;`. |
| [`let_runs.rs`](let_runs.rs) | `let a;\nlet b;` → `let a, b;`. |

## Proofs and assumptions

- The late whole-program passes take the ECMAScript built-ins and the host
  globals `window`, `self`, `document`, `navigator` not to change
  (`constant_slots.rs`, `prototype_methods.rs`); property reads to run no
  getter and give the same value each time (`path_temporaries.rs`; the
  fingerprint probes read host objects only); and a `.length` read to be an
  array's or a string's, hence a never-NaN number (`negated_comparisons.rs`).
  These are output-readability assumptions, not equivalence proofs.
- `constant_slots.rs` needs a single store in the declaring function before
  any read there, takes a read in a created function to run after that
  store, and requires the global name unshadowed at the read.
- `path_temporaries.rs`: no call, `new`, assignment, update or `delete` in the
  consuming statement runs before the last read; a called read of a member
  path declines (receiver).
- `bit_accumulators.rs` and `empty_starts.rs` require every name moved to a
  later point to hold the same value there: no write in between, and no
  function writing it (for `empty_starts.rs`, no call unless no function
  names `x`).
- The thread-only passes keep unknown coercions and member reads:
  `numeric_coercions.rs` keeps the first `Number()` of an unknown input
  (`valueOf`, BigInt), `discarded_values.rs` keeps unknown member reads,
  `nullish_syntax.rs` needs a value proved not `document.all` before a loose
  `== null` becomes `?.`/`??`, and `method_calls.rs` is disabled by explicit
  `call`/`apply` mutation and dynamic scope.
- `optional_chains.rs` keeps a conditional that is called or used as an
  object: a parenthesised `?.` chain binds `this` differently.
- Order inside `finish_program`: `negated_comparisons.rs` runs after
  `step_copied_counters`, `let_runs.rs` after everything else that merges a
  `let`, and `dotted_properties` (in `program/`) last of all, since these
  passes match the bracketed `o["k"]` spelling.

[Emitted JavaScript cleanup](../README.md) ·
[pipeline table](../../../../docs/transforms/emitted-program-cleanup.md#pipeline) ·
[late whole-program passes](../../../../docs/transforms/emitted-program-cleanup.md#late-whole-program-passes)
