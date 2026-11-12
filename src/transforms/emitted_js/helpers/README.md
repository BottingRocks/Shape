# VM helpers and tuples

These passes rename, share, simplify, inline and expand the VM's subroutine
helpers (`function vm_sub_…`, shared as `$VMHELPER_n`) and the tuples (fresh
arrays) they return. In `trace program` they run in `clean_function`'s
`vm_helpers` phase (`11-…11b5-`), in `helper_expansion` (`11h0-…11h5-`) and
again in `late_helpers` (`11p0-…11p-`). `expand_vm_sub.rs` and
`runtime_helpers.rs` run only under `--thread-cleanup`: `cleanup_thread_impl`
calls them once, before its seven phases. `tuple_scalarize.rs` also runs in
that driver's `expression_sweep`.

## Files

| File | What it does |
| --- | --- |
| [`mod.rs`](mod.rs) | Module list |
| [`vm_sub_parameters.rs`](vm_sub_parameters.rs) | `name_vm_sub_parameters` (`11-vm-sub-parameters.js`): parameters of `function vm_sub_<n>_<n>` become `$PARAMA`, `$PARAMB`, …; names only, calls and statements unchanged |
| [`vm_sub_dedupe.rs`](vm_sub_dedupe.rs) | `share_vm_sub_helpers` (`11b-shared-vm-helpers.js`): closed `vm_sub_*` declarations with identical canonical bodies (locals renamed by source order) become one `$VMHELPER_n` at the root; calls are renamed, originals deleted |
| [`vm_sub_simplify.rs`](vm_sub_simplify.rs) | `simplify_vm_sub_bodies` (`11b1-simplified-vm-subs.js`): in linear `vm_sub_*` bodies, a placeholder-array builder goes through `simplify_array_constructions`; a literal array read once moves into the call that reads it; a single initializer read once is substituted at that read |
| [`inline_vm_sub.rs`](inline_vm_sub.rs) | `inline_vm_sub_wrappers` ×4 (`11b2-inline-vm-wrappers.js`): a one-`return` `vm_sub_*`/`$VMHELPER_*` wrapper is replaced at its calls by its expression with the arguments in place (`H($SSA_3)` → `$SSA_3["length"]`); the declaration goes when every reference was inlined || [`tuple_scalarize.rs`](tuple_scalarize.rs) | `scalarize_literal_tuples` (`11b3-scalar-tuples.js`; thread `expression_sweep`): `let t = [a, b]; let x = t[0]; let y = t[1];` → `let x = a; let y = b;` for 2–16 elements |
| [`tuple_unpack.rs`](tuple_unpack.rs) | `unpack_fresh_helper_tuples` (`11b5-unpacked-tuples.js`): `let t = H(…); let a = t[0]; let b = t[1];` → `let [a, b] = H(…);` for 2–7 reads; also `fresh_tuple_helper`, used by `ssa/dead_ssa.rs` and `ssa/forward_values/` |
| [`inline_small_helpers/`](inline_small_helpers/README.md) | One-line and few-statement helpers expanded at their calls (`11h1-expanded-helpers.js`), and every remaining helper expanded exactly (`11p0-remaining-helpers.js`) |
| [`inline_tuple_helpers.rs`](inline_tuple_helpers.rs) | `inline_tuple_helpers` (`11h1-expanded-helpers.js`): `let [a, b] = H(x, y);` → the helper's body with parameters replaced, then `let a = e0; let b = e1;`; also a held `t = H(…)` read only as `t[k]`. Also holds `FreshNames`, `taken_numbers` and `take_number`, the fresh `$SSA_` naming shared with `inline_small_helpers/`, `branches/branch_value.rs` and `ssa/merge_derived/` |
| [`expand_vm_sub.rs`](expand_vm_sub.rs) | `expand_vm_subs` (`--thread-cleanup` only): linear `vm_sub_*` bodies expanded at every call, leaf-first, up to 32 rounds; arguments and locals get fresh `$SSA_VM_n` names; an embedded call becomes a comma sequence whose temporaries are declared at the top of the calling function |
| [`runtime_helpers.rs`](runtime_helpers.rs) | `inline_runtime_helpers` (`--thread-cleanup` only, after `expand_vm_subs`): calls to `vmForInKeys`, `vmPostToWorker` and `vmInheritOrObject` whose definitions match the emitter's `HELPERS` text (`tracing/symbolic/emit.rs`) are written out inline; the definition goes when every call expanded |

## Contracts

- **Prefixes only nominate.** Every inlining or expanding pass checks that
  the helper binding is never written and that the call resolves to it. Expanding passes
  (`inline_small_helpers/`, `inline_tuple_helpers.rs`, `expand_vm_sub.rs`)
  also require every free name in the body to resolve to the same binding at
  the call. `runtime_helpers.rs` checks its dependencies (`Object`,
  `vmInitializeWorker`, `vmWorkerCallbacks`, `vmWorker`) and that strictness
  matches; `expand_vm_sub.rs` also checks strictness.
- **Arguments run once, in order, before the body.** Argument text replaces a
  parameter only when that keeps the order: a stable argument (a literal or a
  never-written local that is not a slot), or one used once, in argument
  order, before the body runs code. Otherwise the argument is bound first to a
  fresh temporary. `inline_vm_sub.rs` takes only literals and generated names
  (`$SSA_`, `$KEY_`, `$CARRY_`, `$PARAM`, `$OBJECT_`) as arguments, with one
  exception for a single-parameter wrapper read once.
- **Bare-call `this`.** A member-valued body that ends up as a callee is
  written `(0, e)` so it does not gain a receiver.
- **Declines.** `eval` or `with` anywhere; bodies with `this`, `arguments`,
  nested functions or classes, `await`, `yield` or comments (each pass lists
  its own). `inline_vm_sub.rs` skips a `Number(x)` wrapper when any binding is
  named `Number`. `vm_sub_parameters.rs` guards `eval`/`with` by text only.
- **Size.** `share_vm_sub_helpers` output must shrink and reparse.
  `inline_vm_sub_wrappers` must shrink output unless the call is a
  scalarizable tuple call.
- **Assumptions.** `tuple_unpack.rs` assumes the standard Array iterator and
  keeps the helper's allocation and call. `inline_small_helpers/` reads a slot
  argument in place past property reads and stores, taking them to run no
  code (no getters or setters). `runtime_helpers.rs` keeps enumeration a key
  snapshot including inherited keys.
- **Order.** `11b1` exposes tuple and call shapes for `11b2`; `11b2` exposes
  literal tuples for `11b3`; `11b4` folds the scalar copies before `11b5`.
  In `helper_expansion`, `forward_values` and `resugar_expressions` first make
  bodies one line (`11h0-resugared-helpers.js`), then `hoist_helper_calls`,
  `inline_small_helpers` and `inline_tuple_helpers` alternate (×12) until
  neither expands. `late_helpers` runs `inline_small_helpers` again, then the
  exact expansion.

[Emitted JavaScript cleanup](../README.md) ·
[Helper and tuple cleanup](../../../../docs/transforms/emitted-helper-cleanup.md) ·
[Pipeline table](../../../../docs/transforms/emitted-program-cleanup.md#pipeline)
