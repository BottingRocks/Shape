# IframeRealmPropertyNameHashes

[Signals index](README.md) · [data-shape example](iframe_realm_property_name_hashes.js) · [schema format](schema.md)

## What it collects

Creates a hidden nested iframe to get a fresh JavaScript realm and looks up five objects there: Navigator.prototype, CanvasRenderingContext2D.prototype, chrome.app, safari and ApplePaySetup.prototype. For each, it lists the property names a for...in loop yields (own and inherited, in engine order), joins them and hashes the list. It sends the five hashes; a missing object gives an empty list.

## Why

Which properties these objects have, and in what order, likely identifies browser family and version (chrome.app for Chromium, safari / ApplePaySetup for Safari), taken from a realm page scripts have not touched so it can be compared with the main window.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `safariHash` | number | 32-bit hash of the enumerable property names of window.safari in the iframe, shifted to the range 0 to 2^32-1; exactly 2147483648 if the object is missing. |
| `chromeAppHash` | number | Same hash for chrome.app (normally present only in Chromium browsers); 2147483648 if missing. |
| `canvasContext2dHash` | number | Same hash for CanvasRenderingContext2D.prototype; 2147483648 if missing. |
| `applePaySetupHash` | number | Same hash for ApplePaySetup.prototype (Safari with Apple Pay); 2147483648 if missing. |
| `navigatorHash` | number | Same hash for Navigator.prototype; 2147483648 if missing. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "applePaySetupHash": 2147483648,
  "canvasContext2dHash": 2167857330,
  "chromeAppHash": 2147483648,
  "navigatorHash": 949388909,
  "safariHash": 2147483648
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.IframeRealmPropertyNameHashes` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
