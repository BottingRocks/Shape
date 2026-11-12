# UiEventIsTrusted

[Signals index](README.md) · [data-shape example](ui_event_is_trusted.js) · [schema format](schema.md)

## What it collects

Creates a UIEvent with document.createEvent('UIEvent') and reads its isTrusted property; the event is never dispatched. Sends whether it claims to be trusted.

## Why

A script-made event should never be trusted, so a 'trusted' answer means isTrusted was patched or spoofed, a typical trick of automation tools that fake user input.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `isTrusted` | boolean (tag byte) | One of two tag values: 'not trusted' (normal; also used when UIEvent does not exist) or 'trusted' (isTrusted spoofed). The two tag numbers change per build. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "isTrusted": false
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.UiEventIsTrusted` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
