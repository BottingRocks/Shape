# GlobalOwnPropertyBits

[Signals index](README.md) · [data-shape example](global_own_property_bits.js) · [schema format](schema.md)

## What it collects

Checks a fixed list of 44 browser global names (recent or engine-specific interfaces, functions and event-handler properties) and records, for each one, whether the global object (window) has it as its own property. A check that throws counts as 'no'. It sends the number of checks followed by the yes/no answers packed 8 per byte.

## Why

Which of these newer or vendor-specific APIs exist is a likely fingerprint of browser engine and version; it also likely catches globals that were deleted or moved onto a prototype by automation or privacy tools.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `probeCount` | number | Number of names checked; always 44. |
| `ownPropertyBits` | flags (byte), repeated (5 bytes from the packing loop, 8 checks per byte, filled from bit 0 in the build's iteration order over the list below (ascending in some builds, descending in others)) | One bit per name: 1 if window has that name as an own property, 0 if not (or if the check threw). |
| `ownPropertyBitsTail` | byte | The trailing byte written after the loop, holding the remaining 4 checks (bits 4-7 are 0): ReadableByteStreamController, OffscreenCanvas, MediaSourceHandle, XRCamera bits 0-3 in ascending builds; RTCDtlsTransport, AbstractRange, crossOriginIsolated, XRHitTestResult in bits 0-3 in descending builds. 1 if window has that name as an own property, 0 if not (or if the check threw). |

Flag names of `ownPropertyBits` (card order; the bit of each flag is per build, see the schema's `bits`):

- `XRHitTestResult`
- `crossOriginIsolated`
- `AbstractRange`
- `RTCDtlsTransport`
- `BarcodeDetector`
- `EventCounts`
- `SharedArrayBuffer`
- `VisualViewport`
- `FinalizationRegistry`
- `WritableStreamDefaultController`
- `HTMLMenuItemElement`
- `HID`
- `CSSCounterStyleRule`
- `FileSystemDirectoryHandle`
- `PerformancePaintTiming`
- `onbeforeinput`
- `ApplicationCache`
- `AudioData`
- `AudioTrack`
- `DOMRectList`
- `FragmentDirective`
- `Sanitizer`
- `onselectionchange`
- `onsecuritypolicyviolation`
- `structuredClone`
- `XRHand`
- `Lock`
- `EyeDropper`
- `CSSLayerBlockRule`
- `ServiceWorkerContainer`
- `IdleDetector`
- `HTMLDialogElement`
- `CSSFontPaletteValuesRule`
- `CSSMathClamp`
- `NavigationPreloadManager`
- `LaunchParams`
- `content`
- `IDBFileHandle`
- `BrowserCaptureMediaStreamTrack`
- `FontData`
- `ReadableByteStreamController`
- `OffscreenCanvas`
- `MediaSourceHandle`
- `XRCamera`

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "ownPropertyBits": {
    "AbstractRange": true,
    "ApplicationCache": false,
    "AudioData": true,
    "AudioTrack": false,
    "BarcodeDetector": true,
    "BrowserCaptureMediaStreamTrack": true,
    "CSSCounterStyleRule": true,
    "CSSFontPaletteValuesRule": true,
    "CSSLayerBlockRule": true,
    "CSSMathClamp": true,
    "DOMRectList": true,
    "EventCounts": true,
    "EyeDropper": true,
    "FileSystemDirectoryHandle": true,
    "FinalizationRegistry": true,
    "FontData": true,
    "FragmentDirective": true,
    "HID": true,
    "HTMLDialogElement": true,
    "HTMLMenuItemElement": false,
    "IDBFileHandle": false,
    "IdleDetector": true,
    "LaunchParams": true,
    "Lock": true,
    "NavigationPreloadManager": true,
    "PerformancePaintTiming": true,
    "RTCDtlsTransport": true,
    "Sanitizer": true,
    "ServiceWorkerContainer": true,
    "SharedArrayBuffer": false,
    "VisualViewport": true,
    "WritableStreamDefaultController": true,
    "XRHand": true,
    "XRHitTestResult": true,
    "content": false,
    "crossOriginIsolated": true,
    "onbeforeinput": true,
    "onsecuritypolicyviolation": true,
    "onselectionchange": true,
    "structuredClone": true
  },
  "ownPropertyBitsTail": {
    "MediaSourceHandle": true,
    "OffscreenCanvas": true,
    "ReadableByteStreamController": true,
    "XRCamera": true
  },
  "probeCount": 44
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.GlobalOwnPropertyBits` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Repeated fields are `type: "array"` with an `item`.
- Flag bytes are `type: "flags"` with `bits` mapping each flag name above to its bit (0 = least significant) in this build. If the bits cannot all be named the field stays `type: "byte"` and is listed under `diagnostics.flags_fallback`.
