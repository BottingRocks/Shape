# Emitted property-write cleanup

`emitted_cleanup::simplify_property_writes` is an **opt-in, source-only** pass
over emitted JavaScript. It rewrites the emitter's slot-write shape

```js
Object["defineProperty"](receiver, key, {
    writable: true,
    configurable: true,
    enumerable: true,
    value: value
});
```

to `receiver[key] = value` when ordinary assignment is observably equivalent.
It does not execute input JavaScript or read capture scripts. The strict pass
runs through the emitted-output wrappers in the trace workflow; it is separate
from the deobfuscation pipeline.

The focused `emitted_property_writes` tests
show complete source strings and exact outcomes for the strict, linked-program,
and readable-program variants. These are distinct calls with distinct proof
requirements; an application should choose the one matching its output path.

The [linked-program property-write module](../../src/transforms/emitted_js/literals/property_writes/README.md)
maps the two program variants and their different assumptions.

The API is source-only:

```rust
pub fn simplify_property_writes(
    source: &str,
) -> Result<PropertyWriteCleanupOutcome, PropertyWriteCleanupError>;
```

`PropertyWriteCleanupOutcome` contains:

| field | meaning |
| --- | --- |
| `code` | rewritten source, or the input unchanged |
| `changed` | number of `defineProperty` calls rewritten |
| `preseeded_allocations` | allocation literals that gained prepopulated keys |
| `preseeded_keys` | keys added to those literals |
| `considered` | `Object.defineProperty`-shaped calls inspected |
| `skipped` | one typed `PropertyWriteSkip` per retained call |
| `status` | `Found`, `Unknown`, or `Unsupported` |
| `notes` | limits and the pass-wide decline reason, if any |
| `input_bytes` / `output_bytes` | byte counts |

## Native authority (fail closed)

The pass only trusts the **unshadowed global `Object.defineProperty`** and proves
that the input cannot replace or escape it:

- every bare unresolved `Object` reference must be the receiver of an
  `Object.defineProperty` member that is not an assignment/update/delete/loop
  target;
- bare `globalThis`, `window`, `self`, `global`, `top`, `parent`, `frames`, and
  every `this` expression decline the whole input, because they can reach the
  global object;
- `with`, any reference named `eval`, any unresolved `Function` reference, and
  any static or provably constant computed `constructor` access (which can reach
  the `Function` constructor) decline the whole input because they are dynamic
  code. An unknown computed member used as a call on a syntactically
  function-like value also declines the input: its key might be `constructor`,
  so spelling checks alone are not authority proof;
- a locally shadowed `Object` declines that call site, not the whole input.

Aliases such as `var D = Object.defineProperty; D(...)` are not rewritten.
Reading the native member is allowed (a read cannot replace the binding), so a
direct `Object.defineProperty` call elsewhere still rewrites.

For example, the strict test `rewrites_fresh_array_numeric_keys_after_preseed`
starts with `let a=[]`, defines index `1` with the exact writable data
descriptor, and returns `a.length`. The result pre-captures index `1` as an own
`void 0` data property (`let a=[,void 0,];`) before changing the call to
`a[1] = 42;`. That pre-capture matters: assigning into an array hole can observe
an inherited setter, while defining an own property does not. The test checks
one rewrite, one prepopulated allocation, one prepopulated key, no skipped calls,
and a reparsable result. A separate test rewrites existing own data properties
without prepopulation; it covers both `a[0]` and `o["a"]`.

## Which calls rewrite

A call is considered only when all of these hold:

1. The callee is exactly `Object.defineProperty(...)` or
   `Object["defineProperty"](...)` with three non-spread arguments and the call
   is a direct expression statement (the call result is discarded).
2. The descriptor is exactly a plain object literal that owns the four static
   data fields `writable: true`, `configurable: true`, `enumerable: true`, and
   `value: <expression>`. Spreads, accessors, methods, computed fields,
   duplicate fields, extra fields, and non-`true` flags are retained.
3. The key is a side-effect-free static string or number literal. The key is
   never an object with coercion hooks. `__proto__` is retained; array `length`
   is retained.
4. The receiver is a simple identifier bound to a fresh array or object literal
   that is a direct statement of the same block. Parameters, call results,
   member reads, `Object.create(null)`, and arbitrary existing objects are
   retained.
5. The receiver property is already a known own writable/configurable/
   enumerable data property of that literal, or it can be prepopulated.
6. The receiver binding is never reassigned, has no formal-parameter role
   (sloppy `arguments` aliasing), and every reference before the last rewritten
   write is the receiver of one of the same block's exact calls. Later
   references are allowed because the object then has the final shape the
   original calls produced. A program-level expression statement is retained
   because its call result is the script completion value; function and nested-
   block calls with discarded results remain eligible.

Evaluation count and order are preserved: the receiver identifier is evaluated
once, then the static key, then the value expression, then the write. Assignment
returns the value, so the call result must already be discarded.

## Safe prepopulation

`[]` or `{}` alone is not proof that a missing property can be assigned:
`Array.prototype` / `Object.prototype` may be patched, `__proto__` is special
for assignment, and array `length` reacts to numeric writes. The pass creates the
property **inside the original allocation literal** instead:

```js
// before
let a = [];                                        // also {} for objects
Object.defineProperty(a, 0, descriptor);
Object.defineProperty(a, 1, descriptor);

// after
let a = [void 0,void 0,];
a[0] = value0;
a[1] = value1;
```

- **Arrays:** numeric keys become `void 0` elements. Indices that were already
  holes stay elisions, so the final `length`, index layout, and hole set match
  the original. The literal is limited to
  `MAX_PRESEED_ARRAY_ELEMENTS` (4096) elements.
- **Objects:** missing keys are appended as computed properties
  (`{["name"]: void 0}`) in call order, preserving the literal's final property
  insertion order. Existing plus added keys are limited to
  `MAX_PRESEED_OBJECT_KEYS` (256).
- Parenthesized sequence expressions remain parenthesized when a value is
  spliced into an assignment or an existing array element is copied into a
  regenerated literal.
- Keys containing lone UTF-16 surrogates are retained. Oxc's cooked AST value
  is intentionally lossy for these literals, so declining avoids creating a
  second property under a replacement-character spelling.
- Pre-seeding is used only when every reference before the last write is an
  exact same-block call receiver. An earlier read, write, call argument, or
  closure capture declines the symbol. A receiver observed only after all
  writes is safe: the final shape is identical.

Objects/arrays with spreads, accessors, dynamic computed keys, or `__proto__`
are not prepopulated. They can still rewrite calls on keys that are already known
own data properties.

## Deliberate limitations

- No alias callee forms (`D(...)`, `.call(...)`, `Reflect.defineProperty`).
- Program-level direct calls are retained to preserve the JavaScript script
  completion value.
- No cross-block or cross-function dataflow. Writes inside a branch/loop are
  retained unless the allocation is in that same straight-line block.
- No runtime JavaScript engine, no capture-JS execution, and no synthetic
  evaluation of the rewritten program.
- Re-generating a prepopulated array literal drops comments inside that literal's
  element list. Comments are not runtime behaviour, but formatting inside the
  edited literal changes.
- Invalid input or an invalid splice returns `PropertyWriteCleanupError`; the
  pass fails closed and never returns a partial rewrite.

## Provenance and CLI integration

Every edit is expression-local: a call expression becomes an assignment
expression, or an allocation literal gains elements/properties. No statement is
added or removed, so the statement and node maps stay index-valid.

For artifact callers there are two helpers:

```rust
use shape::emitted_cleanup::{
    simplify_cleaned_emitted_javascript, simplify_emitted_property_writes,
};

let artifact = simplify_cleaned_emitted_javascript(&cleaned)?;
// artifact.code, artifact.statement_map, artifact.node_map,
// artifact.object_parameters, artifact.statement_map_status == Preserved
```

The integration contract for the CLI owner is: run
`cleanup_emitted_javascript` first, then
`simplify_cleaned_emitted_javascript` (or `simplify_property_writes` on the
cleaned code), and print/return the resulting `code` together with the copied
maps. The pass itself does not change `src/tracing/symbolic/`,
`src/vm_string/`, or the trace CLI; those are owned elsewhere.

## Repeated calls

The pass is idempotent: after a successful rewrite there are no matching
`defineProperty` calls on the rewritten receivers, so a second run reports
`changed == 0` and returns identical code.

## Validation on the saved emitted sample

The optional test `real_emit_sample_if_configured` reads the path in
`SHAPE_PROPERTY_WRITE_SAMPLE` and can write the result to
`SHAPE_PROPERTY_WRITE_OUT`:

```bash
SHAPE_PROPERTY_WRITE_SAMPLE=/path/to/main.before.js \
SHAPE_PROPERTY_WRITE_OUT=/tmp/main.after.js \
cargo test --release --test emitted_property_writes real_emit_sample_if_configured -- --nocapture
```

On the saved `main.before.js` capture used for this change (release build):

| metric | value |
| --- | --- |
| input | 113,829 lines / 4,485,916 bytes |
| output | 42,239 lines / 2,770,177 bytes |
| considered calls | 16,082 |
| rewritten calls | 14,318 |
| prepopulated allocations | 3,805 |
| prepopulated keys | 14,318 |
| retained calls | 1,764 (`non_literal_key` 1,494, `receiver_observed_before_writes` 270) |

The example requested by the emitter cleanup work becomes:

```js
// before (main.before.js line 8115 / 8161)
let trace_value_79504 = [];
// ...
Object["defineProperty"](trace_value_79504, 1, {
    writable: true, configurable: true, enumerable: true,
    value: trace_value_79529
});

// after
let trace_value_79504 = [void 0,void 0,];
// ...
trace_value_79504[1] = trace_value_79529;
```

The retained `non_literal_key` calls use a runtime `vmDecodeString` key; they
are left alone until a separate string-folding pass proves the key. The
retained `receiver_observed_before_writes` calls read or escape the receiver
before their last write, so they are intentionally not prepopulated.

# Linked program output

`simplify_program_property_writes` is a public API that assumes the generated
program runs with the standard `Object.defineProperty` intrinsic and standard
built-in prototypes. That assumption permits `array[2] = value` for a fresh
array without adding a placeholder element first. It does not apply to the
stricter standalone, thread, or corpus cleanup APIs.

It still requires an exact four-field data descriptor, a static string or
numeric key, and a fresh ordinary receiver that has not escaped or been
observed before the rewritten writes. It follows a narrow generated factory
shape: a zero-argument synchronous function creates distinct empty
arrays/objects and returns them as elements of a new array; a local call result
is indexed into local receiver bindings. `cleanup_thread` uses the same policy
(`simplify_thread_property_writes`). `shape trace program` does not run it
(its former `05-property-writes.js` stage changed nothing).

In each function's cleanup, the `06-readable-property-writes.js` stage lowers remaining direct
expression statements with exact four-field data descriptors, including
generated helper parameters and identifier keys. This is a readability choice
for generated programs: it assumes the receiver is an ordinary extensible
array/object with no own accessor or non-writable target property, the global
`Object.defineProperty` is standard, built-in prototypes are standard, and
dynamic keys are primitive and never `__proto__` or array `length`. Those facts
are not proved by this stage. A receiver with an accessor, a Proxy, a patched
intrinsic/prototype, or a special dynamic key can behave differently. The stage
does not run in `trace thread`, `trace corpus`, or the standalone strict API.
It retains non-data descriptors, shadowed `Object`, optional calls, calls whose
result is used, and statically known `__proto__` or `length` keys.

## Linked-program output

Two APIs extend the descriptor syntax above; `trace program` runs only the
second:

- `simplify_program_property_writes` broadens the strict receiver proof to a
  direct fresh `[]`/`{}` or a projection `pair[i]` of a zero-argument factory
  returning fresh allocations. It retains block/order and reference checks.
- `simplify_readable_program_property_writes` is for readable program output
  under standard-built-in assumptions. It assumes an ordinary extensible
  receiver without an own accessor or non-writable destination property and a
  primitive dynamic key. It excludes program-level calls because rewriting
  them can change completion values.

The readable stage is not part of strict `thread`/`corpus` cleanup. Neither
API makes its assumptions into recovered source facts. With stage dumping
enabled, each function's `pipeline/06-readable-property-writes.js` holds the
result, so the changes
under its policy remain inspectable.
See [program cleanup](emitted-program-cleanup.md#pipeline) for the full order
and the [local module guide](../../src/transforms/emitted_js/literals/property_writes/README.md)
for API ownership.
