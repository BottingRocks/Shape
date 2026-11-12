# DocumentReferrerHistory

[Signals index](README.md) · [data-shape example](document_referrer_history.js) · [schema format](schema.md)

## What it collects

Reads, once at init, how many entries the tab's session history has, whether document.referrer is empty, and in most vendor builds the first 100 characters of the referrer itself.

## Why

Bots and scripted sessions often open pages directly in a fresh tab (history length 1, no referrer) or with a referrer that does not fit the navigation path; real users usually arrive with history and a plausible referrer (likely).

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `historyLength` | number | history.length: number of entries in the tab's session history (1 or more; browsers usually cap it at 50). |
| `referrerEmpty` | boolean (tag byte) | Tag byte: one build-specific value means document.referrer is empty, the other means it is not. |
| `referrerPrefix` | string | First 100 characters of document.referrer. Sent only by builds that include it (most vendors); the main build and all puma, starbucks and tdbank builds leave it out. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "historyLength": 2,
  "referrerEmpty": false,
  "referrerPrefix": "https://www.target.com/"
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.DocumentReferrerHistory` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
