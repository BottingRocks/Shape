# UiEventSamplesTouch

[Signals index](README.md) · [data-shape example](ui_event_samples_touch.js) · [schema format](schema.md)

## What it collects

Records touch activity on the document: one record per touchstart, touchmove or touchend event, keeping the newest 20. Each record has a time, the list of touch points (finger positions on the page and relative to the touched element, plus pressure), the event type and two trust flags. All records are sent in one message when the report is built; each record's time is sent as the gap since the previous record, and within a record each point after the first is sent as the difference from the point before it.

## Why

Touch paths, timing and pressure are strong human-vs-bot behaviour signals on mobile; isTrusted and instanceof UIEvent also expose touch events synthesised by scripts or emulators.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `recordCount` | number | Number of touch records that follow (up to 20). |
| `records` | group, repeated (once per touch event, oldest first, up to 20) | One entry per repetition with the fields below. |
| `records[].time` | number | Milliseconds since the script started (usually fractional); for later records, time since the previous record. |
| `records[].pointCount` | number | Number of touch points that follow: event.touches, or event.changedTouches when no fingers remain (so touchend lists the lifted fingers). |
| `records[].points` | group, repeated (once per touch point in the event) | One entry per repetition with the fields below. |
| `records[].points[].force` | number or null (tag byte) | touch.force (pressure, usually 0..1, fractional); after the first point, difference from the previous point. Or a null marker if force is missing. |
| `records[].points[].targetRelY` | number | Touch Y relative to the touched element's top edge, rounded (0 if the target is not an element); after the first point, difference from the previous point. |
| `records[].points[].pageY` | number | Touch Y on the page in CSS pixels, rounded; after the first point, difference from the previous point. |
| `records[].points[].pageX` | number | Touch X on the page in CSS pixels, rounded; after the first point, difference from the previous point. |
| `records[].points[].targetRelX` | number | Touch X relative to the touched element's left edge, rounded (0 if the target is not an element); after the first point, difference from the previous point. |
| `records[].points[].pairedFields` | number | Holder, in some builds, of two neighbouring record fields written under one condition: when the first field is missing it carries that field's null marker and then the second field, otherwise both values. The two fields keep their own names inside it. |
| `records[].flags` | flags (byte) | Flag bits: is touchmove; is touchstart (neither set means touchend); event.isTrusted is true; event is an instanceof UIEvent. |

Flag names of `records[].flags` (card order; the bit of each flag is per build, see the schema's `bits`):

- `isTouchmove`
- `isTouchstart`
- `isTrusted`
- `instanceOfUIEvent`

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "records": []
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.UiEventSamplesTouch` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Repeated fields are `type: "array"` with an `item`; a repeated group's item is a `record` with its own `order` and `fields`.
- Flag bytes are `type: "flags"` with `bits` mapping each flag name above to its bit (0 = least significant) in this build. If the bits cannot all be named the field stays `type: "byte"` and is listed under `diagnostics.flags_fallback`.
