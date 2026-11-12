# NativeApiIntegrityFlags

[Signals index](README.md) · [data-shape example](native_api_integrity_flags.js) · [schema format](schema.md)

## What it collects

Runs 20 yes/no checks that key browser built-ins are still the browser's own native code and have their normal shape: screen and navigator getters, document.createElement/createElementNS, canvas and 2D-context methods and style setters, Function.prototype.toString, and the class tags of navigator.plugins and navigator.mimeTypes. The 20 results are packed into three flag bytes (7 + 7 + 6 bits); 1 means the API looks native. It sends them once at startup and again at the end of the page's life, where a check only stays 1 if it was 1 at startup and still passes.

## Why

Bots, anti-fingerprinting extensions and spoofing tools often override these APIs (user agent, screen size, plugins, canvas output). A 0 bit likely flags such tampering; an unmodified browser should give all ones.

## Fields

This signal sends one of 1 message shapes.

### Message 1: At startup (init) and again at the end of the page's life (last chance); same layout, last-chance bits are 1 only if the check passed both times

| Field | Type | Meaning |
| --- | --- | --- |
| `integrityFlags1` | flags (byte) | 7 flags, 1 = looks native: screen.width getter native; document.createElementNS native; document's prototype has no own createElement; screen.height getter native; navigator.mimeTypes getter native; navigator.plugins getter native; CanvasRenderingContext2D fillText native. Which flags land in which byte changes per build. |
| `integrityFlags2` | byte | 7 flags, 1 = looks native: navigator.plugins class tag is not a plain Object/Array/Function; getImageData native; navigator.userAgent getter native; document.createElement native; navigator.mimeTypes class tag not plain; strokeStyle getter and setter native; fillStyle getter and setter native. |
| `integrityFlags3` | byte | 6 flags, 1 = looks native: HTMLCanvasElement toDataURL native; Function.prototype.toString native; document has no own createElement; HTMLCanvasElement getContext native; 2D-context font getter and setter native; Document.prototype has its own createElement. |

Flag names of `integrityFlags1` (card order; the bit of each flag is per build, see the schema's `bits`):

- `screenWidthGetterNative`
- `createElementNSNative`
- `documentProtoNoOwnCreateElement`
- `screenHeightGetterNative`
- `mimeTypesGetterNative`
- `pluginsGetterNative`
- `fillTextNative`
- `pluginsTagNotPlain`
- `getImageDataNative`
- `userAgentGetterNative`
- `createElementNative`
- `mimeTypesTagNotPlain`
- `strokeStyleAccessorNative`
- `fillStyleAccessorNative`
- `toDataURLNative`
- `functionToStringNative`
- `documentNoOwnCreateElement`
- `getContextNative`
- `fontAccessorNative`
- `documentPrototypeHasOwnCreateElement`

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "integrityChecks": [
    {
      "createElementNative": true,
      "functionToStringNative": true,
      "mimeTypesGetterNative": true,
      "pluginsGetterNative": true,
      "pluginsTagNotPlain": true,
      "screenHeightGetterNative": true,
      "strokeStyleAccessorNative": true
    },
    {
      "createElementNSNative": true,
      "documentProtoNoOwnCreateElement": true,
      "fillTextNative": true,
      "fontAccessorNative": true,
      "getImageDataNative": true,
      "screenWidthGetterNative": true,
      "toDataURLNative": true
    },
    {
      "documentNoOwnCreateElement": true,
      "documentPrototypeHasOwnCreateElement": true,
      "fillStyleAccessorNative": true,
      "getContextNative": true,
      "mimeTypesTagNotPlain": true,
      "userAgentGetterNative": true
    }
  ]
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.NativeApiIntegrityFlags` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- It has 1 message shapes, so the entry carries `messages` (`when`, `mask`, `order`, `fields` each) instead of top-level `order`/`fields`.
- Flag bytes are `type: "flags"` with `bits` mapping each flag name above to its bit (0 = least significant) in this build. If the bits cannot all be named the field stays `type: "byte"` and is listed under `diagnostics.flags_fallback`.
- Fresh schemas/readable payloads group `integrityFlags1` through `integrityFlags3` as `integrityChecks[0..2]`. This is a heterogeneous fixed array of named boolean objects; each index retains its original byte's bit map. Physical field names above identify source writes, not transmitted JSON keys.
