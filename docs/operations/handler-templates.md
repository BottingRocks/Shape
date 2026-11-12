# Handler templates

`handler_templates_from_report` extracts ordered recipes for handlers in a
proven operation table. The symbolic tracer and bounded dispatch
can consume admitted recipes. `Executable` means the extractor
accounted for every step in its supported subset; the consumer still checks
its own operand and resource limits.
It consumes the report's `ProvenOperationTable` (declaration ordinal and exact
array/object shape) and re-parses that output with Oxc. A table name is never
used as identity, and the numeric `handler_index` / `table_key` is retained as
dispatch metadata. Operation functions remain anonymous. A structurally
complete recipe (`executable` status) is emitted only when every expression and
effect in that handler is proved; partial recipes retain explicit unknown steps
for review.

The recipe is a semantic description, not a second CFG or SSA IR. The runnable
tracer and direct Oxc emitter are documented in
[symbolic-tracer.md](../tracing/symbolic-tracer.md).

## Extract once, then inspect admission

For an existing `PipelineReport`, `handler_templates_from_report(&report)`
returns recipes and coverage. If the caller also needs the static graph, use
`handler_templates_and_graphs_from_report` so both reports come from one walk.
The [templates module guide](../../src/operations/templates/README.md) maps the
entry points and implementation files.

Before using a handler, call `admit_recipe(&handler, RecipeBounds::default())`
and apply the chosen consumer's additional operation, operand, and resource
checks. A successful extraction call can return partial/unsupported handlers.
For a command-level view of all four separate statuses, use:

```sh
shape ops show tests/fixtures/ops-inspection/handlers.raw.js --op 0 --format markdown
```

This assumes the [built executable](../../README.md#build-once) is on `PATH`.
The fixture illustrates a partial extracted recipe whose inline byte reads
can be proved by a separate operand lift. See [ops inspection](ops-inspection.md).

## Recipe values and supported forms

Each `RecipeStep` has a deterministic source span and sequence number. Typed
values can refer to `operand` positions, temporary locals, JavaScript literals,
the context stack, memory slots, and selected primitive operators. Local aliases
become temporary dependencies rather than being inlined away. The `+` operator
is `PrimitiveOperator::JsAdd`; it preserves JavaScript ToPrimitive/string
coercion and is never reported as numeric addition. `-`, bitwise operators, and
the selected unary operators have distinct variants.

The companion `HandlerIR` graph is the broader static recovery surface: it
retains object/array literals, non-VM property reads and writes, captured
bindings, explicit effect order, and unknown regions such as calls when those
appear in a handler. That retention is evidence, not execution support. The
concrete executor still rejects the abstract `StackSlice` and
`Call` values. The symbolic tracer can emit the exact unshadowed
`Function.prototype.apply.call` form when its one-argument stack slice resolves
to a known array; this bounded case is not complete HandlerIR execution or full
VM support.

Numeric literals carry the value Oxc already decoded during extraction
(`LiteralValue::DecodedNumber { bits }`), not the source spelling. The recipe
therefore records the exact IEEE-754 binary64 bit pattern, so sloppy legacy
octal (`010` is `8`, never `10`), literals wider than 64 bits
(`0x10000000000000000`), separators, exponents, `NaN`, `Infinity`, and `-0` all
reach the tracer and the Oxc emitter exactly as JavaScript evaluates them.
Lexical provenance stays in each step's source span. A manually constructed
`LiteralValue::Number(String)` recipe is still accepted, but that text is only a
bounded, literal-only normalization: the tracer validates it with Oxc and
requires a single numeric literal (canonical `NaN`/`Infinity` are handled
explicitly), and rejects anything else instead of reparsing it as a Rust float
or evaluating arbitrary source.

The currently proved subset is:

- operand and local-alias reads, literal values, and selected primitive expressions;
- `context.stack[index]` reads/writes and `context.stack.length` reads/assignments;
- `context.memory.readSlot`, two-argument `writeSlot`, and one-argument
  `initializeSlot` (which defines a slot but does not write a value);
- proven VM PC/state assignments and call-frame push/pop are executable through the
  bounded dispatch/tracer handoff when the caller supplies a cursor and decoded numeric
  operands;
- the proven `Function.prototype.apply.call` program-call form is symbolically
  emitted only when its stack-derived argument array has known elements;
- `if (test) { context.byteOffset = target; context.dispatchState = state; }`
  with local or literal targets (`RecipeOperation::JumpIf`), and
  `vmPendingResult = value` on the proven completion register
  (`RecipeOperation::CompleteThread`);
- the VM exception machinery on proven context fields: try-frame push and pop
  (`PushTryFrame` / `PopTryFrame`, fields through `PoppedTryFrame`), the normal
  `finally` entry (`PushFinallyRecord`), error-record pops (`PopErrorRecord`,
  fields through `PoppedErrorRecord`), the pending exception
  (`PopThrownValue`), and `if (record.wasExceptionHandled) { throw … }` /
  `{ thrownValueStack.pop(); }` (`RethrowIfHandled` / `DropThrownIfHandled`);
- a closed `function (…) { … }` expression, one that uses only its own
  parameters and locals and globals (`SemanticValue::FunctionLiteral`, kept as
  source text), and `new callee(arguments…)` with plain arguments
  (`SemanticValue::Construct`);
- `delete object[key]` on a program object (`SemanticValue::Delete`), whose
  result is a value like any other;
- `for (var key in object) { Array.prototype.push.call(keys, key); }` into a
  local array, which becomes `keys = vmForInKeys(object)`: a call to a helper
  the emitter defines at the top of the function when it is used;
- the VM's `if (x === null || x === void 0) throw new TypeError(…)` check
  and the `x = Object(x)` after it, which are left out: `x` stays as it is;
- `context.receiverObject` / `context.receiverValue`, when the pipeline proved
  those context fields: the VM function's `this` (`SemanticValue::Receiver`),
  resolved by the tracer from the invocation's receiver;
- explicit `return` and `throw` steps.

Receiver and helper properties are accepted only when the pipeline's binding and
property mappings prove their roles. Oxc binding identities are used for every
reference, so a local or shadowed `context`, `stack`, or helper-looking name is
not accepted by spelling. Unknown/dynamic calls outside the bounded program-call form,
branches, unsupported expressions,
unproven assignments, and possible exception-producing behavior are represented
by `Unknown` steps and source spans. A handler receives `executable` status only
when no such fragment exists; a handler with both supported steps and unknown
behavior is `partial`, and a handler with no supported step is `unsupported`.
No unknown is silently converted to a no-op.

Two admitted forms need semantic review. The nullish guard and `Object(x)`
conversion above are omitted, so a primitive can remain unboxed and a nullish
value may miss the source throw. The `delete object[key]` form does not carry
its source function's strictness into the trace node; a failed delete can
therefore differ from strict JavaScript behavior. Optional coverage counts are
measurements from local inputs, not current corpus coverage.

The existing operation catalogue may supply a structural fingerprint for
`semantic_identity`. Its recognizer output is discovery metadata only and does
not authorize execution. If the catalogue cannot be built, the extractor uses a
deterministic source-span hash instead.

## Optional coverage measurement

Template coverage can be measured against raw JavaScript or WebAssembly inputs
supplied at run time. The counts depend on those inputs and are not a feature
matrix; the checkout does not embed them. To run the optional coverage test
against a local input directory with a release build:

```text
SHAPE_HANDLER_TEMPLATE_SEEDS_DIR=/abs/path/to/inputs \
  cargo test --release --test handler_templates -- --nocapture
```

The test reports executable, partial, and unsupported counts rather than
promoting a partial handler. Recipe extraction still does not decode an
instruction or walk the WASM interpreter. Bounded dispatch supplies decoded
operands after its separate source proofs. Use the current report and consumer
admission result to judge a handler rather than extrapolating from another
capture's counts.

## Worked extraction: order and identity

The composite test in `tests/handler_templates.rs`
uses a table with one handler whose body contains aliases, stack effects,
memory effects, and a return:

```js
var operations = [function (context, operand1, operand2) {
  var left = operand1;
  var sum = left + operand2;
  context.stack[context.stack.length] = sum;
  context.stack.length = context.stack.length - 1;
  var slot = context.memory.readSlot(3);
  context.memory.writeSlot(4, slot);
  context.memory.initializeSlot(5);
  return slot;
}];
```

The extractor keeps the alias dependency and emits monotonically ordered
`RecipeStep::order` values. The `left + operand2` step uses `JsAdd`; the
stack-length decrement uses `Subtract`. Those are different semantics even
when a concrete test happens to supply two numbers. The memory read must occur
before the write to slot 4, and `initializeSlot(5)` does not implicitly write
the returned value. The test verifies executable admission and these distinct
operation forms; it does not claim that all JavaScript handlers are admitted.

For a caller, `HandlerTemplateReport.handlers` is a per-table-key inventory.
Read each handler's `status`, `steps`, and `unsupported` spans together:

| Status | Meaning for a consumer |
| --- | --- |
| `Executable` | Every required step belongs to the bounded admitted subset; a consumer may still impose its own operand and resource checks. |
| `Partial` | Supported steps coexist with behavior the extractor cannot justify; inspect the unknown fragments, but do not execute the recipe as complete. |
| `Unsupported` | No executable recipe is available for this handler. |

The table key is a dispatch identity **within this capture**. It is not a
semantic name across captures. Source spans are UTF-8 byte coordinates in the
standardized report output; use them with that exact output, not the raw capture.
For broader object/property evidence inspect `HandlerIR`, and for operand byte
provenance inspect [JavaScript operand separation](js-operand-separation.md).

## Three extraction boundaries in the tests

The focused suite in `tests/handler_templates.rs`
checks more than the composite stack handler above. It is useful to inspect
these distinctions before accepting a recipe from another capture.

A handler containing `operand1 + operand2`, `operand1 - operand2`, and
`operand1 ^ operand2` yields three different typed operators. The first is
`JsAdd`, because two runtime numbers can add while a string input can
concatenate. It must not become numeric addition merely because one sample
used numbers. The concrete executor preserves JavaScript coercion for
the admitted primitive cases.

A syntactically plausible `context` may be a shadowed local rather than the
proved VM context. The `unknown_calls_and_shadowed_context_are_explicit_and_unavailable`
test checks that such a body is unavailable for execution. The extractor keeps
unknown spans instead of assigning effects to the wrong stack. Similarly, a
call after a supported stack write makes the whole template partial; the
executor rejects that template before performing the supported prefix.

Numeric source spelling is also separate from the value. The
`numeric_literals_carry_oxc_decoded_values_into_facts_and_emission` test
covers legacy octal, a wide hexadecimal integer, separators, exponent forms,
`NaN`, `Infinity`, and negative zero. The recorded binary64 bits are what a
consumer receives. A source span lets an analyst find the original spelling;
reparsing that spelling as a decimal number would change some of these values.

For a handler with an unknown fragment, compare the recipe's known steps and
unsupported spans with the [HandlerIR graph](../reference/vm-flow.md#handlerir-recovered-micro-op-program).
The graph can still show ordered object/property effects that this executor
cannot run. “Visible in the graph”, “recipe extracted”, “recipe admitted”, and
“operand decoded” are four separate claims; `ops show` displays them
independently. This separation is especially useful when a handler has a
recognizable operation family but an unsupported call or dynamic operand.

### Binding, string, and compound-write examples

The `undefined_extraction_handles_returns_void_and_shadowing` test has four
handlers:

```js
function (context) { return; }
function (context) { return void 0; }
function (context) { var undefined = 1; return undefined; }
function (context) { return undefined; }
```

The first, second, and fourth return literal `Undefined`. The third returns a
`Temporary`, because its local `undefined` shadows the global binding.
Replacing identifiers by spelling would turn that return of 1 into an
incorrect return of `undefined`.

JavaScript strings present a related representation boundary. The fixture
with `return "\uD800"` is `Unsupported`: a lone UTF-16 surrogate cannot be
faithfully stored as a Rust `String` value. `return "\uFFFD"` is executable
and retains the replacement character as its exact value. These are different
source literals, not two spellings for the same accepted recipe.

The tracked [`handler-template-semantics.raw.js`](../../tests/fixtures/handler-template-semantics.raw.js)
contains a compound stack update. Extraction materializes the computed index
as a temporary **before** reading or writing the stack. The write's value is
`JsAdd(StackRead(index), Operand(position: 2))`; both the read and write use
the same index temporary. If the index expression were evaluated twice, a
side effect in that expression could address two positions. The test also
checks that `initializeSlot(slot)` takes operand position 1, a spread call is
unsupported, and a shadowed context cannot be treated as the proven stack.
That is why recipe step order and operand position are inspectable data, not
formatting details.

## Closure plans and context/helper roles

A closed function becomes a source-free `ClosurePlan { params, body }` with
`Expression`/`Return` statements and `NullIndexCall`, `CharCodeAt`, `AddOne`, or
`External` expressions. `to_source` is a closed canonical renderer; the compile
paths enforce at most 32 unique ASCII identifiers, no defaults/rest/
destructuring/generators/async/directives, and one bounded body. Other functions
remain `SemanticValue::FunctionLiteral { source }` for a downstream compiler.

Roles are proved by pipeline mappings and Oxc binding identity, never spelling:

| Role | Recipe effect |
| --- | --- |
| `context`; `stack`, `memory` | `Context`; stack/memory operations; shadowing makes context unknown |
| `byteOffset`, `dispatchState` | PC/state writes and `JumpIf` |
| `callFrameStack` | `PushCallFrame`/`PopCallFrame`, fields via `PoppedCallFrame` |
| `tryFrameStack`, `errorStack`, `thrownValueStack` | Try/finally ops, pending exception, rethrow/drop guards |
| `vmCreateThread` | `CreateThread` |
| `vmPendingResult` | `CompleteThread` |
| `receiverObject`, `receiverValue` | `Receiver` |

A local named `context` is not the VM context, and an unresolved helper-looking
name is only an abstract `Binding`. The walk also recognizes `vmForInKeys`.
A whole body exactly matching `vmInheritOrObject` or `vmPostToWorker` can be
replaced by that helper's steps and marked executable; this does not upgrade
other partial handlers.

## The proof walk

[`build.rs`](../../src/operations/templates/build.rs) walks the body once in source order, records graph
occurrences, and projects the legacy recipe from that graph, so there is one
authority.

1. A declaration emits `Let` for a fresh temporary. The initializer is analyzed
   before the local is published, so `var x = x` is unknown rather than aliasing
   itself; destructuring and uninitialized locals are unknown.
2. Assignments resolve by symbol: locals become `Assign`; proven context fields
   become PC/state writes; stack length and indexed writes keep dedicated
   operations; other objects become `PropertyWrite`.
3. Calls are classified first: frame/stack pushes, memory helpers, exception
   stacks, bounded `Function.prototype.apply.call`, and side-effect-free calls
   have recipes. VM-internal calls, spreads, unproven helpers, and unknown
   targets stay unknown.
4. `value_built` maps the first parameter to `Context` when proved, others to
   `Operand { position }`, locals to `Temporary`, declared-outside names to
   `Binding`, and unresolved names to global `Binding`. It builds stack, memory,
   property, operator, call, construct, closure, delete, slice, and thread values.
5. Every failure calls `unknown` or `unknown_value`, appending a fail-closed
   operation/value with reason, source, and region kind. Nothing becomes a
   silent no-op.

Two guards are deliberately dropped: `if (!(name in host)) throw …` and the
nullish guard before `x = Object(x)`. Omitting these checks can lose a source
throw; see [supported forms](#recipe-values-and-supported-forms) for the related
boxing and strict-delete caveats.
Inline reads such as `bytecode[context.pc]` stay opaque until
[operand separation](js-operand-separation.md) proves a descriptor.

## Admission

`admit_recipe` validates, in order: `Executable` status; no unsupported
fragment; the step bound; strictly increasing order; no `Unknown` operation; no
`Assign` before its `Let`; every value tree left-to-right (work and depth
bounds, then `Context`, unknown values, and temporary reads before `Let`); and
finally duplicate `Let`s.

| Check | Rejections |
| --- | --- |
| Status and fragments | `TemplateStatus`, `UnsupportedFragment` |
| Shape and order | `StepLimit`, `StepOrder`, `UnknownOperation` |
| Temporaries | `TemporaryReadBeforeLet`, `TemporaryAssignedBeforeLet`, `DuplicateTemporary` |
| Values and bounds | `ContextValue`, `UnknownValue`, `ExpressionWork`, `ExpressionDepth` |

Defaults are 100,000 steps, depth 256, and 100,000 expression nodes per tree.
The value walk is iterative and visits children left to right. Passing is
necessary but not sufficient: consumers can impose additional operation,
operand, state, and resource restrictions. For inline byte reads, a separate
[operand lift](js-operand-separation.md) may admit a lifted recipe while the
original template stays partial; `ops show` records the admitted basis.
Admission does not verify table identity, provenance, or span bounds.

Caller-constructed recipes face the same structural checks;
`recipe_admission.rs` builds templates
directly to pin each error, precedence, and boundary. Constructing one for a
synthetic probe is fine; constructing one from raw source to bypass extraction
skips the proof that values match the report.

## Extending recognition

Add the form narrowly in [`build.rs`](../../src/operations/templates/build.rs), keep the proof on Oxc symbols
and report roles, and retain a positive example plus a near miss:

```js
// Recognized: one proven memory helper with supported arity.
function (context, slot) { context.memory.initializeSlot(slot); }

// Near miss: same spelling, wrong arity. It must stay unknown.
function (context, slot) { context.memory.writeSlot(slot); }
```

Then extend `Proof::from_report` if a role is needed; review `admit_recipe` if
recipe structure changes; update graph construction, projection, and validation
together for a new occurrence kind; and touch each executor, tracer, or lowerer
only as far as its contract allows. Add the focused test first: what extracts,
what stays unknown, and which status/fragments prove the boundary.
