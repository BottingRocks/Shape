# Whole-program passes

Passes that need every thread at once: classes, arrows, thread parameters,
VM values and the fingerprint fold, and property spelling. They run once in
`finish_program`, on the linked program, after `12-short-generated-names`
and `13c-role-names`. The exception is `unwrap_host_globals.rs`, which runs
per function in `clean_function` (stage `11h-host-globals`) and is also a
public entry point.

## Files

| File | What it does |
| --- | --- |
| [`mod.rs`](mod.rs) | Declares the modules. |
| [`classes/`](classes/README.md) | `rebuild_classes`: constructor and method threads (TypeScript and Babel lowerings) → `class` declarations. Dump `13c2-classes`. |
| [`arrows.rs`](arrows.rs) | `arrow_callbacks`: a single-use callback thread → an arrow at its one call/`new` argument; `arguments[n]` → `$ARG_n`. Dump `13c3-arrows`. |
| [`thread_parameters.rs`](thread_parameters.rs) | `thread_parameters`: a function reading its `arguments` only as `arguments[n]` gets `$PARAM_0…$PARAM_n`; `f.call(void 0, …)` with `f` a variable → `f(…)`. Dump `13c4-parameters`. |
| [`vm_values.rs`](vm_values.rs) | `replace_vm_values`: `vmNativeFunctions` read once → its array literal; the root's `return vmEmptyResultSentinel;` → `return;`; calls the fingerprint fold for `vmEntry`. First pass of dump `13d-positional-names`. |
| [`vm_fingerprint.rs`](vm_fingerprint.rs) | `fold_entry_fingerprint`: evaluates the program's `toString`/`substring`/`indexOf`/`lastIndexOf`/`length`/`match` chain over `vmEntry` on the raw script and keeps only the numbers (`x = 345595;`). |
| [`unwrap_host_globals.rs`](unwrap_host_globals.rs) | `unwrap_host_globals`: `host["Math"]["floor"]` → `Math["floor"]`, `new host["TypeError"]()` → `new TypeError()`. Stage `11h`, per function. |
| [`dotted_properties.rs`](dotted_properties.rs) | `dotted_properties`: `o["key"]` → `o.key`, `{ ["key"]: v }` → `{ key: v }`, `o["0"]` → `o[0]`. Last stage, dump `13e-dotted-properties`. |

## Contracts

- Order in `finish_program`: classes (`13c2`) before arrows (`13c3`) before
  parameters (`13c4`), then VM values, the late tidies in `expressions/` and
  `literals/`, and `dotted_properties` last. Every pass before it recognises
  code by its bracketed `o["k"]` spelling.
- `arrows.rs`: the use runs at most once per run of the declaring function
  (not in a loop or nested function), the body reads no `this` and its
  `arguments` only as `arguments[n]`, and every name it reads means the same
  at the use. A `"use strict"` directive stays unless the use is strict.
- `thread_parameters.rs` changes only the function's `length`; a member
  callee keeps `.call`, since `o.f(…)` would pass `o`.
- `vm_values.rs` keeps `vmNativeFunctions` a name when read more than once, so
  every read sees one array. `vm_fingerprint.rs` folds only the exact chain
  (UTF-16 units; a global `RegExp` whose every match is one character from a
  known set); anything else leaves the program unchanged.
- `host["Foo"]` → bare `Foo` is intended: `host` is the global object the
  emitted program closes over (`tracing::symbolic::emit::HOST`). The member
  stays when it is a call callee or tag, or an assignment, update, `delete` or
  `for`-`in`/`of` target; when the key is not an identifier or is reserved;
  and when a binding of that name is in scope. If `host` is a local binding
  anywhere, the pass declines the whole program. It runs at `11h`, before
  forwarding, so built-ins it names read as stable values.
- `dotted_properties.rs` changes text only. Non-identifier names keep their
  brackets (object keys keep quotes), a number literal's member keeps
  brackets, and `["__proto__"]` keys stay computed.

[Emitted JavaScript cleanup](../README.md) ·
[pipeline table](../../../../docs/transforms/emitted-program-cleanup.md#pipeline)
