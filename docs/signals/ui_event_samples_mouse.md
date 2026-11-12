# UiEventSamplesMouse

[Signals index](README.md) · [data-shape example](ui_event_samples_mouse.js) · [schema format](schema.md)

## What it collects

Records mouse activity on the document: every mousemove (raw), a second mousemove trail throttled to one sample per 130 ms, and every mousedown, mouseup and click with position, button and details of the clicked element. It keeps the newest records in rings (50 raw moves, 50 throttled moves, 30 button events) and sends all three lists in one message when the report is built. After the first record in each list, times, coordinates and sizes are sent as differences from the previous record.

## Why

Mouse paths, timing and click placement are among the strongest human-vs-bot behaviour signals; isTrusted and instanceof UIEvent also expose synthetic events dispatched by scripts.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `rawMoveCount` | number | Number of raw mousemove records that follow (up to 50). |
| `rawMoves` | group, repeated (once per raw mousemove record, oldest first, up to 50) | One entry per repetition with the fields below. |
| `rawMoves[].pageY` | number or null (tag byte) | Mouse Y position on the page in CSS pixels (not rounded); difference from the previous record after the first. Or a null marker if missing. |
| `rawMoves[].flags` | flags (byte) | Two flag bits: event is an instanceof UIEvent; event.isTrusted is true. |
| `rawMoves[].time` | number | Milliseconds since the script started (usually fractional); for later records, time since the previous record. |
| `rawMoves[].pageX` | number or null (tag byte) | Mouse X position on the page in CSS pixels (not rounded); difference from the previous record after the first. Or a null marker if missing. |
| `rawMoves[].pairedFields` | number | Holder, in some builds, of two neighbouring record fields written under one condition: when the first field is missing it carries that field's null marker and then the second field, otherwise both values. The two fields keep their own names inside it. |
| `throttledMoveCount` | number | Number of throttled mousemove records that follow (up to 50). Only samples older than the oldest raw move are sent. |
| `throttledMoves` | group, repeated (once per throttled mousemove record (at most one per 130 ms), oldest first) | One entry per repetition with the fields below. |
| `throttledMoves[].pageX` | number or null (tag byte) | Mouse X on the page, rounded to whole pixels; difference from the previous record after the first. Or a null marker if missing. |
| `throttledMoves[].flags` | flags (byte) | Two flag bits: event is an instanceof UIEvent; event.isTrusted is true. |
| `throttledMoves[].pageY` | number or null (tag byte) | Mouse Y on the page, rounded to whole pixels; difference from the previous record after the first. Or a null marker if missing. |
| `throttledMoves[].time` | number | Milliseconds since the script started; for later records, time since the previous record. |
| `throttledMoves[].pairedFields` | number | Holder, in some builds, of two neighbouring record fields written under one condition: when the first field is missing it carries that field's null marker and then the second field, otherwise both values. The two fields keep their own names inside it. |
| `buttonEventCount` | number | Number of mousedown / mouseup / click records that follow (up to 30). |
| `buttonEvents` | group, repeated (once per mousedown, mouseup or click, oldest first, up to 30) | One entry per repetition with the fields below. |
| `buttonEvents[].targetHeight` | number or null (tag byte) | offsetHeight of the event target in pixels; difference from the previous record after the first. Or a null marker if there is no target. |
| `buttonEvents[].button` | number or null (tag byte) | event.button: 0 left, 1 middle, 2 right (3/4 back/forward). Or a null marker if missing. |
| `buttonEvents[].targetWidth` | number or null (tag byte) | offsetWidth of the event target in pixels; difference from the previous record after the first. Or a null marker if there is no target. |
| `buttonEvents[].targetName` | string | The target element's name attribute. Or a marker for 'same as previous record' (omitted) or for null. |
| `buttonEvents[].targetId` | string | The target element's id. Or a marker for 'same as previous record' (omitted) or for null. |
| `buttonEvents[].pageX` | number or null (tag byte) | Click X on the page, rounded; difference from the previous record after the first. Or a null marker if missing. |
| `buttonEvents[].offsetX` | number or null (tag byte) | Click X relative to the target's left edge, rounded (0 if the target is not an element); difference after the first record. Or a null marker if missing. |
| `buttonEvents[].pageY` | number or null (tag byte) | Click Y on the page, rounded; difference from the previous record after the first. Or a null marker if missing. |
| `buttonEvents[].time` | number | Milliseconds since the script started; for later records, time since the previous record. |
| `buttonEvents[].flags` | flags (byte) | Flag bits: event is an instanceof UIEvent; is mouseup; is mousedown; event.isTrusted is true. A click sets neither mouseup nor mousedown. |
| `buttonEvents[].offsetY` | number or null (tag byte) | Click Y relative to the target's top edge, rounded (0 if the target is not an element); difference after the first record. Or a null marker if missing. |
| `buttonEvents[].pairedFields` | number | Holder, in some builds, of two neighbouring record fields written under one condition: when the first field is missing it carries that field's null marker and then the second field, otherwise both values. The two fields keep their own names inside it. |

Flag names of `rawMoves[].flags` (card order; the bit of each flag is per build, see the schema's `bits`):

- `instanceOfUIEvent`
- `isTrusted`

Flag names of `throttledMoves[].flags` (card order; the bit of each flag is per build, see the schema's `bits`):

- `instanceOfUIEvent`
- `isTrusted`

Flag names of `buttonEvents[].flags` (card order; the bit of each flag is per build, see the schema's `bits`):

- `instanceOfUIEvent`
- `isMouseup`
- `isMousedown`
- `isTrusted`

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "buttonEvents": [
    {
      "button": 0,
      "flags": {
        "instanceOfUIEvent": true,
        "isMousedown": false,
        "isMouseup": false,
        "isTrusted": true
      },
      "offsetX": 0,
      "offsetY": 0,
      "pageX": 0,
      "pageY": 0,
      "targetHeight": 0,
      "targetId": 130,
      "targetName": 134,
      "targetWidth": 0,
      "time": 0
    },
    {
      "button": 0,
      "flags": {
        "instanceOfUIEvent": true,
        "isMousedown": false,
        "isMouseup": true,
        "isTrusted": true
      },
      "offsetX": 0,
      "offsetY": 0,
      "pageX": 0,
      "pageY": 0,
      "targetHeight": 0,
      "targetId": 130,
      "targetName": 134,
      "targetWidth": 0,
      "time": 66.20000004768372
    },
    {
      "button": 0,
      "flags": {
        "instanceOfUIEvent": true,
        "isMousedown": true,
        "isMouseup": false,
        "isTrusted": true
      },
      "offsetX": -1089,
      "offsetY": -221,
      "pageX": -387,
      "pageY": 3,
      "targetHeight": -413,
      "targetId": "login",
      "targetName": "",
      "targetWidth": -1405,
      "time": 39779.59999990463
    },
    {
      "button": 0,
      "flags": {
        "instanceOfUIEvent": true,
        "isMousedown": false,
        "isMouseup": false,
        "isTrusted": true
      },
      "offsetX": 0,
      "offsetY": 0,
      "pageX": 0,
      "pageY": 0,
      "targetHeight": 0,
      "targetId": 130,
      "targetName": 134,
      "targetWidth": 0,
      "time": 0.10000002384185791
    },
    {
      "button": 0,
      "flags": {
        "instanceOfUIEvent": true,
        "isMousedown": false,
        "isMouseup": true,
        "isTrusted": true
      },
      "offsetX": 0,
      "offsetY": 0,
      "pageX": 0,
      "pageY": 0,
      "targetHeight": 0,
      "targetId": 130,
      "targetName": 134,
      "targetWidth": 0,
      "time": 107.89999997615814
    },
    {
      "button": 0,
      "flags": {
        "instanceOfUIEvent": true,
        "isMousedown": true,
        "isMouseup": false,
        "isTrusted": true
      },
      "offsetX": 1082,
      "offsetY": 233,
      "pageX": 380,
      "pageY": 72,
      "targetHeight": 417,
      "targetId": "",
      "targetName": null,
      "targetWidth": 1405,
      "time": 1075
    },
    {
      "button": 0,
      "flags": {
        "instanceOfUIEvent": true,
        "isMousedown": false,
        "isMouseup": false,
        "isTrusted": true
      },
      "offsetX": 0,
      "offsetY": 0,
      "pageX": 0,
      "pageY": 0,
      "targetHeight": 0,
      "targetId": 130,
      "targetName": 134,
      "targetWidth": 0,
      "time": 0.5
    },
    {
      "button": 0,
      "flags": {
        "instanceOfUIEvent": true,
        "isMousedown": false,
        "isMouseup": true,
        "isTrusted": true
      },
      "offsetX": 0,
      "offsetY": 0,
      "pageX": 0,
      "pageY": 0,
      "targetHeight": 0,
      "targetId": 130,
      "targetName": 134,
      "targetWidth": 0,
      "time": 98.70000004768372
    },
    {
      "button": 0,
      "flags": {
        "instanceOfUIEvent": true,
        "isMousedown": true,
        "isMouseup": false,
        "isTrusted": true
      },
      "offsetX": 261,
      "offsetY": 21,
      "pageX": 963,
      "pageY": 205,
      "targetHeight": 44,
      "targetId": "username",
      "targetName": "username",
      "targetWidth": 500,
      "time": 8689
    }
  ],
  "rawMoves": [
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 971,
      "pageY": 233,
      "time": 49125
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 0,
      "time": 8.399999976158142
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 0,
      "time": 96.20000004768372
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 1,
      "time": 81
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 0,
      "time": 15.799999952316284
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 0,
      "time": 8
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 0,
      "time": 7.899999976158142
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 1,
      "time": 8.300000071525574
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 1,
      "time": 16.299999952316284
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 0,
      "time": 8.300000071525574
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 2,
      "time": 8.299999952316284
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 1,
      "time": 8.399999976158142
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 2,
      "time": 8.399999976158142
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 2,
      "time": 8.300000071525574
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 2,
      "time": 8.100000023841858
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 2,
      "time": 8.599999904632568
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 2,
      "time": 8.100000023841858
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 4,
      "time": 8.600000023841858
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 1,
      "time": 8
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 2,
      "time": 8.299999952316284
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 3,
      "time": 8.300000071525574
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 2,
      "time": 8.399999976158142
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 2,
      "time": 10.100000023841858
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 1,
      "time": 6.5
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 1,
      "time": 8.299999952316284
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 0,
      "time": 8.5
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 1,
      "time": 8.200000047683716
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 0,
      "time": 8.5
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 1,
      "time": 8.199999928474426
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 0,
      "time": 8.300000071525574
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 0,
      "time": 8.299999952316284
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 0,
      "time": 34.200000047683716
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 0,
      "time": 8.199999928474426
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 1,
      "time": 8.100000023841858
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 0,
      "time": 7.899999976158142
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 0,
      "time": 8.300000071525574
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 1,
      "time": 8.299999952316284
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 1,
      "time": 8.300000071525574
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 1,
      "time": 8.399999976158142
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 1,
      "time": 8.399999976158142
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 1,
      "time": 8.200000047683716
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 2,
      "time": 8.399999976158142
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 1,
      "time": 8.299999952316284
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 1,
      "time": 8.300000071525574
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 1,
      "time": 8.399999976158142
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 1,
      "time": 8.299999952316284
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 0,
      "time": 8.200000047683716
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 1,
      "time": 8.5
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 0,
      "time": 8.199999928474426
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 0,
      "time": 26.200000047683716
    }
  ],
  "throttledMoves": [
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 8,
      "pageY": 6,
      "time": 133.29999995231628
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 32,
      "pageY": -21,
      "time": 133.60000002384186
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 43,
      "pageY": -21,
      "time": 133.39999997615814
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 30,
      "pageY": -15,
      "time": 133.20000004768372
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 6,
      "pageY": -12,
      "time": 133.29999995231628
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -151,
      "pageY": 152,
      "time": 131.5
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -27,
      "pageY": -142,
      "time": 37080.200000047684
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 0,
      "time": 216.70000004768372
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": -1,
      "time": 179.79999995231628
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -9,
      "pageY": -5,
      "time": 179.10000002384186
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -30,
      "pageY": -47,
      "time": 133.39999997615814
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -33,
      "pageY": 0,
      "time": 133
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -108,
      "pageY": 35,
      "time": 133.20000004768372
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -133,
      "pageY": 26,
      "time": 139.29999995231628
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 3,
      "pageY": 0,
      "time": 302.2999999523163
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 47,
      "pageY": 5,
      "time": 133.5
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 207,
      "pageY": -21,
      "time": 131.60000002384186
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 122,
      "pageY": -30,
      "time": 303.89999997615814
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 118,
      "time": 613
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 0,
      "pageY": 0,
      "time": 168
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 11,
      "pageY": -1,
      "time": 133.5
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 20,
      "pageY": -11,
      "time": 133.20000004768372
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 45,
      "pageY": -26,
      "time": 132.39999997615814
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 37,
      "pageY": -6,
      "time": 133.60000002384186
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -105,
      "pageY": 43,
      "time": 133.20000004768372
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 20,
      "pageY": -17,
      "time": 133.69999992847443
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 45,
      "pageY": -22,
      "time": 133.60000002384186
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 26,
      "pageY": -26,
      "time": 133
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -122,
      "pageY": 59,
      "time": 1263.2999999523163
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -1,
      "pageY": 0,
      "time": 245.70000004768372
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -6,
      "pageY": -25,
      "time": 132
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": -3,
      "pageY": -34,
      "time": 233.79999995231628
    },
    {
      "flags": {
        "instanceOfUIEvent": true,
        "isTrusted": true
      },
      "pageX": 996,
      "pageY": 271,
      "time": 5497.200000047684
    }
  ]
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.UiEventSamplesMouse` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Repeated fields are `type: "array"` with an `item`; a repeated group's item is a `record` with its own `order` and `fields`.
- Flag bytes are `type: "flags"` with `bits` mapping each flag name above to its bit (0 = least significant) in this build. If the bits cannot all be named the field stays `type: "byte"` and is listed under `diagnostics.flags_fallback`.
