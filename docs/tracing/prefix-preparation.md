# JavaScript VM prefix preparation

This guide explains the source proofs that construct a JavaScript VM
`PrefixPreparation`. It complements the [prefix contract](javascript-vm-prefix.md)
and the short [module guide](../../src/dispatch/prefix/README.md).

Use [WASM prefix preparation](wasm-prefix.md) for the interpreter lift,
[kernel artifacts](../deob/kernel-artifacts.md) for portable preparations, and
[prefix execution](prefix-execution.md) for walking an admitted invocation.
Preparation establishes the available execution model; individual walks can
complete or stop depending on the entry, inputs, recipes, and limits.

## Preparation (`prefix/prepare.rs`)

`prepare_prefix_from_report` first offers the report to the WASM path
(`wasm::prepare_wasm_prefix`); a JavaScript kernel continues in `prepare_prefix`.
The JavaScript path reuses the complete-dispatch proofs and adds the prefix-only
ones. In order:

1. **Exactly one dispatch table** (`Extracted` `DispatchTable`), parsed and bounded
   by `validate_table` — the same state/opcode/bounds discipline as complete
   dispatch.
2. **Prove the read, bind the table.** `proof::prove_dispatch_read`,
   `require_binding_at_span`, `prove_immutable_binding`, and the binding-name
   match — by `SymbolId`, never by spelling.
3. **Extract the image** (`extract_image`): either bytes from the proven base64
   decoder call joined to the exact declaration (`require_decoder_call_binding`)
   or a bounded numeric-literal array. The image must be nonempty and ≤
   `max_image_bytes`.
4. **Exactly one operation table**, re-checked by its code-generation-stable
   declaration ordinal (`validate_operation_table_declaration`).
5. **Recipes + operand descriptors**, joined through the operand binding handoff
   (`merge_lifted_templates` folds in the lifted templates).
6. **Prepared indexes** (`template_index`, `operand_index`).
7. **Table identity agreement** across templates, operands and the proven table.
8. **Prove the patch handler** (`prove_patch_handler`) — exactly one bounded
   bytecode-write handler.
9. **Prove the run loop** (`prove_run_loop`) — the exact topology, joined to the
   dispatch method, operation table and constructor.
10. **Constant string tables** registered on every trace session.

`prepare_prefix_from_saved_report` carries the report's declared raw-capture
identity through while rebuilding every structural proof against the standardized
artifact output.

### Preparation internals: what each step refuses

The ten steps in the previous section are worth reading as a decision table,
because most preparation failures are one of these refusals. The middle column
is the function that decides; the right column is the condition, not the error
string.

| Step | Decider | Refuses when |
| --- | --- | --- |
| 1 | artifact filter + `validate_table` | Zero or several `Extracted` dispatch-table facts; an empty table; an empty state row; a cell count over `max_table_cells`; a `next_state` outside the table; a `next_state` or `selector` over `max_integer` |
| 2 | `prove_dispatch_read`, `require_binding_at_span`, `prove_immutable_binding` | The nested read is not the proven shape; the extracted span does not join the proven table binding; the binding is reassigned; the extracted binding name differs from the proven name |
| 3 | `extract_image` | The image binding is declared zero or several times; the initializer is neither a span-joined decoder artifact nor a bounded numeric-literal array; the image is empty or over `max_image_bytes` |
| 4 | `single_operation_table`, `validate_operation_table_declaration` | No single operation table, or its declaration ordinal does not match the report |
| 5 | `handler_templates_from_report`, `js_operand_separation_from_report_with_handoff_and_templates` | A template cannot be built; operand separation cannot join through the proven handoff; `merge_lifted_templates` finds a conflicting lifted step |
| 6 | `SelectorIndex::from_handler_indices` | Never fails; it builds the selector -> first-position index once, from the handler order |
| 7 | identity comparison | Templates and operands name different operation tables, or either differs from the proven one |
| 8 | `prove_patch_handler` | No write, more than one writing function, more than one write, a body that is not the four-statement template, a non-distinct temp, an image alias or escape, a write outside the handler, a `delete` |
| 9 | `prove_run_loop` | No run loop, more than one, a wrong declaration/guard/dispatch/handler join, a receiver not from the constructor, a mutated sentinel, a register write outside the allowlist, a constructor that is not the complete `this.member = value` shape |
| 10 | `constant_string_tables(&config)` | Nothing here; an absent table simply means no constant table is registered |

Two details in step 1 are easy to misread. `validate_table` rejects an **empty
row**, but rows can have different lengths; an opcode beyond its row stops
at execution. The bounds check uses the *proven states*, not the handler
count: a cell may name a selector that no template covers, which becomes an
`UnsupportedHandler` stop at walk time rather than a preparation failure. The
table's shape and the recipes' coverage are separate claims.

### The image: two admissible initializers

`extract_image` accepts exactly two shapes, and they are not interchangeable:

~~~text
bytecode binding
   ├── bytes from the runtime-config artifact whose span equals the initializer span
   │     -> require_decoder_call_binding(source, span, symbol, "bytecode image")
   └── an array literal of finite integral numbers in 0..=255
         -> every element must be a NumericLiteral; one bad element rejects the array
~~~

The decoder path is span-joined: the artifact's `bytes` are trusted only when
`artifact.span.start == init_span.start` and `artifact.span.end == init_span.end`
for the exact declarator, and only one such artifact may exist. The literal path
needs no artifact, but it is not a decoder: it is the bytes written in the
source. The tracked fixtures use the literal path, which is why they can be
edited byte by byte in a test.

The finder that produces `init_span` is a small Oxc visitor
(`BytecodeDeclaratorFinder`). It counts declarators whose binding identifier is
the proven symbol and whose initializer exists, and it records a literal only
when *every* element parses as a byte. A single `.5`, `-1`, `256` or non-literal
element leaves `literal = None`; if no decoder artifact matches either, the
preparation fails with "neither a proven decoder literal nor a bounded numeric
array". That message is precise: the code has two doors and the input fits
neither.

### Handoff, spans and indexes

Three joins deserve their own note because they are the reason preparation is
not just "collect facts".

1. **The operand handoff.** `prove_dispatch_read` returns a `handoff()` that
   tells operand separation where the bytecode read happens. Operand
   descriptors are lifted against that handoff plus the already-built templates,
   which is why `js_operand_separation_from_report_with_handoff_and_templates`
   takes both. A descriptor that agrees with the templates but not the handoff is
   a hard error, not a warning.
2. **Handler spans.** Immediately after templates and operands are built,
   preparation creates `(handler_index, OperandSpan)` pairs from each template's
   `source_span`. Both the patch-handler proof and the run-loop proof consume
   that list. The patch handler is located by matching its *function span* to a
   handler span; the run loop is accepted only if every completion-register write
   is inside the run loop span or one of those handler spans. One list, two
   proofs, no second source of truth.
3. **Selector indexes.** `SelectorIndex::from_handler_indices` is built once per
   report and stored on the preparation. The walker never scans a handler list
   per step; it looks up the first position for a selector. Duplicate selectors
   therefore resolve deterministically to the first entry, and the index order is
   the report's order, not sorted order.

### What preparation deliberately does not do

| Not done | Why |
| --- | --- |
| Validate the entry or descriptor | They are caller inputs; only `thread()` sees them |
| Clone or retain `source` | The preparation must be cheap to hold and safe to share |
| Execute any handler | Execution is `execute::run_prefix`, under walk limits |
| Decide that the capture is complete | The prefix contract covers a subset by construction |
| Normalize or rewrite the image | The image is the proven snapshot; patches happen only per invocation |
| Fall back to a name-shaped handler when a recipe is missing | A missing handler is a stop, never a guess |

### A note on the saved-report entry point

`prepare_prefix_from_saved_report` does **not** offer the report to
`wasm::prepare_wasm_prefix`; it goes straight to `runtime_config::from_saved_report`
and then to the JavaScript `prepare_prefix`, which requires exactly one
`Extracted` dispatch-table fact. A WASM-dispatched kernel has no JavaScript
dispatch table, so it must be prepared from raw source, or from a
`KernelArtifact` that was compiled from raw source and carries its `wasm` field.
The artifact path preserves that field across `to_json` / `from_json`, so a
source-free WASM runtime is still available — just not through the saved-report
free function. This asymmetry is a property of the two entry points, not a bug
in either.

## Run-loop proof (`prefix/proof.rs`)

`prove_run_loop` recognizes exactly one run loop:

```js
var v, O;
for (;;) {
  if (A !== B) { O = A; A = B; return O; }   // the completion guard
  v = ctx.q();                                // the proven dispatch method
  if (ctx.G.length === 0) ops[v](ctx);        // the proven operation table
  else helper(ops[v], ctx);                   // the exception-helper alternate
}
```

It proves, by `SymbolId`:

* the declaration is exactly **two distinct uninitialized bindings**, whether
  written as one `var` statement or two (`declared_loop_temps`);
* the completion guard is exactly `if (A !== B) { C = A; A = B; return C; }`;
* the dispatch is `selector = context.<method>()` with no arguments;
* the handler join is exactly the guarded operation call plus the exception-helper
  alternate, with the operation member and the context receiver resolved by
  symbol;
* the run-loop **receiver originates from the proven constructor**
  (`prove_consumer_context_origin`) — a method of the right *spelling* on some
  other object is never enough.

`prove_run_loop_provenance` then proves the completion protocol: the not-complete
sentinel is declared once and never mutated; the completion register is declared
once and initialized to exactly that sentinel binding; and register/sentinel writes
occur only in the run-loop guard or a proven operation-table handler.

`prove_constructor_empty_stack` validates the **complete** constructor body: every
statement must be a narrow `this.member = value` assignment, exactly one assigns
the guarded exception-stack field, and that value is either `[]` or a source-proven
zero-argument fresh-stack helper. Other fields may read plain values, call only a
proven fresh-stack helper, use the intrinsic unshadowed `Object(receiver)`
coercion, or build simple comparisons/conditionals from those. No `this` value
escapes and no assignment/update/sequence, computed mutation, alias, early return
or control flow is admitted, so a later `this.G = [1]`, `this.G.push(1)`,
`this.G.length = 1`, alias, return of a different object, or unknown call fails
closed.

## Bytecode-write handler proof

`prove_patch_handler` requires exactly one bounded write handler whose body is the
four-statement template:

```js
var a = image[pc] | (image[pc + 1] | image[pc + 2] << 8) << 8;
var b = image[pc + 3];
pc += 4;
image[a] = b;
```

`WriteScan` rejects a top-level write, an alias/escape of the image into a callee,
more than one writing function, or more than one write. The handler is recognized
by its body and `SymbolId`s — never by a selector constant, name, address, hash or
capture value. An `ImageReferenceScan` whitelist then requires that every other
reference to the image be a computed read (a byte value, not an alias), rejecting
rebinding, `delete`, object/array/conditional aliasing, and escapes that a
member-write scan alone would miss.

### The near-miss matrix

These are the shapes the tracked tests prove are rejected, with the message they
expect. They are a good first checklist when a capture fails preparation with a
patch-handler error.

| Mutation | Expected message fragment |
| --- | --- |
| handler is a generator function | "async or generator" |
| address and value temps resolve to one binding | "distinct fresh bindings" |
| address temp shadows the context parameter | "distinct fresh bindings" |
| write targets a different array `s` | "no bounded bytecode-image write handler" |
| an extra `context.G.push(9);` in the handler | "four-statement template" |
| the prototype alias is reassigned (`x = null;`) | "not uniquely defined" |
| a shadowing `var r = [];` inside the handler | "no bounded bytecode-image write handler" |
| a simple alias `var y = r;` | "aliased or escapes" |
| the image escapes into a callee (`use(r);`) | "aliased or escapes" |
| a second function writes the image | "2 distinct functions" |
| `r = [];` rebinds the image | "identifier is rebound" |
| `delete r[0];` | "deleted" |
| an object holder aliases the image | "identifier is rebound" |
| a transitive alias writes through | "aliased or escapes" |
| `r["fill"](1)`, `new r["constructor"](1)`, ``r["fill"]`x` `` | not a snapshot read; the run-loop/image join fails |
| `BD["fill"]([]);` before `vmRun` | the dispatch-table read is not the proven shape |

The last two rows are worth separating: they do not fail the write scan at all.
They fail because a method call on the image or the table makes the *nested
dispatch read* proof see a call it cannot account for. Different proof, same
practical outcome: preparation refuses.

## Fresh-stack helper (`fresh_stack.rs`)

`fresh_stack.rs` is the standalone proof for the fresh-empty-array helper the
constructor may call. It is deliberately independent of the crate's analysis
pipeline (it depends only on `oxc_*`), so `tests/fresh_stack_proof.rs` can include
the exact file through `#[path]` and call it against its own parsed program. It
matches the body statement by statement — exactly one `var stack = [];`, zero or
more exact
`Object.defineProperty(stack, "<native>", { value: Array.prototype.<native> })`
decorations (a subset of `pop`/`push`/`slice`/`splice` in any order), and one final
`return stack;` — plus the simpler `return [];` and `var stack = []; return
stack;` forms. Nothing is skipped, so member writes, arbitrary calls or aliases,
nested/early returns, an extra statement, or a side-effecting initializer all fail
closed.
