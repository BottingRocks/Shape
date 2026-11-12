# VM operation reference

Status: **static source evidence**. This page is a machine-readable taxonomy of the
**unique primitives needed by the current control-flow worklist**, the composites that
fuse work with control transfer, and the operand layouts those operations consume. It is
not a new runtime, a scan CLI, an exhaustive permutation catalogue, or a claim that every
handler in every kernel is covered.

Machine-readable companion:
[`tests/fixtures/operations/primitive-reference.json`](../../tests/fixtures/operations/primitive-reference.json).
Focused consistency test: `tests/operation_reference.rs`.

The JSON separates `primitives` (one meaning), `operand_layouts` (byte reads
and PC effects), `composites` (ordered fused effects), `sources` (verbatim
evidence), and `support_summary` (recognition versus execution). For example,
`layout.jump_u8` has two one-byte entries: `target_pc` at offset 0 and
`next_state` at offset 1. Its `pc_change` advances past those two operand
bytes, then assigns the decoded target and state. That is more precise than
calling it merely “jump”: the byte reads, PC side effect, and transfer each
have a place. A composite links primitive IDs and orders extra work rather
than inventing a new byte-decoder rule.

To check the tracked contract after editing this page, its worklist, or the
fixture, run `cargo test --test operation_reference`. The test checks schema,
unique IDs, source citations, worklist coverage, and support accounting. It
does not execute the captured kernel or establish a mapping for an unseen
capture.

## Source anchors for the fixture

The fixture retains these exact source-shaped needles so its evidence remains
checkable after consolidating the older crosswalk. They identify control-flow
mechanisms in the fixed JavaScript VM reference and a captured WebAssembly VM
dispatch. They do not establish a cross-capture handler mapping.

```js
var O = BD[this.f][r[this.P++]];
if (BN !== Bi) { O = BN; BN = Bi; return O; }
function vmCaptureThread(byteOffset, dispatchState, memory, initialSlots, capturedSlots,
threadInvocation = vmBuildThreadInvocation(byteOffset, dispatchState, capturedMemory,
var context = new vmContext(byteOffset, dispatchState, capturedMemory, this);
```

The JavaScript dispatch lookup uses the current state and advances the program
counter as it reads the opcode. The run loop returns its pending result only
when that result differs from the no-result sentinel. Thread capture copies
the selected memory slots into a new invocation, and invocation construction
creates a context using the copied memory and call receiver.

```text
br_table <234 branches> on the low byte
```

For the tracked WebAssembly VM reference, the interpreter reads a table whose
low byte selects the dispatch branch and whose high byte carries next state.
Its 234 branch indices do not map one-to-one onto the 201 imported JavaScript
handlers. The branch-to-handler join and per-operation operand widths must be
proved for each captured image before a handler identity can be assigned.
The tracked image's table occupies `[170845, 175525)`: 10 states × 234
opcodes × two bytes. Its JavaScript operation array has 201 entries. The entry
array `[1,0,2,3]` supplies initial memory-slot keys, not operand-stack input.

## 1. Scope

In scope:

- control flow: dispatch, jumps, branches, calls, returns, try/catch/finally, unwind;
- completion and sentinels: terminal selector contracts, run-loop sentinel checks,
  pending-result, stack/undefined completion, interpreter status;
- operands: the proven `u8`, `u16` little-endian and `u24` little-endian readers, and the
  PC-advance effect;
- thread transfer needed to trace a VM thread;
- accounting for every operation in the current control-flow worklist.

Deliberately out of scope:

- an exhaustive composite or permutation catalogue;
- an incoming-capture scan CLI or importer;
- new runtime recognizers or executor features;
- semantic-equivalence proofs between the two placements;
- execution of captured JavaScript or WebAssembly;
- coverage of unseen kernels.

The worklist source is [`docs/reference/vm-control-flow-ops.md`](vm-control-flow-ops.md). This
reference does not edit that worklist or the implementation.

## 2. Two fixed kernel references

Operation identities are independent of rotating acquisition captures. Source citations
name exactly two immutable tracked references:

| Reference | Name | Pinned path | Placement |
|---|---|---|---|
| `kernel.javascript-vm` | **JavaScript VM** | `tests/fixtures/kernels/javascript/kernel.raw.js` | Interpreter responsibilities in JavaScript |
| `kernel.webassembly-vm` | **WebAssembly VM** | `tests/fixtures/kernels/webassembly/kernel.raw.js` | Interpreter responsibilities in a WebAssembly module |

A per-capture handler key or table position is **not** an operation identity. The fixed
snapshots are the naming and source anchors. The recorded raw-capture hashes identify
their immutable bytes; the standardized-output hashes identify historical analysis
outputs and are not hashes of the raw fixture files.

A single supplementary historical source fragment is used only where it proves something
the fixed WebAssembly VM reference does not exhibit: the guarded-call primitive with a
sixth positional operand. Its source fragment and hash are retained, no full kernel is
stored, and the fixture explicitly does **not** claim that the fixed WebAssembly VM
snapshot reproduces it. The fragment is focused contract evidence, not a third
representative capture.

Focused synthetic test fixtures are regression and contract evidence for
specific operations. They are not representative full captures and do not
extend the two references above.

## 3. Identity model

```
prim.<category>.<name>       one primitive semantics
composite.<category>.<name>  one fused work + transfer body
```

- Width and placement variants share one primitive ID. The variant carries its own proven
  operand layout and source references.
- A fused body is a **composite**, not a new semantic primitive. It points at the
  control-flow and operand primitives it uses and keeps its work effects in source order.
  Work effects are not promoted to new semantic operations in this slice.
- No identity contains a numeric handler key, a numeric completion selector, a table
  position, or a site name.
- Every operation records recognition and execution status as `implemented`, `partial`,
  `not_implemented`, or `not_applicable`.
- Provenance is source-based: each source fragment is verbatim and SHA-256 hashed, and
  each document evidence pointer is a verbatim needle in the tracked document. Nothing is
  executed.

## 4. Primitive families

The fixture contains 33 primitives (plus 4 operand primitives and 41 operand layouts).
They are grouped by the following categories.

### 4.1 Dispatch and next-state selection

`prim.control.dispatch.next` selects the next handler from the dispatch table. The
JavaScript VM form reads `table[state][bytecode[pc++]]` and returns a selector. The
WebAssembly VM form reads a u8 opcode, indexes a memory-resident state/opcode table, uses
the low byte as a `br_table` operand and installs the high byte as the next state. The
WebAssembly branch-to-handler join remains unproven.

### 4.2 Jumps

`prim.control.jump.direct` has proven `u8`, `u16` and `u24` target variants, each followed
by a `u8` state. `prim.control.jump.indirect_memory` takes the PC and state from two slots;
`prim.control.jump.indirect_stack` takes them from the old top and top-1 and pops two
values and pops them. Width variants are not normalized. The bounded graph, operand
separation and symbolic dispatch paths recognize and execute these transfers when the
proven operands and concrete VM values are available.

### 4.3 Conditional branches

`prim.control.branch.truthy` and `prim.control.branch.falsy` have `u8`, `u16` and `u24`
target variants; both test the stack top and pop it. `prim.control.branch.slot_not_null`
jumps when a slot is not null and does not pop. `prim.control.branch.not_less_than_slot`
jumps unless `top < readSlot(slot)` and pops the tested value. The fixture does not assume
numeric-only JavaScript comparison or truthiness.

### 4.4 Calls and returns

`prim.control.call.direct` has `u16` and `u24` target variants and pushes a
`{returnByteOffset, returnDispatchState}` frame before transferring. The frame field names
and capture order are proven; the bounded tracer follows the frame and dispatch transfer
with explicit cursor, depth and empty-stack boundaries.
`prim.control.call.guarded` covers the JavaScript VM try/catch wrapper and the WebAssembly
guarded-import/direct-`call_indirect` decision. `prim.control.call.program_value` covers a
fused handler that calls a JavaScript function value. The bounded HandlerIR form recognizes
the exact unshadowed `Function.prototype.apply.call` shape with a one-argument stack slice;
the symbolic tracer emits it only when the resulting argument array has known elements.
`prim.control.call_frame.push` and
`prim.control.call_frame.pop` isolate the frame mechanics. `prim.control.return.handler`
pops the frame and resumes the caller in the bounded tracer; an ordinary handler return is
**not** VM completion. Dynamic/direct program-value calls, guarded-call execution and
receiver/argument shapes outside that proof remain outside full execution.

### 4.5 Try, catch, finally and unwind

The JavaScript VM form pushes a try frame with `{handlerByteOffset, handlerDispatchState,
errorEntryCount: 0}`, saves an error record, jumps to a finally handler, binds or drops a
caught value, and ends finally by either rethrowing the pending value or resuming the
saved PC/state. `errorEntryCount` is always created as 0 in the inspected bodies; its
unwind meaning is unresolved.

The WebAssembly VM form relocates the exception path: a single mutable unwind-request
global, a bounded unwind cascade that restores PC/state and truncates the error stack, a
rethrow import that throws the raw error-stack value, and an error-stack truncate import
that keeps the pending top value. The full unwind-frame layout is not recovered.

### 4.6 Completion and sentinels

- `prim.completion.selector_sentinel`: the caller-supplied selector contract. The selector
  is validated against the dispatch image, is checked before handler lookup, and consumes
  only the opcode byte. It is never inferred from a handler key, and no numeric selector is
  blessed for all kernels.
- `prim.completion.run_loop_sentinel_check`: the JavaScript VM run loop compares a
  completion register with an immutable not-complete sentinel and returns the pending
  result when they differ.
- `prim.completion.result.stack` and `prim.completion.result.undefined`: the
  `CompletionValue` policies exercised by the WebAssembly host run loop (status 1 ->
  `stack.pop()`, status 0 -> bare return).
- `prim.completion.result.register`: the JavaScript VM pending-result register transport.
  It is recognized by the prefix proof but is not mapped into complete dispatch.
- `prim.completion.interpreter_status`: the interpreter's i32 status 0/1 and its source
  mapping to completion.

### 4.7 Thread transfer

`prim.thread.capture`, `prim.thread.build_invocation` and `prim.thread.invoke` describe the
canonical capture/closure/invocation order. The public thread API applies that order to
caller-supplied descriptors and values; those inputs are not source-proven. Live shared
captures are rejected, not modeled. The receiver is an invocation input (`undefined` for
a plain call); `context.receiverValue` reads it as is and `context.receiverObject` as
`this == null ? host : Object(this)`.

### 4.8 Operand readers and layouts

Operand primitives are `prim.operand.byte`, `prim.operand.u16le`, `prim.operand.u24le` and
`prim.operand.pc_advance`. The layouts record role, byte offset, width, endian and PC
change only where the source composition proves them. `u8` uses `endian: "none"`; `u16`
and `u24` use little-endian byte composition. The reference does not assert JavaScript
signedness or masking beyond the proven composition.

## 5. Worklist accounting

The fixture's `worklist` array maps each of the 45 current worklist entries to exactly one
primitive or composite and one variant. Duplicate headings are disambiguated by their
heading line in the tracked worklist document.

| Worklist section | Entries | Mapping |
|---|---:|---|
| Unconditional jumps | 3 | `prim.control.jump.direct` (`u8`/`u16`/`u24`) |
| Indirect jumps | 2 | `prim.control.jump.indirect_memory`, `prim.control.jump.indirect_stack` |
| Conditional jumps | 8 | `prim.control.branch.*` |
| Calls | 2 | `prim.control.call.direct` (`u16`/`u24`) |
| Fused work + call | 16 | 15 `composite.call.*` entries; `push two slots` has `slots_u16` and `slots_u8` variants |
| Returns | 1 | `prim.control.return.handler` |
| Fused work + return | 6 | 6 `composite.return.*` entries |
| Exceptions | 7 | `prim.control.try.*`, `finally.*`, `catch.*` |
| **Total** | **45** | |

The 21 composites reference primitives; none introduces a new semantic operation for its
work portion. Their ordered effects preserve source order, including the call-frame push
or pop relative to work.

Representative fused composites:

- `composite.call.decode_string_property_read` (u16 string key + u24 target/state):
  decode, property read, frame push, transfer, stack replacement, pop one.
- `composite.call.define_property` (u8 immediate + u24 target/state): fixed
  `Object.defineProperty` descriptor, frame push, transfer, `[object, immediate]` result.
- `composite.call.property_write` (u8 slot + u24 target/state): strict-mode property write,
  slot read, frame push, transfer, stack collapse.
- `composite.return.call_program_function` (u8 slot + u8 output slot): program-value call,
  result-slot write, frame pop, resume, pop four.

## 6. Support boundary

Recognition and execution are deliberately separate. The current operation catalogue
recognizes bounded VM control-transfer recipes in addition to property/stack/memory
actions. The implemented subset is direct and proven indirect jumps, direct calls,
call-frame push/pop, VM frame returns, and 15 fused composites whose work effects contain
no dynamic helper call. Conditional branches, loops and exception regions remain explicit
unsupported regions. Operand separation can lift the inline operand reads for the
JavaScript VM; it reports WebAssembly-hosted handlers unresolved without a binding handoff.

Execution support is narrower than source knowledge:

- complete dispatch and the JavaScript VM prefix have proof-backed drivers for their
  admitted topologies;
- completion selector, stack and undefined completion policies are implemented;
- direct/indirect jumps, direct calls, call-frame following and VM frame returns execute
  only with decoded numeric operands, a caller-supplied cursor/frame and configured bounds;
- a proven `Function.prototype.apply.call` program call is symbolically emitted when its
  stack slice has known arguments; the concrete executor rejects the
  abstract `StackSlice`/`Call` values;
- conditional branches, try/catch/finally execution, dynamic helper/direct program-value
  calls outside that proof, the WebAssembly unwind path and the WebAssembly interpreter are
  not executed in this tree.

The current fixture's primitive counts are recognition `implemented=12`, `partial=7`,
`not_implemented=14`; execution `implemented=10`, `partial=8`,
`not_implemented=15`. In addition, 15 of the 21 composites are fully recognized and
bounded-executable; the other six retain explicit dynamic-call gaps. The fixture's
`support_summary` and `unresolved` fields carry the exact gaps.

## 7. Non-claims

- No coverage of every handler, every kernel or unseen kernels.
- No byte-level semantic-equivalence claim between the JavaScript VM and WebAssembly VM.
- No runtime execution of captured JavaScript or WebAssembly, and no runtime facts such as
  taken branches or thrown-value identity. Bounded Rust symbolic execution of extracted
  recipes is not execution of the captured kernels.
- No numeric selector, handler key, site name or table position as operation identity.
- No claim that the fixed WebAssembly VM snapshot reproduces the supplementary source
  fragment.
- No fabricated operand width, signedness or masking.
- No complete WebAssembly unwind-frame layout and no WebAssembly branch-to-handler join.

## 8. Maintenance

To add a needed operation:

1. Identify the unique semantic primitive or composite; do not add a per-capture key,
   site name or selector number to the ID.
2. Add only source fragments that are verbatim and hashed. Cite one of the two fixed kernel
   references. Use a supplementary fragment only when it proves a gap the fixed reference
   does not, and record that it is not the fixed snapshot.
3. Add proven operand entries to a layout. If a width, endian, order or PC effect is not
   proven, leave it out and record the unknown.
4. Point composites at primitive IDs. Keep work effects ordered; do not promote every full
   fused body to a new semantic operation.
5. Update the worklist mapping and run `cargo test --test operation_reference`.
