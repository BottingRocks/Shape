# Bounded handler executor

`shape::handler_executor` (canonical module path:
`shape::operations::executor`) is the concrete, single-handler projection of a proved
`HandlerTemplate`. `HandlerExecutor::execute` accepts explicit `JsValue`
operands and a moved `HandlerContext`, then returns either a typed
`HandlerCompletion` or an explicit failure together with ordered mutation
observations. It is bounded by recipe-step, stack-length, and initialized-slot
limits and performs preflight before any effect.

This API executes one admitted handler. It does not decode bytecode, infer a
dispatch entry, construct a thread, or run JavaScript/WASM. The existing
`shape trace` commands use the dispatch/thread/prefix path and its symbolic
`TraceSession`; they are not CLI wrappers around this concrete executor. The
two consumers therefore have separate input/output boundaries:

| Boundary | Concrete handler executor | Dispatch/thread tracer |
| --- | --- | --- |
| Input | one proved template, `JsValue` operands, one `HandlerContext` | decoded instructions, `SymbolicValue`, prepared dispatch/thread state |
| Output | `HandlerCompletion` plus stack/slot observations | `TraceReport`, dispatch records, and trace provenance/graph |
| Ownership | one isolated handler job | one persistent trace session across handler calls |
| Dispatch | none | existing proof-backed `dispatch/` APIs |

## Resource policy

`HandlerExecutionLimits` defaults to 100,000 recipe steps, a stack length of
1,000,000, and 1,000,000 initialized memory slots. Pass explicit limits to
`HandlerExecutor::new` when the caller needs a smaller job budget. These bounds
limit the concrete handler job; they do not supply a dispatch instruction budget
or bound a multi-handler trace.

## Supported subset

The executor accepts only `TemplateStatus::Executable` recipes with ordered
steps and initialized temporary dependencies. Supported values are
`undefined`, `null`, booleans, IEEE-754 numbers, strings, and explicit opaque
`Object`, `Symbol`, and `BigInt` values. Opaque values are retained but are not
coerced; unsupported coercions fail closed. `LiteralValue::DecodedNumber`
preserves the binary64 value decoded by Oxc, while manually constructed
`LiteralValue::Number` text is normalized through the tracer's bounded,
literal-only Oxc parser.

Primitive string coercion reuses Oxc's ECMA-262 `StringToNumber` helper after
trimming the complete JavaScript whitespace set (including BOM and NBSP). This
keeps `inf`, `infinity`, and case variants as `NaN`, while the exact
`Infinity` spellings remain valid. Oxc's radix slow path accumulates wide
`0x`/`0b`/`0o` strings directly as binary64 rather than routing through a
`u64` intermediate. Number-to-string coercion reuses Oxc's ECMA-262
`ToJsString` implementation, including the `1e-6`/`1e21` fixed-notation
boundaries and canonical handling of `-0`, `NaN`, and infinities.

Supported effects and expressions are:

- operand and local-temporary reads;
- literal values, `void`, and the selected primitive arithmetic, bitwise,
  unary, logical-not, and JavaScript-add operators;
- `context.stack[index]` reads/writes and `context.stack.length` changes;
- `context.memory.readSlot`, `writeSlot`, and `initializeSlot`;
- explicit `return` and `throw`.

Stack holes remain distinct from initialized `undefined`. Indices must be
finite, non-negative integer numbers within the bounded range. Memory reads of
absent slots and writes to absent slots fail explicitly; callers must use
`initializeSlot` before `writeSlot`. A throw payload is a completion, not an
executor error. Effects after the first return or throw are not executed.

Partial/unsupported templates, unknown values or operations, invalid temporary
ordering, unsupported indices/coercions, missing operands, and resource-limit
failures return structured reasons and never become silent no-ops. Preflight
failures retain the original context and emit no observations; a failure after
committed steps returns the committed prefix and its failure observation.

The observations are a bounded single-handler result for callers that need
mutation evidence; they are not a second CFG/SSA representation and do not
replace `HandlerIR`, the symbolic tracer's graph, or the proof-backed
dispatch/thread execution path. The executor does not consume or execute a
trace-artifact draft.

## Worked handler: a stack update and return

The first composite case in `tests/handler_executor.rs`
extracts this handler from a proved operation table:

```js
function (context, index, value) {
  context.stack[index + 1] += value;
  return context.stack[index + 1];
}
```

It starts with stack `[10, 20]` and supplies decoded operands `index = 0`
and `value = 5`. The compound assignment reads stack position 1, adds 5,
writes 25 to that position, and the return reads 25. The test checks
`HandlerCompletion::Return(JsValue::Number(25.0))` and the ordered stack
mutation. This example matters because the index expression is evaluated before
the read/write; a projection that treated `index + 1` as a source string or
reordered the compound assignment would give the wrong evidence.

The complete runnable construction is
`composite_stack_flow_materializes_index_before_read_and_write`:
its `report` helper builds a `PipelineReport` with a proven operation table,
`handler_templates_from_report` extracts the recipe, and `execute_handler`
receives the admitted template, the initialized context, and the two operands.
Those extraction facts are required; constructing a template from this source
text alone would bypass the proof boundary. The selected handler must be
`Executable`. The executor does not
turn a partial recipe into an executable one. Inspect `observations` in step
order rather than inferring effects from the final context alone: an execution
may fail after a committed stack or slot change. A preflight rejection instead
returns the unchanged input context and no observations.

The worked case below is a library integration fragment, not a standalone
binary: `report` is an owned `PipelineReport`, and the enclosing function returns
`Result<(), Box<dyn std::error::Error>>`. Its synthetic report is built in the
test, so the handler count is known.

```rust
use shape::{
    execute_handler, handler_templates_from_report, HandlerCompletion, HandlerContext,
    JsValue, TemplateStatus,
};

let templates = handler_templates_from_report(&report)?;
// The worked test's synthetic report has exactly one handler. Real code should
// select by table_key, not by the handler's position in a capture.
let template = templates
    .handlers
    .first()
    .ok_or("the worked synthetic report has no handler")?;
assert_eq!(template.status, TemplateStatus::Executable);

// Stack [10, 20]; the recipe operands are the decoded index = 0 and value = 5.
let context =
    HandlerContext::from_values(vec![JsValue::number(10.0), JsValue::number(20.0)]);
let execution = execute_handler(
    template,
    context,
    &[JsValue::number(0.0), JsValue::number(5.0)],
);

// A runtime failure returns the context as of the failure and the observations
// recorded through the failing step; a preflight failure returns the original
// context unchanged, with no observations.
if let Some(error) = execution.error() {
    eprintln!("{error}");
}
match execution.completion() {
    Some(HandlerCompletion::Return(JsValue::Number(value))) => assert_eq!(*value, 25.0),
    Some(HandlerCompletion::Return(value)) => eprintln!("returned {value:?}"),
    Some(HandlerCompletion::Throw(value)) => eprintln!("threw {value:?}"),
    None => {}
}
for observation in execution.observations() {
    println!("{observation:?}");
}
```

Use `execute_handler` when operands are already decoded and the question is
what **one** admitted recipe does to a supplied stack and memory. Use
`HandlerExecutor::new(limits)` to set bounds explicitly. Use the dispatch or
prefix driver when the question includes opcode fetch, state transition, call
frames, or multiple handlers; use the symbolic tracer when values or branches
must remain symbolic.

## Three failures with different meanings

The cases below are checked in `tests/handler_executor.rs`.
They distinguish recipe admission from an execution error and from a JavaScript
`throw` completion.

```js
// Partial recipe: the call is outside the admitted subset.
function (context, value) {
  context.stack[0] = value;
  mystery(...value);
}
```

Even though the first assignment is understood, the template is `Partial`.
`execute_handler` rejects it during preflight with `TemplateUnavailable`,
preserves the supplied context, and produces no observations. Running just its
understood prefix would misrepresent the handler.

```js
// Executable recipe whose second effect fails at runtime.
function (context) {
  context.stack[0] = 1;
  return context.memory.readSlot(0);
}
```

With an empty context, the stack write commits. The subsequent read fails with
`ReferenceError { slot: 0 }`. The returned context has stack `[1]`; the
observations include the `None`-to-`1` stack change and a later error. A caller
must inspect both `error()` and `observations()` rather than treating every
error as a rollback. A source `throw`, by contrast, produces
`HandlerCompletion::Throw(value)` and stops later recipe steps; it is not an
`HandlerExecutionError`.

```js
// Initialization and read are separate operations.
function (context, slot) {
  context.memory.initializeSlot(slot);
  return context.memory.readSlot(slot);
}
```

With `slot = 4`, this returns `Undefined` and leaves slot 4 initialized to
`Undefined`. The otherwise identical handler without `initializeSlot` fails
on the read and leaves the context unchanged. This is why an absent memory slot
cannot be represented by an ordinary `undefined` value in a report.

## Stack holes and mutation records

Setting `context.stack.length = 3` on an empty stack creates three holes.
Writing index 2 and then reading index 1 returns JavaScript `undefined`, but
the final stack remains `[None, None, Some(value)]`. The test
`holes_and_length_changes_are_observable_and_bounded` checks both results.
`None` denotes a hole; `Some(JsValue::Undefined)` denotes an assigned element.
They are different states even when a read yields the same JavaScript value.

Observations are ordered by recipe step. Two writes to index 0 in different
steps produce two changes (`1 → 2`, then `2 → 3`), while repeated writes inside
one step are summarized as its net change. The executor compares number bits
for observations: a NaN payload and signed zero need not behave like ordinary
Rust floating-point equality. The focused tests cover untouched and rewritten
NaN slots, signed-zero writes, stack growth/shrinkage, and errors after an
earlier mutation. Those cases explain why inspecting only the final stack or
slot values loses evidence.
