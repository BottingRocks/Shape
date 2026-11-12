# MatchMedia

[Signals index](README.md) · [data-shape example](match_media.js) · [schema format](schema.md)

## What it collects

Asks the browser a long list of CSS media queries with window.matchMedia. For yes/no style features (hover, pointer, color scheme, display mode, ...) it records which of the possible values match. For numeric features (viewport width and height, color depth, resolution, aspect ratio) it finds the boundary value by searching with max-* queries. It also checks a made-up query, (q: 1), that a real browser should never match. One message is sent during collection.

## Why

Media-query answers describe the real device and user settings (screen, input devices, dark mode, reduced motion, colour gamut) and are hard to fake consistently; a vendor likely compares them with the claimed user agent and other screen values to spot spoofing, headless browsers and hooked matchMedia.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `gridCount` | number | grid group: number of candidate values tested, always 2 (0, 1). |
| `gridBits` | flags (byte) | grid group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates. |
| `gridFlush` | flags (byte) | grid group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate ((grid: 0), (grid: 1)); bit order (ascending or descending) varies per build. |
| `overflowInlineCount` | number | overflow-inline group: number of candidate values tested, always 2 (none, scroll). |
| `overflowInlineBits` | flags (byte) | overflow-inline group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates. |
| `overflowInlineFlush` | flags (byte) | overflow-inline group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (none, scroll); bit order (ascending or descending) varies per build. |
| `resolution` | number | Screen resolution boundary: the smallest whole dpi value where (max-resolution: N dpi) matches (e.g. 96 or 192). If min-resolution is not supported, the boundary of -webkit-max-device-pixel-ratio instead (e.g. 1, 2 or 3). |
| `anyPointerCount` | number | any-pointer group: number of candidate values tested, always 3 (none, coarse, fine). |
| `anyPointerBits` | flags (byte) | any-pointer group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates. |
| `anyPointerFlush` | flags (byte) | any-pointer group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (none, coarse, fine); bit order (ascending or descending) varies per build. |
| `maxMonochrome` | number | Bits per pixel of a monochrome display found with max-monochrome; 0 on normal colour screens. |
| `maxHeight` | number | Viewport height in CSS pixels (rounded up), found with max-height. |
| `aspectNumerator` | number | Numerator of a fraction that approximates the viewport width/height ratio from below, found with max-aspect-ratio (e.g. 16 in 16/9). |
| `maxColor` | number | Bits per colour component found with max-color, typically 8 (10 on HDR/wide screens). |
| `prefersContrastCount` | number | prefers-contrast group: number of candidate values tested, always 3 (no-preference, high, low). |
| `prefersContrastBits` | flags (byte) | prefers-contrast group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates. |
| `prefersContrastFlush` | flags (byte) | prefers-contrast group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (no-preference, high, low); bit order (ascending or descending) varies per build. |
| `aspectDenominator` | number | Denominator of the viewport aspect-ratio fraction (pairs with aspectNumerator). |
| `scriptingCount` | number | scripting group: number of candidate values tested, always 3 (none, initial-only, enabled). |
| `scriptingBits` | flags (byte) | scripting group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates. |
| `scriptingFlush` | flags (byte) | scripting group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (none, initial-only, enabled); bit order (ascending or descending) varies per build. |
| `lightLevelCount` | number | light-level group: number of candidate values tested, always 3 (normal, dim, washed). |
| `lightLevelBits` | flags (byte) | light-level group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates. |
| `lightLevelFlush` | flags (byte) | light-level group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (normal, dim, washed); bit order (ascending or descending) varies per build. |
| `displayModeCount` | number | display-mode group: number of candidate values tested, always 4 (fullscreen, standalone, minimal-ui, browser). |
| `displayModeBits` | flags (byte) | display-mode group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates. |
| `displayModeFlush` | flags (byte) | display-mode group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (fullscreen, standalone, minimal-ui, browser); bit order (ascending or descending) varies per build. |
| `forcedColorsCount` | number | forced-colors group: number of candidate values tested, always 2 (none, active). |
| `forcedColorsBits` | flags (byte) | forced-colors group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates. |
| `forcedColorsFlush` | flags (byte) | forced-colors group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (none, active); bit order (ascending or descending) varies per build. |
| `maxColorIndex` | number | Size of the colour lookup table found with max-color-index; 0 on normal screens. |
| `hoverCount` | number | hover group: number of candidate values tested, always 2 (none, hover). |
| `hoverBits` | flags (byte) | hover group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates. |
| `hoverFlush` | flags (byte) | hover group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (none, hover); bit order (ascending or descending) varies per build. |
| `prefersColorSchemeCount` | number | prefers-color-scheme group: number of candidate values tested, always 3 (no-preference, light, dark). |
| `prefersColorSchemeBits` | flags (byte) | prefers-color-scheme group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates. |
| `prefersColorSchemeFlush` | flags (byte) | prefers-color-scheme group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (no-preference, light, dark); bit order (ascending or descending) varies per build. |
| `colorGamutCount` | number | color-gamut group: number of candidate values tested, always 3 (srgb, p3, rec2020). |
| `colorGamutBits` | flags (byte) | color-gamut group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates. |
| `colorGamutFlush` | flags (byte) | color-gamut group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (srgb, p3, rec2020); bit order (ascending or descending) varies per build. |
| `invertedColorsCount` | number | inverted-colors group: number of candidate values tested, always 2 (none, inverted). |
| `invertedColorsBits` | flags (byte) | inverted-colors group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates. |
| `invertedColorsFlush` | flags (byte) | inverted-colors group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (none, inverted); bit order (ascending or descending) varies per build. |
| `orientationCount` | number | orientation group: number of candidate values tested, always 2 (landscape, portrait). |
| `orientationBits` | flags (byte) | orientation group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates. |
| `orientationFlush` | flags (byte) | orientation group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (landscape, portrait); bit order (ascending or descending) varies per build. |
| `scanCount` | number | scan group: number of candidate values tested, always 2 (interlace, progressive). |
| `scanBits` | flags (byte) | scan group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates. |
| `scanFlush` | flags (byte) | scan group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (interlace, progressive); bit order (ascending or descending) varies per build. |
| `prefersReducedTransparencyCount` | number | prefers-reduced-transparency group: number of candidate values tested, always 2 (no-preference, reduce). |
| `prefersReducedTransparencyBits` | flags (byte) | prefers-reduced-transparency group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates. |
| `prefersReducedTransparencyFlush` | flags (byte) | prefers-reduced-transparency group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (no-preference, reduce); bit order (ascending or descending) varies per build. |
| `fakeQueryCheck` | boolean (tag byte) | Whether the made-up query (q: 1) matches: one fixed byte value for yes and another for no (the pair changes per build). A real browser always answers no; yes likely means matchMedia is hooked or emulated. |
| `updateCount` | number | update group: number of candidate values tested, always 3 (none, slow, fast). |
| `updateBits` | flags (byte) | update group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates. |
| `updateFlush` | flags (byte) | update group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (none, slow, fast); bit order (ascending or descending) varies per build. |
| `pointerCount` | number | pointer group: number of candidate values tested, always 3 (none, coarse, fine). |
| `pointerBits` | flags (byte) | pointer group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates. |
| `pointerFlush` | flags (byte) | pointer group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (none, coarse, fine); bit order (ascending or descending) varies per build. |
| `anyHoverCount` | number | any-hover group: number of candidate values tested, always 2 (none, hover). |
| `anyHoverBits` | flags (byte) | any-hover group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates. |
| `anyHoverFlush` | flags (byte) | any-hover group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (none, hover); bit order (ascending or descending) varies per build. |
| `maxWidth` | number | Viewport width in CSS pixels (rounded up), found with max-width. |
| `prefersReducedMotionCount` | number | prefers-reduced-motion group: number of candidate values tested, always 2 (no-preference, reduce). |
| `prefersReducedMotionBits` | flags (byte) | prefers-reduced-motion group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates. |
| `prefersReducedMotionFlush` | flags (byte) | prefers-reduced-motion group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (no-preference, reduce); bit order (ascending or descending) varies per build. |
| `overflowBlockCount` | number | overflow-block group: number of candidate values tested, always 4 (none, scroll, optional-paged, paged). |
| `overflowBlockBits` | flags (byte) | overflow-block group: in-loop flush byte of the match-bit loop, written once per 8 candidates; never written because the group has fewer than 8 candidates. |
| `overflowBlockFlush` | flags (byte) | overflow-block group: the trailing flush byte holding the packed match bits, 1 = matches, one bit per candidate (none, scroll, optional-paged, paged); bit order (ascending or descending) varies per build. |

Flag names of `gridBits` (card order; the bit of each flag is per build, see the schema's `bits`):

- `0`
- `1`

Flag names of `gridFlush` (card order; the bit of each flag is per build, see the schema's `bits`):

- `0`
- `1`

Flag names of `overflowInlineBits` (card order; the bit of each flag is per build, see the schema's `bits`):

- `none`
- `scroll`

Flag names of `overflowInlineFlush` (card order; the bit of each flag is per build, see the schema's `bits`):

- `none`
- `scroll`

Flag names of `anyPointerBits` (card order; the bit of each flag is per build, see the schema's `bits`):

- `none`
- `coarse`
- `fine`

Flag names of `anyPointerFlush` (card order; the bit of each flag is per build, see the schema's `bits`):

- `none`
- `coarse`
- `fine`

Flag names of `prefersContrastBits` (card order; the bit of each flag is per build, see the schema's `bits`):

- `no-preference`
- `high`
- `low`

Flag names of `prefersContrastFlush` (card order; the bit of each flag is per build, see the schema's `bits`):

- `no-preference`
- `high`
- `low`

Flag names of `scriptingBits` (card order; the bit of each flag is per build, see the schema's `bits`):

- `none`
- `initial-only`
- `enabled`

Flag names of `scriptingFlush` (card order; the bit of each flag is per build, see the schema's `bits`):

- `none`
- `initial-only`
- `enabled`

Flag names of `lightLevelBits` (card order; the bit of each flag is per build, see the schema's `bits`):

- `normal`
- `dim`
- `washed`

Flag names of `lightLevelFlush` (card order; the bit of each flag is per build, see the schema's `bits`):

- `normal`
- `dim`
- `washed`

Flag names of `displayModeBits` (card order; the bit of each flag is per build, see the schema's `bits`):

- `fullscreen`
- `standalone`
- `minimal-ui`
- `browser`

Flag names of `displayModeFlush` (card order; the bit of each flag is per build, see the schema's `bits`):

- `fullscreen`
- `standalone`
- `minimal-ui`
- `browser`

Flag names of `forcedColorsBits` (card order; the bit of each flag is per build, see the schema's `bits`):

- `none`
- `active`

Flag names of `forcedColorsFlush` (card order; the bit of each flag is per build, see the schema's `bits`):

- `none`
- `active`

Flag names of `hoverBits` (card order; the bit of each flag is per build, see the schema's `bits`):

- `none`
- `hover`

Flag names of `hoverFlush` (card order; the bit of each flag is per build, see the schema's `bits`):

- `none`
- `hover`

Flag names of `prefersColorSchemeBits` (card order; the bit of each flag is per build, see the schema's `bits`):

- `no-preference`
- `light`
- `dark`

Flag names of `prefersColorSchemeFlush` (card order; the bit of each flag is per build, see the schema's `bits`):

- `no-preference`
- `light`
- `dark`

Flag names of `colorGamutBits` (card order; the bit of each flag is per build, see the schema's `bits`):

- `srgb`
- `p3`
- `rec2020`

Flag names of `colorGamutFlush` (card order; the bit of each flag is per build, see the schema's `bits`):

- `srgb`
- `p3`
- `rec2020`

Flag names of `invertedColorsBits` (card order; the bit of each flag is per build, see the schema's `bits`):

- `none`
- `inverted`

Flag names of `invertedColorsFlush` (card order; the bit of each flag is per build, see the schema's `bits`):

- `none`
- `inverted`

Flag names of `orientationBits` (card order; the bit of each flag is per build, see the schema's `bits`):

- `landscape`
- `portrait`

Flag names of `orientationFlush` (card order; the bit of each flag is per build, see the schema's `bits`):

- `landscape`
- `portrait`

Flag names of `scanBits` (card order; the bit of each flag is per build, see the schema's `bits`):

- `interlace`
- `progressive`

Flag names of `scanFlush` (card order; the bit of each flag is per build, see the schema's `bits`):

- `interlace`
- `progressive`

Flag names of `prefersReducedTransparencyBits` (card order; the bit of each flag is per build, see the schema's `bits`):

- `no-preference`
- `reduce`

Flag names of `prefersReducedTransparencyFlush` (card order; the bit of each flag is per build, see the schema's `bits`):

- `no-preference`
- `reduce`

Flag names of `updateBits` (card order; the bit of each flag is per build, see the schema's `bits`):

- `none`
- `slow`
- `fast`

Flag names of `updateFlush` (card order; the bit of each flag is per build, see the schema's `bits`):

- `none`
- `slow`
- `fast`

Flag names of `pointerBits` (card order; the bit of each flag is per build, see the schema's `bits`):

- `none`
- `coarse`
- `fine`

Flag names of `pointerFlush` (card order; the bit of each flag is per build, see the schema's `bits`):

- `none`
- `coarse`
- `fine`

Flag names of `anyHoverBits` (card order; the bit of each flag is per build, see the schema's `bits`):

- `none`
- `hover`

Flag names of `anyHoverFlush` (card order; the bit of each flag is per build, see the schema's `bits`):

- `none`
- `hover`

Flag names of `prefersReducedMotionBits` (card order; the bit of each flag is per build, see the schema's `bits`):

- `no-preference`
- `reduce`

Flag names of `prefersReducedMotionFlush` (card order; the bit of each flag is per build, see the schema's `bits`):

- `no-preference`
- `reduce`

Flag names of `overflowBlockBits` (card order; the bit of each flag is per build, see the schema's `bits`):

- `none`
- `scroll`
- `optional-paged`
- `paged`

Flag names of `overflowBlockFlush` (card order; the bit of each flag is per build, see the schema's `bits`):

- `none`
- `scroll`
- `optional-paged`
- `paged`

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "anyHoverBits": {},
  "anyHoverCount": 2,
  "anyHoverFlush": {
    "hover": true,
    "none": false
  },
  "anyPointerBits": {},
  "anyPointerCount": 3,
  "anyPointerFlush": {
    "coarse": false,
    "fine": true,
    "none": false
  },
  "aspectDenominator": 1125899906842624,
  "aspectNumerator": 7532152686891421,
  "colorGamutBits": {},
  "colorGamutCount": 3,
  "colorGamutFlush": {
    "p3": false,
    "rec2020": false,
    "srgb": true
  },
  "displayModeBits": {},
  "displayModeCount": 4,
  "displayModeFlush": {
    "browser": true,
    "fullscreen": false,
    "minimal-ui": false,
    "standalone": false
  },
  "fakeQueryCheck": false,
  "forcedColorsBits": {},
  "forcedColorsCount": 2,
  "forcedColorsFlush": {
    "active": false,
    "none": true
  },
  "gridBits": {},
  "gridCount": 2,
  "gridFlush": {
    "0": true,
    "1": false
  },
  "hoverBits": {},
  "hoverCount": 2,
  "hoverFlush": {
    "hover": true,
    "none": false
  },
  "invertedColorsBits": {},
  "invertedColorsCount": 2,
  "invertedColorsFlush": {
    "inverted": false,
    "none": false
  },
  "lightLevelBits": {},
  "lightLevelCount": 3,
  "lightLevelFlush": {
    "dim": false,
    "normal": false,
    "washed": false
  },
  "maxColor": 8,
  "maxColorIndex": 0,
  "maxHeight": 287,
  "maxMonochrome": 0,
  "maxWidth": 1920,
  "orientationBits": {},
  "orientationCount": 2,
  "orientationFlush": {
    "landscape": true,
    "portrait": false
  },
  "overflowBlockBits": {},
  "overflowBlockCount": 4,
  "overflowBlockFlush": {
    "none": false,
    "optional-paged": false,
    "paged": false,
    "scroll": true
  },
  "overflowInlineBits": {},
  "overflowInlineCount": 2,
  "overflowInlineFlush": {
    "none": false,
    "scroll": true
  },
  "pointerBits": {},
  "pointerCount": 3,
  "pointerFlush": {
    "coarse": false,
    "fine": true,
    "none": false
  },
  "prefersColorSchemeBits": {},
  "prefersColorSchemeCount": 3,
  "prefersColorSchemeFlush": {
    "dark": true,
    "light": false,
    "no-preference": false
  },
  "prefersContrastBits": {},
  "prefersContrastCount": 3,
  "prefersContrastFlush": {
    "high": false,
    "low": false,
    "no-preference": true
  },
  "prefersReducedMotionBits": {},
  "prefersReducedMotionCount": 2,
  "prefersReducedMotionFlush": {
    "no-preference": true,
    "reduce": false
  },
  "prefersReducedTransparencyBits": {},
  "prefersReducedTransparencyCount": 2,
  "prefersReducedTransparencyFlush": {
    "no-preference": true,
    "reduce": false
  },
  "resolution": 192,
  "scanBits": {},
  "scanCount": 2,
  "scanFlush": {
    "interlace": false,
    "progressive": false
  },
  "scriptingBits": {},
  "scriptingCount": 3,
  "scriptingFlush": {
    "enabled": true,
    "initial-only": false,
    "none": false
  },
  "updateBits": {},
  "updateCount": 3,
  "updateFlush": {
    "fast": true,
    "none": false,
    "slow": false
  }
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.MatchMedia` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Flag bytes are `type: "flags"` with `bits` mapping each flag name above to its bit (0 = least significant) in this build. If the bits cannot all be named the field stays `type: "byte"` and is listed under `diagnostics.flags_fallback`.
