# Carry lifetimes

Coalesces a local copy-out cell with its source when their live ranges do not
interfere: the destination takes over the source's storage and its name stays,
`let $SSA_value = read(); var slot = $SSA_value; …` →
`var slot = read(); …`, with later `$SSA_value` reads renamed. Runs only in the thread cleanup
driver (`cleanup_thread_impl` in [`pipeline/thread.rs`](../../pipeline/thread.rs),
used by `trace program --thread-cleanup`): `coalesce_local_handoffs` in
`storage_sweep` and `late_propagation`, `finish_private_channels` in
`branch_cells`.

## Files

| File | What it does |
| --- | --- |
| [`mod.rs`](mod.rs) | The two entry points (`finished` selects the finished-channel mode), the `Facts` pre-scan (function owners, `try` regions, writes, shorthand properties, `with`/`eval`) and edit application |
| [`scan.rs`](scan.rs) | The scan for handoffs whose storage the destination can take over: initializers, copy-outs per statement list, candidate checks |
| [`hazards.rs`](hazards.rs) | What may intervene between handoff and commit: definite writes on every path (`MustWrite`), named block-level functions (`NamedFunction`), and `Hazards`: loops, `break`/`continue`, `return`/`throw` |
| [`tests.rs`](tests.rs) | Structural positive and decline cases for `coalesce_local_handoffs` |
| [`uninitialized_channels_tests.rs`](uninitialized_channels_tests.rs) | Private multi-store channels share only a non-interfering destination |
| [`finished_channel_semantics_tests.rs`](finished_channel_semantics_tests.rs) | Node differential check that `finish_private_channels` keeps undefined old reads and short-circuiting |

## Contracts

- An earlier write to the destination is legal only when no read, capture,
  abrupt exit or enclosing exception handler can observe the change. A `try`
  whose handler or finalizer can read the outer cell declines; a cell private
  to the arm does not.
- Copy-in initialization may instead keep the existing destination when the
  source's remaining reads finish before any statement that updates the
  destination.
- A block function declaration with the source's name is a write and blocks
  the rewrite.
- `with`/`eval` returns the input unchanged. Protected (`shared_slots`) names
  are never coalesced; comments in a touched span decline.
- `finish_private_channels` runs after existing snapshot coalescing has chosen
  storage, so a new channel does not take a destination an earlier,
  longer-lived snapshot needs. It never invents a storage name.

[Emitted JavaScript cleanup](../../README.md) ·
[thread cleanup](../../../../../docs/transforms/thread-cleanup.md)
