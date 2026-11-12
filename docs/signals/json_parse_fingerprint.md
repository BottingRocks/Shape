# JsonParseFingerprint

[Signals index](README.md) · [data-shape example](json_parse_fingerprint.js) · [schema format](schema.md)

## What it collects

Runs JSON.parse then JSON.stringify on four fixed test strings: one valid object whose key contains a NUL character, and three malformed inputs (a missing comma, and inputs starting with odd Unicode characters). For each it keeps either the re-serialized JSON or "INVALID:" plus the error text, joins the four results and sends a hash of them.

## Why

SyntaxError messages are worded differently in each JavaScript engine and version, so the hash identifies the real engine. It also catches patched or polyfilled JSON and Error functions (likely).

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `jsonOutcomeHash` | number | 32-bit signed hash of the four joined parse/stringify outcomes. It is mostly determined by the engine's JSON error messages. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

No decoded sample available.

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.JsonParseFingerprint` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
