# VisibilityEventLog

[Signals index](README.md) · [data-shape example](visibility_event_log.js) · [schema format](schema.md)

## What it collects

Listens for visibilitychange events on the page. Each event bumps a total counter and adds a record (visible or hidden, plus a timestamp) to a ring that keeps the last 20. Just before a payload is built it re-checks the current visibility and, if anything changed since the last report, sends the records, the total count and the current/initial visibility.

## Why

Real users switch tabs and minimize windows; the pattern and timing of visibility changes, and a page that is never visible, likely help separate humans from automated or background sessions.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `recordCount` | number | Number of records that follow, 0 to 20. |
| `records` | group, repeated (once per kept visibility change, oldest to newest) | One entry per repetition with the fields below. |
| `records[].visible` | boolean (tag byte) | Tag byte: one value means the page became visible, another means it became hidden (16 / 17 in the main build). |
| `records[].timestamp` | number | Milliseconds since the script started when the change happened (usually fractional). |
| `visibilityChangeCount` | number | Total visibilitychange events seen since start (not capped at 20). |
| `stateNullTag` | null (tag byte) | Tag byte written just before state only when the current visibility is unknown (null): 130 or 131 depending on the build, and state then carries only the initially-visible flag. In the analyzed builds this branch is never taken, since the current visibility is always re-sampled as a boolean. |
| `state` | flags (byte) | Packed flags: currently visible (page visible right now), initially visible (page was visible when the script started). |

Flag names of `state` (card order; the bit of each flag is per build, see the schema's `bits`):

- `currentlyVisible`
- `initiallyVisible`

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "records": [
    {
      "timestamp": 36164.39999997616,
      "visible": false
    },
    {
      "timestamp": 47865.39999997616,
      "visible": true
    }
  ],
  "state": {
    "currentlyVisible": true,
    "initiallyVisible": true
  },
  "visibilityChangeCount": 2
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.VisibilityEventLog` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Repeated fields are `type: "array"` with an `item`; a repeated group's item is a `record` with its own `order` and `fields`.
- Flag bytes are `type: "flags"` with `bits` mapping each flag name above to its bit (0 = least significant) in this build. If the bits cannot all be named the field stays `type: "byte"` and is listed under `diagnostics.flags_fallback`.
