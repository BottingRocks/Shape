# WebAssembly VM host exception boundary

This page describes static source evidence for the host exception boundary in the
tracked WebAssembly VM reference. It follows the shared VM operations through
module imports, a mutable unwind flag, and JavaScript exception handling. It does
not execute the fixture or prove a dispatch branch-to-handler join.

The tracked WebAssembly VM reference imports 12 functions. That is a property of
the tracked module, not a VM type or admission rule. The tracked JavaScript VM
reference is used only for the JavaScript-side comparison; it has no WebAssembly
module and is outside this boundary.

Bindings, indices, operand layouts, and interpreter placement must be extracted
from each input. Offsets below are byte positions in the standardized output of
the tracked WebAssembly VM reference, identified by the recorded standardized
hash. They are static evidence, not a promise that another input has the same
offsets. Terminology follows the [architecture and VM variants](../architecture.md).

## 0. Input, method, and immutability

The commands use the immutable tracked WebAssembly VM reference. Build the
release executable once as described in [Build once](../../README.md#build-once);
the examples below use `shape` directly.

```sh
./target/release/shape deob tests/fixtures/kernels/webassembly/kernel.raw.js \
  --output /tmp/shape-wasm-standardized.js
```

This produces the standardized JavaScript for source inspection. The module
inventory and byte offsets below additionally require decoding the proven module
literal and parsing the resulting bytes with `wasmparser`; the [`wasm_boundary`
tests](../../tests/wasm_boundary.rs) exercise the current static API without
requiring a local capture. A deobfuscated JavaScript file alone does not verify a
WebAssembly function index or exception edge.

| Reference | Raw SHA-256 prefix | Standardized SHA-256 prefix | Standardized bytes |
| --- | --- | --- | ---: |
| JavaScript VM reference | `bb2523a588e294d1` | `e7c5f0a08df50fe6` | 529,802 |
| WebAssembly VM reference | `2e9b94dcf73fa5fe` | `48f111e31c70d171` | 509,321 |

The offsets, spans, hashes, byte lengths, and module inventory below are recorded
static positions for that standardized output, not a fresh measurement.
Regenerate standardized stdout, decoded modules, and derived tables as a unit
rather than patching individual fields. The commands use the consolidated
executable and do not imply a new benchmark, a runtime executor, or complete VM
support.

`RENAME` may rename JavaScript bindings, but the import/export section is an ABI
and keeps its raw names (`f`, `s`, `Te`, `W`, `TT`, …). Names are discovery hints,
not proof; the identities below are resolved by module function index and byte
offset, not by spelling.

Decoded artifacts (SHA-256 over the decoded bytes, not over base64 text):

| Reference | Handler-table module | Interpreter module | Linear image |
|---|---|---|---|
| WebAssembly VM reference | 1,853 B `8a3ce5970dd1596e…` | 2,997 B `850a986f7107b081…` | 175,525 B `03a3112cdbad29d4…` |

The interpreter module is an MVP module: no tag section, no `try`/`catch`/
`throw` exception-handling instructions. A JavaScript exception raised by an
imported host function therefore leaves the WASM activation as a normal JS
exception; the module cannot catch it. The module tracks its own "unwind
requested" state in a mutable global instead (see §2). This is the whole boundary.

## 1. Boundary inventory

### 1.1 Exported entry/reset pair

`vmRun` calls the interpreter export; the export result is the VM completion
status. The interpreter module also exports a zero-argument flag setter.

| Reference | Interpreter export | Function index | Body range | Result | Standardized JS span |
|---|---|---:|---|---:|---:|
| WebAssembly VM reference | `W` | **13** (imports `0..11`) | `0xce..0xbb5` | `i32` | `exports.W)` = `264839` |
| WebAssembly VM reference | `Te` | **12** | `0xc6..0xcc` | none | `exports.Te)()` = `265427` |

The setter body is exactly a constant-1 store, verified operator by operator:

```text
WebAssembly VM reference fn 12 (`Te`)   0xc7 i32.const 1   0xc9 global.set 0   0xcb end
```

### 1.2 Mutable exception flag

The interpreter module defines exactly one global:

```text
WebAssembly VM reference  (global 0) (mut i32) i32.const 0     ; init at 0xb2, end at 0xb4
```

It is not exported. It is written only at the setter and at two in-interpreter
throw sites, and read/cleared at exactly one in-interpreter site:

| Op | Offset | Meaning |
|---|---:|---|
| `global.set 0` (setter) | `0xc9` (fn 12) | host catch requests unwind |
| `i32.const 1` + `global.set 0` | `0x4a2` + `0x4a4` | VM throw inside a protected region |
| `i32.const 1` + `global.set 0` | `0x4c3` + `0x4c5` | VM throw inside a protected region |
| `global.get 0` (read) | `0xacf` | unwind requested? |
| `i32.const 0` + `global.set 0` (clear) | `0xad3` + `0xad5` | consume the request |

So the flag is a **request/acknowledge** bit, not a value carrier. The thrown
value is carried separately on the JS `context.errorStack` array (§3).

### 1.3 Interpreter imports (actual module index space)

The function-import namespace is `f` (12 imports). Function index order is the
import declaration order (the actual module index space, before the two defined
functions).

| Role | idx / raw | Type |
|---|---:|---|
| push host value | 0 `Tv` | `(externref f64)` |
| push literal switch | 1 `TM` | `(externref i32)` |
| guarded operation call | 2,3,4,5,6 `b,D,h,m,d` | arity 1..5 |
| error-stack write to slot | 7 `l` | `(externref i32)` |
| **rethrow raw value** | **8 `TT`** | `(externref)` |
| error-stack truncate | 9 `TJ` | `(externref i32)` |
| truthiness pop | 10 `L` | `(externref) -> i32` |
| stack pop | 11 `Tl` | `(externref) -> i32` |
| memory | — `f.A` | `memory 3` |
| handler table | — `s.Z` | 201 `funcref` |

The guarded wrapper set has five wrappers (`b,D,h,m,d`). The handler-table size
and import arities are properties of this tracked module; they must not be
promoted into a rule for unseen inputs.

## 2. Normal / return / throw / unwind

### 2.1 The guarded-versus-direct call decision

Inside the dispatch branch, the interpreter builds the call operands and then
chooses guarded vs direct by the error-region depth in local 8:

```text
WebAssembly VM reference 0x721..0x73e (representative site 1 of 23):
  0x721 local.get 2                       ; context (externref)
  0x723/0x728/0x72d i32.load8_u ...       ; operand reads from pc
  0x732 local.get 3                       ; last operand = handler key
  0x734 local.get 8                       ; error-region depth
  0x736 if (func type 4)
  0x738   call 5                          ; f.m = vmInvokeOperation (JS try/catch)
  0x73a else
  0x73b   call_indirect (type 3)          ; direct handler-table call (no JS guard)
  0x73e end
```

The tracked kernel has **23** guarded-wrapper call sites and **23** direct
`call_indirect` sites. The guard is the interpreter's `local 8`, the same
error-region depth used by the throw sites below. This is the WASM counterpart of
the JS reference's `if (context.G.length === 0) vmOperations[v](context); else
BH(vmOperations[v], context);`.

### 2.2 The outer JS try/catch wrapper

`vmInvokeOperation` is the only JS `try`/`catch` on the exception path. (The other
`try` in the kernel is `vmInitializeWorker` around `Blob`/`Worker` creation at
stdout lines `11726-11736`; it is unrelated.)

The WebAssembly VM reference, stdout `265149`:

```js
var vmInvokeOperation = function(context, operand1, operand2, operand3, operand4) {
  "use strict";
  try {
    vmOperations[arguments[arguments.length - 1]](context, operand1, operand2, operand3, operand4);
  } catch (error) {
    (0, vmInterpreterInstance.exports.Te)();   // set WASM global 0 = 1
    context.errorStack.push(error);            // carry the raw value
  }
};
```

Weird but load-bearing ABI fact: the handler key is the **last argument**, read
via `arguments[arguments.length - 1]`. The number of preceding operands is the
wrapper arity, not a field.

The retained [supplementary source fragment](operation-reference.md)
shows the same guarded-call primitive with a sixth positional operand:
it passes `operand5` too. It is source-scoped contract evidence from another
captured kernel, not a third full capture; the canonical WebAssembly VM fixture
does not exhibit this supplemental ABI or prove a six-operand wrapper.

### 2.3 Throw inside the interpreter

When the interpreter executes a VM throw while an error region is active (`local 8
!= 0`) it sets global 0 and leaves toward the unwind cascade; when no region is
active it calls the rethrow import, which throws the raw value out of WASM:

```text
WebAssembly VM reference 0x49a..0x4ad:
  0x49a local.get 24  0x49c if
    0x49e local.get 8  0x4a0 if
      0x4a2 i32.const 1   0x4a4 global.set 0   0x4a6 br 45
    0x4a8 else   0x4a9 local.get 2   0x4ab call 8   ; f.TT rethrow
    0x4ad end
WebAssembly VM reference 0x4bb..0x4ce repeats the pattern: 0x4c3 const 1, 0x4c5 global.set 0,
  0x4c7 br 44, else 0x4ca local.get 2, 0x4cc call 8 (f.TT).
```

The rethrow import body is exactly a raw `throw` of the top of the JS error stack:

```text
WebAssembly VM reference stdout 500571:  TT: function(context) { throw context.errorStack.pop(); }
```

### 2.4 Flag-driven unwind

At the loop tail, after a dispatch step, the interpreter reads the flag. When set
it clears it and enters a bounded unwind cascade that restores the saved pc/state
locals and truncates the JS error stack to preserve the pending value:

```text
WebAssembly VM reference 0xacf..
  0xacf global.get 0
  0xad1 if
  0xad3 i32.const 0
  0xad5 global.set 0
  0xad7 i32.const 0
  0xad9 local.set 7
  0xadb..0xaec nested blocks + br_table on local 8 (error-region depth)
  ... restore pc/state from locals 9..20, accumulate count into local 7 ...
  0xb83 call 9 (f.TJ)          ; truncate errorStack keeping the top value
  0xbb3 unreachable            ; loop never falls through
```

`f.TJ` is the error-stack truncation import (the WebAssembly VM reference
`0x62a`, `0x638`, `0xb83`). The error-stack value is copied into a VM memory slot
by `f.l` (the WebAssembly VM reference `0x3a0`), so the raw thrown value can flow
back into VM memory without being re-encoded.

### 2.5 Completion status

`W` only returns through explicit returns:

| Return site | WebAssembly VM reference | Status |
|---|---|---|
| value completion | `0x355` `i32.const 1; 0x357 return` | `1` |
| empty completion | `0x537` `i32.const 0; 0x539 return`; `0x6f0` `i32.const 0; 0x6f2 return` | `0` |

The JS run loop interprets the status (the WebAssembly VM reference stdout
`264753`):

```js
var vmRun = function(context) {
  var executionStatus = (0, vmInterpreterInstance.exports.W)(context.byteOffset, context.dispatchState, context);
  switch (executionStatus) {
    case 0: { return; }                  // VM completion, no value
    case 1: { return context.stack.pop(); }  // VM completion with value
  }
};
```

No other status is produced by this module. A path that throws never reaches this
switch: it propagates as a JS exception out of `W`.

## 3. Trap vs arbitrary JS thrown value vs VM completion

These are three distinct outcomes and an executor must not collapse them:

1. **WASM trap** — an executed `unreachable`. Recorded locations in the
   WebAssembly VM reference include `0x4dc`, `0x597`, `0x63e`, `0xb3e`, `0xb75`,
   `0xbab`, `0xbb1`, `0xbb3`. Some sit immediately behind a rethrow call or a
   branch and are unreachable in a well-formed state; the outer trailing
   `unreachable` guards a loop that never falls through. Reaching one is a trap
   (VM invariant violation), not a JS-exception carrier.
2. **Arbitrary JS thrown value** — any value `errorStack.pop()` returns is thrown
   by the JS import `TT` and crosses the WASM frame untouched (the WebAssembly VM
   reference `0x4ab`, `0x4cc`), or is caught by `vmInvokeOperation` and pushed
   back onto `context.errorStack` as the same value. `errorStack` is a plain JS
   array, so objects, strings, numbers, `undefined`, and functions survive; no
   wrapping into `Error` occurs.
3. **VM completion** — `W` returns `i32` 0 or 1, mapped by `vmRun` to `undefined`
   or `stack.pop()`.

There is no WASM exception tag and no WASM catch, so a JS exception from a direct
`call_indirect` handler bypasses the flag path entirely and is only observable at
the JS caller of `W`.

## 4. The deliberate `null[0]()` signal

The two tracked references contain the same handler template under different
handler keys — direct evidence that handler keys are input-local and must be
resolved, not assumed:

| Reference | handler key | push-closure span | `null[0]()` span |
|---|---:|---:|---:|
| JavaScript VM reference | 47 | `241235` | `241275` |
| WebAssembly VM reference | 83 | `217897` | `217937` |

```js
83: function(context) {
  context.stack.push(function() {
    null[0]();
  });
},
```

The handler **creates and pushes** a closure; it does not throw. The throw only
happens if a call handler later invokes that slot. In the WebAssembly VM
reference, method-call handler 64 (`context.stack[len-2][len-1]()`, stdout span
`212646`) is one such consumer: invoking the pushed closure evaluates `null[0]`,
i.e. a deliberate `TypeError`. This must be modeled as a real dynamic throw on
invocation, not as an unsupported state or a silent no-op — and the push itself
must not be treated as throwing.

The user confirms the purpose: this callable is invoked inside a try/catch to
deliberately generate a specific exception signal. Its creation, invocation, and
catch handling are separate steps of that protocol. Preserve the original failing
expression and the caught value when implementing the signal.

The source sequence above establishes two distinct effects: creating and pushing
the closure, then throwing when a later call invokes it. A handler classification
or emitted name cannot collapse those into one throwing push. The tracer must
preserve the invocation and surrounding catch path.

## 5. Actionable executor mapping

An executor for this boundary can be driven entirely from the evidence above:

1. Call the interpreter export with `(context.byteOffset, context.dispatchState,
   context)`; treat context as `externref` identity, never copy it.
2. For each guarded call site, execute the wrapper in a `try`. On a throw:
   set the WASM global 0 to 1, then push the **raw** caught value onto
   `context.errorStack`; continue at the instruction after the import.
3. For each direct `call_indirect` site, execute without a JS guard; a throw
   escapes the interpreter activation as the same JS value.
4. Execute the throw opcodes: with an active error region, set global 0 and
   branch to the unwind cascade; without one, throw `errorStack.pop()`.
5. At the loop tail, if global 0 is set, clear it and run the unwind cascade
   (restore pc/state, truncate `errorStack` keeping the pending top value).
6. On return, map status 0 to completion/`undefined` and status 1 to
   `stack.pop()`.
7. Execute `unreachable` as a trap, distinct from steps 2/3/4.

### 5.1 Minimal acceptance cases for tracing

Check these cases against the existing Rust trace and machine-state model.

1. **Flag setter is not a value carrier.** `Te` stores `did_throw` and returns no
   value; assert `i32.const 1` + `global.set 0` and function index 12.
2. **Guarded throw.** `vmInvokeOperation` -> `did_throw = true`,
   `exception = raw JS value`, normal successor resumes after the import, and
   the flag is observed set at the loop tail (`0xacf`).
3. **Unguarded throw.** `call_indirect` has no exception successor inside the
   kernel; the exception leaves the `W` activation.
4. **Rethrow import.** `TT` is `throw errorStack.pop()`, not a status return;
   assert its index and call offsets (`0x4ab`/`0x4cc`).
5. **Completion.** Status 1 returns `stack.pop()`, status 0 returns
   `undefined`; assert the three return offsets.
6. **Trap.** An `unreachable` is a distinct terminal from a thrown value.
7. **Deliberate signal.** `context.stack.push(function(){ null[0](); })`
   pushes without throwing; a later call inside try/catch deliberately throws
   `TypeError` to generate the signal. Preserve the caught value and catch path.

## 6. Unresolved paths and limits

- **No dispatch mapping.** This page does not join a `br_table` target to a
  handler key; the 234 branch operands and the 201 handler keys remain separate.
  That is the dispatch-recovery task's scope.
- **Semantic local names.** WASM local indices include parameters: in the
  WebAssembly VM reference, `0` is the byte offset, `1` the dispatch state, and
  `2` the context. Names such as error-region depth (`8`) and truncation count
  (`7`) come from reads, writes, and control flow. Missing debug names do not
  prevent proving those roles; extraction must record that dataflow and recover
  it for each input.
- **Unwind-frame layout.** In the WebAssembly VM reference, the restore cascade
  directly copies `9 -> 0` and `10 -> 1`, or `13 -> 0` and `14 -> 1`, or `17 -> 0`
  and `18 -> 1`. Those are saved byte-offset/state pairs. The other locals in
  `9..20` hold associated counters; they are not all saved targets. Recover each
  group's save/restore and counter rules to reconstruct the frame completely.
- **Unguarded-throw destination.** An unguarded handler exception propagates
  through the interpreter export and `vmRun` to their enclosing caller. Follow
  that call chain to locate a catch or the host's uncaught-exception boundary.
  The `vmInitializeWorker` catch covers synchronous worker setup; it does not
  automatically catch an exception later thrown in the worker.
- **Input-specific correspondence.** The shared VM structure and semantics are
  the project contract. Recover each input's handler composition, table export,
  namespace, import signatures, and offsets to connect those shared semantics to
  its concrete WASM/JavaScript wiring. Different spellings or shuffled indices do
  not establish a semantic difference.

## Primary sources

- The tracked WebAssembly VM reference
  [`tests/fixtures/kernels/webassembly/kernel.raw.js`](../../tests/fixtures/kernels/webassembly/kernel.raw.js)
  and its [`manifest.json`](../../tests/fixtures/kernels/manifest.json) identity.
- The tracked JavaScript VM reference where the outer JS wrapper is compared.
- The standardized output identified by `48f111e31c70d171…` and the decoded
  module/image artifacts produced by the analysis commands in §0.
- `wasmparser` 0.261.0 module/index/operator validation.
- [Operation reference](operation-reference.md) for dispatch source anchors.
