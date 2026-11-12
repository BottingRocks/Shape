# CrossOriginIsolationFlags

[Signals index](README.md) · [data-shape example](cross_origin_isolation_flags.js) · [schema format](schema.md)

## What it collects

Checks two facts about the page's cross-origin isolation and sends them once as a packed byte: whether window.crossOriginIsolated is true, and whether the SharedArrayBuffer constructor is available.

## Why

Browsers expose SharedArrayBuffer only on cross-origin isolated pages (with some legacy and flag exceptions), so comparing the two bits likely reveals the engine/version or a tampered environment.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `isolationFlags` | flags (byte) | Two flags, other bits 0: sharedArrayBufferAvailable (typeof SharedArrayBuffer is 'function'), crossOriginIsolated (window.crossOriginIsolated is true; 0 if false or missing). Each bit is 1 if true, 0 if false. |

Flag names of `isolationFlags` (card order; the bit of each flag is per build, see the schema's `bits`):

- `sharedArrayBufferAvailable`
- `crossOriginIsolated`

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "crossOriginIsolated": false,
  "sharedArrayBufferAvailable": false
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.CrossOriginIsolationFlags` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Flag bytes are `type: "flags"` with `bits` mapping each flag name above to its bit (0 = least significant) in this build. If the bits cannot all be named the field stays `type: "byte"` and is listed under `diagnostics.flags_fallback`.
- When the entire resolved message is one packed flag byte, fresh schemas/readable payloads expose its named booleans at the signal root. The schema retains the original byte under `physicalFlags`; the wrapper is a projection name, not a transmitted object.
