# JavaScript inline operand separation

Terminology follows the canonical [architecture and VM variants](../architecture.md).

`js_operand_separation_from_report` analyzes one standardized
`PipelineReport`. It does not change `PipelineReport::output` or the output of
`shape deob`. It uses Oxc parsing and semantic `SymbolId` resolution; it does
not execute JavaScript or WebAssembly. Callers can request it independently.
Complete dispatch and the JavaScript VM prefix also use the report-backed
handoff during preparation, where an unproved required handler prevents that
consumer from proceeding.

The question is narrow: when a JavaScript handler reads bytes from the same
image used by dispatch, which reads are decoded operands, where do they begin,
and which recipe values do they supply? Compare these two handler shapes:

```js
// JavaScript VM: the handler decodes its own argument from the image.
function (context) {
  var slot = r[context.P] | r[context.P + 1] << 8;
  context.P += 2;
  context.stack[context.stack.length] = context.memory.readSlot(slot);
}

// WASM-dispatched VM: the interpreter supplies a decoded argument.
function (context, operand1) {
  var slot = operand1;
  context.stack[context.stack.length] = context.memory.readSlot(slot);
}
```

The first shape can yield a two-byte descriptor and replace the recipe's
decoder-local `slot` with `SemanticValue::Operand { position: 1 }`. The second
has no inline byte read to lift, but a WASM-dispatched report also lacks the
JavaScript dispatch proof, so the standalone report API marks its handlers
`Unresolved`, not `NotApplicable`. Its operand-to-handler connection belongs
to the WASM interpreter proof. The five paired source cases are in
[JS/WASM operand cases](../reference/js-wasm-operand-cases.md).

The result combines:

- `OperandDescriptor` evidence: handler-relative byte offset, width, endian,
  mask, JS signedness, PC delta, standardized-output spans, and the resolved
  bytecode input identity;
- `LiftedHandler`, which keeps the existing ordered `RecipeOperation` and
  `SemanticValue` model while replacing only direct decoder locals with
  `SemanticValue::Operand`; and
- captured helper/external dependencies identified by their Oxc declaration
  identity, rather than by a structural hash alone.

`bounded_decode_operands(image, handler_entry_pc, handler)` is a bounded helper
for these descriptors. It checks every range before returning and never mutates
the supplied image. Four-byte bitwise compositions use JavaScript Int32
semantics, so a high bit produces a signed `i64` result unless the source uses
zero-fill conversion.

## From report to a usable operand

The report supplies standardized JavaScript and one proven operation-table
identity. The extractor parses that output, resolves the bytecode declaration
and the context PC field, walks each handler in the proven table, and records
byte reads in source order. It lifts a template only when those reads can be
replaced by operand positions **and** the remaining recipe has no unknown
semantic value or unsupported step. The report-wide `coverage` is a count of
handler outcomes, not a count of decoded bytes or executed instructions.

| Result field | What to read from it |
|---|---|
| `source_hash`, `table` | Input/report identity and the proven table used for every handler. A different report's templates cannot be joined by matching numeric keys alone. |
| `input_identity` | Display name plus byte span of the actual bytecode declaration. The Oxc symbol relation is the proof; the name is only a readable label. `None` means no image binding was proved. |
| `entry_coordinate` | `AfterOpcode` when dispatch consumed the opcode first, or `AtOpcode` when a supplied handoff explicitly says it has not. `None` means the report-wide binding proof failed. |
| `LiftedHandler.handler_index`, `table_key`, `source_span` | The handler's table occurrence and its span in standardized output. Keep these beside the descriptor when inspecting a case. |
| `operands`, `pc_delta` | Per-operand byte layout and the handler's tracked final PC advance. `pc_delta` does not, by itself, prove the next PC of a handler that later jumps. |
| `captured_dependencies` | Captured helper/external bindings with declaration spans and roles; the proven bytecode binding is kept separately as `input_identity`. A same-shaped function body can refer to a different captured value. |
| `template`, `status`, `unresolved` | Ordered recipe, operand-lift outcome, and reasons retained when the proof stops. Template status and operand status answer different questions. |

Each `OperandDescriptor` has a **1-based** `position` used by the recipe,
`byte_offset` relative to the handler-entry coordinate, `width`, `endian`,
`signed`, optional `mask`, `source_spans`, and `input_identity`. Its own
`pc_delta` is the PC offset at the source read; the handler's `pc_delta` is
the final tracked advance. The span records where the read came from in
standardized JavaScript; it is not a span into the byte image. For a one-byte
field, endianness is immaterial even though the enum still has a value.

`bounded_decode_operands` takes the **handler-entry** PC, not necessarily the
opcode PC. It uses checked addition and range slices; a short image reports
`Truncated { position, required_end, image_len }`. An invalid descriptor or
offset overflow reports `Unsupported`. It returns a complete vector only on
success, with integer values in descriptor order. It does not dispatch an
opcode, run a handler, or infer which branch was taken.

The fragment below is a library integration example: `source` is `&str`,
`table_key` is `u32`, `entry_pc` is `usize`, `image` is a `Vec<u8>`, and the
enclosing function returns `Result<(), Box<dyn std::error::Error>>`.

```rust
use shape::{
    bounded_decode_operands, deobfuscate_with_report, js_operand_separation_from_report,
    OperandHandlerStatus,
};

let report = deobfuscate_with_report(&source)?;
let separation = js_operand_separation_from_report(&report)?;

let handler = separation
    .handlers
    .iter()
    .find(|handler| handler.table_key == table_key)
    .ok_or("no separated handler for that table key")?;

match handler.status {
    OperandHandlerStatus::Lifted => {
        // entry_pc is the handler-entry coordinate, not necessarily the opcode PC.
        let operands = bounded_decode_operands(&image, entry_pc, handler)?;
        for operand in operands {
            println!("position {} = {}", operand.position, operand.value);
        }
    }
    _ => {
        // Keep the original template and inspect why the lift stopped.
        eprintln!("{:?}: {:?}", handler.status, handler.unresolved);
    }
}
```

**Note:** select by `table_key`, not by the handler's position in
`separation.handlers`. The extractor currently gives `handler_index` the same
value as `table_key`; the fields remain separate, and `table_key` is the join
field. A sparse operation table makes the vector position differ from the key, so
selecting position 2 can open key 3.

## Proof boundary

For the JavaScript VM layout, the module proves the bytecode input from a dispatch
method that indexes the byte image using the context PC and increments that PC
when consuming the opcode. The PC field is linked to the proven `vmContext`
constructor by its `this.<field> = parameter` assignment; `context`, `P`, and
the byte-image spelling are never accepted as evidence by themselves. The
handler table is still resolved by the report's declaration ordinal and exact
shape.

Captures whose interpreter dispatch is in WebAssembly have no JavaScript
dispatch method, so this opt-in
module reports all handlers unresolved instead of treating WASM branch bytes as
JS handler indices. This is a proof-boundary result, not a claim that the WASM
backend's semantics are unsupported. A caller with independent binding proof can
provide the documented
`JsOperandBindingHandoff` (bytecode declaration span, proven PC property, and
entry coordinate); the module resolves that span back to an Oxc `SymbolId` and
does not load or duplicate runtime configuration.

The JS dispatch consumes the opcode first, so the normal entry coordinate is
`OperandEntryCoordinate::AfterOpcode`: byte offset zero is the first operand
byte. A non-constant PC assignment, decrement, byte-image write, mutable alias,
escape, shadowed context, unsupported control flow, or unknown call keeps the
handler partial/unresolved. Decoder reads and ordered effects are not moved
across calls or getters.

PC offsets are tracked in source evaluation order. A later `r[context.P]`
therefore observes the earlier `P++`/`P += N`, while `var saved = context.P`
stores a snapshot offset and does not follow later PC movement. Post/pre-
increment expressions used directly as byte indices are deliberately declined.
The bounded recipe rejects duplicate byte terms, incomplete byte coverage,
inner masks on one term of a multi-byte OR, and `>>> N` for nonzero `N`;
`>>> 0` is accepted only as unsigned coercion. These cases remain partial and
are never silently decoded as a different operand.

### Why source order matters

The [sequential-PC fixture](../../tests/fixtures/js-operands/sequential-pc.raw.js)
reads three fields while moving the PC twice:

```js
var first = r[context.P];  // entry + 0
context.P += 1;
var saved = context.P;
var second = r[saved];      // entry + 1, snapshot of P
context.P += 1;
var third = r[context.P];   // entry + 2
```

The descriptors have byte offsets `0, 1, 2`, while their read-time `pc_delta`
values are `0, 1, 2`. The handler's final `pc_delta` is `2`. A later move of
`context.P` does not change `saved`, and the third read does not become an
offset-0 read merely because its syntax again says `r[context.P]`. The test
also proves that a saved PC alias can be used after a move when its declaration
is a known snapshot. If the saved PC itself participates in an unsupported
recipe value, the whole handler can still be `Partial`; a certified byte
position does not certify every operation in that handler.

Consider the placement of the PC update relative to a memory read:

```js
var slot = r[context.P];
context.P += 1;
var value = context.memory.readSlot(slot); // order can be lifted

var slot = r[context.P];
var value = context.memory.readSlot(slot); // may throw before the update
context.P += 1;                           // cannot be moved before it
```

The second shape is `Partial` in the focused test. Moving `P += 1` upward
would change the PC visible if `readSlot` throws. The same rule matters for
stack writes and unknown calls; a later operand read may still have a proved
byte position, but the unknown call stays between recipe steps.

### Accepted syntax and deliberate declines

| Source expression | Current result | Reason |
|---|---|---|
| `r[P] \| r[P + 1] << 8` | Two-byte little-endian descriptor | Each byte contributes once at a covered shift. Here `P` abbreviates the proven `context.P` relation. |
| `r[P] \| r[P + 1] << 8 \| r[P + 2] << 16 \| r[P + 3] << 24` | Four-byte **signed** descriptor | JavaScript bitwise OR and shifts produce Int32; bytes `00 00 00 80` decode to `-2147483648`. |
| `(r[P] \| r[P + 1] << 8 \| r[P + 2] << 16 \| r[P + 3] << 24) >>> 0` | Four-byte **unsigned** descriptor | Zero-fill by zero changes the same bytes to `2147483648`. |
| `(r[P] & 1) \| r[P + 1] << 8` | Partial; no descriptor for this expression | An inner mask on one term changes the composed field's meaning. |
| `r[P] \| r[P]` | Partial; no descriptor | The same byte is used twice, so this is not a covered two-byte decode. |
| `r[P] >>> 1` or `r[context.P++]` inside the handler | Partial; no descriptor | Nonzero zero-fill shift and index-side PC update are outside the bounded decoder grammar. |

These are the actual accept/reject boundaries exercised by
`tests/js_operands.rs`. They do not describe every
possible JavaScript decoder spelling. A renamed `r` or `P` can still be
accepted if the symbol and context relation is proved; a variable literally
named `r` is insufficient without that relation.

`OperandHandlerStatus::NotApplicable` means no inline operand read was found.
It is separate from `Partial`/`Unresolved`, and the returned
`HandlerTemplate` is the original template, including its original status.
This keeps existing complete handlers outside the opt-in lifted path.

## Optional local coverage check

The optional integration test reads local inputs only when
`SHAPE_OPERAND_SEEDS_DIR` is set. It expects the input filenames configured by
that test; a newly fetched variant is a different measurement.
From the repository root:

```sh
SHAPE_OPERAND_SEEDS_DIR=/absolute/path/to/inputs \
  cargo test --release --test js_operands optional_actual_seed_coverage_reports_measured_counts -- --nocapture
```

The test reports template `executable / partial / unsupported` counts before
inline lifting and `lifted / partial / unresolved / not_applicable` counts after
it. Counts depend on the supplied inputs and are input-specific evidence, not a
current cross-provider coverage claim. The JavaScript VM categories describe
this extractor's result: some handlers have certified operand separation and
complete lifted recipes; others need no inline operand separation and retain
their complete original recipes. Partial cases retain unresolved behavior.
These categories must not be compared by subtracting the aggregate executable
count: certification now also rejects remaining unknown values and unsafe
effect/PC ordering.

The checks also compare shared semantic recipes: reading a memory slot,
writing a memory slot, and an ordered write/read composite. The same memory
and stack behavior can be represented by inline JavaScript byte reads or by
parameters supplied by a WebAssembly interpreter. Those pairings compare
recipes; they do not establish a runtime dispatch join or current coverage
for a newly acquired capture.

## Worked byte read

[`tests/fixtures/js-operands/inline-dispatch.raw.js`](../../tests/fixtures/js-operands/inline-dispatch.raw.js)
contains a compact dispatch proof and this handler:

```js
var first = r[context.P];
var second = r[context.P + 1] | r[context.P + 2] << 8;
context.P += 3;
return first + second;
```

The dispatch method has already consumed the opcode with `r[this.P++]`.
Therefore handler entry is **after** that opcode. The first descriptor is a
one-byte operand at relative offset 0; the second is a little-endian two-byte
operand at offset 1. Together they advance the handler PC by 3. The two
`SemanticValue::Operand` positions are 1 and 2. This is a proof of the
handler's byte reads and PC effect, not a claim that their numerical values
are constant across images.

The focused test calls `bounded_decode_operands` with image bytes
`[0x07, 0xff, 0xff, 0x99]` and entry PC 0. It receives operand 1 = `7` and
operand 2 = `65535`; the trailing `0x99` is not consumed. With only the first
two bytes available, decoding fails before returning a partial operand list,
and the image remains unchanged. This example is covered by
`tests/js_operands.rs`.

## Reading a failed lift

Inspect `OperandHandlerStatus` separately from the template status. A handler
can have a partial original recipe and a certified operand descriptor for a
particular read; a complete original recipe with no inline byte read is
`NotApplicable`, not a failed lift. `Partial` identifies a handler where some
byte relationships were found but unsafe syntax or ordering remains.
`Unresolved` means the image/PC/dispatch binding needed to anchor the reads
could not be proved. Keep the unresolved source span and reason when reporting
coverage. In particular, a WASM-dispatched capture cannot borrow its
JavaScript handler index from a WASM `br_table` branch number.

| Status | What a consumer may use | Example from the focused tests |
|---|---|---|
| `Lifted` | The descriptors and operand-lifted ordered recipe are complete for this handler. A caller still needs a valid image and entry coordinate to decode concrete values. | The inline-dispatch fixture yields u8 and u16le operands and an executable lifted recipe. |
| `Partial` | Keep proved fragments and `unresolved` reasons for inspection; do not execute the recipe as if the missing effects were absent. | A memory read before the PC update, an unknown call, or a duplicate byte term. |
| `Unresolved` | No certified operand lift; inspect the retained template and reasons separately. | A WASM-dispatched report has no JavaScript dispatch method to prove the bytecode symbol. |
| `NotApplicable` | Keep the original template unchanged. This status is meaningful only after the report-wide JS binding proof succeeded. | `function (context) { return 1; }` has no inline operand read. |

The four statuses are independent of *recipe admission*. In complete JS
dispatch preparation, a selector used by the proven dispatch table must have
`Lifted` operand status. Preparation joins the runtime source hash, table,
templates, and operand report, replaces only successfully lifted templates,
then checks those required selectors. A separate inspection path such as
[`ops show`](ops-inspection.md) keeps inventory/classification, extracted
recipe, recipe admission, and operand lift as distinct fields. A display row
with an operand width is evidence about that field; it is not proof that a
whole VM occurrence executed or that the displayed handler was selected by a
particular opcode.

## Near misses and top-level errors

Near misses stay in the handler row. An inner mask on one term, a duplicated
byte, a coverage gap, a nonzero `>>>`, width above four, or a PC update used as
the index all yield no descriptor and leave the handler `Partial`; the first five
also carry "unsupported or conflicting bytecode decoder composition".

| Error | When |
| --- | --- |
| `Parse` | Output does not parse, or template extraction fails |
| `NoProvenTable` | No `proven_operation_tables[0]` in `_and_templates` |
| `TableIdentityMismatch` | Templates from another table, or no handlers |
| `InvalidHandoff` | Span is not a declarator, or no proof found |

`InvalidHandoff` is also the internal discovery failure: without a handoff it
becomes an unresolved report; an asserted bad handoff is an error.

## Lifted recipes and extracted templates

`LiftedHandler.template` copies the extracted template and replaces decoder
machinery; the caller's report is untouched.

- A `Let` matching a direct operand declaration becomes
  `SemanticValue::Operand { position }`.
- A pure decoder subtree whose leaf spans match one descriptor is replaced in
  place, including a decoder used as a larger expression's index.
- Decoder-only `Unknown` steps, effect spans, declarations, and unreferenced PC
  snapshot lets are removed by exact span.
- Status is recomputed: `Executable`, `Partial`, or `Unsupported`.

An extracted recipe can stay partial even when lifting produces an admitted
lifted recipe. `Lifted` is not admission, and dispatch preparation merges only
`Lifted` rows, so a partial lift never overwrites a template.

## The binding handoff contract

`JsOperandBindingHandoff` carries `bytecode_declaration_span`, `pc_property`, and
`entry_coordinate`. It is for a caller that already proved
those facts, such as source-backed dispatch preparation. The module resolves the span to an Oxc `SymbolId`, copies the name
and symbol span into `input_identity`, and accepts the property and coordinate as
the caller's proof.

It does not skip parsing, table identity, template extraction, or the handler
walk; it does not prove an image, selector, or PC; and a wrong `pc_property`
matches no read, so it cannot certify another field. All in-repo handoff
producers supply `AfterOpcode`; `AtOpcode` is for a handoff that proves the
opcode entry, and the dispatch caller owns that PC.
