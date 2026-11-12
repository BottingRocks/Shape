# BraveDuckDuckGoNavigatorFlags

[Signals index](README.md) · [data-shape example](brave_duckduckgo_navigator_flags.js) · [schema format](schema.md)

## What it collects

At init, reads navigator.brave and navigator.duckduckgo, converts each value to an integer, and packs the results into one byte of two flags. The conversion is not a real presence test: undefined, null, false and objects (including the real Brave/DuckDuckGo objects) all give 0. Only primitive values such as true or 1 give 1.

## Why

Likely meant to detect privacy-focused browsers (Brave, DuckDuckGo) that block or randomise fingerprinting, so their other signals can be judged differently. As written, it mostly catches pages, extensions or spoofing tools that set these properties to true or 1.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `browserFlags` | flags (byte) | Two packed flags: navigator.brave converted to an integer, and navigator.duckduckgo converted the same way (each 1 only if the value is true or an odd number, else 0). Other bits are normally 0. |

Flag names of `browserFlags` (card order; the bit of each flag is per build, see the schema's `bits`):

- `navigator.brave`
- `navigator.duckduckgo`

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

No decoded sample available.

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.BraveDuckDuckGoNavigatorFlags` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Flag bytes are `type: "flags"` with `bits` mapping each flag name above to its bit (0 = least significant) in this build. If the bits cannot all be named the field stays `type: "byte"` and is listed under `diagnostics.flags_fallback`.
- When the entire resolved message is one packed flag byte, fresh schemas/readable payloads expose its named booleans at the signal root. The schema retains the original byte under `physicalFlags`; the wrapper is a projection name, not a transmitted object.
