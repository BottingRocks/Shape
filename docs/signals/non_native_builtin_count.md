# NonNativeBuiltinCount

[Signals index](README.md) · [data-shape example](non_native_builtin_count.js) · [schema format](schema.md)

## What it collects

At startup the script captures a fixed list of 27 built-in functions (TypeError, Object, RegExp, Array, Function.prototype.bind/call/apply, Array.prototype.push/slice/join/map, JSON.stringify, Object.defineProperty, Math.floor, String.prototype.indexOf, Uint8Array and others). It prints each one with Function.prototype.toString and counts how many do NOT look like '{ [native code] }'.

## Why

A non-zero count means some core built-ins were already replaced by JavaScript (polyfills, monkey-patches, automation or instrumentation shims), which likely flags a tampered or instrumented environment.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `nonNativeCount` | number | How many of the 27 listed built-ins have source text that does not look native, 0 to 27. 0 on an unmodified browser. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "nonNativeCount": 0
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.NonNativeBuiltinCount` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
