# NamespacePropertyNameHashes

[Signals index](README.md) · [data-shape example](namespace_property_name_hashes.js) · [schema format](schema.md)

## What it collects

For five fixed browser objects (Navigator.prototype, CanvasRenderingContext2D.prototype, chrome.app, safari and ApplePaySetup.prototype), it collects every enumerable property name (for...in), joins them with a NUL character and hashes the result. An object that does not exist counts as an empty list. The five hashes are sent as five numbers once all are done.

## Why

The presence and API surface of these objects show the browser vendor and version (chrome.app only in Chrome, safari and ApplePaySetup only in Safari) and likely reveal tampering such as injected or hidden properties on navigator or canvas.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `navigatorProtoHash` | number | Hash of the enumerable property names of Navigator.prototype, as an unsigned 32-bit value (0 to 4294967295). An empty or missing object gives 2147483648. |
| `canvas2dProtoHash` | number | Same hash for CanvasRenderingContext2D.prototype. 2147483648 if missing or empty. |
| `chromeAppHash` | number | Same hash for window.chrome.app (Chrome only). 2147483648 if missing or empty. |
| `applePaySetupProtoHash` | number | Same hash for ApplePaySetup.prototype (Safari only). 2147483648 if missing or empty. |
| `safariHash` | number | Same hash for window.safari (Safari only). 2147483648 if missing or empty. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "applePaySetupProtoHash": 2147483648,
  "canvas2dProtoHash": 2167857330,
  "chromeAppHash": 3451360684,
  "navigatorProtoHash": 949388909,
  "safariHash": 2147483648
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.NamespacePropertyNameHashes` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
