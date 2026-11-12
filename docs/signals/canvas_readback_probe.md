# CanvasReadbackProbe

[Signals index](README.md) · [data-shape example](canvas_readback_probe.js) · [schema format](schema.md)

## What it collects

Draws four simple scenes on a tiny 5x5 canvas: solid black, a black filled path, a fill with negative (transparent) alpha, and shadowed emoji text drawn outside the canvas. For each scene it reads the pixels back with getImageData, hashes the canvas's toDataURL() output, and also decodes that PNG through an Image, draws it onto a fresh canvas and reads the pixels again. It sends, per scene, the distinct pixel colours seen by each readback and how many there were; the order of a colour's components differs between builds. It is sent after the 4th image decodes, or when the drawing queue empties if that comes first.

## Why

On an unmodified browser every readback should be one uniform opaque-black pixel, so extra or different colours likely reveal canvas noise injection (anti-fingerprinting farbling, privacy extensions) or non-standard rendering in headless or emulated stacks. The data-URL hash adds an encoder fingerprint.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `seedFraction` | number | Fraction (0 to under 1) from the random state used to pick the scenes' random colours, so the server can rebuild them. |
| `recordCount` | number | Number of scene records; always 4. |
| `records` | group, repeated (once per scene, 4 times) | One entry per repetition with the fields below. |
| `records[].dataUrlHash` | number or null (tag byte) | Signed 32-bit hash of the canvas's toDataURL() string for this scene, or a null marker if it was not computed (for example toDataURL failed). |
| `records[].directNewCount` | number | How many of the 25 direct-readback pixels were a new colour when read; equals the distinct colour count unless there are more than 6. |
| `records[].roundTripColourCount` | number | Number of distinct colours stored from the image round-trip readback, 0 to 6 (0 if the image had not loaded yet). |
| `records[].roundTripColours` | group, repeated (once per stored round-trip colour, up to 6) | One entry per repetition with the fields below. |
| `records[].roundTripColours[].pixelLength` | number | Number of components in the stored colour (always 4); only sent by builds that write it before the components. |
| `records[].roundTripColours[].red` | number | Red component, 0 to 255. |
| `records[].roundTripColours[].green` | number | Green component, 0 to 255. |
| `records[].roundTripColours[].alpha` | number | Alpha (opacity) component, 0 to 255. |
| `records[].roundTripColours[].blue` | number | Blue component, 0 to 255. |
| `records[].roundTripColours[].components` | number, repeated (once per component of the stored colour (pixelLength times, ImageData RGBA order)) | Colour component, 0 to 255; sent instead of red/green/alpha/blue by builds that write the components with a loop. |
| `records[].roundTripNewCount` | number | How many round-trip pixels were a new colour when read; same meaning as directNewCount. |
| `records[].directColourCount` | number | Number of distinct colours stored from the direct readback, 0 to 6. |
| `records[].directColours` | group, repeated (once per stored direct colour, up to 6, last seen first) | One entry per repetition with the fields below. |
| `records[].directColours[].pixelLength` | number | Number of components in the stored colour (always 4); only sent by builds that write it before the components. |
| `records[].directColours[].red` | number | Red component, 0 to 255. |
| `records[].directColours[].green` | number | Green component, 0 to 255. |
| `records[].directColours[].alpha` | number | Alpha (opacity) component, 0 to 255. |
| `records[].directColours[].blue` | number | Blue component, 0 to 255. |
| `records[].directColours[].components` | number, repeated (once per component of the stored colour (pixelLength times, ImageData RGBA order)) | Colour component, 0 to 255; sent instead of red/green/alpha/blue by builds that write the components with a loop. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "records": [
    {
      "dataUrlHash": 640789961,
      "directColours": [
        {
          "alpha": 255,
          "blue": 0,
          "green": 0,
          "red": 0
        }
      ],
      "directNewCount": 1,
      "roundTripColours": [
        {
          "alpha": 255,
          "blue": 0,
          "green": 0,
          "red": 0
        }
      ],
      "roundTripNewCount": 1
    },
    {
      "dataUrlHash": 640789961,
      "directColours": [
        {
          "alpha": 255,
          "blue": 0,
          "green": 0,
          "red": 0
        }
      ],
      "directNewCount": 1,
      "roundTripColours": [
        {
          "alpha": 255,
          "blue": 0,
          "green": 0,
          "red": 0
        }
      ],
      "roundTripNewCount": 1
    },
    {
      "dataUrlHash": 640789961,
      "directColours": [
        {
          "alpha": 255,
          "blue": 0,
          "green": 0,
          "red": 0
        }
      ],
      "directNewCount": 1,
      "roundTripColours": [
        {
          "alpha": 255,
          "blue": 0,
          "green": 0,
          "red": 0
        }
      ],
      "roundTripNewCount": 1
    },
    {
      "dataUrlHash": 640789961,
      "directColours": [
        {
          "alpha": 255,
          "blue": 0,
          "green": 0,
          "red": 0
        }
      ],
      "directNewCount": 1,
      "roundTripColours": [
        {
          "alpha": 255,
          "blue": 0,
          "green": 0,
          "red": 0
        }
      ],
      "roundTripNewCount": 1
    }
  ],
  "seedFraction": 0.30215498399752305
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.CanvasReadbackProbe` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Repeated fields are `type: "array"` with an `item`; a repeated group's item is a `record` with its own `order` and `fields`.
