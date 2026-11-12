# LifecycleEntropySamples

[Signals index](README.md) · [data-shape example](lifecycle_entropy_samples.js) · [schema format](schema.md)

## What it collects

Each time one of the script's lifecycle steps runs (startup, data collection, and the final step before sending), it draws a random 32-bit number and adds it to a list. The random number mixes Math.random, performance.now, the current date and crypto.getRandomValues. At the final step it sends how many numbers it has, then all of them. The numbers carry no device data; what matters is mostly how many lifecycle steps actually ran. A background timer that re-arms itself every 2–4 s also draws numbers, but it does not add them to the list.

## Why

Likely a liveness and ordering check: the count shows whether every lifecycle step ran normally, and the random values tie the report to one real page session, so replayed or partly executed scripts stand out.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `sampleCount` | number | How many random words follow. Normally 3 (startup, collection, final). Less if collection failed or had not run yet, 4 if collection ran twice after an error. |
| `samples` | group, repeated (once per lifecycle step that ran, in the order they ran) | One entry per repetition with the fields below. |
| `samples[].entropyWord` | number | A random unsigned 32-bit number (0 to 4294967295). The last one is always from the final step. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

No decoded sample available.

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.LifecycleEntropySamples` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Repeated fields are `type: "array"` with an `item`; a repeated group's item is a `record` with its own `order` and `fields`.
