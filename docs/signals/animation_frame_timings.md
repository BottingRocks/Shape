# AnimationFrameTimings

[Signals index](README.md) · [data-shape example](animation_frame_timings.js) · [schema format](schema.md)

## What it collects

Runs a requestAnimationFrame loop during collection and records up to 20 frame timestamps, rounded to whole milliseconds. At the last-chance phase it sends the first timestamp followed by the interval between each pair of consecutive frames. If requestAnimationFrame or cancelAnimationFrame is missing, nothing is sampled.

## Why

Real browsers paint at the display refresh rate (for example about 16.7 ms per frame at 60 Hz) with natural jitter. Headless, background-tab and virtualised environments likely show missing, throttled or unnaturally regular frames, so the timing helps tell real users from bots.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `timeQuantum` | number | Rounding unit for the timestamps in milliseconds; always 1. |
| `sampleCount` | number | Number of frame samples collected, 0 to 20. |
| `frameTime` | number, repeated (once per sample, up to 20) | First entry: the first frame's timestamp in whole milliseconds since page time origin. Each later entry: milliseconds since the previous frame (about 16-17 at 60 Hz). With only one sample it is just that timestamp. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "frameTime": [
    9,
    8,
    9,
    8,
    9,
    8,
    8,
    8,
    9,
    8,
    9,
    8,
    8,
    8,
    8,
    9,
    8,
    8,
    10,
    1056
  ],
  "timeQuantum": 1
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.AnimationFrameTimings` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Repeated fields are `type: "array"` with an `item`.
