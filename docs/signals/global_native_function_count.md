# GlobalNativeFunctionCount

[Signals index](README.md) · [data-shape example](global_native_function_count.js) · [schema format](schema.md)

## What it collects

Takes the first 2000 own property names of the global object (window) and counts those that are non-enumerable data properties holding a function whose source text still reads as native code ("{ [native code] }"). That covers built-ins like Object, Array and parseInt and DOM interfaces like HTMLElement; enumerable globals such as fetch, setTimeout or page-defined functions are not counted.

## Why

The count is characteristic of the browser engine and version, and drops when natives are deleted or replaced with non-native wrappers, so it likely helps verify the claimed browser and spot tampered or emulated environments.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `nativeFunctionCount` | number | Number of non-enumerable, function-valued global properties that stringify as native code; an integer from 0 to 2000 (typically several hundred in a real browser). |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "nativeFunctionCount": 977
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.GlobalNativeFunctionCount` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
