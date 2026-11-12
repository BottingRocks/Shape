# TimestampsEpoch

[Signals index](README.md) · [data-shape example](timestamps_epoch.js) · [schema format](schema.md)

## What it collects

Reads the current wall-clock time once, as Number(new Date()), when the script starts, and sends it.

## Why

Lets the server compare the client clock with its own time and with other timing signals; a skewed or faked clock is likely a sign of emulation, replay or tampering.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `epochMs` | number | Milliseconds since the Unix epoch (about 1.7e12 in 2026). Whatever a patched Date returns if the page overrode it. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "epochMs": 1794273726990
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.TimestampsEpoch` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
