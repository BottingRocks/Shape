# SeleniumIdeArtifactFlags

[Signals index](README.md) · [data-shape example](selenium_ide_artifact_flags.js) · [schema format](schema.md)

## What it collects

Looks for traces of the SideeX / Selenium IDE browser-automation extension: it watches document.body for a 'sideexplayingflag' attribute change, and checks for an extension-served prompt.js script or a global 'error' value matching 'from-page-runscript'. Both flags are sticky once set, and the byte is resent when a flag changes.

## Why

Directly flags test-automation tooling (Selenium IDE / SideeX), a common way to drive scripted browsers.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `automationFlags` | flags (byte) | Two flag bits (0..3): 'SideeX playing' (the sideexplayingflag attribute was added, changed or removed on body) and 'prompt script / runscript' (a chrome-extension:// or moz-extension:// prompt.js script is on the page, or the global error matches from-page-runscript). 0 means neither was seen. |

Flag names of `automationFlags` (card order; the bit of each flag is per build, see the schema's `bits`):

- `sideexPlaying`
- `promptScript`

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "promptScript": false,
  "sideexPlaying": false
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.SeleniumIdeArtifactFlags` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Flag bytes are `type: "flags"` with `bits` mapping each flag name above to its bit (0 = least significant) in this build. If the bits cannot all be named the field stays `type: "byte"` and is listed under `diagnostics.flags_fallback`.
- When the entire resolved message is one packed flag byte, fresh schemas/readable payloads expose its named booleans at the signal root. The schema retains the original byte under `physicalFlags`; the wrapper is a projection name, not a transmitted object.
