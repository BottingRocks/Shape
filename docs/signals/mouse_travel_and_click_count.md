# MouseTravelAndClickCount

[Signals index](README.md) · [data-shape example](mouse_travel_and_click_count.js) · [schema format](schema.md)

## What it collects

Watches the mouse for the whole time the page is open. It counts clicks that land outside interactive elements (not on or inside an input, select, option, textarea, button or link), and adds up how far the mouse pointer travelled across mousemove events, in CSS pixels. At startup it sends a placeholder of two zeros; at the end of the page's life it sends the real totals.

## Why

Real users move the mouse a fair distance and click in empty areas now and then; scripted sessions often show no movement, no clicks, or implausible totals. This is likely used as a simple human-versus-bot behaviour check.

## Fields

This signal sends one of 2 message shapes.

### Message 1: At startup (placeholder, before any activity is measured)

| Field | Type | Meaning |
| --- | --- | --- |
| `backgroundClickCount` | number | Always 0. |
| `mouseTravelDistance` | number | Always 0. |

### Message 2: At the end of the page's life (last chance), with the real totals

| Field | Type | Meaning |
| --- | --- | --- |
| `backgroundClickCount` | number | Number of clicks on the page that did not hit or sit inside an input, select, option, textarea, button or link. 0 or more, never reset. |
| `mouseTravelDistance` | number | Total mouse travel in CSS pixels, summed straight-line distance between consecutive mousemove positions, rounded to 2 decimals. The first step is measured from (0,0). 0 if the mouse never moved. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

No decoded sample available.

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.MouseTravelAndClickCount` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- It has 2 message shapes, so the entry carries `messages` (`when`, `mask`, `order`, `fields` each) instead of top-level `order`/`fields`.
