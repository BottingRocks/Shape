# Linked-program property writes

[`program.rs`](program.rs) contains property-write cleanup for linked program
output. Its parent [`mod.rs`](mod.rs) owns the
standalone strict `Object.defineProperty` rewrite and re-exports these program
entry points.

## Choosing a policy

| API | Required reasoning | Intended consumer |
| --- | --- | --- |
| `simplify_program_property_writes` | Exact data descriptor plus a fresh, unobserved receiver; includes supported direct allocations and zero-argument factory projections | Public API; `cleanup_thread` uses the same policy. `trace program` does not run it |
| `simplify_readable_program_property_writes` | Explicit assumptions about standard built-ins, ordinary extensible receivers, and primitive dynamic keys | `trace program`'s per-function readability stage (`06-readable-property-writes.js`) |

Both are re-exported directly from `shape::emitted_cleanup`. They
accept source text and return a `PropertyWriteCleanupOutcome` or parse error.
The outcome's report distinguishes considered, rewritten, and skipped calls;
a successful parse is not evidence that every call was eligible.

`cleanup_thread` also uses the fresh-receiver program policy to simplify
generated data writes before SSA reconstruction. It does not use the broader
readable policy as a fallback for an unproved receiver. See the
[thread cleanup guide](../../../../../docs/transforms/thread-cleanup.md) for integration.

The strict program pass retains the descriptor and receiver checks of the
standalone pass. The readable-program pass is output-only and assumes standard
built-ins; it is a separate policy rather than a stronger proof of semantic
equivalence. Keep that distinction visible to callers and reports.

## Why a call stays intact

The strict pass checks the declaration and use in the same block and rejects
earlier observation, reassignment, closure capture, or references outside the
supported block. The exact descriptor flags and receiver proof matter:
`Object.defineProperty` can define a property where assignment would invoke
an inherited setter or encounter a non-writable property.

The readable pass deliberately skips calls at program level, where changing
the expression's completion value would be observable. It also declines
dynamic scope and unsupported call/descriptor forms. Its assumptions are
broader than the strict pass, so it must not become an implicit fallback for a
strict thread/corpus rewrite that failed proof.

When changing a rewrite, inspect the individual skip reasons and the stage
snapshots from `trace program --dump-stages DIR`. This source-string API does
not update a standalone `EmittedJavaScript` provenance map; artifact-aware
cleanup must use the parent module's appropriate entry point. Relevant cases
are in `tests/emitted_property_writes.rs`.

The [emitted-JS overview](../../README.md) explains pass ordering. The
[property-write guide](../../../../../docs/transforms/emitted-property-writes.md) details
which descriptor and receiver cases the strict rewrite admits or declines.
