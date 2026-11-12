# CanvasScenePixelSamples

[Signals index](README.md) · [data-shape example](canvas_scene_pixel_samples.js) · [schema format](schema.md)

## What it collects

Draws a fixed scene on an off-screen canvas of about 250x250 pixels: filled rectangles, dozens of stroked circles in solid, translucent or gradient colours, and a pentagram. It reads back all pixels and sends the colour of about 40 to 60 pixels at fixed positions (the positions are constants that differ per build). Some sampled pixels sit side by side on shape edges, to capture anti-aliasing. It sends raw pixel samples, not an image hash.

## Why

How a browser rasterises the same scene (anti-aliasing, gradient interpolation, alpha blending, colour handling) varies with GPU, OS, graphics driver and browser engine. This makes it a classic canvas fingerprint and likely also exposes canvas noise injection or headless software rendering.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `sampleCount` | number | Number of sampled pixels; a per-build constant, 39 in the main build (38 to 59 across builds). |
| `samples` | group, repeated (once per sampled pixel) | One entry per repetition with the fields below. |
| `samples[].channelCount` | number | Number of channel values that follow; always 4. |
| `samples[].channel` | number, repeated (4 times: alpha, blue, green, red) | Colour channel value of the pixel, 0 to 255. |
| `terminator` | number | End marker; always -1. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "samples": [
    [
      255,
      198,
      84,
      66
    ],
    [
      255,
      250,
      110,
      100
    ],
    [
      255,
      250,
      110,
      100
    ],
    [
      255,
      250,
      110,
      100
    ],
    [
      255,
      90,
      154,
      150
    ],
    [
      255,
      250,
      110,
      100
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      86,
      199,
      65
    ],
    [
      255,
      228,
      107,
      107
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      250,
      110,
      100
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      190,
      100,
      120
    ],
    [
      255,
      244,
      189,
      188
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      250,
      110,
      100
    ],
    [
      255,
      82,
      127,
      175
    ],
    [
      255,
      187,
      101,
      120
    ],
    [
      255,
      250,
      110,
      100
    ],
    [
      255,
      250,
      110,
      100
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      250,
      110,
      100
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      250,
      110,
      100
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      241,
      232,
      235
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      132,
      203,
      145
    ],
    [
      255,
      177,
      143,
      74
    ],
    [
      255,
      250,
      110,
      100
    ],
    [
      255,
      216,
      101,
      115
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      241,
      235,
      239
    ],
    [
      255,
      250,
      110,
      100
    ],
    [
      255,
      250,
      110,
      100
    ],
    [
      255,
      217,
      117,
      86
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      137,
      185,
      104
    ],
    [
      255,
      82,
      127,
      175
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      250,
      110,
      100
    ],
    [
      255,
      242,
      108,
      104
    ],
    [
      255,
      250,
      110,
      100
    ],
    [
      255,
      250,
      110,
      100
    ],
    [
      255,
      151,
      62,
      121
    ]
  ],
  "terminator": -1
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.CanvasScenePixelSamples` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Repeated fields are `type: "array"` with an `item`; a repeated group's item is a `record` with its own `order` and `fields`.
