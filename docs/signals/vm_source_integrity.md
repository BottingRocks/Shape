# VmSourceIntegrity

[Signals index](README.md) · [data-shape example](vm_source_integrity.js) · [schema format](schema.md)

## What it collects

Reads the source text of the obfuscated script's own VM interpreter function with Function.prototype.toString, keeps the body between the first '{' and the last '}', and measures its length, whitespace count and punctuation count. Only the low 8 bits of each count are sent, each hidden behind a random pad byte, together with a build marker string and a random nonce.

## Why

Pretty-printing, re-minifying or editing the script changes these counts, so this likely detects researchers or bots that have tampered with or reformatted the anti-bot script; the marker tells the server which build's expected values to compare against.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `bodyLength` | number | Low 8 bits of the function body length (UTF-16 units), sent as pad*256 + (low8 XOR pad) with a random pad 0-255. |
| `nonce` | number | Fractional part of a random/clock mix, 0 to &lt;1 (usually a float). Absent in some builds. |
| `whitespaceCount` | number | Low 8 bits of the number of whitespace characters in the body, padded the same way. |
| `punctuationCount` | number | Low 8 bits of the number of . { ( [ , ; = / characters in the body, padded the same way. |
| `buildMarker` | string | Fixed 16-hex-digit string unique to the script build, e.g. "a8243d24060c1c6b". |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "bodyLength": 36773,
  "buildMarker": "e5cfce5772068bdc",
  "nonce": 0.515963662070013,
  "punctuationCount": 29783,
  "whitespaceCount": 6660
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.VmSourceIntegrity` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
