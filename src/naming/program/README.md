# Program rewrites

This module is the rewrite engine of the program layer: it applies **explicit**
binding renames and **opt-in** signal comments to recovered JavaScript. It never
decides which names or signals exist — upstream structural identification
supplies that — and it never executes the code it rewrites. The low-level entry
point is `shape::naming::program::transform_program`; the high-level adapter
is `identify_and_transform`.

The engine fails closed. A rename is skipped with a typed reason when no binding
identifier begins at the requested offset, the replacement name is unusable,
the name would be captured (in either direction) or capture a free global, the
source uses `with`/`eval`, or a binding site cannot be rewritten exactly. Jobs
are also checked against each other: if two plans in one batch would give two
different bindings the same name where they collide (the same scope, or an outer
binding referenced inside a nested scope of the same batch), both are skipped
rather than collapsing the bindings. Sibling scopes that cannot see each other
may still share a name. If any edit is rejected or the result no longer parses,
the whole rewrite is discarded and the original source is returned.

## High-level identification

```rust
use shape::naming::program::{identify_and_transform, IdentifyOptions};

let identified = identify_and_transform(&source, IdentifyOptions::full())?;
println!("{}", identified.transform.transformed_source);
```

`identify_and_transform` extracts `ProgramProperties`, takes its renames from
the unified naming plan (`plan_semantic_names_with_properties`, see
[`docs/naming/naming.md`](../../../docs/naming/naming.md)), renders one
comment per signal wire and routes both plan sets through the same rewrite
engine. The result also exposes the original properties, the naming plan, the
rename/comment plans, the rewrite outcomes and both source hashes.

- There is one role → name vocabulary, owned by the naming plan; the former
  `canonical_role_name` / `role_rename_plans` path is retired. An unproven,
  ambiguous or conflicting role is never renamed.
- A plan addresses a binding by its declaring identifier start — not the whole
  declarator — because that is what the rewrite engine resolves.
- Comment plans are anchored at parsed statement boundaries and contain only
  stable generator-input facts: signal position/route, `STRING`/`NUMBER`/`BYTE`
  counts, `REPEAT`/`CHOICE` containers, prefix/mask roles, mask literals and
  explicit unknown counts. They deliberately omit generated names, so a role
  rename cannot make the text stale and a second annotation pass is a no-op.
- On the command line, `shape signals show` renders the typed report,
  `shape naming plan` prints the naming plan and `shape naming threads
  --rename --out <file>` applies the focused renames (the old `annotate` and
  `rename` commands were removed).

## Using the engine

```rust
use shape::naming::program::{
    transform_program, ProgramBindingRename, ProgramTransformOptions, SignalAnnotation,
};

let renames = [ProgramBindingRename::new(declaration_offset, "readSignal")];
let comments = [SignalAnnotation::new(statement_offset, "signal 0: NUMBER")];
let result = transform_program(
    &program,
    &renames,
    &comments,
    &ProgramTransformOptions { rename_bindings: true, annotate_signals: true },
)?;
println!("{}", result.transformed_source);
for outcome in &result.renames {
    println!("{} -> {} applied={}", outcome.declaration_start, outcome.canonical_name, outcome.applied);
}
```

Offsets are **UTF-8 byte offsets into the exact input string**. A rename's
`declaration_start` is the first byte of the declared binding identifier (the
`x` in `let x = 1`), never the `let`/`var`/`function` keyword. An annotation's
`statement_start` is the first byte of an existing statement.

## Files

| File | Responsibility |
| --- | --- |
| [`mod.rs`](mod.rs) | Public request/result types, options, diagnostics, and orchestration |
| [`identified.rs`](identified.rs) | Properties -> canonical rename and signal comment plans |
| [`rename.rs`](rename.rs) | Lexical symbol resolution, collision checks, and rename edits |
| [`annotate.rs`](annotate.rs) | Statement-boundary comment insertion, escaping, and idempotence |
| [`apply_semantic_names.rs`](apply_semantic_names.rs) | Applies the descriptive [`plan_semantic_names`] plan through this engine |

## Essential distinctions

Renames edit only identifier spans from the AST, so literals, comments, member
names, static property keys and literal computed keys (`obj["x"]`) keep their
spelling. A computed key that is an identifier *reference* to the renamed
binding (`obj[x]`) is a reference, so it moves with the binding. Object and
destructuring shorthands are expanded (`{ x }` becomes `{ x: canonical }`) so the
property name survives.
A rename is resolved by Oxc's lexical symbol identity, so a shadowed inner
binding moves without disturbing an outer binding that shares its spelling, and
references that appear before the declaration (hoisted functions) still move.

Comments are inserted as whole-line `//` comments and the output is reparsed, so
the pass cannot change ASI or single-statement branches. The comment text is
escaped, a comment already present is not duplicated, and the pass is
idempotent.

```sh
cargo test --test program_transforms
cargo test --test program_generator_inputs
```
