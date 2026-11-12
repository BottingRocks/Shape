# UserAgentData

[Signals index](README.md) · [data-shape example](user_agent_data.js) · [schema format](schema.md)

## What it collects

Asks the User-Agent Client Hints API (navigator.userAgentData.getHighEntropyValues) for ten hints, one request per hint, and waits for all of them. Each hint that comes back is written; each one that is missing or rejected is replaced by a null marker. The two booleans (wow64, mobile) are packed into one flags byte. Browsers without userAgentData send nothing.

## Why

Client hints give the exact browser brand/version, OS version, CPU architecture and device model, which can be checked against the User-Agent string to catch spoofed or headless browsers (likely).

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `brands` | number or null (tag byte) | Number of entries in the low-entropy brand list (e.g. Chromium, Google Chrome, Not A Brand), or a null marker if the hint is missing. |
| `brandEntries` | group, repeated (once per brand in the list (only when brands is present)) | One entry per repetition with the fields below. |
| `brandEntries[].brand` | string | Brand name, e.g. "Google Chrome". |
| `brandEntries[].version` | string | Major version for that brand, e.g. "120". |
| `platformVersion` | string or null (tag byte) | OS version, e.g. "15.0.0" or "10.0.0", or a null marker if missing. |
| `architecture` | string or null (tag byte) | CPU architecture, e.g. "x86" or "arm" (empty string if unknown), or a null marker if missing. |
| `fullVersionList` | number or null (tag byte) | Number of entries in the full-version brand list, or a null marker if missing. |
| `fullVersionEntries` | group, repeated (once per entry in fullVersionList (only when present)) | One entry per repetition with the fields below. |
| `fullVersionEntries[].version` | string | Full version for that brand, e.g. "120.0.6099.130". |
| `fullVersionEntries[].brand` | string | Brand name; usually a repeat of a name already sent in brands. |
| `bitness` | string or null (tag byte) | CPU bitness, "64" or "32" (or empty), or a null marker if missing. |
| `platform` | string or null (tag byte) | OS name, e.g. "Windows", "macOS", "Android", or a null marker if missing. |
| `wow64Placeholder` | null (tag byte) | Written at wow64's place in the hint list only when the wow64 hint is missing: a null marker byte. When wow64 is present nothing is written here and its value goes into the flags byte. |
| `mobilePlaceholder` | null (tag byte) | Written at mobile's place in the hint list only when the mobile hint is missing: a null marker byte. When mobile is present nothing is written here and its value goes into the flags byte. |
| `flags` | flags (byte) | Packed booleans: wow64 (32-bit browser on 64-bit Windows) and mobile (mobile device). A flag is 0 when its hint is missing; then a null marker is also written at that hint's place in the list. |
| `uaFullVersion` | string or null (tag byte) | Full browser version (deprecated hint), e.g. "120.0.6099.130", or a null marker if missing. |
| `model` | string or null (tag byte) | Device model, e.g. "Pixel 7"; empty string on desktop, or a null marker if missing. |

Flag names of `flags` (card order; the bit of each flag is per build, see the schema's `bits`):

- `wow64`
- `mobile`

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "architecture": "arm",
  "bitness": "64",
  "brands": [
    {
      "brand": "Google Chrome",
      "version": "156"
    },
    {
      "brand": "Chromium",
      "version": "156"
    },
    {
      "brand": "Not:A-Brand",
      "version": "8"
    }
  ],
  "flags": {
    "mobile": false,
    "wow64": false
  },
  "fullVersionList": [
    {
      "brand": "Google Chrome",
      "version": "156.0.8078.81"
    },
    {
      "brand": "Chromium",
      "version": "156.0.8078.81"
    },
    {
      "brand": "Not:A-Brand",
      "version": "8.0.0.0"
    }
  ],
  "model": "",
  "platform": "macOS",
  "platformVersion": "27.0.1",
  "uaFullVersion": "156.0.8078.81"
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.UserAgentData` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Repeated fields are `type: "array"` with an `item`; a repeated group's item is a `record` with its own `order` and `fields`.
- Flag bytes are `type: "flags"` with `bits` mapping each flag name above to its bit (0 = least significant) in this build. If the bits cannot all be named the field stays `type: "byte"` and is listed under `diagnostics.flags_fallback`.
