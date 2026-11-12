# OuterArgsGlobalProcess

[Signals index](README.md) · [data-shape example](outer_args_global_process.js) · [schema format](schema.md)

## What it collects

Serialises the arguments/config passed to the outer loader (outerArgs) with JSON.stringify, keeps the first 256 characters, and adds a flags byte recording whether outerArgs was defined, whether stringify threw, and whether the global object looks like Node.js (global.global === global, global.process exists).

## Why

global.global and process are Node.js markers, so this likely detects scripts run under Node, jsdom, Electron or test harnesses instead of a real browser; the outerArgs JSON likely checks how the loader was invoked.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `outerArgsJson` | string or null (tag byte) | First 256 characters of JSON.stringify(outerArgs) ("null" if outerArgs is null), or a null marker if outerArgs is undefined, cannot be serialised, or stringify threw. |
| `flags` | flags (byte) | Packed flags (positions vary per build): outerArgs is defined; JSON.stringify threw; global.global === global; global.process is defined. Other bits 0. |

Flag names of `flags` (card order; the bit of each flag is per build, see the schema's `bits`):

- `outerArgsDefined`
- `stringifyThrew`
- `globalIsSelf`
- `processDefined`

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "flags": {
    "globalIsSelf": false,
    "outerArgsDefined": false,
    "processDefined": false,
    "stringifyThrew": false
  },
  "outerArgsJson": null
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.OuterArgsGlobalProcess` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Flag bytes are `type: "flags"` with `bits` mapping each flag name above to its bit (0 = least significant) in this build. If the bits cannot all be named the field stays `type: "byte"` and is listed under `diagnostics.flags_fallback`.
