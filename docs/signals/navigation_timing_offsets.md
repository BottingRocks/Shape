# NavigationTimingOffsets

[Signals index](README.md) · [data-shape example](navigation_timing_offsets.js) · [schema format](schema.md)

## What it collects

Reads the page-load timeline from performance.timing at the end of the page's life. It sends 20 milestones (DNS lookup, connect, request, response, DOM loading, load event and so on) as milliseconds after navigationStart, plus navigationStart itself as an absolute time. If performance.timing does not exist, nothing is sent.

## Why

A real page load has a plausible, consistent timeline; headless or scripted loads, replayed pages and spoofed timing objects likely show missing, zero or impossible values. The absolute navigationStart also gives the page-load time on the client clock.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `domLoading` | number | Milliseconds from navigationStart to domLoading; 0 if not reached or missing. |
| `connectStart` | number | Milliseconds from navigationStart to connectStart; 0 if not reached or missing. |
| `domainLookupEnd` | number | Milliseconds from navigationStart to domainLookupEnd; 0 if not reached or missing. |
| `unloadEventEnd` | number | Milliseconds from navigationStart to the previous page's unloadEventEnd; 0 if there was none. |
| `unloadEventStart` | number | Milliseconds from navigationStart to the previous page's unloadEventStart; 0 if there was none. |
| `navigationStart` | number | navigationStart itself as an absolute time, milliseconds since 1970 (around 1.7e12). No zero fallback. |
| `connectEnd` | number | Milliseconds from navigationStart to connectEnd; 0 if not reached or missing. |
| `fetchStart` | number | Milliseconds from navigationStart to fetchStart; 0 if not reached or missing. |
| `loadEventStart` | number | Milliseconds from navigationStart to loadEventStart; 0 if the load event has not started. |
| `loadEventEnd` | number | Milliseconds from navigationStart to loadEventEnd; 0 if the load event has not finished. |
| `domContentLoadedEventEnd` | number | Milliseconds from navigationStart to domContentLoadedEventEnd; 0 if not reached or missing. |
| `redirectEnd` | number | Milliseconds from navigationStart to redirectEnd; 0 if there was no redirect. |
| `responseStart` | number | Milliseconds from navigationStart to responseStart; 0 if not reached or missing. |
| `requestStart` | number | Milliseconds from navigationStart to requestStart; 0 if not reached or missing. |
| `domComplete` | number | Milliseconds from navigationStart to domComplete; 0 if not reached or missing. |
| `domContentLoadedEventStart` | number | Milliseconds from navigationStart to domContentLoadedEventStart; 0 if not reached or missing. |
| `domainLookupStart` | number | Milliseconds from navigationStart to domainLookupStart; 0 if not reached or missing. |
| `domInteractive` | number | Milliseconds from navigationStart to domInteractive; 0 if not reached or missing. |
| `redirectStart` | number | Milliseconds from navigationStart to redirectStart; 0 if there was no redirect. |
| `secureConnectionStart` | number | Milliseconds from navigationStart to secureConnectionStart; 0 on plain HTTP or a reused connection. |
| `responseEnd` | number | Milliseconds from navigationStart to responseEnd; 0 if not reached or missing. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

No decoded sample available.

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.NavigationTimingOffsets` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
