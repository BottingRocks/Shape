# HistoricDateToString

[Signals index](README.md) · [data-shape example](historic_date_to_string.js) · [schema format](schema.md)

## What it collects

Formats one fixed moment in the past, 1945-08-05 23:16 UTC, with new Date(...).toString() and sends the resulting text. Because toString uses the device's local time zone, the text shows the local date and time for that moment, the historical GMT offset, and the browser's name for the time zone. Runs once at startup.

## Why

Likely a time-zone fingerprint: zones that share an offset today often had different rules in 1945, and the zone display name depends on engine and locale, so it can likely expose time-zone spoofing that only patches the current offset.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `historicDateString` | string | Date.prototype.toString of 1945-08-05T23:16:00Z in local time, e.g. 'Mon Aug 06 1945 08:16:00 GMT+0900 (Japan Standard Time)'. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "historicDateString": "Sun Aug 05 1945 16:16:00 GMT-0700 (Pacific Daylight Time)"
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.HistoricDateToString` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
