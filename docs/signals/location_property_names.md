# LocationPropertyNames

[Signals index](README.md) · [data-shape example](location_property_names.js) · [schema format](schema.md)

## What it collects

Lists the property names on window.location (not their values) in the order the browser enumerates them. Each known name becomes one letter: ancestorOrigins a, assign b, fragmentDirective c, hash d, host e, hostname f, href g, origin h, pathname i, port j, protocol k, reload l, replace m, search n, toString o. Any other name becomes u. If enumeration finds more names than Object.keys(location) reports (capped at 30), the list is cut off after one extra entry.

## Why

The set and order of location properties differ between browser engines, and extra or unknown names reveal a spoofed, proxied or patched location object, as used by some bot frameworks (likely).

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `locationKeyCodes` | string | One letter code per enumerated location property, each followed by a NUL separator, e.g. "a\0g\0h\0k\0…". Empty string if location is missing. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "locationKeyCodes": "a\u0000g\u0000h\u0000k\u0000e\u0000f\u0000j\u0000i\u0000n\u0000d\u0000b\u0000l\u0000m\u0000o\u0000"
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.LocationPropertyNames` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
