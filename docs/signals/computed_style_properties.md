# ComputedStyleProperties

[Signals index](README.md) · [data-shape example](computed_style_properties.js) · [schema format](schema.md)

## What it collects

Creates a detached &lt;a&gt; element, calls getComputedStyle on it, and lists every CSS property name on the resulting style object (dropping the numbered index entries). It joins the names into one string with no separator and sends the name count, the string length, and a sample of about 16 characters taken at even spacing through the string. It never reads style values.

## Why

The set and order of supported CSS properties depends on the browser engine, version and flags, so it likely identifies the real engine/version; the character sample makes a faked property list hard to pass without matching the real one exactly.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `propertyCount` | number | Number of CSS property names found on the computed style (after removing the numbered index entries); 0 if the style object was null. |
| `joinedLength` | number | Length in characters of all property names joined together with no separator. |
| `sampledChars` | string | About 16 characters picked at even intervals from the joined names string (every character if it is shorter than 32 characters). |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "joinedLength": 10959,
  "propertyCount": 749,
  "sampledChars": "tnoitixaetltktrs"
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.ComputedStyleProperties` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
