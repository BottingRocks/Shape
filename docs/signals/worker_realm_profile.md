# WorkerRealmProfile

[Signals index](README.md) · [data-shape example](worker_realm_profile.js) · [schema format](schema.md)

## What it collects

Runs inside a Web Worker and reads five values from the worker's own global scope: the CPU core count, the origin, a count of global property names, the user agent and the platform. The worker builds the record and posts it back to the page.

## Why

Comparing worker values with the same values read on the page likely catches browsers that spoof navigator properties only on the main page, and the property count fingerprints the worker realm and detects patched or emulated workers.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `hardwareConcurrency` | number or null (tag byte) | navigator.hardwareConcurrency in the worker: logical CPU cores, e.g. 8, or a null marker if null/undefined. |
| `origin` | string | location.origin of the worker, e.g. "https://www.example.com". |
| `globalPropertyCount` | number | Total number of own property names on the worker global (self) and every object on its prototype chain. |
| `userAgent` | string | navigator.userAgent as seen inside the worker. |
| `platform` | string | navigator.platform as seen inside the worker, e.g. "Win32", "MacIntel". |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "globalPropertyCount": 401,
  "hardwareConcurrency": 18,
  "origin": "https://www.target.com",
  "platform": "MacIntel",
  "userAgent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Safari/537.36"
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.WorkerRealmProfile` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
