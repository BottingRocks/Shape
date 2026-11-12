# Handler templates

This module extracts ordered `HandlerTemplate` recipes from the proven operation
table in a `PipelineReport`. Extraction retains unsupported fragments; admission
checks whether a recipe is structurally valid and within bounds. Consumers then
apply their own supported-operation, operand, and state policies.

The [template reference](../../../docs/operations/handler-templates.md) owns the vocabulary,
closure plans, proof walk, admission errors, and worked examples. Use this guide
for API selection and code ownership.

## Entry points

| Need | API |
| --- | --- |
| Recipes and coverage | `handler_templates_from_report` |
| Static graphs only | `handler_ir_from_report` / `handler_graphs_from_report` |
| Both projections from one extraction walk | `handler_templates_and_graphs_from_report` |
| Structural admission | `admit_recipe(&template, bounds)` |

Extraction uses standardized source and the first proven table's declaration
ordinal, shape, and counts. Names are display aliases. `Parse`, `NoProvenTable`,
and `TableIdentityMismatch` are report failures; a successful call can still
contain partial or unsupported handlers.

## Contracts to preserve

`Executable` means the extractor accounted for every step in its supported
vocabulary; `Partial` retains both understood and unknown behavior;
`Unsupported` has no supported executable step. Coverage counts handlers, not
executed instructions. Unknown operations and values never become silent no-ops.

Steps preserve evaluation order and source spans. Initializers are analyzed
before publishing a temporary, and references resolve by Oxc binding identity.
Numeric literals retain decoded binary64 bits. `JsAdd` remains JavaScript
addition, including coercion; it is not a numeric-add assertion.

`admit_recipe` checks status, fragments, order, temporary dependencies, value
shape, and bounds. It does not validate source identity or spans. Default bounds
are 100,000 steps, depth 256, and 100,000 expression nodes per tree. Admission
is necessary before execution but does not expand a consumer's supported subset.

Inline byte reads may require [operand separation](../operands/README.md).
A lifted recipe can be admitted while the original stays partial; preserve both
results and their basis. Graph recovery and recipe projection share one builder,
so changes to recognized values must keep their graphs and recipes consistent.

## Files and consumers

| File | Responsibility |
| --- | --- |
| [`mod.rs`](mod.rs) | Recipe/value types, bounds, admission, public exports |
| [`extract.rs`](extract.rs) | Parse, table identity, shared extraction and report assembly |
| [`build.rs`](build.rs) | Source-order proof walk and graph construction |

The concrete [executor](../../../docs/operations/handler-executor.md) preflights before
any effect and retains committed steps on runtime failure. The symbolic
[session](../../../docs/tracing/symbolic-session.md) handles transactional attempts and
rollback. [Inspection](../view/README.md) displays these facts
without authorizing execution.

## Changing recognition

Start in `build.rs`, and extend report role proof only when new evidence is
needed. Add the exact positive shape and a close rejection case, such as a
shadowed helper or wrong arity. New occurrence kinds also need graph construction,
validation, and projection changes. Extend consumers only within their stated
contracts; a static graph can legitimately retain more than a consumer supports.

```sh
cargo test --test handler_templates --test recipe_admission --test micro_op_graph
```

Use [the reference's extension example](../../../docs/operations/handler-templates.md#extending-recognition)
for the complete checklist and [ops inspection](../../../docs/operations/ops-inspection.md)
when extraction, lift, and admission appear to disagree.
