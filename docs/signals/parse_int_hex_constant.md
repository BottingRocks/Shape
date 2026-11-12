# ParseIntHexConstant

[Signals index](README.md) · [data-shape example](parse_int_hex_constant.js) · [schema format](schema.md)

## What it collects

Sends the result of parseInt("0xFFFFFFFFFFFFFBFF", 16). It reads no browser state; on any standard engine the result is the same fixed number.

## Why

Likely either a fixed sentinel/version marker or a canary that reveals a hooked or replaced parseInt or a non-conforming number parser.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `parsedValue` | number | The parseInt result as a double; normally 18446744073709549568 (2^64 - 2048). Any other value means parseInt was tampered with. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "parsedValue": 18446744073709549568
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.ParseIntHexConstant` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
