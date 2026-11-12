# MemoryBreakdownSummary

[Signals index](README.md) · [data-shape example](memory_breakdown_summary.js) · [schema format](schema.md)

## What it collects

Uses performance.measureUserAgentSpecificMemory() to ask the browser how much memory the page uses. It first reports a status (started, API missing, request pending). When the measurement arrives it sends up to five memory-breakdown entries that belong to this page or its windows/workers (the current page's own entry first), each with its attributions (url, scope, iframe container) and memory types, plus the total bytes and the total number of breakdown entries. Only the newest message counts.

## Why

Whether this API exists (it needs a cross-origin-isolated Chromium page) and what the memory numbers look like likely helps confirm the real browser and environment; instrumented or headless browsers, extra injected frames or workers may show up in the breakdown.

## Fields

This signal sends one of 2 message shapes.

### Message 1: Status only: right at setup (status 0), when the API is missing (status 3), or when the request has been sent and is pending (status 1)

| Field | Type | Meaning |
| --- | --- | --- |
| `status` | number | 0 = set up, collection not run yet; 1 = measurement requested, result pending (or it failed); 3 = measureUserAgentSpecificMemory not available. |
| `entriesPlaceholder` | null (tag byte) | Null when no measurement entry list is available; the concrete marker byte is build-specific. |
| `totalBytesPlaceholder` | null (tag byte) | Null when no measurement total is available; the concrete marker byte is build-specific. |
| `breakdownCountPlaceholder` | null (tag byte) | Null when no breakdown count is available; the concrete marker byte is build-specific. |

### Message 2: Measurement result arrived (status 2)

| Field | Type | Meaning |
| --- | --- | --- |
| `status` | number | Always 2 = measurement done. |
| `entryCount` | number | Number of breakdown entries that follow, 0 to 5. |
| `entry` | group, repeated (once per reported breakdown entry, entryCount times (last entry first)) | One entry per repetition with the fields below. |
| `entry[].attributionCount` | number | Number of attributions in this entry. |
| `entry[].attribution` | group, repeated (once per attribution, attributionCount times) | One entry per repetition with the fields below. |
| `entry[].attribution[].containerMarker` | null (tag byte) | Null marker byte written instead of containerSrc and containerId when the attribution has no container element. |
| `entry[].attribution[].containerSrc` | string | src of the iframe/frame element that holds this context (written only when a container exists; otherwise containerMarker is written instead of containerSrc and containerId). |
| `entry[].attribution[].containerId` | string | id of that container element (written only when a container exists). |
| `entry[].attribution[].scope` | string | Kind of context, e.g. "Window", "DedicatedWorkerGlobalScope", "SharedWorkerGlobalScope". |
| `entry[].attribution[].url` | string | URL of the context with http:// or https:// removed, first 99 characters. |
| `entry[].typeCount` | number | Number of memory type labels for this entry. |
| `entry[].memoryType` | string, repeated (once per type label, typeCount times) | Memory type label, e.g. "JavaScript" or "DOM". |
| `entry[].entryBytes` | number | Bytes of memory attributed to this entry. |
| `totalBytes` | number | Total bytes reported by the measurement (result.bytes). |
| `breakdownCount` | number | Total number of entries in the full breakdown, before filtering and the cap of 5. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "breakdownCountPlaceholder": null,
  "entriesPlaceholder": null,
  "status": 3,
  "totalBytesPlaceholder": null
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.MemoryBreakdownSummary` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- It has 2 message shapes, so the entry carries `messages` (`when`, `mask`, `order`, `fields` each) instead of top-level `order`/`fields`.
- Repeated fields are `type: "array"` with an `item`; a repeated group's item is a `record` with its own `order` and `fields`.
