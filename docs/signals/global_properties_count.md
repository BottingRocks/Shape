# GlobalPropertiesCount

[Signals index](README.md) · [data-shape example](global_properties_count.js) · [schema format](schema.md)

## What it collects

Counts the own property names of the global object (window), using Object.getOwnPropertyNames(window).length. This includes enumerable and non-enumerable names, but not symbols or properties inherited from Window's prototypes. It runs once at startup and sends that one number.

## Why

The count depends on browser engine and version, and it rises when the page, extensions or automation tools add globals, so it likely serves as both an environment fingerprint and a tampering/automation hint.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `globalPropertyCount` | number | Number of own string-named properties on window; a non-negative integer, usually around a thousand in a real browser. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "globalPropertyCount": 1261
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.GlobalPropertiesCount` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
