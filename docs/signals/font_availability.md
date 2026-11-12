# FontAvailability

[Signals index](README.md) · [data-shape example](font_availability.js) · [schema format](schema.md)

## What it collects

Tests whether each of 47 named fonts (Andale Mono to Wingdings 3, plus the fake name 'Helkevtrica') is installed: it measures a test string on a canvas with the font and a sans-serif, then a serif fallback, and calls the font present if the width differs from the plain fallback. Fonts are probed a few at a time in the background.

## Why

The set of installed fonts strongly identifies the operating system and the device's installed software; the fake font should always be absent, so a 'present' result points to a spoofed measureText (likely).

## Fields

This signal sends one of 2 message shapes.

### Message 1: family A (most builds, including the main build): fonts are probed in a shuffled order; one message at init with all results 0, then one after every probed font with all results so far.

| Field | Type | Meaning |
| --- | --- | --- |
| `fontCount` | number | Number of result slots (47). |
| `fontResults` | number, repeated (each font) | 0 = not probed yet, 1 = not installed, 2 = installed. Slots are in shuffled probe order and written last slot first in the main build. |
| `fontListVersion` | number | Version of the font list (5). |
| `shuffleSeed` | number | Random fraction between 0 and 1 that seeded the shuffle, so the receiver can rebuild the probe order. |

### Message 2: family B (starbucks, tdbank and some puma builds): no shuffle; one message after all fonts are probed, nothing if there is no 2D canvas.

| Field | Type | Meaning |
| --- | --- | --- |
| `fontCount` | number | Number of fonts tested (47). |
| `fontPresenceBits` | flags (byte), repeated (each full group of 8 fonts) | 8 fonts per byte, one bit each (1 = installed), in font-list order (some builds walk the list from the end); one byte per full group of 8 fonts (5 bytes for 47 fonts). |
| `fontPresenceTail` | flags (byte) | The leftover fonts that do not fill a whole byte (7 for 47 fonts), one bit each, packed the same way; not sent when the font count is a multiple of 8. |
| `fontListVersion` | number | Version of the font list (5). |

Flag names of `fontPresenceBits` (card order; the bit of each flag is per build, see the schema's `bits`):

- `Andale Mono`
- `Arial Narrow`
- `Arial Unicode MS`
- `Batang`
- `Bell MT`
- `Brush Script`
- `Brush Script MT`
- `Calibri`
- `Charter`
- `Courier`
- `Courier New`
- `Curlz MT`
- `DejaVu Sans`
- `DejaVu Sans Mono`
- `DejaVu Serif Condensed`
- `Droid Sans`
- `Droid Sans Fallback`
- `Droid Serif`
- `Forte`
- `Futura`
- `Geneva`
- `Hei`
- `Leelawadee`
- `Levenim MT`
- `Liberation Sans`
- `Liberation Sans Narrow`
- `Marlett`
- `Meiryo UI`
- `Microsoft Uighur`
- `Microsoft YaHei UI`
- `MS Mincho`
- `MS UI Gothic`
- `NanumGothic`
- `Nirmala UI`
- `Palatino`
- `Papyrus`
- `PMingLiU`
- `PT Serif`
- `SimHei`
- `STIXVariants`
- `STSong`
- `Traditional Arabic`
- `Urdu Typesetting`
- `Verdana`
- `Wingdings`
- `Wingdings 3`
- `Helkevtrica`

Flag names of `fontPresenceTail` (card order; the bit of each flag is per build, see the schema's `bits`):

- `Andale Mono`
- `Arial Narrow`
- `Arial Unicode MS`
- `Batang`
- `Bell MT`
- `Brush Script`
- `Brush Script MT`
- `Calibri`
- `Charter`
- `Courier`
- `Courier New`
- `Curlz MT`
- `DejaVu Sans`
- `DejaVu Sans Mono`
- `DejaVu Serif Condensed`
- `Droid Sans`
- `Droid Sans Fallback`
- `Droid Serif`
- `Forte`
- `Futura`
- `Geneva`
- `Hei`
- `Leelawadee`
- `Levenim MT`
- `Liberation Sans`
- `Liberation Sans Narrow`
- `Marlett`
- `Meiryo UI`
- `Microsoft Uighur`
- `Microsoft YaHei UI`
- `MS Mincho`
- `MS UI Gothic`
- `NanumGothic`
- `Nirmala UI`
- `Palatino`
- `Papyrus`
- `PMingLiU`
- `PT Serif`
- `SimHei`
- `STIXVariants`
- `STSong`
- `Traditional Arabic`
- `Urdu Typesetting`
- `Verdana`
- `Wingdings`
- `Wingdings 3`
- `Helkevtrica`

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "fontListVersion": 5,
  "fontResults": [
    1,
    2,
    1,
    1,
    1,
    1,
    1,
    1,
    1,
    1,
    1,
    1,
    2,
    1,
    2,
    1,
    1,
    1,
    2,
    2,
    1,
    1,
    2,
    2,
    1,
    1,
    1,
    2,
    2,
    2,
    2,
    2,
    2,
    2,
    1,
    1,
    1,
    1,
    1,
    2,
    2,
    1,
    2,
    1,
    1,
    1,
    1
  ],
  "shuffleSeed": 0.5613393995242942
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.FontAvailability` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- It has 2 message shapes, so the entry carries `messages` (`when`, `mask`, `order`, `fields` each) instead of top-level `order`/`fields`.
- Repeated fields are `type: "array"` with an `item`.
- Flag bytes are `type: "flags"` with `bits` mapping each flag name above to its bit (0 = least significant) in this build. If the bits cannot all be named the field stays `type: "byte"` and is listed under `diagnostics.flags_fallback`.
