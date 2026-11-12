# ScrollOffsets

[Signals index](README.md) · [data-shape example](scroll_offsets.js) · [schema format](schema.md)

## What it collects

Reads the page's vertical scroll position (window.pageYOffset, rounded to a whole number) twice: once when the script sets up, and again when the report is built. Both readings are sent.

## Why

Shows whether and how far the page was scrolled between load and report; a real visitor usually scrolls, while many bots never do (likely used as a simple human-activity hint).

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `initialScrollY` | number | Vertical scroll offset in CSS pixels at setup time, rounded to an integer. 0 on an unscrolled page; negative during overscroll; NaN if pageYOffset is missing. |
| `currentScrollY` | number | Vertical scroll offset in CSS pixels when the report is built, rounded the same way. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

No decoded sample available.

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.ScrollOffsets` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
