# SelectMetaInjectionFlags

[Signals index](README.md) · [data-shape example](select_meta_injection_flags.js) · [schema format](schema.md)

## What it collects

Watches the page for four fixed traces of one particular injected script or extension that customises &lt;select&gt; dropdowns: a _selectMeta global with a known set of keys, an animationstart listener whose code mentions _selectOpen/animationName/wasOpened, a hidden measurement DIV with a known inline style added under &lt;html&gt; or &lt;body&gt;, and a known STYLE block added there. Each flag starts false and turns true (and stays true) when its trace appears; every new match sends the object with all four flags.

## Why

Likely detects a specific browser extension or injected tool (the product is not identified), which can mark automation setups or tampered browsers.

## Fields

Sent by name through the generic encoder: the value is handed to the shared encoder as one named object (or bare value), not written field by field with a positional index.

| Field | Type | Meaning |
| --- | --- | --- |
| `selectMeta` | byte | Boolean: true once page code assigns a window._selectMeta object whose sorted key list matches a known hash. |
| `animationStart` | byte | Boolean: true once addEventListener('animationstart', fn) is called with a function whose source contains _selectOpen, animationName and wasOpened. |
| `measurementDiv` | byte | Boolean: true once a DIV whose inline style matches known hashes is added directly under &lt;html&gt; or &lt;body&gt;. |
| `animationStyle` | byte | Boolean: true once a STYLE element whose text matches a known hash is added directly under &lt;html&gt; or &lt;body&gt;. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

No decoded sample available.

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.SelectMetaInjectionFlags` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Sent **by name** through the generic encoder: `type: "name"` with the build's routing `name` (no `positional`), signal `mask: null`; each value is typed `"value"` and keeps its wire `key`.
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
