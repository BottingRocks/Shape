# DeviceMemory

[Signals index](README.md) · [data-shape example](device_memory.js) · [schema format](schema.md)

## What it collects

Reads navigator.deviceMemory once and sends it. If the property is missing (null or undefined), it sends a missing marker instead.

## Why

Device memory is a coarse hardware trait that adds entropy to a fingerprint and can be cross-checked against the claimed device and browser (e.g. it does not exist in Firefox or Safari).

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `deviceMemory` | number or null (tag byte) | Approximate RAM in gigabytes as reported by the browser (Chrome reports buckets such as 0.25, 0.5, 1, 2, 4, 8), or a null marker if navigator.deviceMemory is missing. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "deviceMemory": 32
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.DeviceMemory` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
