# Operation-table presentation

The readable kernel can print a proven `vmOperations` array as a numeric-keyed
object. Keys retain their capture-specific indices, and handler functions remain
anonymous. An index identifies a handler in one kernel, not a semantic name
across captures.

Array-to-object presentation must preserve handler references and call receivers. A handler that observes the table through `this`, including a nested arrow capture, is unsupported for this conversion. Array length, prototype behavior, iteration, writes, and escaping table references require explicit evidence or a diagnostic decline. Computed reads must use a supported numeric-index form.

WASM import namespaces retain their original names and the same function references. Presentation does not modify module bytes or import/export link names. The implementation and negative cases are in `src/transforms/deob/operation_table.rs` and `tests/operation_object.rs`.

## What changes in a readable kernel

The input may contain an array whose element position is the capture's dispatch
key. When the pipeline proves that table and its uses, the readable output
uses an object with the **same numeric keys**:

```js
// Source-shaped input
function dispatch(context, index, operand) {
  var operations = [
    function (context, value) { return value + 1; },
    function (context, value) { return value - 1; }
  ];
  return operations[index](context, operand);
}

// Readable presentation of the proven table and access
function dispatch(context, index, operand) {
  var vmOperations = {
    0: function (context, operand1) { return operand1 + 1; },
    1: function (context, operand1) { return operand1 - 1; }
  };
  return vmOperations[index](context, operand);
}
```

This illustrates presentation, not a standalone input that establishes proof
by itself. The pipeline uses the table's binding and exact shape, not the name
`operations`. An array with a hole does not have its keys renumbered to fill
the hole; a numeric-key object keeps its original keys. `handler_index` is a
row coordinate, while `table_key` is the dispatch key. Do not substitute one
for the other when a sparse table makes them differ.

The `operation_object` tests exercise large
synthetic tables, including 201- and 241-handler cases. For each, the output
reparses, contains a numeric-keyed `vmOperations` object, preserves the highest
original key, and reports the same operation count through extraction. Negative
cases cover receiver-sensitive uses and other array behavior whose object
replacement would change JavaScript semantics.

To see an actual seven-handler table in the readable output, use the
[built `shape` executable](../../README.md#build-once) from the repository root:

```sh
shape deob tests/fixtures/ops-inspection/handlers.raw.js \
  --output /tmp/shape-ops-readable.js
shape ops list tests/fixtures/ops-inspection/handlers.raw.js \
  --format markdown
```

The first file contains a `var vmOperations = {` declaration whose first key
is `0`; the second command lists keys and bounded classifications from the
same fixture. `ops list` is an inspection view, so a listed function need not
have an executable recipe. Use `ops show --op 0` to inspect one key's
micro-ops, recipe, and operand status before using it as an execution input.
The fixture is intentionally small; the larger synthetic-table cases are
covered by the focused tests rather than dumped into this guide.

`shape deob` may perform this presentation step while producing readable
JavaScript. `shape ops list` and `ops show` subsequently inspect the proven
table through the report; their key means the local dispatch key, never a
cross-capture operation name. `shape kernel show --op KEY` filters that same
proven key in the compact properties view. The source-free `shape report`
artifact retains the information needed by the bounded tracer separately;
changing this JavaScript presentation does not rewrite WASM module bytes.

## Sparse keys and declined conversions

The sparse case in `tests/operation_object.rs`
starts with handlers at array positions 0, 2, and 3:

```js
var operations = [function (context) { return 10; }, ,
                  function (context) { return 12; },
                  function (context) { return 13; }];
function dispatch(index) { return operations[index](null); }
```

The readable object has keys `0`, `2`, and `3`; there is no key `1`. The test
also runs deobfuscation on its own output and checks byte-for-byte idempotence.
This matters to both `ops show --op 2` and operand/dispatch joins: compacting
keys would silently change which handler an opcode selects.

The same test suite gives three distinct reasons to retain the array and emit
`OPERATION_OBJECT_DECLINED`:

```js
consume(operations.length);                   // Array length is observable.
operations[a] = function (context) { return 5; }; // Computed write.
return this.length + value;                    // Handler observes call receiver.
```

These lines come from separate test inputs. An object with numeric properties
has different `length`, computed-write, and receiver behavior from an array.
A computed *read* through a proven numeric dispatch index can be converted;
`operations["length"]`, `operations["toString"]`, and
`operations["custom"]` cannot. The suite also accepts the proven selector
`operations[arguments[arguments.length - 1]]` and declines a same-spelled
sibling binding, so the rule depends on binding identity and index proof,
not how the variable happens to be named.

A declined conversion still leaves an inspectable operation-table fact when
extraction proved it; the presentation pass does not promote a questionable
rewrite to a semantic proof. The catalogue and handler template status must
be read separately from whether the printed kernel uses an object. For a
WASM capture, imported function references and their ABI names additionally
have to remain identical; [the boundary inventory](../reference/wasm-boundary.md) explains
why an import named `"7"` is not automatically operation key 7.
