# NestedFrameAnchorPropertyNamesHash

[Signals index](README.md) · [data-shape example](nested_frame_anchor_property_names_hash.js) · [schema format](schema.md)

## What it collects

Creates a hidden iframe that loads a second, nested iframe, then creates an &lt;a&gt; element inside that nested frame's document. It lists every enumerable property name of the anchor (own and inherited, in for-in order), joins them into one string with a space before each name, and sends a 32-bit hash of that string. The frames are removed right afterwards.

## Why

The set and order of anchor properties differ between browser engines and versions, so the hash is a compact engine fingerprint; doing it in a fresh nested frame likely bypasses property patches that stealth tools apply to the top-level page.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `keysHash` | number | Signed 32-bit hash of the string " key1 key2 ... keyN" built from the anchor's enumerable property names. 0 if the anchor has no enumerable keys. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "keysHash": 1865700265
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.NestedFrameAnchorPropertyNamesHash` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
