# FontMetrics

[Signals index](README.md) · [data-shape example](font_metrics.js) · [schema format](schema.md)

## What it collects

Measures 43 fixed characters (symbols, currency signs, CJK, Hangul, special and private-use characters) at 1000px in each of the five generic font families (sans-serif, serif, monospace, cursive, fantasy) with canvas measureText, joins each character's width and four bounding-box values as hex text, and hashes the text per family. It sends the family count and one hash per family; nothing is sent if there is no 2D canvas.

## Why

Which real fonts the generic families map to, and their exact glyph sizes, depend on the OS, installed fonts and text renderer, so these hashes fingerprint the platform and reveal spoofed text metrics (likely).

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `familyCount` | number | Number of font-family hashes that follow (5). |
| `familyMetricsHash` | number, repeated (each generic font family) | Signed 32-bit hash of the measurement text for one family; in the main build in order sans-serif, serif, monospace, cursive, fantasy. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "familyMetricsHash": [
    -2112551573,
    -1100428147,
    1514521280,
    157432748,
    412628950
  ]
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.FontMetrics` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Repeated fields are `type: "array"` with an `item`.
