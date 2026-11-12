# Handler IR reference

Handler IR records the recovered values, effects, and unknown regions of one
static handler. Use this reference for graph identity, validation, and recipe
projection. The [module guide](../../src/operations/graph/README.md) covers API
selection and implementation ownership; [VM flow](../reference/vm-flow.md) places the graph
between source recovery and execution.

## Three different things

A `HandlerIR` is static and reusable: every instruction occurrence
selecting the handler reads the same graph. Dispatch PC and operands
belong to the dispatch record. A recipe is the interpreter-oriented
projection; an instruction is one dispatch event. See the
[VM flow guide](../reference/vm-flow.md#handlerir-recovered-micro-op-program).

| Part | What to follow |
| --- | --- |
| Occurrences | Dense handler-local ids with typed payloads |
| Dependencies | Operand edges and exact temporary versions |
| Effects | Ordered reads, writes, and throwing work |
| Unknowns | Retained source, span, and reason |
| Provenance | Table, handler, source hash, and spans |

The graph does not decode the image, choose a handler, execute a recipe,
prove admission, or re-derive itself from the AST. It does not rename
functions or assign cross-capture names; retaining evidence is not
permission to run it.

## Getting a graph

Use `handler_ir_from_report(&report)` for a `HandlerIRReport`, or
`handler_templates_and_graphs_from_report` for both. `HandlerIR` /
`HandlerIRReport` alias `HandlerGraph` / `HandlerGraphReport`, not
alternate representations; type identity and schema pins are unchanged.

| Entry point | Use it when |
| --- | --- |
| `handler_ir_from_report` | Only graphs are needed; aliases `handler_graphs_from_report` |
| `handler_templates_and_graphs_from_report` | Both reports are needed; one walk supplies both |

Both names are re-exported as
`shape::{handler_ir_from_report, HandlerIR, HandlerIRReport}`. Extraction
takes a `PipelineReport`, not raw source, because it follows standardized
output and proven table identity:

```rust
let irs = handler_ir_from_report(&deobfuscate_with_report(&source)?)?;
```

The table is selected by declaration ordinal plus shape and counts. A
familiar name or numeric key from another capture is not identity.

## Occurrences and dense IDs

An occurrence id is a `u32` that is dense and unique within one exact
`(source hash, table key, handler index)` identity. It is provenance
metadata, never a semantic name. `occurrence(id)` indexes the slice
directly: it panics on a non-dense or out-of-range id, which
`validate()` rejects first. Validate before indexing a modified graph.

`semantic_name()` gives the stable family (`context.stack.read`,
`call_frame.push`), never counter-derived names such as `runLoop_7`.

| Shape | Members | Ordering |
| --- | --- | --- |
| Value (`is_value`) | Context, operands, literals, reads, temporary refs, binary/unary, object/array literals, property reads, calls | No `step_order`; may have an effect |
| Operation (`is_operation`) | Bind/assign, writes, control, return/throw, unknown | `step_order` contiguous from zero |

Each occurrence carries `id`, `op`, `span`, `SpanGranularity`,
`step_order`, `effect`, retained `source`, and `provenance`.
`Statement` means no narrower span was retained. `operands()` returns
consumed ids in evaluation order; `value_operands()` drops the
`TemporaryRef` version edge because it projects to a temporary leaf.

## Temporaries, versions, and dependencies

Dependencies are SSA-like. `Bind` and `Assign` define fresh versions,
and every `TemporaryRef` points at the exact version it observed. A
temporary assigned twice has two definitions; a read between them
points at the first, a later read at the second.

The builder keeps a latest-version map. Validation requires the
referenced occurrence to define the same temporary and be the latest
definition. All edges are backwards (`UseBeforeDefinition`); the
`TemporaryRef` version edge is also checked as a definition, while other
operands must be values.

## Effects

Effects are an ordered chain, not counters. Each operation owns at most
one effect, effect ids are dense, and the chain is monotonic in
occurrence order.

| Family | Occurrences | May throw |
| --- | --- | --- |
| Reads | stack length/index/range, memory slot | Memory reads only |
| Writes | stack, stack length, memory, non-VM property | No |
| Control | try frame, exception, call frame, PC/state, jump | No |
| Program | thread capture, evaluate, return, throw, call/construct/delete, unknown | Calls, constructs, deletes |

Reads record the write they observe. `observed_write` is the latest
preceding write of a comparable location; a dynamic index has no key.
An unknown write clears witnesses, a stack-length write clears the
stack namespace, and `"length"` may alias `stack.length`. A stale
witness is a `StaleEffectWitness`.

Throwing order is observable: a memory read can throw before a later
stack write, and a throw does not erase earlier effects.

## Unknown regions and provenance

An `Unknown` operation gets an effect and an `UnknownRegion` with kind
(`ControlFlow`, `Call`, `Property`, `Assignment`, `Expression`,
`Local`, `Other`), span, reason, and source. An `UnknownValue` needs a
region with the same span; removing either fails validation.

An id never makes a region executable. Status is `Executable` only with
no unknowns, `Partial` with a supported operation, else `Unsupported`;
stored `status` must match.

Every occurrence repeats standardized source hash and length, raw-input
hash, table ordinal and counts, `table_key`, `handler_index`, and span.
A copied occurrence is `ForeignProvenance`; `table_key` is
capture-local.

## Source and report identity

The report and every graph share one immutable `Arc<str>` of
standardized source. `Arc::ptr_eq` confirms it; report validation
hashes once, then checks graphs without rehashing. Without `source`,
validation is structural only.

`HandlerIR.source_hash` is SHA-256 over standardized bytes
(`PipelineReport::output`); `HandlerIR.raw_input_hash` is the legacy input
identity kept for joins. The same `source_hash` spelling also names
raw-input FNV-1a-64 fields on the catalogue and `Analyses` view; see
[identity fields named `source_hash`](operations.md#identity-fields-named-source_hash)
before joining across records. Compare only within one extraction; hashes do
not establish AST authenticity. Key caches on `GRAPH_SCHEMA_VERSION`
and `PRIMITIVE_REGISTRY_VERSION`.

## Validation rules

Validate an external graph before indexing; `occurrence(id)` assumes
dense ids.

| Rule | Rejected as |
| --- | --- |
| Ids equal their slots | `NonDenseId`, `NonDenseEffectId` |
| Spans ordered, bounded, UTF-8 safe | `HandlerSpanInvalid`, `InvertedSpan`, `SpanOutOfBounds`, `InvalidUtf8Boundary` |
| Source and handler provenance agree | `SourceLengthMismatch`, `SourceHashMismatch`, `ForeignProvenance` |
| Inputs exist, precede uses, are values; versions are latest | `DanglingInput`, `UseBeforeDefinition`, `WrongOperandKind`, `InvalidTemporaryVersion` |
| Step orders and effects are typed, linked, monotonic | `StepOrderMismatch`, `MissingEffect`, `UnexpectedEffect`, `IncorrectEffect`, `EffectLinkMismatch`, `NonMonotonicEffect` |
| Witnesses are safe and latest | `UnsafeEffectReorder`, `StaleEffectWitness` |
| Unknown regions match; no orphans | `UnknownRegionMismatch`, `UnknownRegionMissing`, `OrphanUnknownValue` |
| Bounds hold; status matches | `ValueProjectionTooDeep`, `ValueProjectionTooComplex`, `StatusMismatch` |

This proves structural consistency and, with source, coordinate
ownership. It is not an AST re-derivation, so it is not authenticity
against a caller that rewrites public fields; the builder owns that
correspondence. It is also not runtime admission. The legacy template
projection is not one-to-one with nodes and needs `admit_recipe` before
execution. Keep errors and unknown regions visible.

## Projection is fallible and bounded

`try_value(id)` rebuilds the legacy `SemanticValue` tree.
`try_project_steps()` validates first, then converts operation
occurrences into `RecipeStep`s with order, span, and typed operation.
`project_unsupported()` returns retained fragments.

| API | Success | Failure |
| --- | --- | --- |
| `try_value(id)` | Value subtree | `InvalidId`, `NotValue`, `Cycle`, `ValueTooDeep`, `ValueTooComplex`, `InvalidGraph` |
| `try_project_steps()` | `Vec<RecipeStep>` | `InvalidGraph` with the validation error |
| `project_steps()` | The same vector | Empty vector |

Shared edges expand a tree by cloning, so the build is iterative and
fails closed before materializing: `MAX_PROJECTED_VALUE_DEPTH` is 1024
and `MAX_PROJECTED_VALUE_NODES` is 1,000,000; `validate()` uses the
same bounds.

`try_value()` checks kind, cycles, and bounds, but is not full
validation. The extractor's crate-private trusted projection skips the
public limit for its own builder; callers cannot use it. Steps are a
compatibility view; recipes still need `admit_recipe` and consumer
checks. `project_status()` returns `Unsupported` on invalid graphs.

## Object and property evidence

The graph also retains `Binding`, `PropertyRead`, `PropertyWrite`,
`ObjectLiteral`, `ArrayLiteral`, `FunctionLiteral`, `Construct`,
`Delete`, `Call`, `CreateThread`, `Receiver`, `PoppedCallFrame`, and
`ExceptionRead`, with explicit object, key, callee, and argument edges.

This evidence is not execution permission. The executor rejects
object-model and VM-control nodes before a step, and the symbolic
tracer handles only documented bounded cases. `is_executable()` comes
from unknowns and occurrence support, not admission.

## `explain()` and `canonical_signature()`

`explain()` prints a header with handler, table, status, span, and
counts, then each occurrence's id, semantic name, span, inputs, defined
temporary, and effect (`may_throw`, `observes`), followed by unknown
regions. It is a reading aid; use typed fields for logic.

`canonical_signature()` is for structural comparison, not general
semantic equivalence. It validates first and returns `"invalid-graph"`
on failure. Temporaries are alpha-renamed in definition order; each
occurrence adds operation spelling, operand ids, effect link, and
source; effects add kind, `may_throw`, observed write, and location.

It keeps every definition/use edge, never inlines captured reads, and
assigns aliases at definitions. Alpha-renamed locals can match while
split and inline forms do not; operators, string versus numeric add,
access order, and throw order differ. Equal signatures do not imply
identity: identical bodies in different slots match. It is never
identity, admission, or execution authority.

## Troubleshooting

| Symptom | Likely cause | Look at |
| --- | --- | --- |
| `occurrence(id)` panics | Unvalidated or compacted graph | `validate()`; `NonDenseId` |
| `InvalidTemporaryVersion` | Reference copied to another handler | Rebuild through the walk |
| `StaleEffectWitness` / `UnsafeEffectReorder` | Reordered steps or invalidated witness | Aliasing rules |
| `MissingEffect` / `UnexpectedEffect` | `op` changed without its effect | Rebuild; keep them paired |
| `ForeignProvenance` | Node copied across handler or source | Re-extract |
| `SourceHashMismatch` | Source or length replaced | Keep the report's source |
| `ValueProjectionTooDeep` / `TooComplex` | Oversized or cyclic DAG | Fix the graph; no public trusted path |
| `try_project_steps()` returns `InvalidGraph` | Validation failed | Read the wrapped error |
| Unsupported-looking handler with typed nodes | Retention is not admission | `admit_recipe`; consumer checks |
| Admitted recipe still fails at an instruction | Graph is static; PC/state are dispatch evidence | Dispatch preparation |

For broader contracts, see the [operations overview](../../src/operations/README.md),
[operation contract](operations.md), and
[VM flow](../reference/vm-flow.md). The
[handler template guide](handler-templates.md) covers
admission; the [symbolic tracer](../../src/tracing/symbolic/README.md)
covers the bounded consumer.
