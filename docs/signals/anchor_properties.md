# AnchorProperties

[Signals index](README.md) · [data-shape example](anchor_properties.js) · [schema format](schema.md)

## What it collects

Creates one detached &lt;a&gt; element and lists every enumerable property name that for-in finds on it and its prototype chain (HTMLAnchorElement, HTMLElement, Element, Node, EventTarget, Object), in the engine's enumeration order. It joins the names with spaces, hashes the string to a 32-bit number, and sends only that hash. If the element cannot be created, the signal completes without a payload.

## Why

The set and order of DOM properties differs by browser engine, version and enabled features, and it changes when automation tools or extensions add enumerable properties to element or Object prototypes. This likely makes it a compact engine fingerprint and a tampering check.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `keysHash` | number | Signed 32-bit hash of every enumerable property name on a new &lt;a&gt; element, each preceded by a space (empty string if there are none). |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "keysHash": 1865700265
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.AnchorProperties` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
