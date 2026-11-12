# KeystrokeDynamics

[Signals index](README.md) · [data-shape example](keystroke_dynamics.js) · [schema format](schema.md)

## What it collects

Listens to keydown and keyup on the page and keeps running typing statistics: typing speed, how long keys are held down and how much that varies, the gap between releasing one key and pressing the next, and how often certain special keys are used (Ctrl, C, Meta/Windows, Backspace, Escape, Alt, Tab, ContextMenu, F1–F12). It reports these just before the payload is sent, but only if new keys were released since the last report. The first report is always sent, even with no typing.

## Why

Human typing has natural rhythm and variance, while scripts type at fixed or impossible speeds. Copy/paste and editing key use (Ctrl, C, Meta, Backspace) also hints at pasted credentials (likely).

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `keystrokesPerMinute` | number | Key releases per minute of typing, with pauses of 2 s or more left out. 0 when there have been 5 or fewer key releases. |
| `dwellVariance` | number | Sample variance of key hold time (keydown to keyup of the same key), in ms². 0 with fewer than 2 key releases. |
| `specialKeySlotCount` | number | Number of special-key counters that follow; always 22. |
| `specialKeyCounts` | group, repeated (once per special key, 22 in this order: Ctrl, C, Meta/Win left, Win right, Meta (Firefox), Backspace, Escape, Alt, Tab, ContextMenu, F1–F12) | One entry per repetition with the fields below. |
| `specialKeyCounts[].count` | number | How many times this key was released. |
| `specialKeyTotal` | number | Total key releases of any of the 22 special keys. |
| `meanShortGapMs` | number | Average time from one key release to the next key press, in ms, counting only gaps under 2000 ms. 0 if there are none. |
| `meanDwellMs` | number | Average key hold time (keydown to keyup), in ms. 0 if nothing was typed. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "dwellVariance": 1200.5,
  "keystrokesPerMinute": 0,
  "meanDwellMs": 24.5,
  "meanShortGapMs": 0,
  "specialKeyCounts": [
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0
  ],
  "specialKeyTotal": 0
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.KeystrokeDynamics` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Repeated fields are `type: "array"` with an `item`; a repeated group's item is a `record` with its own `order` and `fields`.
