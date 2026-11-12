# MathOwnPropertyNames

[Signals index](README.md) · [data-shape example](math_own_property_names.js) · [schema format](schema.md)

## What it collects

Lists every own property name of the page's built-in Math object (Object.getOwnPropertyNames(Math), e.g. "abs", "PI", "sumPrecise") and sends them three ways: a hash of the whole list, the number of names, and every name in full. It never reads Math values or calls Math functions. It runs once when the collector starts.

## Why

The set and order of Math members differ between browser engines and versions, so this likely helps confirm the claimed browser and spot patched or polyfilled environments (extra or missing members).

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `namesHash` | number | Signed 32-bit hash of all the names joined with a NUL character, in engine order. |
| `nameCount` | number | How many own property names Math has (typically around 40-45 in current browsers). |
| `propertyName` | string, repeated (once per name, nameCount times, in engine order (some builds send them in reverse)) | One Math property name, e.g. "abs" or "E". |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "namesHash": -297135577,
  "propertyName": [
    "abs",
    "acos",
    "acosh",
    "asin",
    "asinh",
    "atan",
    "atanh",
    "atan2",
    "ceil",
    "cbrt",
    "expm1",
    "clz32",
    "cos",
    "cosh",
    "exp",
    "floor",
    "f16round",
    "fround",
    "hypot",
    "imul",
    "log",
    "log1p",
    "log2",
    "log10",
    "max",
    "min",
    "pow",
    "random",
    "round",
    "sign",
    "sin",
    "sinh",
    "sqrt",
    "sumPrecise",
    "tan",
    "tanh",
    "trunc",
    "E",
    "LN10",
    "LN2",
    "LOG10E",
    "LOG2E",
    "PI",
    "SQRT1_2",
    "SQRT2"
  ]
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.MathOwnPropertyNames` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Repeated fields are `type: "array"` with an `item`.
