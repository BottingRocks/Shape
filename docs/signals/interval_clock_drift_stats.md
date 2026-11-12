# IntervalClockDriftStats

[Signals index](README.md) · [data-shape example](interval_clock_drift_stats.js) · [schema format](schema.md)

## What it collects

Starts a timer that should fire once per second (setInterval at 1000 ms) and measures, with Date, how many milliseconds really passed between ticks. It keeps the last 10 tick gaps, plus the smallest gap, the largest gap and the number of ticks. At startup it sends a placeholder record. Just before the payload is sent, it reports the statistics if new ticks have arrived since the last report.

## Why

Real timer gaps show background-tab throttling, event-loop stalls, system clock jumps, and fake or sped-up timers in automation and emulation setups (likely).

## Fields

This signal sends one of 2 message shapes.

### Message 1: At startup, before any tick (placeholder)

| Field | Type | Meaning |
| --- | --- | --- |
| `meanTickDeltaMs` | number | Always -1 (no data yet). |
| `minTickDeltaMs` | number | Always -1 (no data yet). |
| `maxTickDeltaMs` | number | Always -1 (no data yet). |
| `tickCount` | number | Always 0. |

### Message 2: Before the payload is sent, if at least one tick happened and new ticks arrived since the last report

| Field | Type | Meaning |
| --- | --- | --- |
| `meanTickDeltaMs` | number | Average of the last (up to 10) tick-to-tick gaps, in milliseconds. About 1000 normally; may be fractional. |
| `minTickDeltaMs` | number | Smallest tick-to-tick gap seen, in whole milliseconds. Can be negative if the system clock jumped backwards. |
| `maxTickDeltaMs` | number | Largest tick-to-tick gap seen, in whole milliseconds. Large values mean throttling or stalls. |
| `tickCount` | number | Total number of timer ticks since the start (one per ~second). |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "maxTickDeltaMs": 1838,
  "meanTickDeltaMs": 909.7,
  "minTickDeltaMs": 185,
  "tickCount": 49
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.IntervalClockDriftStats` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- It has 2 message shapes, so the entry carries `messages` (`when`, `mask`, `order`, `fields` each) instead of top-level `order`/`fields`.
