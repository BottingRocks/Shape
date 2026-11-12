# FunctionToStringComparison

[Signals index](README.md) · [data-shape example](function_to_string_comparison.js) · [schema format](schema.md)

## What it collects

Creates an empty function and gives it its own 'toString' getter that counts how often it is read and returns the real Function.prototype.toString. It then compares the function turned into a string implicitly ("" + f) with Function.prototype.toString.call(f), and reports how many times the getter was read, whether the two strings matched, and whether the comparison threw.

## Why

Likely detects patched or hooked Function.prototype.toString / valueOf (a common trick of automation tools and anti-detection extensions that try to make their patches look native); a normal browser gives count 1, equal, no error.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `toStringGetterCalls` | number | How many times the function's own toString getter was read during the comparison. 1 on an unmodified browser; 0 means valueOf or Symbol.toPrimitive was patched, more than 1 means a hooked toString reads it again. |
| `probeFlags` | flags (byte) | Two flags: threw (the comparison raised an exception) and equal (both string forms were identical). Other bits are 0. Normal browser: equal set, threw clear. |

Flag names of `probeFlags` (card order; the bit of each flag is per build, see the schema's `bits`):

- `threw`
- `equal`

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "probeFlags": {
    "equal": true,
    "threw": false
  },
  "toStringGetterCalls": 1
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.FunctionToStringComparison` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Flag bytes are `type: "flags"` with `bits` mapping each flag name above to its bit (0 = least significant) in this build. If the bits cannot all be named the field stays `type: "byte"` and is listed under `diagnostics.flags_fallback`.
