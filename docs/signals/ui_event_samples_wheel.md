# UiEventSamplesWheel

[Signals index](README.md) · [data-shape example](ui_event_samples_wheel.js) · [schema format](schema.md)

## What it collects

Listens for wheel events on the document and keeps two counters: all wheel events, and wheel 'bursts' (an event counted only if more than 500 ms passed since the last counted one). Sends both counters after every wheel event. The event's details (deltas, position, isTrusted) are not read.

## Why

Real users produce wheel activity in bursts while scrolling; likely used to tell human scrolling from no scrolling or scripted scrolling.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `wheelSampleCount` | number | Number of wheel bursts so far (events more than 500 ms after the previous counted one; the first event always counts). Integer &gt;= 1. |
| `wheelEventCount` | number | Total number of wheel events so far, including this one. Integer &gt;= 1. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

No decoded sample available.

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.UiEventSamplesWheel` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
