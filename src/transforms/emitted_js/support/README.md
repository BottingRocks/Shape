# Shared support

Helpers the pass families share. Nothing here rewrites code or runs as a stage
by itself; these modules are called from passes in both the `clean_function`
and `finish_program` stages and from the `--thread-cleanup` driver.
[`mod.rs`](mod.rs) re-exports the small helpers (`apply_checked`,
`for_each_list`, `owner`, `unwrap`, …); `purity`, `primitives` and `index` are
reached as modules.

## Files

| File | What it does |
| --- | --- |
| [`mod.rs`](mod.rs) | Declares the modules and re-exports the edit, list, owner and syntax helpers |
| [`edits.rs`](edits.rs) | `apply_checked` applies a pass's edits, failing if any overlap or the output does not parse; `whole_line` widens a statement span to its line when nothing else is on it; `indent_before`/`indent_at` give a line's leading whitespace |
| [`lists.rs`](lists.rs) | `for_each_list` visits every statement list, outer first: program, function bodies, blocks, `switch` cases |
| [`owners.rs`](owners.rs) | `owner` (the function, arrow or program a node runs in), `function_of` (`None` at top level), `unclosed` (every reference to a symbol is in its declaring function) |
| [`syntax.rs`](syntax.rs) | `unwrap` strips parentheses; `is_loop` matches the five loop kinds; `is_void_zero` matches `void <number>` |
| [`purity.rs`](purity.rs) | `expression_is_pure`: literal-only expressions that cannot run code or throw; identifier reads are rejected outright. Used by the standalone declaration pass and the dead-store passes |
| [`primitives.rs`](primitives.rs) | `Types` types never-written bindings (and typed rewritten `let`s and directly called parameters) as Number, String, Boolean or nullish, and judges `pure` expressions; `allocates`, `is_builtin_member`, `is_stable_global` |
| [`index.rs`](index.rs) | `SourceNames` (generated-name freshness by substring, strings and comments included) and `SpanIndex` (nested function/block/loop regions: `enclosing` finds the innermost one holding a span, `repeated_between` the outermost repeated region entered between an evaluation and its consumer) |

## Contracts

- `purity.rs` "pure" means no effect *and* cannot throw. Calls, member reads,
  assignments, `new`, `in`, `instanceof`, `delete`, spreads and template
  substitutions are always effectful. Coercions are accepted only over literal
  primitives; a computed object key only when it is a string or number literal.
- `primitives.rs` is broader and states its assumptions: standard built-ins
  (`Math` calls, `Number(…)`, named properties of unresolved `Object`, `Math`,
  `JSON`, …) are unchanged. `RegExp` and `Error` are left out of
  `is_builtin_member` because their statics change; host (DOM) globals are not
  stable globals. Parameter and rewritten-`let` types are assumed, then checked
  against every write and call until no assumption drops; a function called
  with a spread, mentioning `arguments`, or used other than by a direct call
  gets no parameter types.
- `Types::pure` accepts an identifier only when it is initialized at the read
  (a `var`, plain parameter, leading `let a, b;`, hoisted function or earlier
  declaration in the same function) or is a stable global; the only calls it
  accepts are of factories (function declarations, or names stored once with
  one, whose body only builds and returns its own values), with pure arguments.
- `allocates` is true for any object-creating expression (and any call): its
  text must not be copied, which would split one identity into several.
- `SourceNames::contains` deliberately matches substrings in strings and
  comments too, so a fresh generated name never collides with any spelling in
  the source.
- `apply_checked` is the edit contract: overlapping edits are an error, not a
  silent skip, and the result must reparse.

[Emitted JavaScript cleanup](../README.md) ·
[pipeline table](../../../../docs/transforms/emitted-program-cleanup.md#pipeline)
