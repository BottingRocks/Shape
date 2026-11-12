# Classes

`rebuild_classes` turns the strict constructor and method threads that
TypeScript and Babel lower a class into back into a `class` declaration. It
runs once in `finish_program` on the linked program (dump `13c2-classes`),
since the methods are other threads, and repeats (×4) so a base class
converts once its subclasses no longer call it.

## Files

| File | What it does |
| --- | --- |
| [`mod.rs`](mod.rs) | `rebuild_classes`, one scan per round, and the strict-mode checks (`SloppyOnly`: a sloppy function that runs the same when strict). |
| [`members.rs`](members.rs) | A class's parts: member stores (`C["prototype"]["m"] = M`, `C["s"] = S`), descriptors, the variables holding the class, `arguments[n]` reads in threads it is handed to, and the reads that must run after it. |
| [`build.rs`](build.rs) | Writing the `class`: elements, `extends` from the TypeScript subclass link, `super(...arguments)` and `S.prototype.m.call(this, …)` → `super.m(…)`. |
| [`babel.rs`](babel.rs) | Babel's lowering: `_classCallCheck`/`instanceof` constructors, `getPrototypeOf(C).call(this, …)` super calls, `_inherits` blocks and `_createClass` descriptor arrays. |
| [`tests.rs`](tests.rs) | Rebuild and decline cases. |

## Contracts

- A member store joins the class only when nothing between the two can see
  the class: no call, no `new`, no read of it.
- Every other use, through any variable it is copied to, is `new`, a member
  read or store, a copy or a return. A constructor that is called, or passed
  through `apply`/`call`/`bind`, stays a function: a class cannot be called.
- No use may run before the class (it is not hoisted) or sit outside the
  statement list that declares it.
- Each moved function is strict or runs the same when strict, and is used
  only by its store. `constructor`, `prototype`, `name` and `length` stay
  stores.
- Edits go through `support::apply_checked`; a round that finds nothing
  returns the input unchanged.

[Program passes](../README.md) · [Emitted JavaScript cleanup](../../README.md) ·
[pipeline table](../../../../../docs/transforms/emitted-program-cleanup.md#pipeline)
