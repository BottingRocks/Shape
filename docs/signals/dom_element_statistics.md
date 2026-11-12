# DomElementStatistics

[Signals index](README.md) · [data-shape example](dom_element_statistics.js) · [schema format](schema.md)

## What it collects

Walks every element in the document once and reports a census: total element count, counts of script, img, svg, link, style and iframe elements, how many children BODY has and the first two characters of its first child's id, how many distinct tag names appear in HEAD and in BODY, and the average squared child count per element.

## Why

The shape of the DOM shows whether the page really loaded and rendered as expected, and reveals injected elements from automation tools or extensions or a stripped page served to a headless bot (likely).

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `headDistinctTagCount` | number | Number of distinct tag names seen from HEAD up to BODY (HEAD itself included). |
| `svgCount` | number | Number of &lt;svg&gt; root elements. |
| `elementCount` | number | Total number of elements in the document. |
| `bodyChildElementCount` | number | Number of element children directly under BODY. |
| `styleCount` | number | Number of &lt;style&gt; elements. |
| `scriptCount` | number | Number of &lt;script&gt; elements. |
| `linkCount` | number | Number of &lt;link&gt; elements. |
| `iframeCount` | number | Number of &lt;iframe&gt; elements. |
| `meanSquaredChildCount` | number | Average over all elements of (number of child elements) squared; usually a fraction. |
| `bodyFirstChildIdPrefix` | string | First two characters of the id of BODY's first child element; empty string if it has no id or BODY has no element children. |
| `bodyDistinctTagCount` | number | Number of distinct tag names seen from BODY onward (BODY itself included). |
| `imgCount` | number | Number of &lt;img&gt; elements. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "bodyChildElementCount": 16,
  "bodyDistinctTagCount": 14,
  "bodyFirstChildIdPrefix": "",
  "elementCount": 102,
  "headDistinctTagCount": 6,
  "iframeCount": 0,
  "imgCount": 0,
  "linkCount": 28,
  "meanSquaredChildCount": 29.774509803921568,
  "scriptCount": 23,
  "styleCount": 1,
  "svgCount": 1
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.DomElementStatistics` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
