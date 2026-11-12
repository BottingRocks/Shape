# DeviceOrientation

[Signals index](README.md) · [data-shape example](device_orientation.js) · [schema format](schema.md)

## What it collects

Listens to deviceorientation events for the whole page visit and keeps running averages of the alpha, beta and gamma angles, of their squares, and of the time between events. At the last-chance phase it sends these nine statistics once, if at least one event arrived.

## Why

Real phones and tablets produce a stream of small, noisy orientation readings; emulators, desktops and bots produce none, constant or impossible values, so the averages and variances (mean of squares minus squared mean) help tell real handheld devices from fakes (likely).

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `meanBeta` | number | Average beta angle (front-back tilt, degrees, about -180..180) over all events; float 0 if the browser reported null angles. |
| `meanSquaredIntervalMs` | number | Average of the squared time gap between consecutive events, in ms squared; 0 with fewer than two events. |
| `meanGamma` | number | Average gamma angle (left-right tilt, degrees, about -90..90). |
| `meanBetaSquared` | number | Average of beta squared; with meanBeta it gives the beta variance. |
| `meanAlphaSquared` | number | Average of alpha squared; with meanAlpha it gives the alpha variance. |
| `meanGammaSquared` | number | Average of gamma squared; with meanGamma it gives the gamma variance. |
| `eventCount` | number | Number of deviceorientation events handled (always at least 1 when sent). |
| `meanAlpha` | number | Average alpha angle (compass heading, degrees, 0..360). |
| `meanIntervalMs` | number | Average time between consecutive events, in whole-millisecond clock units; 0 with fewer than two events. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "eventCount": 2,
  "meanAlpha": 0,
  "meanAlphaSquared": 0,
  "meanBeta": 0,
  "meanBetaSquared": 0,
  "meanGamma": 0,
  "meanGammaSquared": 0,
  "meanIntervalMs": 47880,
  "meanSquaredIntervalMs": 2292494400
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.DeviceOrientation` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
