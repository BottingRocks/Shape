# WebAssembly VM prefix preparation

This reference covers the structural interpreter lift into the shared prefix
walker. It is separate from the [static boundary inventory](../reference/wasm-boundary.md)
and its fixture-scoped dispatch adapter. Start with the
[kernel WASM module guide](../../src/kernel/wasm/README.md) for ownership.

Raw-source preparation can admit this path. The saved-`PipelineReport` free
function prepares only JavaScript dispatch; source-free WebAssembly tracing uses
a [kernel artifact](../deob/kernel-artifacts.md) compiled while raw source is available.

## WASM-dispatched kernels (`prefix/wasm.rs`)

A WASM kernel keeps the VM's dispatch loop in a WebAssembly interpreter: its
exported run function reads the opcode and dispatch cell from linear memory and
either calls one of the JavaScript operations (through the handler module's table)
or does the VM's own bookkeeping. `kernel::wasm_lift` names what every selector
does from the interpreter's bytes; `wasm.rs` turns that into the walker's inputs:

* the dispatch table, read from memory the way the run function reads it;
* each operation's recipe, with the operands the interpreter decodes;
* the bookkeeping selectors as the recipes a JavaScript VM's handlers for the same
  operations have (jumps, calls, `try`/`catch`/`finally`, returns), so the walker
  and emitter run them unchanged;
* the image write as the patch handler;
* the bounds a WASM machine adds: its try, record and call banks, and the
  interpreter's own tables in memory (`frozen_from`; a write there stops the walk).

Before any of it, the proof-view report must link the handler module's imports to
`vmOperations` key for key, and the interpreter's helper imports must be exactly
the helper functions the lift assumes. A selector path the lift cannot name fails
preparation. `WasmMachine` bounds that a JavaScript VM does not: at most three try
frames, two error records, and one live VM call. The checked-in WebAssembly capture is
a regression case, not a name or hash admission rule for this path. The older
[one-cell adapter](../../src/kernel/README.md) is SHA-pinned and separate from this
structural prefix path.

## WASM preparation, selector by selector

The WASM path exists so the same walker can execute a kernel whose dispatch loop
is a WebAssembly interpreter. It does not run the interpreter; it proves what
each selector does and lowers it to the same `HandlerTemplate` /
`LiftedHandler` shapes the JavaScript path uses.

### The gate and the proof view

`prepare_wasm_prefix` returns `Ok(None)` immediately when `report.output` does
not contain `"WebAssembly"`. That is the only dispatch between the two kernel
families. For a WASM kernel it builds a **proof view** — the pipeline report
without the WASM presentation adapter — and requires the first proven operation
table. The view is used because the presentation adapter's copies of module
bindings are not proof; the proof view still says which JavaScript function each
import is.

### `admit`: what must line up before lifting

| Step | Requirement |
| --- | --- |
| operation table | the proof view's table has the same canonical name and initializer element count as the report's |
| interpreter module | exactly one `validated` module imports both a memory and a table |
| table import | exactly one table import |
| producer | that import has a proven producing instance |
| handler module | exactly one validated module in that instance exports the imported table name |
| handler imports | every function import of the handler module is `status == "proven"` and linked as `vmOperations[<its own name>]` on the view's declaration ordinal |
| mappings | `handler_mappings` is non-empty and every row maps element `k` to operation `k` (`original_handler_key == element_index`) |
| interpreter helpers | each function import's source is a property whose body matches a known helper role |
| literal helper | at most one helper pushes literals |

If any row fails, preparation fails with a `SourceProof` error prefixed `WASM
kernel:`. A module that can be inventoried but not linked is exactly the case
this step rejects: a namespace/name pair is not provenance.

### Helper roles

The helper names are not matched by spelling; their normalized source bodies are.
`body` is the property initializer after whitespace normalization.

| Role | Recognized body |
| --- | --- |
| `Truth` | `function(context) { return !!context.stack.pop(); }` |
| `Number` | `function(context, operand1) { Array.prototype.push.call(context.stack, operand1); }` |
| `CatchBind` | `function(context, operand1) { context.memory.writeSlot(operand1, context.errorStack.pop()); }` |
| `Rethrow` | `function(context) { throw context.errorStack.pop(); }` |
| `Pop` | the declaration `vmPopStack` equal to `function(context) { return context.stack.pop(); }` |
| `Invoke` | the declaration `vmInvokeOperation`, checked separately by `check_wrappers` |
| `DropUnder` | `var x = top; length -= n; top = x`, for any local name |
| `Literal` | `switch (operand1) { case k: push(literal); return; … }` with literals `{}`, `[]`, `true`, `false`, `void 0`, `null`, `Infinity` |

`check_wrappers` requires the declared `vmInvokeOperation` to be exactly

```js
function(context, operand1, …, operandN) {
  "use strict";
  try { vmOperations[arguments[arguments.length - 1]](context, operand1, …); }
  catch (error) { (0, vmInterpreterInstance.exports.<flag>)(); context.errorStack.push(error); }
}
```

for the maximum operand count any lifted operation passes (at least 1, at most
`MAX_WRAPPER_OPERANDS` = 8). The flag export's name comes from the lift, not from
the source.

### Run statuses

`run_statuses` parses the declared `vmRun` switch:

| Case body | Meaning |
| --- | --- |
| `{ return; }` | the interpreter returns `undefined`; the VM result is `undefined` |
| `{ return context.stack.pop(); }` | the interpreter returns the top of stack; that is the VM result |

Each status may appear once. The map is used by `bookkeeping` for `Return`
selectors: a status that pops produces `CompleteThread { value: top(1) }` plus
`StackLengthSet`, while a status that does not produces
`CompleteThread { value: undefined }`.

### Memory, table and number table

The runtime config must contain exactly one extracted `WasmMemoryImage` with
bytes. Its span becomes every handler's `source_span` and the input identity
(`vmLinearMemory`). The image is copied and zero-padded to the interpreter's
declared memory size; a larger image is rejected.

The dispatch table is read the way the run function reads it:

~~~text
cell(state, opcode) = memory[base + state * stride + opcode * 2 .. +2]
                      -> [selector, next_state]
out of range         -> [trap_selector, 0]
~~~

All 256 states and all 256 opcodes are materialized, so the walker's table is
total: a byte with no real cell maps to the first trapping selector. This is the
WASM-specific preparation policy: unavailable cells become traps, whereas a
JavaScript table may have shorter rows and report a missing cell.

The number table, when used, is a single f64 table at one base:
`table[i] = f64::from_le_bytes(memory[base + i*8 .. +8])` for `i` in 0..256. More
than one *distinct* table base is rejected ("numbers are read from more than one
table"); the same base repeated is accepted. A table that runs past memory is
rejected. `frozen_from` is then
`min(number_table_base, lifted.base)`, so the walk stops on any image write at or
past the interpreter's own tables.

### Selector ops to recipes

Every selector from 0 to 255 gets a template, an operand handler and a `pc_delta`:

| `SelectorOp` | Recipe shape | Operands | `pc_delta` |
| --- | --- | --- | --- |
| `Operation { operation, operands, length }` | the report's template for that operation, with `handler_index` replaced by the selector | the lift's operands | `length` |
| `ImageWrite { address, value }` | an unsupported template with reason "runs as the image patch handler"; the selector becomes `patch_selector` | address (3 bytes at 0), value (1 byte at 3) | 4 |
| `PushLiteral { kind }` | one `StackWrite` of the literal kind | none | 0 |
| `PushNumber { operand }` | one `StackWrite` of the operand | the operand | `offset + width` |
| `PushTableNumber { operand, .. }` | one `StackWrite` of `vmNumberTable[operand]` | the operand | `offset + width` |
| `Jump { pc, state }` | let 0, let 1, set PC, set state | pc, state | 0 |
| `JumpIf { negated, pc, state, length }` | let 0, let 1, `JumpIf`, stack length - 1 | pc, state | `length` |
| `PopJump` | set PC from `top(1)`, set state from `top(2)`, stack length - 2 | none | 0 |
| `Return { status }` | `CompleteThread` (popped or undefined) | none | 0 |
| `TryEnter { pc, state, length }` | let 0, let 1, push try frame | pc, state | `length` |
| `TryLeave` | pop try frame | none | 0 |
| `FinallyEnter` | push finally record, pop try frame, PC/state from the popped frame | none | 0 |
| `FinallyLeave` | pop error record, rethrow if handled, PC/state from the record | none | 0 |
| `CatchDrop` | pop error record, drop thrown-under if handled | none | 0 |
| `CatchBind { slot }` | let 0 = slot, pop error record, write popped thrown value to slot | slot | `offset + width` |
| `Call { pc, state, length }` | let 0, let 1, push call frame, set PC, set state | pc, state | `length` |
| `CallReturn` | pop call frame, PC/state from the frame | none | 0 |
| `Trap` | unsupported template with `TRAP_REASON` | none | 0 |

The recipe operations are the JavaScript VM's own vocabulary — `PushTryFrame`,
`PopErrorRecord`, `PoppedCallFrame`, `PoppedTryFrame`, `PopThrownValue`,
`RethrowIfHandled` — so the walker and emitter run a WASM kernel without a
separate code path. That is the whole point of the lift: same walker, same
emitter, different proof of what a selector means.

### The machine bounds

`WasmMachine` is the part that has no JavaScript analogue:

| Field | Value | Effect |
| --- | --- | --- |
| `frozen_from` | `min(number_table_base, lifted.base)` | A write at or past it stops with `prefix-image-patch` |
| `number_table` | up to 256 f64 bits | Registered as the `vmNumberTable` constant table |
| `try_frames` | 3 (`TRY_FRAMES`) | More open tries is a `wasm-trap` |
| `error_records` | 2 (`ERROR_RECORDS`) | More records is a `wasm-trap` |
| `call_frames` | 1 | A VM call inside a VM call is an `UnsupportedHandler` |

`wasm_bound` is checked at the start of every step, before the handler executes.
The number table and the try-depth trap are installed on the session by
`run_prefix`, not by the walker's step logic. The checked-in WebAssembly capture
is a regression case for this path; it is not a name or hash admission rule.
