# Names

Passes that rename generated bindings; none removes or moves a statement.
Loop and catch naming runs per function in `clean_function`'s
`dead_code_and_loops` phase (`11g-loop-catch-names.js`) and again in
`helper_expansion` (`11h4-lifted-loop-counters.js`). The other three run in
`finish_program` on the linked program: short names at stage `12`, role names
at `13c`, positional names at `13d`. The `--thread-cleanup` driver runs none of
them.

## Files

| File | What it does |
| --- | --- |
| [`mod.rs`](mod.rs) | Declares the four modules |
| [`name_loop_and_catch_bindings.rs`](name_loop_and_catch_bindings.rs) | `name_loop_and_catch_bindings`: a generated `for` counter (written and read in the header) becomes `i`, `j`, `k`, then `i_2`, …; a generated `catch` parameter becomes `e`, `e_2`, … per function |
| [`short_generated_names.rs`](short_generated_names.rs) | `shorten_generated_names` (`12-short-generated-names.js`): `$SSA_<function identity>_<serial>` → `$SSA_<serial>_<function number>`, likewise `$CARRY_`, `$MERGE_`, `$ARRAY_`, `$OBJECT_`; the number is the function's place in the linker's sorted order |
| [`role_names.rs`](role_names.rs) | `name_generated_roles` (`13c-role-names.js`): each remaining `$SSA_`/`$CARRY_` binding keeps its suffix and takes a prefix from `role_of`: `$LOOP_`, `$JOIN_`, `$VAR_`, `$TUPLE_`, `$SNAP_`, `$ONCE_`, `$REUSED_`, `$UNUSED_`. `role_of` and `Role` are public |
| [`positional_names.rs`](positional_names.rs) | `name_bindings_by_position` (last pass of `13d-positional-names.js`): every generated `$UPPER_…` binding except `$THREAD_` and `$KEY_` becomes `$A`, `$B`, …, `$Z`, `$AA`, … in source order per function |

## Contracts

- Every rename is by resolved symbol, so the declaration and all references
  move together; evaluation order is untouched. Short names are the exception:
  they replace every identifier token with the exact name, which keeps
  shadowing without semantic lookup, and leave strings and comments alone.
- `with` or `eval` leaves the source unchanged in loop/catch, role and
  positional naming.
- Collisions: loop/catch naming skips a candidate already used anywhere in the
  declaring function's subtree. Role and positional naming check against every
  `$`-name in the source, strings and comments included; positional naming
  skips a taken candidate, while role naming and short naming return an error,
  which fails `finish_program`.
- Positional names never shadow: a function skips every name its enclosing
  functions use; sibling functions may reuse names.
- Order: short names (`12`) run before role names (`13c`), which match the
  `$SSA_`/`$CARRY_` prefixes. Positional naming (`13d`) renames the role
  prefixes too, so they appear only in the `13c`–`13c4` dumps. Loop/catch
  naming touches only names starting with `$`, so its second round (`11h4`)
  leaves the first round's `i`, `e` alone.

[Emitted JavaScript cleanup](../README.md) ·
[pipeline table](../../../../docs/transforms/emitted-program-cleanup.md#pipeline)
