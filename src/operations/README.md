# Operation handlers

This module turns a proven operation table into an inventory, ordered recipes,
static handler graphs, operand descriptions, and inspection views. It also
provides a bounded concrete executor for one admitted recipe. It consumes the
standardized source and proof facts in `PipelineReport`; it does not execute
captured JavaScript or infer dispatch from handler names.

Start with [VM flow](../../docs/reference/vm-flow.md) for the end-to-end path. The
[operation catalogue](../../docs/operations/operations.md) owns inventory and serialization,
while the guides below own the separate recipe, graph, and execution contracts.

## Choose an entry point

| Need | API or command | Detailed reference |
| --- | --- | --- |
| Inventory and source evidence | `catalogue_from_report`; `shape ops list` | [Catalogue](../../docs/operations/operations.md) |
| Compare abstract behavior across captures | `classify_report`, `classify_template`, `classify_seeds` | [Classification](../../docs/operations/operation-classification.md) |
| Extract ordered recipes | `handler_templates_from_report` | [Templates](../../docs/operations/handler-templates.md) |
| Recover values, effects, and unknown regions | `handler_ir_from_report` | [Handler IR](../../docs/operations/handler-ir.md) |
| Obtain recipes and graphs together | `handler_templates_and_graphs_from_report` | [Shared extraction](templates/README.md) |
| Separate inline byte reads from semantics | `js_operand_separation_from_report` | [Operands](../../docs/operations/js-operand-separation.md) |
| Inspect a handler and its independent statuses | `OpsView`; `shape ops show` | [Inspection](../../docs/operations/ops-inspection.md) |
| Execute one recipe against supplied values | `execute_handler`, `HandlerExecutor` | [Concrete executor](../../docs/operations/handler-executor.md) |

The crate root keeps compatibility exports alongside `shape::operations::*`.
Reuse the combined extraction API when both projections are needed. Consumer
APIs still check their own supported operations and limits after admission.

## Four separate decisions

| Decision | What it establishes | What it does not establish |
| --- | --- | --- |
| Catalogue classification | Atomic, composite, or unknown source behavior | An executable recipe |
| Template extraction | Executable, partial, or unsupported ordered semantics | Valid caller state or consumer support |
| Operand lifting | Certified inline reads and a lifted recipe | Dispatch selection or a next-PC proof |
| Recipe admission | Structural and bounded validity of the selected recipe | Source identity or permission for every consumer |

A partial extracted recipe can become usable through a proved operand lift.
`ops show` reports the admitted basis explicitly. Unknown fragments remain
visible; classification, a matching fingerprint, or a familiar table key never
turns them into no-ops.

## Identity and source boundaries

Table selection follows declaration ordinal and exact shape in the report.
Catalogue and template extraction use strict table scanning; operand analysis
has a lenient scan for its narrower task. Keep these policies separate when
sharing helpers. Sparse table keys are selectors, not positions in a vector.

Handler and operand spans index `PipelineReport::output`. The report's input
hash identifies raw input; graph source hashes identify standardized source.
Keep the algorithm and byte basis with each hash. A handler graph is static;
an instruction occurrence additionally needs image, PC/state, decoded operands,
and dispatch evidence. Numeric keys cannot join different captures.

## Files and ownership

| File or folder | Responsibility |
| --- | --- |
| [`mod.rs`](mod.rs) | Module exports |
| [`catalogue.rs`](catalogue.rs) | Inventory, micro-operations, fingerprints, coverage, serialization |
| [`classify.rs`](classify.rs) | Abstract stack behavior and cross-capture kind grouping |
| [`table_scan.rs`](table_scan.rs) | Shared table discovery and scanning policies |
| [`templates/`](templates/README.md) | Report-backed extraction, recipe vocabulary, admission |
| [`graph/`](graph/README.md) | Occurrences, dependencies, effects, validation, projection |
| [`operands/`](operands/README.md) | Bytecode binding proof, ordered decoding, recipe lifting |
| [`view/`](view/README.md) | Validated joins, display, instruction-occurrence projection |
| [`executor.rs`](executor.rs) | Concrete single-handler preflight, execution, observations |

A wrong source role belongs in extraction; a wrong byte offset belongs in
operand analysis; a missing or inconsistent displayed row belongs in the view.
A consumer refusal can be its supported-subset boundary even when graph recovery
is correct. Dispatch, repeated execution, symbolic state, and program emission
belong to their own modules.

## Checks

Use the focused targets for the boundary being changed:

```sh
cargo test --test operation_catalogue --test handler_templates --test recipe_admission
cargo test --test micro_op_graph --test js_operands --test ops_inspection
cargo test --test handler_executor
```

The submodule guides identify narrower fixtures and source tests. Preserve
positive cases alongside near misses, especially shadowed bindings, sparse keys,
unknown fragments, source-coordinate mismatches, and failures after effects.
