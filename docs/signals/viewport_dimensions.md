# ViewportDimensions

[Signals index](README.md) · [data-shape example](viewport_dimensions.js) · [schema format](schema.md)

## What it collects

At the last moment before the payload is built, reads eleven window, screen and visual-viewport sizes and hands them as one plain object to the shared encoder. If any read throws (e.g. no visualViewport), nothing is sent and an error is reported instead.

## Why

Comparing these sizes exposes window chrome (outer minus inner), OS taskbars (screen minus avail) and zoom state, and impossible combinations likely reveal headless browsers or device emulation.

## Fields

Sent by name through the generic encoder: the value is handed to the shared encoder as one named object (or bare value), not written field by field with a positional index.

| Field | Type | Meaning |
| --- | --- | --- |
| `innerWidth` | number | window.innerWidth: layout viewport width in CSS pixels. |
| `innerHeight` | number | window.innerHeight: layout viewport height in CSS pixels. |
| `outerHeight` | number | window.outerHeight: full browser window height in CSS pixels, including toolbars. |
| `outerWidth` | number | window.outerWidth: full browser window width in CSS pixels. |
| `screenWidth` | number | screen.width: screen width in CSS pixels. |
| `screenHeight` | number | screen.height: screen height in CSS pixels. |
| `screenAvailWidth` | number | screen.availWidth: screen width minus OS bars such as a side dock. |
| `screenAvailHeight` | number | screen.availHeight: screen height minus the OS taskbar/dock/menu bar. |
| `visualViewportWidth` | number | visualViewport.width: visible area width after pinch-zoom (may be fractional). |
| `visualViewportHeight` | number | visualViewport.height: visible area height after pinch-zoom (may be fractional). |
| `visualViewportScale` | number | visualViewport.scale: pinch-zoom factor, 1 when not zoomed. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

No decoded sample available.

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.ViewportDimensions` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Sent **by name** through the generic encoder: `type: "name"` with the build's routing `name` (no `positional`), signal `mask: null`; each value is typed `"value"` and keeps its wire `key`.
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
