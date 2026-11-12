# EmojiGlyphRgbaSamples

[Signals index](README.md) · [data-shape example](emoji_glyph_rgba_samples.js) · [schema format](schema.md)

## What it collects

Draws about 20 emoji (ZWJ sequences, flags, very recent code points) one at a time in black 94px sans-serif on a 96x96 canvas and reads the full RGBA value of two fixed pixels for each, packed into one 32-bit number. After each emoji it sends the rows collected so far, and it sends a final message with all rows when done.

## Why

The pixels change with the emoji font, the emoji/Unicode version supported, colour vs. fallback rendering and anti-aliasing, so they fingerprint the OS and browser and detect canvas spoofing (likely).

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `rowCount` | number | Number of emoji rows that follow (grows by one per partial message; 20 or 21 when complete). |
| `rows` | group, repeated (each emoji drawn so far) | One entry per repetition with the fields below. |
| `rows[].sampleCount` | number | Number of pixel samples for this emoji (2 normally; fewer if drawing failed). |
| `rows[].pixels` | number, repeated (each sampled pixel of this emoji) | Pixel colour packed as R&lt;&lt;24 \| G&lt;&lt;16 \| B&lt;&lt;8 \| A, a signed 32-bit number (0 = transparent; negative when red is 128 or more). |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "rows": [
    [
      -1114946561,
      -677437185
    ],
    [
      -1417522945,
      -253251073
    ],
    [
      -1738996225,
      1701144063
    ],
    [
      1701144063,
      1296911871
    ],
    [
      1972876799,
      -2036485633
    ],
    [
      -241423617,
      -274452737
    ],
    [
      -1466342145,
      1216982783
    ],
    [
      -687734273,
      -1219471617
    ],
    [
      -480881921,
      -1045342721
    ],
    [
      -1432582401,
      -2123276289
    ],
    [
      -211612417,
      -110620929
    ],
    [
      -2127616425,
      -2027219713
    ],
    [
      759454205,
      1016160767
    ],
    [
      -44288001,
      -801953793
    ],
    [
      608003327,
      810520575
    ],
    [
      857872127,
      1161046015
    ],
    [
      -527497217,
      -3189761
    ],
    [
      -185272833,
      -421009153
    ],
    [
      -3395073,
      -391373569
    ],
    [
      508038815,
      727391231
    ],
    [
      4,
      -1467976193
    ]
  ]
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.EmojiGlyphRgbaSamples` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Repeated fields are `type: "array"` with an `item`; a repeated group's item is a `record` with its own `order` and `fields`.
