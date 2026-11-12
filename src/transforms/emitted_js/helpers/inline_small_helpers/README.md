# Small helper expansion

Expands the VM's small shared helpers (`$VMHELPER_64(x)` is `x`,
`$VMHELPER_96(x)` is `x["length"]`) back into the expressions or statements
they hide. Runs in `clean_function`'s `helper_expansion` phase
(`11h1-expanded-helpers.js`, alternating with `inline_tuple_helpers` ×12) and
in `late_helpers`: `inline_small_helpers` ×4, then the exact expansion
(`hoist_remaining_helper_calls`, `inline_remaining_helpers`) into
`11p0-remaining-helpers.js`.

## Files

| File | What it does |
| --- | --- |
| [`mod.rs`](mod.rs) | Entry points: `inline_small_helpers` (bounded shapes), `inline_remaining_helpers` (exact: any size, own `let`s renamed per call, unused helpers deleted), `hoist_helper_calls` and `hoist_remaining_helper_calls`; plans outermost sites and composes nested ones in one parse |
| [`found.rs`](found.rs) | Which helpers qualify: `return E;` (expression shape) or up to `MAX_STATEMENTS` expression statements with an optional final `return` of a name or empty literal (statement shape); records parameter uses, effects, free names and own locals |
| [`sites.rs`](sites.rs) | Call sites and their text: which arguments are stable, whether uses keep argument order, fresh `$SSA_` temporaries for the rest, `(0, E)` for member callees |
| [`hoist.rs`](hoist.rs) | `g(a, H(x), b)` → `let t = H(x); g(a, t, b);` for a statement-shaped `H`, when nothing evaluated before the call can run code or read through a name it is handed; the statement's own store target `o[k]` / `o.k` (never-written `o`, literal key) is no such read. The exact round also hoists a one-`return` helper whose arguments need temporaries (`H(…, H(…))` with code in the body before the inner call's use), innermost first, and binds unconditional code the statement runs before the call to temporaries ahead of it (`let t = k(1); let u = H(x); t(u)`) when no property or changeable name read precedes that code; otherwise it binds every earlier piece (code, property reads, changeable name reads, a store's object) in order, when each is an unconditional part of the value and no method callee, `delete`/`typeof` operand, spread, template or computed key is in the way. A sequence `x = (a, H(..), b)` holding a call splits into statements |
| [`loops.rs`](loops.rs) | A loop whose test or update holds a helper call: `while (T)` → `for (;;) { if (!(T)) break; … }`, `do … while (T)` → test at the end of a `for (;;)`, `for (I; T; U)` → test first / update last in the body; a `continue` to the loop leaves a labelled body (`break iteration`) so it still reaches the test or update; the body keeps its own block when it declares block-scoped names; a `let` loop whose bindings a closure may capture keeps its update |
| [`tests.rs`](tests.rs) | Expression and statement expansion, slot arguments read in place, empty helpers and literal factories, hoisting, exact-mode renaming, reorder refusal |

## Contracts

- Candidates are never-reassigned `$VMHELPER_`/`vm_sub_` declarations with
  plain unwritten parameters and no `this`, `arguments`, nested function or
  class, `await`, `yield` or comment. Every other name the body reads must
  resolve the same at the call.
- Arguments run once, in order, before the body. A parameter is replaced by
  argument text only when the argument is stable (a literal or a
  never-written local that is not a slot) or is used once, in argument order,
  with nothing in the body running code first. An argument whose parameter is
  unused must be pure. Otherwise the argument is bound to a fresh `$SSA_`
  temporary and the call expands into statements (statement, declaration
  initializer, assignment right side or `return` only).
- A read-only slot argument may be read in place past property reads and
  stores: this takes them to run no code (no getters or setters).
- Non-exact mode leaves array returns of non-literals to
  `inline_tuple_helpers`.
- A helper is deleted only when every reference expanded.

[Emitted JavaScript cleanup](../../README.md) ·
[VM helpers and tuples](../README.md) ·
[Pipeline table](../../../../../docs/transforms/emitted-program-cleanup.md#pipeline)
