# JS inline operand separation: JavaScript VM and WebAssembly VM paired cases

This is a source-backed comparison of five handler **meanings** that occur in the
tracked JavaScript VM and WebAssembly VM references. In the JavaScript VM, each
handler reads its own operand bytes. In the WebAssembly VM, the interpreter
reads those bytes and passes the decoded numbers as handler parameters. The
ordered memory and stack work can match even though the decoder lives on a
different side of the call.

The WebAssembly reference imports 12 functions. That is a property of the
tracked module, not a VM type or admission rule. Module identities stay with the
provenance below.

The evidence is static: standardized `shape` output, decoded interpreter modules
inspected with `wasm2wat`, and the checked-in paired-case fixture. It does not
execute these references or prove which `br_table` case reaches any listed
handler key. The machine-readable bodies, spans, normalized forms, and operand
examples are in
[tests/fixtures/js-operands-reference/paired-cases.json](../../tests/fixtures/js-operands-reference/paired-cases.json).

**What is paired:** handler source shapes, operand roles, byte layout, and ordered
effects. **What is not joined:** WASM branch selector to catalogue handler
occurrence, runtime PC/state to a taken branch, or handler keys across references.
The original numeric keys in the table below are local catalogue coordinates, not
common opcodes. This distinction matters most for duplicate handler bodies and for
two members of the same family with different operand widths.

## 1. The two operand origins

**JavaScript VM reference — handler entry `pc` is already past the opcode.**
Dispatch is `javascript_vm.std.js:13719`:

```js
var O = BD[this.f][r[this.P++]];   // r[P] is the opcode; P then points at operand 0
this.f = O[0];
return O[1];                       // handler key; vmOperations[v](context) at 16139
```

So handler entry `P = opcode_pc + 1`, operand *j* is at `P + offset_j`, and the
handler advances `context.P += width`. JavaScript VM reference handlers are
`function(context)` (arity 1) and carry the bytecode array `r` as a captured
binding.

**WebAssembly VM — interpreter `pc` is opcode-relative.** At the loop top the
interpreter sets `local0` to the byte offset and reads the opcode at offset 0
(`local.get 0; i32.load8_u`). It then indexes the linear image table and
`br_table`s. In each case it reads operands from `local0 + offset`, where `offset`
starts at 1 (opcode is offset 0). A straight-line case advances `local0` by the
**whole instruction length**: one opcode byte plus the operand bytes. In the
paired records, `pc_delta` counts only the operand bytes. The interpreter with 12
imports contains this case-site shape:

```
local.get 2 ; local.get 0; i32.load8_u offset=1 ; ... ; local.get 3 ; local.get 8
if call 5 else call_indirect (type 3)   ; guarded (f.m = vmInvokeOperation) vs direct
i32.const <instruction byte length> ; local.get 0 ; i32.add ; local.set 0
```

`i32.load16_u` is a 2-byte little-endian read. The WebAssembly reference uses
image base 170,845 and stride 468 (=2×234). The image entry is `{low byte =
br_table selector, high byte = next dispatchState}`. The selector is passed as the
last call argument (the handler-key ABI), but **the case/handler-table-index →
catalogue handler-index join is UNPROVEN** and is not asserted here. The two
origins therefore agree on the absolute operand byte (`opcode_pc + 1 + offset`),
but differ on the stored `pc`: the JavaScript VM stores the post-opcode pc, WASM
stores the opcode pc and adds `offset + 1`.

The tracked JavaScript VM reference has no WASM interpreter module. Module
identities are kept with the [fixture
provenance](#8-fixture-provenance-and-how-to-read-it) so the PC explanation stays
focused on the two coordinate systems.

## 2. The five paired cases (original numeric keys retained)

| Pair (family) | JavaScript VM reference key / span | WebAssembly VM reference key / span | JavaScript VM reference decode → pc_delta |
|---|---|---|---|
| `readslot_push_u16le` | h9 `227493..227674` | h52 `208752..208893` | `r[P] \| r[P+1]<<8` → 2 |
| `writeslot_stacktop_u8` | h14 `229844..230038` | h22 `196304..196482` | `r[P]` → 1 |
| `writeslot_then_readslot_u8x3` | h64 `248043..248322` | h129 `234186..234453` | `r[P]`,`r[P+1]`,`r[P+2]` → 3 |
| `writeslot_array_then_readslot_mixed_width` | h190 `295081..295370` | h181 `254847..255098` | `r[P]`, `r[P+1]\|r[P+2]<<8` → 3 |
| `push_array_value_readslot_u8x2` | h216 `303536..303864` | h111 `228462..228776` | `r[P]`,`r[P+1]` → 2 |

The blocks below show the decisive lines, with local names preserved from the
recorded bodies. They are excerpts for comparison; the complete function sources
and normalized forms are in the JSON fixture. The `h` numbers are local table
occurrences, not shared opcodes.

### Pair 1: read a two-byte slot index, then push its value

```js
// JavaScript VM reference h9
var value1 = r[context.P] | r[context.P + 1] << 8;
context.P += 2;
context.stack[context.stack.length] = context.memory.readSlot(value1);

// WebAssembly VM reference h52
var value1 = operand1;
context.stack[context.stack.length] = context.memory.readSlot(value1);
```

The operand role is `slot_index`, not a value to push directly. Bytes `[0x34,
0x12]` mean slot `4660`; the handler pushes the *value stored in that slot*. The
JS handler forms the integer by OR/shift. The WASM interpreter's `i32.load16_u
offset=1` forms it before the handler call. `readSlot` occurs after decoding in
both cases and may have its own observable failure; the source comparison does not
replace it with a byte read.

### Pair 2: write the stack top to a one-byte slot index

```js
// JavaScript VM reference h14
var value1 = r[context.P];
context.P += 1;
context.memory.writeSlot(value1, context.stack[context.stack.length - 1]);
context.stack.length -= 1;

// WebAssembly VM reference h22
var value1 = operand1;
context.memory.writeSlot(value1, context.stack[context.stack.length - 1]);
context.stack.length -= 1;
```

Byte `0x2a` chooses slot `42`. The stack top is read, written to that slot, and
only then removed by shortening the stack. The paired fixture keeps the handler's
table key so the contract remains tied to the source coordinate.

### Pair 3: write one byte as a value, then read a third byte as a slot

```js
// JavaScript VM reference h64: value, target slot, source slot
var value1 = r[context.P];
var value2 = r[context.P + 1];
var value3 = r[context.P + 2];
context.P += 3;
context.memory.writeSlot(value2, value1);
context.stack[context.stack.length] = context.memory.readSlot(value3);

// WebAssembly VM reference h129 receives operand1, operand2, operand3.
context.memory.writeSlot(operand2, operand1);
context.stack[context.stack.length] = context.memory.readSlot(operand3);
```

For `[0x07, 0x08, 0x09]`, this writes number `7` to slot `8` and pushes the value
read from slot `9`. All three fields are `u8`, but their *roles* differ: the first
is a value, the next two are slot indices. A comparison based only on width would
lose that distinction. The order also matters if the target and source indices are
equal: the read occurs after the write.

### Pair 4: same broad family, different field boundaries

```js
// JavaScript VM reference h190: target slot u8, source slot u16le
var value1 = r[context.P];
var value2 = r[context.P + 1] | r[context.P + 2] << 8;
context.P += 3;
var value3 = [];
context.memory.writeSlot(value1, value3);
context.stack[context.stack.length] = context.memory.readSlot(value2);

// WebAssembly VM reference h181 receives operand1, operand2.
var value3 = [];
context.memory.writeSlot(operand1, value3);
context.stack[context.stack.length] = context.memory.readSlot(operand2);
```

Bytes `[0x05, 0x34, 0x12]` mean target slot `5` and source slot `4660`. The fresh
array is the written value; it is not a third encoded operand. Pair 3 uses the
same broad `slot_write_then_read` family and also consumes three bytes, but
divides them into **three** `u8` fields. Equal PC deltas and similar helper calls
do not prove equal operand layouts. Treating pair 4 as three bytes would read slot
`0x12` instead of `0x1234` while still advancing to the correct next PC.

### Pair 5: push an array, a byte value, and a slot value in that order

```js
// JavaScript VM reference h216. The WebAssembly reference h111 receives two parameters.
var value1 = r[context.P];
var value2 = r[context.P + 1];
context.P += 2;
var value3 = [];
var value4 = context.stack.length;
context.stack[value4] = value3;
context.stack[value4 + 1] = value1;
context.stack[value4 + 2] = context.memory.readSlot(value2);
```

For `[0x03, 0x04]`, the second pushed value is literal number `3`; the third is
the value read from slot `4`. The first pushed value is a *new array*. The WASM
handlers have the same three stack writes after parameter substitution. Calling
this merely “push a slot value” would miss both the fresh-object identity and the
stack permutation.

## 3. Concrete decode example (pair 1, u16 little-endian)

`opcode_pc = 1000`, opcode byte unresolved (join unproven). Operand copy of the
bytecode at `opcode_pc+1..+2` is `[0x34, 0x12]`.

| | JavaScript VM capture | WebAssembly VM interpreter |
|---|---|---|
| origin stored | entry `P = 1001` | `local0 = 1000` |
| read | `r[1001] \| r[1002]<<8` | `i32.load16_u offset=1` |
| decoded | `value1 = 0x1234 = 4660` | operand1 = 4660 |
| next origin | `P = 1003` | `local0 = 1003` |

Examples for the other pairs (fixture `operand_mapping[].example`): pair 2
`[0x2a]`→42, next 1002; pair 3 `[0x07,0x08,0x09]`→7/8/9, next 1004; pair 4
`[0x05,0x34,0x12]`→5 and 4660, next 1004; pair 5 `[0x03,0x04]`→3/4, next 1003.
Each mapping records `part`, `width`, `offset`, `endian`, the JavaScript VM
capture local, the WebAssembly VM parameter, `abs_byte_positions` and `wasm_load`.

This is also the safe way to inspect a new candidate pair: establish the handler's
source span and table key, identify every operand part and its byte range, compare
the ordered helper/stack effects, and only then compare the WASM parameter list. A
matching `pc_delta` is a useful consistency check, but it does not supply the
field partition. A matching normalized body is a shape comparison; it does not
establish captured-binding identity or a dispatch-branch join.

## 4. Semantic part → operand mapping

| Pair | semantic parts in order | JavaScript VM capture locals | WebAssembly VM params |
|---|---|---|---|
| 1 | slot_index | `value1` | `operand1` |
| 2 | slot_index | `value1` | `operand1` |
| 3 | value, slot_index_target, slot_index_source | `value1`,`value2`,`value3` | `operand1`,`operand2`,`operand3` |
| 4 | slot_index_target, slot_index_source | `value1`,`value2` | `operand1`,`operand2` |
| 5 | literal_value_pushed, slot_index_source | `value1`,`value2` | `operand1`,`operand2` |

The memory helper (`readSlot`/`writeSlot`) is the identical shared body in both
tracked references. The pairing matches the JavaScript VM's inline fetch to the
WebAssembly interpreter's parameter substitution, with the same helper call and
stack effect.

## 5. Negative / stress cases from the paired references

- **Bytecode mutated.** Changing operand bytes changes decoded values but not
  width or `pc_delta` (pair 1: `[0xff,0x00]` → 255, still 2/2). Changing the
  **opcode** byte changes `BD[dispatchState][opcode]` / the image-table entry, so
  the operand contract must be keyed to the dispatched opcode, not the byte
  position; the mutated handler is not inferred (join unproven).
- **Explicit `pc` branch/jump.** JavaScript VM reference h229
  `308680..309274` decodes 6 bytes, then
  `context.I.push({p:context.P,A:context.f}); context.P = value3; context.f =
  value4;`. `pc_delta` is not a straight-line increment; `next_pc` is a decoded
  operand. A decoder model must compose both.
- **Decode interleaved with an unknown call.** JavaScript VM reference h188
  `294340..294912` decodes u16le+u8, then `vmCreateThread(...)` (possible
  throw), then a computed call (possible throw), then `readSlot(value2)`. Order
  is `decode → call → memory`; three unknown fragments and
  `calls_unknown_possible_throw=true` are retained.
- **Captured binding differs despite identical canonical shape.** Canonical
  `function(b1) { b1.stack[b1.stack.length] = b0; }` appears with two different
  captured constants: JavaScript VM reference h77 `253123..253211` pushes
  `vmNativeFunctions`, and JavaScript VM reference h165 `286273..286351` pushes
  `vmEntry`. The WebAssembly reference has the same shape at h65/h149. Canonical
  equality is a structural metric, not semantic equality.

## 6. Recipe order, exceptions, and the deliberate signal

Recipe order is source order with a deterministic span per step
([handler-templates.md](../operations/handler-templates.md)): the decode step precedes the
memory/call step, and a `calls_unknown_possible_throw` call keeps its position; an
unsupported fragment is an extractor gap, not an unknown VM purpose (user-
confirmed semantic parity). Exception effects keep their order too: JavaScript VM
reference `BH` (`try { v(ctx) } catch(e) { Bn(e,ctx) }`,
`javascript_vm.std.js:13734-13739`) with try-entry handlers 81/221, pop-or-throw
120, pop 224, throw 112.

The `null[0]()` signal is intentional: the push handler creates and pushes the
callable **without throwing** — JavaScript VM reference h47 `241209..241301`,
the WebAssembly VM reference h83 `217871..217963`
(`function(b0){ b0.stack.push(function(){ null[0](); }); }`). A later call handler
invokes it inside the try/catch — JavaScript VM reference h29 `234707..234897`,
WebAssembly VM reference h64 `212578..212768`
(`stack[len-2][stack[len-1]]()`), where `null[0]` raises the deliberate
`TypeError`. Preserve the original failing expression and the caught raw value;
do not model the push as a throw or the invocation as a no-op.

## 7. Limits

No dispatch mapping (`dispatch_link` stays `unproven`); handler indices/table keys
are input-local and are not cross-input operation names; the five widths/endianness
records are proved only for these five pairs. No runtime fact (taken branch,
thrown value identity, worker ordering) is asserted. The two WebAssembly module
identities used here — handler table and interpreter — are decoded-byte lengths
and SHA-256 values from the tracked reference; a layout digest is a separate
release-local hash. The JavaScript operand extractor described in
[JavaScript inline operand
separation](../operations/js-operand-separation.md) certifies the JS handler byte reads, but it
does not supply the missing WASM branch-to-handler mapping.

## 8. Fixture provenance and how to read it

The JSON fixture is a retained **static comparison record** from baseline commit
`47e7190`, not a newly measured current run. Its `pairs` array holds the
paired handler bodies, their input-local keys, byte spans, normalized bodies,
semantic summary, and the per-field `operand_mapping`. `negative_cases` preserves
examples that should prevent an over-broad pairing; `signal_case` preserves the
delayed `null[0]()` exception example. `unresolved` lists the joins the static
record could not make. The source spans are Oxc **byte** offsets into the
standardized `shape deob` output, not line numbers or offsets into a fetched raw
input.

Focused historical comparison data in the same JSON is limited contract
evidence; it is not a representative full capture and does not add a third
reference. The tables here use only the two tracked references.

The recorded standardized-output identities and table sizes are:

| Reference | Standardized SHA-256 prefix | Bytes | Operation table |
| --- | --- | ---: | --- |
| JavaScript VM reference | `e7c5f0a08df50fe6` | 529,802 | `BI`, 241 handlers |
| WebAssembly VM reference | `48f111e31c70d171` | 509,321 | `Ou`, 201 handlers |

The five pair spans were re-sliced against the tracked references' catalogue
snippets when this fixture was made. Recompute the coordinates for a different
input.

Decoded-module identities (SHA-256 prefix / byte length) are retained separately:

| Reference | Handler-table module | Interpreter module | Interpreter exports |
| --- | --- | --- | --- |
| WebAssembly VM reference | `8a3ce5970dd1596e` / 1,853 | `850a986f7107b081` / 2,997 | `Te`, `W` |

These identify the static modules used for comparison; they do not turn a
`br_table` selector into a catalogue handler key.
