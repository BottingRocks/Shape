# IframeRealmNativeFunctionCount

[Signals index](README.md) · [data-shape example](iframe_realm_native_function_count.js) · [schema format](schema.md)

## What it collects

Creates a hidden nested iframe to get a fresh JavaScript realm, then takes the first 2000 own property names of that iframe's window and keeps the non-enumerable data properties. After the iframe is removed, it counts (in small time slices) how many of those values are functions whose source text reads as native code. It sends that count.

## Why

A fresh iframe realm shows the browser's untouched built-in surface, so comparing this count with the main window's likely reveals the engine/version and patches or automation frameworks that also tamper with new frames.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `nativeFunctionCount` | number | Number of non-enumerable, function-valued global properties in the fresh iframe realm that stringify as native code; an integer from 0 to 2000 (0 if nothing qualified). If the hidden iframe cannot be created or never reports back, nothing is sent. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "nativeFunctionCount": 979
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.IframeRealmNativeFunctionCount` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
