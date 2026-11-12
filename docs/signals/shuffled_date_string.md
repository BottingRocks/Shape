# ShuffledDateString

[Signals index](README.md) · [data-shape example](shuffled_date_string.js) · [schema format](schema.md)

## What it collects

Renders one fixed historical instant (1945-08-05T23:16:00Z) with Date.prototype.toString in the browser's local time zone, shuffles the characters of that string with a seeded random generator, and sends the shuffled string together with the generator's seed so the receiver can undo the shuffle. A constant 0 is sent alongside.

## Why

The local rendering of a 1945 date depends on the host's historical time-zone rules and zone naming, so it checks that the claimed time zone and tz database are genuine (likely harder to fake than today's offset); the shuffle just hides the plain string.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `zero` | number | Always 0 (likely a format or version marker). |
| `shuffledDateString` | string | The characters of new Date(-770172240000).toString() in shuffled order, e.g. a scrambled 'Sun Aug 05 1945 ... GMT+0100 (zone name)'. |
| `shuffleSeed` | number | Seed of the random generator used for the shuffle (a fractional number); lets the receiver replay and reverse the shuffle. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "shuffleSeed": 0.04153433085533542,
  "shuffledDateString": " ul1Th041-0M6 GTi765ae09g:iP aAc (ni  0f1m ci0tD yu:0)5gS",
  "zero": 0
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.ShuffledDateString` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
