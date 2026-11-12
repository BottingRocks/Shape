# Text trace validation and rendering

`shape::tracing::text` accepts typed observations from a producer, validates
their structure, and renders a readable trail with details and a line map.
The same API is available as `shape::trace_text`. A producer supplies the
facts; this module preserves their scope, evidence, and uncertainty in the
output.

The [text-renderer guide](../../../docs/tracing/trace-text-renderer.md) contains the
full observation model, validation rules, rendering conventions, error
reference, and integration guidance. This README keeps the essential handoff,
invariants, implementation map, and test pointers.

The renderer is implemented, but the `shape trace` commands do not currently
produce this observation model from a capture. Symbolic execution's live debug
text is a separate surface. This module evaluates no expressions and defines
no persistence or replay protocol.

## Producer handoff

Construct a header, declared invocation scopes, and ordered events, then call
the public API:

```rust
let rendered = shape::tracing::text::Trace::new(header, scopes, events)
    .with_limits(limits)
    .render()?;
```

This fragment assumes typed inputs and a surrounding function returning a
compatible `Result`. `render()` validates first. Use `validate()` when you
need to check the observations without producing text.

| Output | Use it for |
| --- | --- |
| `RenderedTrace.text` | The readable trail of supplied observations |
| `RenderedTrace.details` | Evidence and supplied source/IR coordinates |
| `RenderedTrace.line_map` | Navigation from rendered line ranges to event IDs and provenance |

Keep trail text and details separate when presenting a result. A line-map
range is one-based and inclusive. Events may render no lines, one line, or
several lines, so event IDs cannot be used as line numbers.

## Contracts the producer must satisfy

| Input | Essential rule |
| --- | --- |
| Scopes | Unique IDs; root scope `0` has no parent; other scopes name an existing parent |
| Events | Unique stable IDs, valid scope references, and execution order within the active invocation |
| Calls | Child events occur between `CallStart` and `CallEnd`; child scope parent and invocation must match |
| Evidence | Keep observed, statically derived, and assumed records distinct; assumptions cite their opening branch |
| Values | Supply typed values; retain unknown, unavailable, and uninitialized distinctions |
| Provenance | Supply coordinates only when known; missing fields remain `unavailable` |

Scope parent links must be acyclic. Each child invocation is entered once, and
its terminal record must agree exactly with `CallEnd`. A returned child is
followed by a matching `Resume` in the caller scope; a stopped child has no
normal resume. Floating-point completion comparisons use recorded bit
patterns, preserving negative zero and NaN payload distinctions.

A `ProvenDead` branch requires a nonempty proof and evidence reference.
Validation checks that those claims are present and structurally consistent;
it does not establish logical reachability. A `Suspected` branch remains
unconfirmed, and equal values cannot establish shared-cell identity.

See [producer contracts](../../../docs/tracing/trace-text-renderer.md#contracts-the-producer-owns)
and [claim versus check](../../../docs/tracing/trace-text-renderer.md#claim-versus-check)
for the detailed evidence boundary.

## Validation and completion

Validation checks fields and references before walking the event structure.
The structure validator maintains an active invocation stack and control
frames for branches, loops, catches, and finally blocks. Events must belong
to the active invocation and respect its pending or terminal completion.

Normal completion of validation leaves only the root invocation, with no open
control frames, awaiting child, or unresolved finally completion. A producer
must close the structure it opened, even when the outcome remains unknown.

Use explicit boundary records such as `OpaqueStop`, `BranchExit::Stopped`,
`CallCompletion::Stopped`, `Value::Unavailable`, and `NoUnconditionalResult`.
A valid observation can describe incomplete knowledge. It must still agree
with the surrounding call and control-flow records.

Two completion rules are especially easy to break:

- A pending return through finally must match `FinallyStart`; replacement
  returns or throws use `FinallyEnd`. Nested finally frames are unsupported.
- A normally ending loop needs a recorded false exit test outside its
  iteration. An explicit return, throw, or stop follows its own abrupt path.

Report validation errors to the producer. Replacing an invalid sequence with
a plausible partial narrative would conceal the missing or conflicting
observation. The [validation reference](../../../docs/tracing/trace-text-renderer.md#validation)
explains the exact order and accepted structures.

## Rendering and bounds

Rendering is deterministic for the same observations and limits. It derives
indentation from the validated structure, escapes producer text, and formats
typed values without getters, coercion, or expression evaluation.

Every producer-controlled display field is escaped for newlines, control
characters, line separators, and bidi controls. Preview truncation affects
only display; it does not change the recorded value. Missing full-value
references remain explicit.

| Limit | Default |
| --- | --- |
| Events | 4,096 |
| Scopes | 1,024 |
| Nesting depth | 64 |
| Bytes per producer text field | 16,384 |
| Value preview bytes | 256 |
| Combined trail and details bytes | 524,288 |

The output budget includes both text and details. Exceeding a work or output
bound returns `TraceError`; it does not return a truncated successful trail.
The guide explains [escaping and previews](../../../docs/tracing/trace-text-renderer.md#escaping-and-previews),
[line maps](../../../docs/tracing/trace-text-renderer.md#the-line-map), and
[error variants](../../../docs/tracing/trace-text-renderer.md#errors).

## Invariants to preserve

- Keep producer claims separate from checks the renderer can perform. A
  validated trail does not certify the underlying execution or proof.
- Preserve assumed-path evidence throughout that path; do not promote it to
  observed or statically derived evidence when rendering descendants.
- Keep event order tied to the active invocation and close control frames
  explicitly. Printed indentation must follow validated structure.
- Match child completion, caller resume, and finally replacement values
  exactly rather than coercing or comparing their display text.
- Keep text escaping and byte accounting in the rendering path. Producer
  strings must not create extra lines or spoof structural output.
- Preserve line-map semantics when an event renders multiple lines or no
  text. Merge only adjacent ranges with matching event and provenance.
- Retain deterministic error selection and output ordering. Update exact
  snapshots when an intentional format change alters the contract.

## Source map

Start with the types in `mod.rs`, then follow validation through field and
reference checks into the invocation/control-frame machine. Read `render.rs`
afterward to see how accepted structure becomes text and line ranges.

| File | Responsibility |
| --- | --- |
| [`mod.rs`](mod.rs) | Observation types, values, evidence, limits, errors, and the public `Trace` API |
| [`validate.rs`](validate.rs) | Field/reference checks, scope graph, evidence rules, and structural validation |
| [`render.rs`](render.rs) | Trail/details output, indentation, escaping, formatting, and line-map construction |

For a rejected record, use the guide's
[symptom-to-code map](../../../docs/tracing/trace-text-renderer.md#where-a-bug-belongs).
The [public API inventory](../../../docs/tracing/trace-text-renderer.md#the-public-api)
and [producer integration guidance](../../../docs/tracing/trace-text-renderer.md#integrating-a-producer)
cover the surrounding types and handoff rules.

## Tests

```sh
cargo test --test trace_text --test trace_text_review --test trace_text_structure
```

The focused suites construct synthetic observations covering branch, call,
exception, loop, and stopped-path cases.
The [complete rendered trail](../../../docs/tracing/trace-text-renderer.md#a-complete-rendered-trail)
shows the output and its event-to-line relationship.

`trace_text.rs` covers public output and maps;
`trace_text_review.rs` covers adversarial
inputs; `trace_text_structure.rs`
covers nesting, completion, and structural validation. `render.rs` also keeps
a deterministic golden corpus for stability checks.

The [tracing overview](../README.md) maps this renderer alongside symbolic
execution and live diagnostic output.
