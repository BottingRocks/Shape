# LifecyclePhaseTimings

[Signals index](README.md) · [data-shape example](lifecycle_phase_timings.js) · [schema format](schema.md)

## What it collects

Records the time (from Date) when the script is set up, then how many milliseconds later each of its three lifecycle steps ran: startup (init), data collection, and the final step before sending (last chance). It sends one report at each of the three steps, so each report shows which steps have run so far and how far apart they were.

## Why

How long the script took to reach each step, and the absolute clock time, show unusually fast or slow execution, cut-short page visits and clock tampering, which are typical of headless or scripted sessions (likely).

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `collectionElapsedMs` | number | Milliseconds from setup to the collection step, or -3600001 if collection has not run yet. |
| `startEpochMs` | number | Date.now()-style timestamp of setup, in milliseconds since 1970 (about 1.7e12). |
| `lastChanceElapsedMs` | number | Milliseconds from setup to the final step before sending, or -3600001 if it has not run yet. |
| `initElapsedMs` | number | Milliseconds from setup to the startup (init) step, or -3600001 if it has not run yet. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "collectionElapsedMs": 192,
  "initElapsedMs": 0,
  "lastChanceElapsedMs": 49815,
  "startEpochMs": 1794273726998
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.LifecyclePhaseTimings` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
