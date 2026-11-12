# Array and object literals

These passes build arrays and objects as one literal, not as a declaration
followed by stores, move a literal to where it is used, and lay arrays out.
They run at several points: post-emission (`03-array-constructions.js`); per
function in `ssa_and_carries` (`06-readable-property-writes.js`),
`dead_code_and_loops` (`11e2-`, `11f-`), `helper_expansion` (`11h3-`), the
forward loop (`11k3-`), `late_helpers` (`11p1-`) and `final_tidy` (`13b-`);
whole-program in `finish_program` (`13e-`); and under `--thread-cleanup` in the
`expression_sweep`, `literal_relocation`, `recycling` and `reprint` phases.

## Files

| File | What it does |
| --- | --- |
| [`mod.rs`](mod.rs) | Module list |
| [`property_writes/`](property_writes/README.md) | `Object.defineProperty` data writes → plain assignments, strict and readable-program policies (`06-readable-property-writes.js`; thread `expression_sweep`) |
| [`array_initializers.rs`](array_initializers.rs) | `simplify_array_constructions`: `let a = [void 0, void 0]; a[0] = 1; a[1] = 2;` → `let a = [1, 2];` in the first function's direct body, primitive-literal values only, with skip reasons. Post-emission `03-array-constructions.js` (via `simplify_cleaned_emitted_property_writes`, which remaps statement maps), thread `expression_sweep`, and `vm_sub_simplify.rs` |
| [`dense_arrays.rs`](dense_arrays.rs) | `fold_dense_arrays` (thread `expression_sweep`, `literal_relocation`): consecutive `a[i] = v;` after `let a = [...]` fold into the literal, overwriting inert elements or appending; also `ordinary_prototypes`, the native-prototype guard other thread passes use |
| [`fold_literal_fills.rs`](fold_literal_fills.rs) | `fold_literal_fills` (`11e2-fold-literal-fills.js`, `11h3-expanded-literal-fills.js`, forward loop `11k3-`): `{ ["k"]: void 0 }` + `o["k"] = v;` → `{ ["k"]: v }`; `let a = []; … a[0] = x; a[1] = y;` → `let a = [x, y];` at the first fill; `{}` filled by string keys likewise; a literal with real values extended by the stores right after it |
| [`nest_literals.rs`](nest_literals.rs) | `nest_literals` (`11h3-`, `11p1-`, `13b-`): Fill folds pure stores into a generated `$` literal; Nest moves a pure literal read once to that read; Project reads `r[k]` of a fixed element (literal, standard global, or named property of one) as that element. Reprints the program with Oxc codegen, up to 16 rounds |
| [`inline_single_use_literals.rs`](inline_single_use_literals.rs) | `inline_single_use_literals` (`11f-single-use-literals.js`): a pure `$ARRAY_`/`$OBJECT_` literal read once in the same function moves to that read |
| [`point_literals.rs`](point_literals.rs) | `inline_property_keys` (thread `expression_sweep`, computed keys and `defineProperty` key arguments only) and `inline_point_literals` (thread `recycling`): a private `$SSA_`/`$CARRY_` read whose cell fact is a primitive literal becomes that literal |
| [`array_fills.rs`](array_fills.rs) | `fold_array_fills` (whole-program `13e-`): `let q = [x, void 0]; q[1] = y;` → `let q = [x, y];` for ascending stores right after the declaration; a `RegExp` of literals is written as a regex literal |
| [`adjacent_literals.rs`](adjacent_literals.rs) | `inline_adjacent_literals` (whole-program `13e-`): `let e = {…}; f(e);` → `f({…});` when only inert names, literals, `this` and paths are evaluated before the read |
| [`array_format.rs`](array_format.rs) | `compact_arrays`: whitespace inside array expressions collapsed; thread `reprint`, after `06-thread-pipeline.js`, and with `--no-cleanup` |

## Five ways to fill an array

Each pass proves its own case; none is a fallback for another.

| Pass | Runs | Appending an index | Values it moves |
| --- | --- | --- | --- |
| `array_initializers.rs` | post-emission, thread | never: all `void 0` seeds, each overwritten once, in order | primitive literals only |
| `dense_arrays.rs` | thread only | only when `ordinary_prototypes` finds no write, delete or escape reaching the native `Array`/`Object` prototypes | effectful values only with a private receiver unobserved during construction |
| `fold_literal_fills.rs` | per function | no guard; its `//!` assumes the standard Array prototype | any value that does not read the literal |
| `nest_literals.rs` | per function | no guard | pure values only |
| `array_fills.rs` | whole program | no guard | values that do not name the array; calls only when no function names it |

So `dense_arrays.rs` is the only one that checks for an inherited setter.
`fold_literal_fills.rs`, `nest_literals.rs` and `array_fills.rs` append on the
program cleanup's standard-built-ins assumption; the two that add object keys
(`fold_literal_fills.rs`, `nest_literals.rs`) exclude only `__proto__`.

## Contracts

- Store values keep their order relative to anything with an effect. A value
  that replaces an existing element must replace one that runs no code
  (`void 0` or a pure element) in `array_initializers.rs`, `dense_arrays.rs`,
  `fold_literal_fills.rs` and `array_fills.rs`.
- A store value must not read the binding being built, and nothing may read
  it before the last folded store.
- `__proto__` keys decline: assignment runs the inherited setter, a literal
  key does not.
- Moving a literal: `inline_single_use_literals.rs` and Nest require a pure
  literal; the first declines a read in a loop the declaration is not in or in
  a class body, Nest one inside a nested function or loop; `adjacent_literals.rs` moves an impure literal
  only into the next statement.
- Source text is copied verbatim (escapes, lone surrogates, `-0`), except in
  `nest_literals.rs`, which reprints, and the regex rewrite in `array_fills.rs`.
- `with`/`eval` declines in `array_initializers.rs`, `dense_arrays.rs`,
  `fold_literal_fills.rs`, `inline_single_use_literals.rs` and
  `point_literals.rs`; so do comments in the rewritten region, where checked.
- `compact_arrays` changes whitespace only; the reparse must print the same
  canonical AST.
- Order: `fold_literal_fills` runs before `nest_literals` at `11h3`;
  `point_literals` and `dense_arrays` relocation run late in the thread driver,
  after reconstruction, to keep construction provenance; `adjacent_literals`
  precedes `join_let_runs` and `dotted_properties`, which must stay last.

[Emitted JavaScript cleanup](../README.md) ·
[Pipeline table](../../../../docs/transforms/emitted-program-cleanup.md#pipeline) ·
[Thread cleanup](../../../../docs/transforms/thread-cleanup.md)
