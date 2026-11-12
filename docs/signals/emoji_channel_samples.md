# EmojiChannelSamples

[Signals index](README.md) · [data-shape example](emoji_channel_samples.js) · [schema format](schema.md)

## What it collects

Draws 8 emoji (sunglasses face, keycap 6, baby bottle, England flag twice, bar chart, circled 'congratulation' ideograph, Ophiuchus) in black 94px sans-serif on a 96x96 canvas and reads one channel of one pixel for each: alpha for seven of them and red for the bar chart. It sends the sample count and the 8 values once.

## Why

Whether ink appears at those points, and whether the bar chart is coloured, depends on the emoji font, its version and the rasteriser, giving a cheap OS/browser/font fingerprint that also catches canvas spoofing (likely).

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `sampleCount` | number | Number of samples that follow (8). |
| `samples` | number, repeated (each emoji probe) | The sampled channel byte, 0..255 (alpha: 0 = no ink at that point; red on the bar chart: 0 = monochrome glyph); written last probe first in the main build. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "samples": [
    255,
    255,
    0,
    253,
    0,
    0,
    255,
    255
  ]
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.EmojiChannelSamples` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Repeated fields are `type: "array"` with an `item`.
