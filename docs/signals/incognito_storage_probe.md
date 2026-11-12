# IncognitoStorageProbe

[Signals index](README.md) · [data-shape example](incognito_storage_probe.js) · [schema format](schema.md)

## What it collects

Detects whether the browser is in private/incognito mode. It first guesses the browser family from navigator.vendor, the userAgent, a few engine-specific properties and the length of eval.toString(). It then runs that browser's private-mode test: on Safari, openDatabase/localStorage write failures or an IndexedDB Blob store error; on Chromium, a temporary storage quota smaller than the JS heap limit (or webkitRequestFileSystem failing); on Firefox, a missing navigator.serviceWorker; on IE/legacy Edge, a missing indexedDB. It cleans up the temporary database and storage key afterwards. On some paths where the needed API is missing, it never reports.

## Why

Private browsing and incognito windows are common with fraud, account farming and bot tooling. The browser-family code also lets the vendor check the claimed user agent against how the engine actually behaves (likely).

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `browserCode` | string | Browser family the probe detected: "F" Safari, "A" Brave, "B" Edge (Chromium), "C" Opera, "D" other Chrome, "E" Google-vendor browser without "Chrome" in the userAgent, "G" Firefox, "H" IE/legacy Edge, "UD" unknown. |
| `isPrivateMode` | boolean (tag byte) | Whether private/incognito mode was detected. The byte holds one of two tag values meaning true or false, and the two values differ between builds. Always false when browserCode is "UD". |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "browserCode": "D",
  "isPrivateMode": false
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.IncognitoStorageProbe` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
