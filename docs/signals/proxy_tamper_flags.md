# ProxyTamperFlags

[Signals index](README.md) · [data-shape example](proxy_tamper_flags.js) · [schema format](schema.md)

## What it collects

Checks whether the page's global object and its navigator are wrapped in a JavaScript Proxy. It calls Date.prototype.toString on each and looks for 'Proxy.toString' or 'incompatible Proxy' in the error; for navigator it also tries structuredClone(navigator) and looks for '#&lt;Navigator&gt;' in the error.

## Why

Anti-detect and stealth tools often wrap window or navigator in a Proxy to fake properties, so this likely detects spoofed browser environments.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `proxyFlags` | flags (byte) | Packed flags (order swapped in some builds): global object looks like a Proxy; navigator looks like a Proxy. Value 0 to 3, other bits 0. |

Flag names of `proxyFlags` (card order; the bit of each flag is per build, see the schema's `bits`):

- `globalLooksLikeProxy`
- `navigatorLooksLikeProxy`

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "globalLooksLikeProxy": false,
  "navigatorLooksLikeProxy": false
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.ProxyTamperFlags` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Flag bytes are `type: "flags"` with `bits` mapping each flag name above to its bit (0 = least significant) in this build. If the bits cannot all be named the field stays `type: "byte"` and is listed under `diagnostics.flags_fallback`.
- When the entire resolved message is one packed flag byte, fresh schemas/readable payloads expose its named booleans at the signal root. The schema retains the original byte under `physicalFlags`; the wrapper is a projection name, not a transmitted object.
