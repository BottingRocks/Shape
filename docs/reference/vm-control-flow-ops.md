# VM control-flow operations

Every operation below was read from the deobfuscated output of a real
JS-interpreter capture.

**How to identify an op.** Handler keys rotate between captures (the same op can be
key 24301 in one capture and 57 in another), and ops have no names of their own.
So nothing here uses a key: each op is identified by its **full deobfuscated
source**. The heading above each source block is only a plain-English description
of what the op does. To find an op in another capture, deobfuscate it and match the
source shape (operand widths and statements), not the key.

This is the worklist for making the symbolic tracer walk real VM threads through
jumps, calls, branches, loops and try/catch/finally.

## Tracing model

The tracer is **symbolic and driven by the bytecode**. Every value is one of two
kinds:

| Kind | Examples | How the tracer treats it |
|---|---|---|
| **VM value** | PC (`context.byteOffset`), dispatch state, operands, stack/slot *positions*, `callFrameStack`, `tryFrameStack`, `errorStack`, `thrownValueStack`, bytecode, string table | Concrete. Computed while tracing. Control flow that depends on it is **followed**. |
| **Program value** | anything from the page: `host` (= `window`/`global`, the IIFE argument), objects the script builds, results of calls | Abstract **AST node** built as the op executes. Control flow that depends on it becomes an **AST control node** (`if`, `while`, `try`). |

Rule of thumb for any control-flow op:

1. If the decision is a VM value → follow it concretely.
2. If the decision is a program value → emit an AST control node and trace every
   successor as its body.

## VM state used by control flow

All live on the VM context object:

| Field | Shape | Role |
|---|---|---|
| `byteOffset` | number | PC into the bytecode image |
| `dispatchState` | number | row of the dispatch table (`vmDispatchTable[state][opcode]`) |
| `callFrameStack` | `[{returnByteOffset, returnDispatchState}]` | VM call stack |
| `tryFrameStack` | `[{handlerByteOffset, handlerDispatchState, errorEntryCount}]` | active `try` blocks |
| `errorStack` | `[{wasExceptionHandled, savedByteOffset?, savedDispatchState?}]` | "we are inside a catch/finally body, and why" |
| `thrownValueStack` | `[error]` | pending exception values |

The run loop calls an op directly when `tryFrameStack` is empty, otherwise
through `vmInvokeOperation`, which wraps it in a JS `try` and on error calls
`vmHandleException`:

```js
operationKey = context.dispatch();
if (context.tryFrameStack.length === 0) {
  vmOperations[operationKey](context);
} else {
  vmInvokeOperation(vmOperations[operationKey], context);
}

function vmInvokeOperation(operation, context) {
  try {
    operation(context);
  } catch (error) {
    vmHandleException(error, context);
  }
}

function vmHandleException(error, context) {
  var tryFrame = context.tryFrameStack.pop();
  for (var errorIndex = 0; errorIndex < tryFrame.errorEntryCount; ++errorIndex) {
    var errorTracker = context.errorStack.pop();
    if (errorTracker.wasExceptionHandled) {
      context.thrownValueStack.pop();
    }
  }
  context.errorStack.push({ wasExceptionHandled: true });
  context.thrownValueStack.push(error);
  context.byteOffset = tryFrame.handlerByteOffset;
  context.dispatchState = tryFrame.handlerDispatchState;
}
```

### How the three try forms compile

Using the op descriptions from section 6:

- **try/catch**: *enter try* → body → *exit try* → after. A throw goes through
  `vmHandleException` to the catch body, which starts with *catch (e): bind the
  error* or *catch without a binding*.
- **try/finally**: *enter try* → body → *enter finally (normal path)* → finally
  body → *end finally* (resumes). A throw goes through `vmHandleException` to the
  finally body → *end finally* (rethrows).
- **try/catch/finally**: the two nested: an outer try frame for the finally and
  an inner one for the catch.

Open question: `errorEntryCount` is always created as `0` and nothing seen so far
increments it. It looks like an unwind counter for nested catch/finally bodies;
confirm before relying on it.

## String decoding

`vmDecodeString(encoded, keyIndex, vmStringTable, vmStringCache, vmDecodeBase64, String.fromCharCode)`
is called from 30 sites, including several fused ops below. With both arguments
known (VM values: a table entry and a `u16` key operand) it is a pure function the
tracer can evaluate to an AST string literal:

```js
function vmDecodeString(encodedString, keyIndex, stringTable, cache, decodeBase64, fromCharCode, computeOnly) {
  if (!computeOnly) {
    var cacheKey = encodedString + "," + keyIndex;
    var cachedString = cache[cacheKey];
    if (typeof cachedString !== "undefined") {
      return cachedString;
    }
  }
  var keyBytes = decodeBase64(stringTable[keyIndex]);
  var encodedBytes = decodeBase64(encodedString);
  var xorMask = keyBytes[0] + encodedBytes[0] & 255;
  var decodedString = "";
  for (var byteIndex = 1; byteIndex < keyBytes.length; ++byteIndex) {
    decodedString += fromCharCode(encodedBytes[byteIndex] ^ keyBytes[byteIndex] ^ xorMask);
  }
  if (!computeOnly) {
    cache[cacheKey] = decodedString;
  }
  return decodedString;
}
```

The pipeline already recognizes this decoder structurally
(`src/transforms/deob/string_decode.rs`). Plain `vmStringTable[i]` reads (no decode)
already resolve to literals in the tracer.

## Operand notation

Operands are read from the bytecode at the PC, little-endian:
`u8` = 1 byte, `u16` = `b0 | b1 << 8`, `u24` = `b0 | (b1 | b2 << 8) << 8`.
`target`/`state` are the jump destination PC and dispatch state.
`top` = `stack[length-1]`, `top-1` = `stack[length-2]`, and so on.

## How to use this inventory

Read these bodies as **source examples**. For an incoming capture, `shape ops list` locates its
proven table and `shape ops show CAPTURE --op KEY` exposes one handler's current
source, micro-ops, recipe status, and operand evidence. Compare the complete
body and byte-read effects with the family below; a matching key number or a
similar expression is insufficient. The machine-readable primitive and
composite identities are in the [operation reference](operation-reference.md).

The families are organized by transfer: jumps (§1–3), calls (§4 and §4a),
returns (§5 and §5a), and exceptions (§6). Within each body, identify (1) the
bytecode reads and PC delta, (2) stack/slot effects before transfer, (3) the
saved or restored dispatch state, and (4) whether a program value makes the
successor symbolic. A fused call or return must retain its preceding effects;
reducing it to a jump loses observable work. The bounded prefix and dispatch
contracts explain which of these bodies can currently execute:
[javascript-vm-prefix.md](../tracing/javascript-vm-prefix.md) and
[trace-cli.md](../tracing/trace-cli.md).

For a reproducible first walk, the tracked JavaScript VM fixture has known
startup coordinates and initial slots:

```sh
shape trace thread tests/fixtures/kernels/javascript/kernel.raw.js \
  --pc 0 --state 0 --initialslots 2,0,3,1 --steps 2 --partial
```

It commits an image-patch dispatch step and stops on an unsupported handler;
that is an explicit partial trace, not a complete program. The exact current
fields and the meaning of `steps_committed` are in
[javascript-vm-prefix.md](../tracing/javascript-vm-prefix.md#tested-javascript-vm-fixture-run).

## 1. Unconditional jumps

Set the PC and dispatch state from operands.

### jump (u16 target)

```js
function(context) {
  var value1 = vmBytecode[context.byteOffset] | vmBytecode[context.byteOffset + 1] << 8;
  var value2 = vmBytecode[context.byteOffset + 2];
  context.byteOffset = value1;
  context.dispatchState = value2;
}
```

- **Does:** `pc = target; state = state` from a `u16` target and `u8` state.
- **Tracer:** VM value → **follow**.

### jump (u24 target)

```js
function(context) {
  var value1 = vmBytecode[context.byteOffset] | (vmBytecode[context.byteOffset + 1] | vmBytecode[context.byteOffset + 2] << 8) << 8;
  var value2 = vmBytecode[context.byteOffset + 3];
  context.byteOffset = value1;
  context.dispatchState = value2;
}
```

- **Does:** Same, `u24` target.
- **Tracer:** VM value → **follow**.

### jump (u8 target)

```js
function(context) {
  var value1 = vmBytecode[context.byteOffset];
  var value2 = vmBytecode[context.byteOffset + 1];
  context.byteOffset = value1;
  context.dispatchState = value2;
}
```

- **Does:** Same, `u8` target and `u8` state.
- **Tracer:** VM value → **follow**.

## 2. Indirect jumps

Take the destination from VM storage instead of operands.

### jump to PC/state held in memory slots

```js
function(context) {
  var value1 = vmBytecode[context.byteOffset];
  var value2 = vmBytecode[context.byteOffset + 1];
  context.byteOffset += 2;
  var value3 = context.memory.readSlot(value1);
  var value4 = context.memory.readSlot(value2);
  context.byteOffset = value4;
  context.dispatchState = value3;
}
```

- **Does:** `pc = readSlot(pcSlot); state = readSlot(stateSlot)`.
- **Tracer:** Follow when both slots hold VM values; otherwise stop with a diagnostic.

### jump to PC/state held on the stack

```js
function(context) {
  context.byteOffset = context.stack[context.stack.length - 1];
  context.dispatchState = context.stack[context.stack.length - 2];
  context.stack.length -= 2;
}
```

- **Does:** `pc = top; state = top-1`; pop 2.
- **Tracer:** Follow when both are VM values; otherwise stop with a diagnostic.

## 3. Conditional jumps

How `if` and `while` are compiled. All pop the tested value except jump-if-slot-not-null.

### jump if top is truthy (u24 target)

```js
function(context) {
  var value1 = vmBytecode[context.byteOffset] | (vmBytecode[context.byteOffset + 1] | vmBytecode[context.byteOffset + 2] << 8) << 8;
  var value2 = vmBytecode[context.byteOffset + 3];
  context.byteOffset += 4;
  if (context.stack[context.stack.length - 1]) {
    context.byteOffset = value1;
    context.dispatchState = value2;
  }
  context.stack.length -= 1;
}
```

- **Does:** `if (top) goto target`; pop 1.
- **Tracer:** Condition is a VM value → **follow** the taken branch. Program value → emit an AST `if` and trace both successors; a program-value branch back to a PC/state already on the current path is a loop → AST `while`.

### jump if top is truthy (u16 target)

```js
function(context) {
  var value1 = vmBytecode[context.byteOffset] | vmBytecode[context.byteOffset + 1] << 8;
  var value2 = vmBytecode[context.byteOffset + 2];
  context.byteOffset += 3;
  if (context.stack[context.stack.length - 1]) {
    context.byteOffset = value1;
    context.dispatchState = value2;
  }
  context.stack.length -= 1;
}
```

- **Does:** Same, `u16` target.
- **Tracer:** Condition is a VM value → **follow** the taken branch. Program value → emit an AST `if` and trace both successors; a program-value branch back to a PC/state already on the current path is a loop → AST `while`.

### jump if top is truthy (u8 target)

```js
function(context) {
  var value1 = vmBytecode[context.byteOffset];
  var value2 = vmBytecode[context.byteOffset + 1];
  context.byteOffset += 2;
  if (context.stack[context.stack.length - 1]) {
    context.byteOffset = value1;
    context.dispatchState = value2;
  }
  context.stack.length -= 1;
}
```

- **Does:** Same, `u8` target.
- **Tracer:** Condition is a VM value → **follow** the taken branch. Program value → emit an AST `if` and trace both successors; a program-value branch back to a PC/state already on the current path is a loop → AST `while`.

### jump if top is falsy (u24 target)

```js
function(context) {
  var value1 = vmBytecode[context.byteOffset] | (vmBytecode[context.byteOffset + 1] | vmBytecode[context.byteOffset + 2] << 8) << 8;
  var value2 = vmBytecode[context.byteOffset + 3];
  context.byteOffset += 4;
  if (!context.stack[context.stack.length - 1]) {
    context.byteOffset = value1;
    context.dispatchState = value2;
  }
  context.stack.length -= 1;
}
```

- **Does:** `if (!top) goto target`; pop 1.
- **Tracer:** Condition is a VM value → **follow** the taken branch. Program value → emit an AST `if` and trace both successors; a program-value branch back to a PC/state already on the current path is a loop → AST `while`.

### jump if top is falsy (u16 target)

```js
function(context) {
  var value1 = vmBytecode[context.byteOffset] | vmBytecode[context.byteOffset + 1] << 8;
  var value2 = vmBytecode[context.byteOffset + 2];
  context.byteOffset += 3;
  if (!context.stack[context.stack.length - 1]) {
    context.byteOffset = value1;
    context.dispatchState = value2;
  }
  context.stack.length -= 1;
}
```

- **Does:** Same, `u16` target.
- **Tracer:** Condition is a VM value → **follow** the taken branch. Program value → emit an AST `if` and trace both successors; a program-value branch back to a PC/state already on the current path is a loop → AST `while`.

### jump if top is falsy (u8 target)

```js
function(context) {
  var value1 = vmBytecode[context.byteOffset];
  var value2 = vmBytecode[context.byteOffset + 1];
  context.byteOffset += 2;
  if (!context.stack[context.stack.length - 1]) {
    context.byteOffset = value1;
    context.dispatchState = value2;
  }
  context.stack.length -= 1;
}
```

- **Does:** Same, `u8` target.
- **Tracer:** Condition is a VM value → **follow** the taken branch. Program value → emit an AST `if` and trace both successors; a program-value branch back to a PC/state already on the current path is a loop → AST `while`.

### jump if slot is not null

```js
function(context) {
  var value1 = vmBytecode[context.byteOffset];
  var value2 = vmBytecode[context.byteOffset + 1] | (vmBytecode[context.byteOffset + 2] | vmBytecode[context.byteOffset + 3] << 8) << 8;
  var value3 = vmBytecode[context.byteOffset + 4];
  context.byteOffset += 5;
  var value4 = context.memory.readSlot(value1);
  var value5 = null != value4;
  if (value5) {
    context.byteOffset = value2;
    context.dispatchState = value3;
  }
}
```

- **Does:** `if (readSlot(slot) != null) goto target`; no pop.
- **Tracer:** Condition is a VM value → **follow** the taken branch. Program value → emit an AST `if` and trace both successors; a program-value branch back to a PC/state already on the current path is a loop → AST `while`.

### jump unless top < slot

```js
function(context) {
  var value1 = vmBytecode[context.byteOffset];
  var value2 = vmBytecode[context.byteOffset + 1] | (vmBytecode[context.byteOffset + 2] | vmBytecode[context.byteOffset + 3] << 8) << 8;
  var value3 = vmBytecode[context.byteOffset + 4];
  context.byteOffset += 5;
  var value4 = context.memory.readSlot(value1);
  var value5 = context.stack[context.stack.length - 1];
  var value6 = value5 < value4;
  if (!value6) {
    context.byteOffset = value2;
    context.dispatchState = value3;
  }
  context.stack.length -= 1;
}
```

- **Does:** `if (!(top < readSlot(slot))) goto target`; pop 1.
- **Tracer:** Condition is a VM value → **follow** the taken branch. Program value → emit an AST `if` and trace both successors; a program-value branch back to a PC/state already on the current path is a loop → AST `while`.

## 4. Calls

Push `{returnByteOffset: pc after operands, returnDispatchState: state}` on `callFrameStack`, then jump.

### call (u24 target)

```js
function(context) {
  var value1 = vmBytecode[context.byteOffset] | (vmBytecode[context.byteOffset + 1] | vmBytecode[context.byteOffset + 2] << 8) << 8;
  var value2 = vmBytecode[context.byteOffset + 3];
  context.byteOffset += 4;
  context.callFrameStack.push({
    returnByteOffset: context.byteOffset,
    returnDispatchState: context.dispatchState
  });
  context.byteOffset = value1;
  context.dispatchState = value2;
}
```

- **Does:** Pure call.
- **Tracer:** VM value → **follow**. Maintain `callFrameStack`.

### call (u16 target)

```js
function(context) {
  var value1 = vmBytecode[context.byteOffset] | vmBytecode[context.byteOffset + 1] << 8;
  var value2 = vmBytecode[context.byteOffset + 2];
  context.byteOffset += 3;
  context.callFrameStack.push({
    returnByteOffset: context.byteOffset,
    returnDispatchState: context.dispatchState
  });
  context.byteOffset = value1;
  context.dispatchState = value2;
}
```

- **Does:** Pure call, `u16` target.
- **Tracer:** VM value → **follow**. Maintain `callFrameStack`.

## 4a. Fused work + call

Superinstructions: some work, then a call. The call target is always the last `u24 target, u8 state` operand pair.

### decode-string property read, then call

```js
function(context) {
  var value1 = vmBytecode[context.byteOffset] | vmBytecode[context.byteOffset + 1] << 8;
  var value2 = vmBytecode[context.byteOffset + 2] | (vmBytecode[context.byteOffset + 3] | vmBytecode[context.byteOffset + 4] << 8) << 8;
  var value3 = vmBytecode[context.byteOffset + 5];
  context.byteOffset += 6;
  var value4 = context.stack[context.stack.length - 1];
  var value5 = value4;
  var value6 = vmDecodeString(value5, value1, vmStringTable, vmStringCache, vmDecodeBase64, String.fromCharCode);
  var value7 = context.stack[context.stack.length - 2];
  var value8 = value7[value6];
  context.callFrameStack.push({
    returnByteOffset: context.byteOffset,
    returnDispatchState: context.dispatchState
  });
  context.byteOffset = value2;
  context.dispatchState = value3;
  context.stack[context.stack.length - 2] = value8;
  context.stack.length -= 1;
}
```

- **Does:** `name = vmDecodeString(top, key)`; `top-1 = (top-1)[name]`; pop 1.
- **Tracer:** Trace the work as ordinary HandlerIR, then **follow** the call (push the frame on the tracer's `callFrameStack`).

### string-table property read into a slot, then call

```js
function(context) {
  var value1 = vmStringTable[vmBytecode[context.byteOffset] | vmBytecode[context.byteOffset + 1] << 8];
  var value2 = vmBytecode[context.byteOffset + 2];
  var value3 = vmBytecode[context.byteOffset + 3] | (vmBytecode[context.byteOffset + 4] | vmBytecode[context.byteOffset + 5] << 8) << 8;
  var value4 = vmBytecode[context.byteOffset + 6];
  context.byteOffset += 7;
  var value5 = context.stack[context.stack.length - 1];
  var value6 = value5[value1];
  context.memory.writeSlot(value2, value6);
  context.callFrameStack.push({
    returnByteOffset: context.byteOffset,
    returnDispatchState: context.dispatchState
  });
  context.byteOffset = value3;
  context.dispatchState = value4;
  context.stack.length -= 1;
}
```

- **Does:** `writeSlot(slot, top[strings[str]])`; pop 1.
- **Tracer:** Trace the work as ordinary HandlerIR, then **follow** the call (push the frame on the tracer's `callFrameStack`).

### copy slot to slot, then call

```js
function(context) {
  var value1 = vmBytecode[context.byteOffset];
  var value2 = vmBytecode[context.byteOffset + 1];
  var value3 = vmBytecode[context.byteOffset + 2] | (vmBytecode[context.byteOffset + 3] | vmBytecode[context.byteOffset + 4] << 8) << 8;
  var value4 = vmBytecode[context.byteOffset + 5];
  context.byteOffset += 6;
  var value5 = context.memory.readSlot(value1);
  context.memory.writeSlot(value2, value5);
  context.callFrameStack.push({
    returnByteOffset: context.byteOffset,
    returnDispatchState: context.dispatchState
  });
  context.byteOffset = value3;
  context.dispatchState = value4;
}
```

- **Does:** `writeSlot(to, readSlot(from))`.
- **Tracer:** Trace the work as ordinary HandlerIR, then **follow** the call (push the frame on the tracer's `callFrameStack`).

### store two stack values into slots, then call

```js
function(context) {
  var value1 = vmBytecode[context.byteOffset];
  var value2 = vmBytecode[context.byteOffset + 1];
  var value3 = vmBytecode[context.byteOffset + 2] | (vmBytecode[context.byteOffset + 3] | vmBytecode[context.byteOffset + 4] << 8) << 8;
  var value4 = vmBytecode[context.byteOffset + 5];
  context.byteOffset += 6;
  var value5 = context.stack[context.stack.length - 1];
  context.memory.writeSlot(value1, value5);
  var value6 = context.stack[context.stack.length - 2];
  context.memory.writeSlot(value2, value6);
  context.callFrameStack.push({
    returnByteOffset: context.byteOffset,
    returnDispatchState: context.dispatchState
  });
  context.byteOffset = value3;
  context.dispatchState = value4;
  context.stack.length -= 2;
}
```

- **Does:** `writeSlot(a, top); writeSlot(b, top-1)`; pop 2.
- **Tracer:** Trace the work as ordinary HandlerIR, then **follow** the call (push the frame on the tracer's `callFrameStack`).

### define property, then call

```js
function(context) {
  var value1 = vmBytecode[context.byteOffset];
  var value2 = vmBytecode[context.byteOffset + 1] | (vmBytecode[context.byteOffset + 2] | vmBytecode[context.byteOffset + 3] << 8) << 8;
  var value3 = vmBytecode[context.byteOffset + 4];
  context.byteOffset += 5;
  var value4 = context.stack[context.stack.length - 3];
  var value5 = context.stack[context.stack.length - 2];
  var value6 = context.stack[context.stack.length - 1];
  Object.defineProperty(value4, value5, {
    writable: true,
    configurable: true,
    enumerable: true,
    value: value6
  });
  context.callFrameStack.push({
    returnByteOffset: context.byteOffset,
    returnDispatchState: context.dispatchState
  });
  context.byteOffset = value2;
  context.dispatchState = value3;
  var value7 = context.stack.length - 3;
  context.stack[value7] = value4;
  context.stack[value7 + 1] = value1;
  context.stack.length -= 1;
}
```

- **Does:** `Object.defineProperty(top-2, top-1, {value: top, …})`; leaves `[obj, imm]`.
- **Tracer:** Trace the work as ordinary HandlerIR, then **follow** the call (push the frame on the tracer's `callFrameStack`).

### push two slots, then call

```js
function(context) {
  var value1 = vmBytecode[context.byteOffset] | vmBytecode[context.byteOffset + 1] << 8;
  var value2 = vmBytecode[context.byteOffset + 2];
  var value3 = vmBytecode[context.byteOffset + 3] | (vmBytecode[context.byteOffset + 4] | vmBytecode[context.byteOffset + 5] << 8) << 8;
  var value4 = vmBytecode[context.byteOffset + 6];
  context.byteOffset += 7;
  var value5 = context.memory.readSlot(value1);
  var value6 = context.memory.readSlot(value2);
  context.callFrameStack.push({
    returnByteOffset: context.byteOffset,
    returnDispatchState: context.dispatchState
  });
  context.byteOffset = value3;
  context.dispatchState = value4;
  var value7 = context.stack.length;
  context.stack[value7] = value5;
  context.stack[value7 + 1] = value6;
}
```

- **Does:** Push `readSlot(a)` (`u16` slot) and `readSlot(b)`.
- **Tracer:** Trace the work as ordinary HandlerIR, then **follow** the call (push the frame on the tracer's `callFrameStack`).

### store top into a slot, then call

```js
function(context) {
  var value1 = vmBytecode[context.byteOffset];
  var value2 = vmBytecode[context.byteOffset + 1] | (vmBytecode[context.byteOffset + 2] | vmBytecode[context.byteOffset + 3] << 8) << 8;
  var value3 = vmBytecode[context.byteOffset + 4];
  context.byteOffset += 5;
  var value4 = context.stack[context.stack.length - 1];
  context.memory.writeSlot(value1, value4);
  context.callFrameStack.push({
    returnByteOffset: context.byteOffset,
    returnDispatchState: context.dispatchState
  });
  context.byteOffset = value2;
  context.dispatchState = value3;
  context.stack[context.stack.length - 1] = value4;
}
```

- **Does:** `writeSlot(slot, top)`.
- **Tracer:** Trace the work as ordinary HandlerIR, then **follow** the call (push the frame on the tracer's `callFrameStack`).

### push two immediates, then call

```js
function(context) {
  var value1 = vmBytecode[context.byteOffset];
  var value2 = vmBytecode[context.byteOffset + 1];
  var value3 = vmBytecode[context.byteOffset + 2] | (vmBytecode[context.byteOffset + 3] | vmBytecode[context.byteOffset + 4] << 8) << 8;
  var value4 = vmBytecode[context.byteOffset + 5];
  context.byteOffset += 6;
  context.callFrameStack.push({
    returnByteOffset: context.byteOffset,
    returnDispatchState: context.dispatchState
  });
  context.byteOffset = value3;
  context.dispatchState = value4;
  var value5 = context.stack.length;
  context.stack[value5] = value1;
  context.stack[value5 + 1] = value2;
}
```

- **Does:** Push the `u8` immediates `a`, `b`.
- **Tracer:** Trace the work as ordinary HandlerIR, then **follow** the call (push the frame on the tracer's `callFrameStack`).

### decode top as a string and push a slot, then call

```js
function(context) {
  var value1 = vmBytecode[context.byteOffset] | vmBytecode[context.byteOffset + 1] << 8;
  var value2 = vmBytecode[context.byteOffset + 2];
  var value3 = vmBytecode[context.byteOffset + 3] | (vmBytecode[context.byteOffset + 4] | vmBytecode[context.byteOffset + 5] << 8) << 8;
  var value4 = vmBytecode[context.byteOffset + 6];
  context.byteOffset += 7;
  var value5 = context.stack[context.stack.length - 1];
  var value6 = value5;
  var value7 = vmDecodeString(value6, value1, vmStringTable, vmStringCache, vmDecodeBase64, String.fromCharCode);
  var value8 = context.memory.readSlot(value2);
  context.callFrameStack.push({
    returnByteOffset: context.byteOffset,
    returnDispatchState: context.dispatchState
  });
  context.byteOffset = value3;
  context.dispatchState = value4;
  var value9 = context.stack.length - 1;
  context.stack[value9] = value7;
  context.stack[value9 + 1] = value8;
}
```

- **Does:** `top = vmDecodeString(top, key)`; push `readSlot(slot)`.
- **Tracer:** Trace the work as ordinary HandlerIR, then **follow** the call (push the frame on the tracer's `callFrameStack`).

### replace top with a slot, then call

```js
function(context) {
  var value1 = vmBytecode[context.byteOffset];
  var value2 = vmBytecode[context.byteOffset + 1] | (vmBytecode[context.byteOffset + 2] | vmBytecode[context.byteOffset + 3] << 8) << 8;
  var value3 = vmBytecode[context.byteOffset + 4];
  context.byteOffset += 5;
  var value4 = context.memory.readSlot(value1);
  context.callFrameStack.push({
    returnByteOffset: context.byteOffset,
    returnDispatchState: context.dispatchState
  });
  context.byteOffset = value2;
  context.dispatchState = value3;
  context.stack[context.stack.length - 1] = value4;
}
```

- **Does:** `top = readSlot(slot)`.
- **Tracer:** Trace the work as ordinary HandlerIR, then **follow** the call (push the frame on the tracer's `callFrameStack`).

### property write, then call

```js
function(context) {
  "use strict";
  var value1 = vmBytecode[context.byteOffset];
  var value2 = vmBytecode[context.byteOffset + 1] | (vmBytecode[context.byteOffset + 2] | vmBytecode[context.byteOffset + 3] << 8) << 8;
  var value3 = vmBytecode[context.byteOffset + 4];
  context.byteOffset += 5;
  var value4 = context.stack[context.stack.length - 3];
  var value5 = context.stack[context.stack.length - 2];
  var value6 = context.stack[context.stack.length - 1];
  value4[value5] = value6;
  var value7 = context.memory.readSlot(value1);
  context.callFrameStack.push({
    returnByteOffset: context.byteOffset,
    returnDispatchState: context.dispatchState
  });
  context.byteOffset = value2;
  context.dispatchState = value3;
  context.stack[context.stack.length - 3] = value7;
  context.stack.length -= 2;
}
```

- **Does:** `(top-2)[top-1] = top`; `top-2 = readSlot(slot)`; pop 2.
- **Tracer:** Trace the work as ordinary HandlerIR, then **follow** the call (push the frame on the tracer's `callFrameStack`).

### store top into a slot and load another, then call

```js
function(context) {
  var value1 = vmBytecode[context.byteOffset];
  var value2 = vmBytecode[context.byteOffset + 1];
  var value3 = vmBytecode[context.byteOffset + 2] | (vmBytecode[context.byteOffset + 3] | vmBytecode[context.byteOffset + 4] << 8) << 8;
  var value4 = vmBytecode[context.byteOffset + 5];
  context.byteOffset += 6;
  var value5 = context.stack[context.stack.length - 1];
  context.memory.writeSlot(value1, value5);
  var value6 = context.memory.readSlot(value2);
  context.callFrameStack.push({
    returnByteOffset: context.byteOffset,
    returnDispatchState: context.dispatchState
  });
  context.byteOffset = value3;
  context.dispatchState = value4;
  context.stack[context.stack.length - 1] = value6;
}
```

- **Does:** `writeSlot(w, top); top = readSlot(r)`.
- **Tracer:** Trace the work as ordinary HandlerIR, then **follow** the call (push the frame on the tracer's `callFrameStack`).

### property read by byte key, then call

```js
function(context) {
  var value1 = vmBytecode[context.byteOffset];
  var value2 = vmBytecode[context.byteOffset + 1] | (vmBytecode[context.byteOffset + 2] | vmBytecode[context.byteOffset + 3] << 8) << 8;
  var value3 = vmBytecode[context.byteOffset + 4];
  context.byteOffset += 5;
  var value4 = context.stack[context.stack.length - 1];
  var value5 = value4[value1];
  context.callFrameStack.push({
    returnByteOffset: context.byteOffset,
    returnDispatchState: context.dispatchState
  });
  context.byteOffset = value2;
  context.dispatchState = value3;
  context.stack[context.stack.length - 1] = value5;
}
```

- **Does:** `top = top[key]` with a `u8` key.
- **Tracer:** Trace the work as ordinary HandlerIR, then **follow** the call (push the frame on the tracer's `callFrameStack`).

### property read by string-table key, then call

```js
function(context) {
  var value1 = vmStringTable[vmBytecode[context.byteOffset] | vmBytecode[context.byteOffset + 1] << 8];
  var value2 = vmBytecode[context.byteOffset + 2] | (vmBytecode[context.byteOffset + 3] | vmBytecode[context.byteOffset + 4] << 8) << 8;
  var value3 = vmBytecode[context.byteOffset + 5];
  context.byteOffset += 6;
  var value4 = context.stack[context.stack.length - 1];
  var value5 = value4[value1];
  context.callFrameStack.push({
    returnByteOffset: context.byteOffset,
    returnDispatchState: context.dispatchState
  });
  context.byteOffset = value2;
  context.dispatchState = value3;
  context.stack[context.stack.length - 1] = value5;
}
```

- **Does:** `top = top[strings[str]]`.
- **Tracer:** Trace the work as ordinary HandlerIR, then **follow** the call (push the frame on the tracer's `callFrameStack`).

### push two slots, then call

```js
function(context) {
  var value1 = vmBytecode[context.byteOffset];
  var value2 = vmBytecode[context.byteOffset + 1];
  var value3 = vmBytecode[context.byteOffset + 2] | (vmBytecode[context.byteOffset + 3] | vmBytecode[context.byteOffset + 4] << 8) << 8;
  var value4 = vmBytecode[context.byteOffset + 5];
  context.byteOffset += 6;
  var value5 = context.memory.readSlot(value1);
  var value6 = context.memory.readSlot(value2);
  context.callFrameStack.push({
    returnByteOffset: context.byteOffset,
    returnDispatchState: context.dispatchState
  });
  context.byteOffset = value3;
  context.dispatchState = value4;
  var value7 = context.stack.length;
  context.stack[value7] = value5;
  context.stack[value7 + 1] = value6;
}
```

- **Does:** Push `readSlot(a)`, `readSlot(b)` (`u8` slots).
- **Tracer:** Trace the work as ordinary HandlerIR, then **follow** the call (push the frame on the tracer's `callFrameStack`).

### push an immediate and a slot, then call

```js
function(context) {
  var value1 = vmBytecode[context.byteOffset];
  var value2 = vmBytecode[context.byteOffset + 1];
  var value3 = vmBytecode[context.byteOffset + 2] | (vmBytecode[context.byteOffset + 3] | vmBytecode[context.byteOffset + 4] << 8) << 8;
  var value4 = vmBytecode[context.byteOffset + 5];
  context.byteOffset += 6;
  var value5 = context.memory.readSlot(value2);
  context.callFrameStack.push({
    returnByteOffset: context.byteOffset,
    returnDispatchState: context.dispatchState
  });
  context.byteOffset = value3;
  context.dispatchState = value4;
  var value6 = context.stack.length;
  context.stack[value6] = value1;
  context.stack[value6 + 1] = value5;
}
```

- **Does:** Push `imm`, `readSlot(slot)`.
- **Tracer:** Trace the work as ordinary HandlerIR, then **follow** the call (push the frame on the tracer's `callFrameStack`).

## 5. Returns

Pop `callFrameStack` and jump to `returnByteOffset` / `returnDispatchState`.

### return

```js
function(context) {
  var value1 = context.callFrameStack.pop();
  context.byteOffset = value1.returnByteOffset;
  context.dispatchState = value1.returnDispatchState;
}
```

- **Does:** Pure return.
- **Tracer:** VM value → **follow**. Maintain `callFrameStack`.

## 5a. Fused work + return

Superinstructions: some work, then a return.

### call a program function, then return

```js
function(context) {
  var value1 = vmBytecode[context.byteOffset];
  var value2 = vmBytecode[context.byteOffset + 1];
  context.byteOffset += 2;
  var value3 = context.memory.readSlot(value1);
  var value4 = context.stack[context.stack.length - 4];
  var value5 = context.stack[context.stack.length - 3];
  var value6 = context.stack[context.stack.length - 2];
  var value7 = context.stack[context.stack.length - 1];
  var value8 = value4;
  var value9 = value8(value5, value6, value7, value3);
  context.memory.writeSlot(value2, value9);
  var value10 = context.callFrameStack.pop();
  context.byteOffset = value10.returnByteOffset;
  context.dispatchState = value10.returnDispatchState;
  context.stack.length -= 4;
}
```

- **Does:** `writeSlot(out, (top-3)(top-2, top-1, top, readSlot(slot)))`; pop 4.
- **Tracer:** The called function is a program value → emit an AST call node. Trace the work as ordinary HandlerIR, then **follow** the return (pop the tracer's `callFrameStack`).

### push two slots, then return

```js
function(context) {
  var value1 = vmBytecode[context.byteOffset];
  var value2 = vmBytecode[context.byteOffset + 1];
  context.byteOffset += 2;
  var value3 = context.memory.readSlot(value1);
  var value4 = context.memory.readSlot(value2);
  var value5 = context.callFrameStack.pop();
  context.byteOffset = value5.returnByteOffset;
  context.dispatchState = value5.returnDispatchState;
  var value6 = context.stack.length;
  context.stack[value6] = value3;
  context.stack[value6 + 1] = value4;
}
```

- **Does:** Push `readSlot(a)`, `readSlot(b)`.
- **Tracer:** Trace the work as ordinary HandlerIR, then **follow** the return (pop the tracer's `callFrameStack`).

### define property and push a decoded string, then return

```js
function(context) {
  var value1 = vmStringTable[vmBytecode[context.byteOffset] | vmBytecode[context.byteOffset + 1] << 8];
  var value2 = vmBytecode[context.byteOffset + 2] | vmBytecode[context.byteOffset + 3] << 8;
  context.byteOffset += 4;
  var value3 = context.stack[context.stack.length - 3];
  var value4 = context.stack[context.stack.length - 2];
  var value5 = context.stack[context.stack.length - 1];
  Object.defineProperty(value3, value4, {
    writable: true,
    configurable: true,
    enumerable: true,
    value: value5
  });
  var value6 = value1;
  var value7 = vmDecodeString(value6, value2, vmStringTable, vmStringCache, vmDecodeBase64, String.fromCharCode);
  var value8 = context.callFrameStack.pop();
  context.byteOffset = value8.returnByteOffset;
  context.dispatchState = value8.returnDispatchState;
  var value9 = context.stack.length - 3;
  context.stack[value9] = value3;
  context.stack[value9 + 1] = value7;
  context.stack.length -= 1;
}
```

- **Does:** `Object.defineProperty(top-2, top-1, {value: top})`; replace with `[obj, vmDecodeString(strings[str], key)]`.
- **Tracer:** Trace the work as ordinary HandlerIR, then **follow** the return (pop the tracer's `callFrameStack`).

### push a slot and a decoded string, then return

```js
function(context) {
  var value1 = vmBytecode[context.byteOffset];
  var value2 = vmStringTable[vmBytecode[context.byteOffset + 1] | vmBytecode[context.byteOffset + 2] << 8];
  var value3 = vmBytecode[context.byteOffset + 3] | vmBytecode[context.byteOffset + 4] << 8;
  context.byteOffset += 5;
  var value4 = context.memory.readSlot(value1);
  var value5 = value2;
  var value6 = vmDecodeString(value5, value3, vmStringTable, vmStringCache, vmDecodeBase64, String.fromCharCode);
  var value7 = context.callFrameStack.pop();
  context.byteOffset = value7.returnByteOffset;
  context.dispatchState = value7.returnDispatchState;
  var value8 = context.stack.length;
  context.stack[value8] = value4;
  context.stack[value8 + 1] = value6;
}
```

- **Does:** Push `readSlot(slot)`, `vmDecodeString(strings[str], key)`.
- **Tracer:** Trace the work as ordinary HandlerIR, then **follow** the return (pop the tracer's `callFrameStack`).

### string-table property read, then return

```js
function(context) {
  var value1 = vmStringTable[vmBytecode[context.byteOffset] | vmBytecode[context.byteOffset + 1] << 8];
  context.byteOffset += 2;
  var value2 = context.stack[context.stack.length - 1];
  var value3 = value2[value1];
  var value4 = context.callFrameStack.pop();
  context.byteOffset = value4.returnByteOffset;
  context.dispatchState = value4.returnDispatchState;
  context.stack[context.stack.length - 1] = value3;
}
```

- **Does:** `top = top[strings[str]]`.
- **Tracer:** Trace the work as ordinary HandlerIR, then **follow** the return (pop the tracer's `callFrameStack`).

### XOR property write, then return

```js
function(context) {
  "use strict";
  var value1 = vmBytecode[context.byteOffset];
  context.byteOffset += 1;
  var value2 = context.stack[context.stack.length - 1];
  var value3 = value2 ^ value1;
  var value4 = context.stack[context.stack.length - 3];
  var value5 = context.stack[context.stack.length - 2];
  value4[value5] = value3;
  var value6 = context.callFrameStack.pop();
  context.byteOffset = value6.returnByteOffset;
  context.dispatchState = value6.returnDispatchState;
  context.stack.length -= 3;
}
```

- **Does:** `(top-2)[top-1] = top ^ mask`; pop 3.
- **Tracer:** Trace the work as ordinary HandlerIR, then **follow** the return (pop the tracer's `callFrameStack`).

## 6. Exceptions: try / catch / finally

Maintained concretely: `tryFrameStack`, `errorStack`, `thrownValueStack`.

### enter try (u24 handler)

```js
function(context) {
  var value1 = vmBytecode[context.byteOffset] | (vmBytecode[context.byteOffset + 1] | vmBytecode[context.byteOffset + 2] << 8) << 8;
  var value2 = vmBytecode[context.byteOffset + 3];
  context.byteOffset += 4;
  context.tryFrameStack.push({
    handlerByteOffset: value1,
    handlerDispatchState: value2,
    errorEntryCount: 0
  });
}
```

- **Does:** Push try frame `{handlerByteOffset, handlerDispatchState, errorEntryCount: 0}`.
- **Tracer:** Push the frame on the tracer's `tryFrameStack`; the frame's handler is the catch/finally entry.

### enter try (u16 handler)

```js
function(context) {
  var value1 = vmBytecode[context.byteOffset] | vmBytecode[context.byteOffset + 1] << 8;
  var value2 = vmBytecode[context.byteOffset + 2];
  context.byteOffset += 3;
  context.tryFrameStack.push({
    handlerByteOffset: value1,
    handlerDispatchState: value2,
    errorEntryCount: 0
  });
}
```

- **Does:** Same, `u16` handler offset.
- **Tracer:** Same as enter try (u24 handler).

### exit try (normal path, try/catch)

```js
function(context) {
  context.tryFrameStack.pop();
}
```

- **Does:** Pop the try frame.
- **Tracer:** Pop the tracer's try frame.

### enter finally (normal path)

```js
function(context) {
  context.errorStack.push({
    wasExceptionHandled: false,
    savedByteOffset: context.byteOffset,
    savedDispatchState: context.dispatchState
  });
  var value1 = context.tryFrameStack.pop();
  context.byteOffset = value1.handlerByteOffset;
  context.dispatchState = value1.handlerDispatchState;
}
```

- **Does:** Push `{wasExceptionHandled: false, saved pc/state}`; pop the try frame; jump to its handler.
- **Tracer:** Follow into the finally body with no pending error.

### catch (e): bind the error

```js
function(context) {
  var value1 = vmBytecode[context.byteOffset];
  context.byteOffset += 1;
  context.errorStack.pop();
  context.memory.writeSlot(value1, context.thrownValueStack.pop());
}
```

- **Does:** Pop the error record; `writeSlot(slot, thrownValueStack.pop())`.
- **Tracer:** Bind the caught error (an AST value) to the slot.

### catch without a binding

```js
function(context) {
  var value1 = context.errorStack.pop();
  if (value1.wasExceptionHandled) {
    context.thrownValueStack.pop();
  }
}
```

- **Does:** Pop the error record; drop the pending error if any.
- **Tracer:** Drop the pending error.

### end finally

```js
function(context) {
  var value1 = context.errorStack.pop();
  if (value1.wasExceptionHandled) {
    throw context.thrownValueStack.pop();
  }
  context.byteOffset = value1.savedByteOffset;
  context.dispatchState = value1.savedDispatchState;
}
```

- **Does:** Pop the error record; if an error is pending, **rethrow** it; otherwise jump to the saved pc/state.
- **Tracer:** Pending error → AST `throw` of that error. Otherwise follow the saved pc/state.
