# Handler IR

`HandlerIR` records the values, dependencies, effects, and unknown regions of
one recovered handler. It shares the extraction walk with the ordered recipe,
so a handler that cannot execute can still expose useful static evidence.
It contains no dispatch PC, byte image, or particular instruction execution.

The [Handler IR reference](../../../docs/operations/handler-ir.md) owns occurrence fields,
effect ordering, temporary versions, validation errors, bounded projection,
source identity, and signatures. [VM flow](../../../docs/reference/vm-flow.md) explains
how the graph relates to recipes and dispatch records.

## Entry points

| Need | API |
| --- | --- |
| Graphs from a standardized report | `handler_ir_from_report(&report)` |
| Both graphs and recipes from one walk | `handler_templates_and_graphs_from_report(&report)` |
| Validate a constructed or modified graph | `validate()` before indexing or projection |
| Fallible recipe projection | `try_project_steps()` |
| Human-readable evidence | `explain()` |

`HandlerIR` and `HandlerIRReport` are aliases of `HandlerGraph` and
`HandlerGraphReport`; `handler_ir_from_report` aliases
`handler_graphs_from_report`. They are exported at the crate root as well as
through the operations modules. Extraction follows the report's proven table
identity and standardized output.

## Essential contracts

Occurrence IDs are dense and local to one handler identity. `occurrence(id)`
indexes directly and can panic for an invalid ID: validate external or edited
graphs first. Dependency edges point backward, temporary reads refer to the
latest version, and effect order preserves source evaluation order.

Unknown regions keep their source, spans, and reasons. Retained object/property
nodes and calls are evidence, not consumer support. Recipe admission is a
separate check; concrete execution and symbolic tracing each impose
additional restrictions.

Projection is fallible and bounded. Prefer `try_project_steps` when callers
need the reason; the compatibility `project_steps` returns an empty vector on
failure. A structural `canonical_signature` is useful for comparison but is
not a semantic-equivalence proof. Keep source identity with every result.

## Files and debugging

| File | Responsibility |
| --- | --- |
| [`mod.rs`](mod.rs) | Graph API, validation, bounded projection, signatures, explanation |
| [`types.rs`](types.rs) | Occurrence/effect/report types, payloads, limits, report validation |
| [`build.rs`](build.rs) | Dependency/effect construction and temporary versions |
| [`../templates/extract.rs`](../templates/extract.rs) | Report identity and shared extraction |
| [`../templates/build.rs`](../templates/build.rs) | Source proof walk that populates the graph |

For a missing occurrence, start with template recognition. For the wrong
version or effect witness, inspect graph construction. For a valid graph that
cannot project or execute, inspect the projection or consumer boundary before
broadening recognition. The [reference troubleshooting table](../../../docs/operations/handler-ir.md#troubleshooting)
maps specific failures to their checks.

## Checks and examples

```sh
cargo test --test micro_op_graph --test handlerir_object_nodes
cargo test --test handler_templates --test recipe_admission
```

Tests cover dense IDs, source spans, effect
chains, temporary versions, unknown regions, bounded projection, and retained
object nodes. When adding an occurrence kind, update construction, validation,
projection, and explanation together.
