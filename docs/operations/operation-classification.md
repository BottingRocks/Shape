# Cross-capture operation classification

[`classify.rs`](../../src/operations/classify.rs) describes handler behavior in a
VM-independent vocabulary and groups that behavior across captures. A JavaScript
VM and a WebAssembly VM can implement the same stack effect with different
source shapes. Classification compares those abstract effects; table keys remain
local to their capture.

## The abstract machine

Each handler recipe is run against an abstract operand stack whose entry slots
are named inputs (`in[-1]` is the top). The machine does not execute JavaScript;
it symbolically tracks:

- which entry values a handler reads, writes, or truncates (`lowest`);
- the current stack length as a constant offset, or `None` once it depends on a
  non-constant value;
- temporary values and stack writes as expressions over those inputs;
- ordered side effects, including unknown ones;
- how many slots the handler pops and which values it leaves in their place.

At the end, every pushed value and side effect is a **component** named after
the JavaScript or VM operation it performs: `binary.greater_than`,
`unary.typeof`, `property.get`, `memory.write`, `vm.call`, and so on. A handler
with one component has that component as its kind. Several components become a
fused kind: the *component names* joined with ` + `, such as
`memory.write + memory.read` — never the source expression text. Anything the
machine cannot represent becomes `unrecognized.<reason>`.

```js
// Synthetic bounded case from tests/op_classify.rs (contract example, not a capture).
// Shown as an expression so the snippet parses on its own.
const handler = function (context, operand1, operand2, operand3) {
  context.memory.writeSlot(operand2, operand1);
  context.stack[context.stack.length] = context.memory.readSlot(operand3);
};
```

This handler's `kind` is `memory.write + memory.read` and its `nature` is
`Fused`. A single-action handler such as `context.stack.length -= 1;` is
`stack.pop` (`OpNature::Vm`), and a handler that adds the top two stack values
(the `+` operator case) is `binary.add` (`OpNature::JavaScript`).

## Kinds and scopes

`ClassifiedOp` carries `handler_index`, `kind`, `nature`, `pops`, `pushes`, and
the handler's source text. `OpNature` is `javascript`, `vm`, `fused`, or
`unrecognized`. `classify_seeds` groups kinds across captures and assigns each
kind an `OpScope`:

| Scope | Meaning |
| --- | --- |
| `common` | Seen in both the JavaScript VM and the WebAssembly VM |
| `javascript-vm-only` | Seen only in the JavaScript VM |
| `webassembly-vm-only` | Seen only in the WebAssembly VM |

Grouping is by kind name and returns `KindSummary` values in name order, each
with every `(capture position, handler index)` that produced it.

The classifier is available through `shape ops classify` and the Rust API.
This function-body example uses the Rust API: `js_source` and `wasm_source` are `&str`, and the
enclosing function returns `Result<(), Box<dyn std::error::Error>>`.

```rust
use shape::{classify_report, classify_seeds, deobfuscate_with_report};

// Each report must come from its own capture; kinds are grouped by name.
let javascript = classify_report(&deobfuscate_with_report(&js_source)?)?;
let webassembly = classify_report(&deobfuscate_with_report(&wasm_source)?)?;

for summary in classify_seeds(&[javascript, webassembly]) {
    println!(
        "{} ({:?}, {:?}): {} handlers",
        summary.kind, summary.nature, summary.scope, summary.handlers.len()
    );
}
```

## Which VM variant a capture is

`classify_report` first extracts templates and then attempts operand
separation. A capture is treated as the JavaScript VM only when separation
produces an entry coordinate, i.e. the dispatch method really reads the
bytecode. In that case the lifted template replaces the extracted one for each
handler, except `Unresolved` rows, so classification sees operand positions
instead of inline byte reads. Otherwise the classifier assigns the WebAssembly
VM variant. This fallback is
an analysis convention, not proof that the interpreter is WebAssembly: a missing
JavaScript dispatch proof can also mean unresolved source. The chosen variant
is exposed as `SeedClassification.variant`; use kernel and dispatch evidence
before choosing an execution path.

## What classification is and is not

A shared kind is a behavioral similarity under this abstract model. It is not a
shared table key, not a shared handler identity, not proof of semantic
equivalence, and not execution authority. An `unrecognized.*` kind is an
honest limit of the machine, not a claim that the handler is malicious or
unusual. The public helpers `classification_json` and
`classification_markdown` serialize `NamedSeed` inputs for reports; like the
catalogue markdown, they are presentation, not a versioned execution contract.

See the [operation catalogue](operations.md) for inventory and fingerprints,
[handler templates](handler-templates.md) for admission, and the
[operations module](../../src/operations/README.md) for API ownership.
