# EmojiChannelSampleHalves

[Signals index](README.md) · [data-shape example](emoji_channel_sample_halves.js) · [schema format](schema.md)

## What it collects

Draws 8 emoji, one at a time, in black 94px sans-serif on a 96x96 canvas and reads a single colour channel (R, G, B or A) of one pixel per emoji, at a build-specific position. The 8 samples are split into two halves and each half is sent as a count followed by its values. If the page is cleaned up before all emoji are drawn, the samples collected so far are sent.

## Why

The pixel values depend on the installed emoji font, its version and the graphics rasteriser, so they fingerprint the operating system and browser and expose spoofed or randomised canvas output (likely).

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `firstHalfCount` | number | Number of samples in the first half (4 normally, fewer if cut short). |
| `firstHalfSamples` | number, repeated (each sample of the first half) | One channel byte, 0..255, of the sampled pixel, or -1 if drawing or reading failed; written last sample first. |
| `secondHalfCount` | number | Number of samples in the second half (4 normally). |
| `secondHalfSamples` | number, repeated (each sample of the second half) | One channel byte, 0..255, or -1 on error; written last sample first. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "firstHalfSamples": [
    0,
    0,
    232,
    162
  ],
  "secondHalfSamples": [
    255,
    31,
    142,
    9
  ]
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.EmojiChannelSampleHalves` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Repeated fields are `type: "array"` with an `item`.
