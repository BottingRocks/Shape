# Join channel common values

Part of `normalize_join_channels` in the parent
[`join_channels.rs`](../join_channels.rs). The parent moves identical trailing
expression statements of both `if` arms after the `if` and initializes a private
undefined cell at its one later unconditional store. This folder factors a
channel value that every arm stores, wherever the store sits among the arm's
other work:

```js
let $CARRY_0; if (flag) { $CARRY_0 = saved; left(); } else { right(); $CARRY_0 = saved; }
// -> let $CARRY_0; if (flag) { left(); } else { right(); }
//    $CARRY_0 = saved;
```

Runs only in the thread cleanup driver (`cleanup_thread_impl` in
[`pipeline/thread.rs`](../../pipeline/thread.rs), used by
`trace program --thread-cleanup`): `expression_sweep`, `storage_sweep` after
tail joins, and `late_propagation` after null flags or tightening.

## Files

| File | What it does |
| --- | --- |
| [`common_values.rs`](common_values.rs) | `common_values`: collects arm store statements (`Collect`), proves every normal path stores (`reaches`, `reaches_if`), deletes the stores and appends one after the `if`; structural tests |

## Contracts

- The channel is an uninitialized `let` declared before the `if`, not
  redeclared or protected, with every reference in the same function. No read
  may come before the `if` ends, and its only writes are the factored stores,
  so a closure that reads it declines.
- The stored value is byte-identical in every store and inert: a primitive
  literal, a pure unary/binary expression, `this` (only in a function
  declaration), or an input identifier. An input must be a block-scoped
  binding declared before the `if`, initialized at the store and after the
  `if`, never written inside the `if` or from another function, resolve to the
  same binding from the arm (no shadowing), and not be protected.
- Every normal path must reach a store; a path that ends in `return`/`throw`
  needs none. Loops, `switch`, labels, `break` and other unmodelled statements
  in the arms decline (`reaches` returns `None`).
- Any `try` in the same function, an empty `$THREAD_` child marker
  (unlinked children), comments on a store, `with`/`eval` and `using`
  declarations decline.
- When the `if` is not directly in a statement list, the edit wraps the `if`
  and the new store in a block.

[Emitted JavaScript cleanup](../../README.md) ·
[thread cleanup](../../../../../docs/transforms/thread-cleanup.md)
