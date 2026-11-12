# ConstantEmptyString

[Signals index](README.md) · [data-shape example](constant_empty_string.js) · [schema format](schema.md)

## What it collects

Sends a single fixed empty string and nothing else. It reads no browser data. Before sending, it makes 8 to 14 random throwaway calls to built-in functions (such as isFinite, JSON.parse, new Audio, new MessageChannel) whose results are discarded.

## Why

It carries no information about the browser; it is likely a decoy or padding message, or a check that the client's encoder and mask stream work (a guess).

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `constant` | string | Always the empty string "". |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

No decoded sample available.

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.ConstantEmptyString` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
