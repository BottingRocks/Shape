# CanvasToDataUrl

[Signals index](README.md) · [data-shape example](canvas_to_data_url.js) · [schema format](schema.md)

## What it collects

Creates a fresh, detached canvas at the default 300x150 size, draws the text "Hel$&amp;?6%){mZ+#@" in 18pt Sans at (2, 2), and exports it with toDataURL("image/png"). It hashes the whole data URL in small time-sliced steps and sends the hash, the time since a start stamp, and the first 50 characters of the data URL. If anything throws while drawing or exporting, it sends a fixed placeholder record instead.

## Why

Text rendering (fonts, anti-aliasing, sub-pixel layout) and PNG encoding differ by OS, GPU, installed fonts and browser, so the image hash is a classic canvas fingerprint. Comparing runs likely also reveals canvas noise injection, and the timing may expose slow or emulated environments.

## Fields

This signal sends one of 2 message shapes.

### Message 1: drawing and export worked

| Field | Type | Meaning |
| --- | --- | --- |
| `dataUrlHash` | number | Signed 32-bit hash of the full PNG data URL string. |
| `elapsedMs` | number | Milliseconds from a start stamp taken earlier in the collection step until hashing finished; includes other drawing work, toDataURL and scheduler delay. |
| `dataUrlPrefix` | string | First 50 characters of the data URL (normally "data:image/png;base64," plus the start of the base64 PNG). |

### Message 2: drawing or export threw an error

| Field | Type | Meaning |
| --- | --- | --- |
| `dataUrlHash` | number | Always 92629711, the hash of the placeholder string. |
| `elapsedMs` | null (tag byte) | Always the null marker (no timing). |
| `dataUrlPrefix` | string | Always the placeholder "data:image/png;base64,____________SAFE____________". |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "dataUrlHash": -1952225390,
  "dataUrlPrefix": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAASwA",
  "elapsedMs": 20
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.CanvasToDataUrl` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- It has 2 message shapes, so the entry carries `messages` (`when`, `mask`, `order`, `fields` each) instead of top-level `order`/`fields`.
