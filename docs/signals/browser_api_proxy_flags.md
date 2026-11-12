# BrowserApiProxyFlags

[Signals index](README.md) · [data-shape example](browser_api_proxy_flags.js) · [schema format](schema.md)

## What it collects

Checks 17 browser API objects and functions to see if any of them is a Proxy or otherwise behaves abnormally when its prototype is changed. For each one it tries to build a prototype cycle and watches whether the engine's normal cycle error happens. It also records whether Proxy exists. The 18 results are packed into three bytes of flags.

## Why

Anti-fingerprinting extensions, automation frameworks and spoofing tools often wrap APIs like canvas readback, navigator properties or Function.prototype.toString in a Proxy to fake their values. In an unmodified browser all detector flags should be 0 (and the Proxy flag 1), so any set flag likely marks a tampered environment.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `proxyFlags1` | flags (byte) | 7 of the 18 flags in the low 7 bits. A detector flag is 1 if the API value exists and behaves like a Proxy or has a locked or abnormal prototype, 0 otherwise. Detector flags cover: HTMLMediaElement.prototype.canPlayType, speechSynthesis, the Navigator.prototype getters for languages, hardwareConcurrency and plugins, HTMLCanvasElement.prototype.toDataURL, navigator.mimeTypes, navigator.plugins, navigator.getBattery, navigator itself, Function.prototype.toString, Intl, Intl.supportedValuesOf, document.createElement, WebGLRenderingContext.prototype.getParameter, CanvasRenderingContext2D.prototype.getImageData and Permissions.prototype.canPlayType (normally missing, so 0). The 18th flag is 1 if Proxy is available. |
| `proxyFlags2` | byte | Another 7 of the 18 flags in the low 7 bits; same meaning as proxyFlags1. |
| `proxyFlags3` | byte | The last 4 of the 18 flags in the low 4 bits; same meaning as proxyFlags1. The upper 4 bits are 0. |

Flag names of `proxyFlags1` (card order; the bit of each flag is per build, see the schema's `bits`):

- `HTMLMediaElement.prototype.canPlayType`
- `speechSynthesis`
- `Navigator.prototype.languages`
- `Navigator.prototype.hardwareConcurrency`
- `Navigator.prototype.plugins`
- `HTMLCanvasElement.prototype.toDataURL`
- `navigator.mimeTypes`
- `navigator.plugins`
- `navigator.getBattery`
- `navigator`
- `Function.prototype.toString`
- `Intl`
- `Intl.supportedValuesOf`
- `document.createElement`
- `WebGLRenderingContext.prototype.getParameter`
- `CanvasRenderingContext2D.prototype.getImageData`
- `Permissions.prototype.canPlayType`
- `Proxy`

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "proxyChecks": [
    {
      "CanvasRenderingContext2D.prototype.getImageData": false,
      "Function.prototype.toString": false,
      "HTMLCanvasElement.prototype.toDataURL": false,
      "Intl.supportedValuesOf": false,
      "Navigator.prototype.hardwareConcurrency": false,
      "Proxy": true,
      "navigator.mimeTypes": false
    },
    {
      "HTMLMediaElement.prototype.canPlayType": false,
      "Intl": false,
      "Navigator.prototype.languages": false,
      "Navigator.prototype.plugins": false,
      "Permissions.prototype.canPlayType": false,
      "document.createElement": false,
      "navigator.getBattery": false
    },
    {
      "WebGLRenderingContext.prototype.getParameter": false,
      "navigator": false,
      "navigator.plugins": false,
      "speechSynthesis": false
    }
  ]
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.BrowserApiProxyFlags` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Flag bytes are `type: "flags"` with `bits` mapping each flag name above to its bit (0 = least significant) in this build. If the bits cannot all be named the field stays `type: "byte"` and is listed under `diagnostics.flags_fallback`.
- Fresh schemas/readable payloads group `proxyFlags1` through `proxyFlags3` as `proxyChecks[0..2]`. This is a heterogeneous fixed array of named boolean objects; each index retains its original byte's bit map. Physical field names above identify source writes, not transmitted JSON keys.
