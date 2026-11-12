# StorageAccessApiPresence

[Signals index](README.md) · [data-shape example](storage_access_api_presence.js) · [schema format](schema.md)

## What it collects

Checks, without calling anything, whether three API methods exist as own properties of their prototypes: hasStorageAccess and requestStorageAccess on Document.prototype (Storage Access API), and getHostEnvironmentValue on External.prototype (old EdgeHTML-only API). Sends the three answers as bits of one byte.

## Why

Which of these APIs are present tells the browser engine and version apart, so it catches user-agent spoofing (likely also catches shims added to document rather than the prototype).

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `apiFlags` | flags (byte) | Three flag bits (0..7): hasStorageAccess present on Document.prototype, requestStorageAccess present on Document.prototype, getHostEnvironmentValue present on External.prototype. A missing XMLDocument or External counts as not present. |

Flag names of `apiFlags` (card order; the bit of each flag is per build, see the schema's `bits`):

- `hasStorageAccess`
- `requestStorageAccess`
- `getHostEnvironmentValue`

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "getHostEnvironmentValue": false,
  "hasStorageAccess": true,
  "requestStorageAccess": true
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.StorageAccessApiPresence` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Flag bytes are `type: "flags"` with `bits` mapping each flag name above to its bit (0 = least significant) in this build. If the bits cannot all be named the field stays `type: "byte"` and is listed under `diagnostics.flags_fallback`.
- When the entire resolved message is one packed flag byte, fresh schemas/readable payloads expose its named booleans at the signal root. The schema retains the original byte under `physicalFlags`; the wrapper is a projection name, not a transmitted object.
