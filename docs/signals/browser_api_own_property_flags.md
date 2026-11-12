# BrowserApiOwnPropertyFlags

[Signals index](README.md) · [data-shape example](browser_api_own_property_flags.js) · [schema format](schema.md)

## What it collects

At init, checks with hasOwnProperty whether four names are defined directly on specific objects: "clipboard" on Navigator.prototype, "ServiceWorker" and "ApplePaySession" on the global object, and "WebkitOverflowScrolling" on CSSStyleDeclaration.prototype. Only own properties count; inherited values or shims added on instances give 0, and no property value is read. The four results are sent as one byte of flags.

## Why

Which of these APIs a browser defines, and where, depends on the engine, platform and secure-context state; two of them are Apple/WebKit-specific. This likely helps confirm the claimed browser and OS (for example real Safari) and catch user-agent spoofing.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `ownPropertyFlags` | flags (byte) | Four packed flags, each 1 if the property is an own property, 0 otherwise: Navigator.prototype has clipboard, global has ServiceWorker, global has ApplePaySession, CSSStyleDeclaration.prototype has WebkitOverflowScrolling. The upper 4 bits are always 0. |

Flag names of `ownPropertyFlags` (card order; the bit of each flag is per build, see the schema's `bits`):

- `clipboard`
- `ServiceWorker`
- `ApplePaySession`
- `WebkitOverflowScrolling`

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "ApplePaySession": false,
  "ServiceWorker": true,
  "WebkitOverflowScrolling": false,
  "clipboard": true
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.BrowserApiOwnPropertyFlags` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Flag bytes are `type: "flags"` with `bits` mapping each flag name above to its bit (0 = least significant) in this build. If the bits cannot all be named the field stays `type: "byte"` and is listed under `diagnostics.flags_fallback`.
- When the entire resolved message is one packed flag byte, fresh schemas/readable payloads expose its named booleans at the signal root. The schema retains the original byte under `physicalFlags`; the wrapper is a projection name, not a transmitted object.
