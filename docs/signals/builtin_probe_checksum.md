# BuiltinProbeChecksum

[Signals index](README.md) · [data-shape example](builtin_probe_checksum.js) · [schema format](schema.md)

## What it collects

Keeps a table of 58 built-in functions and constructors (isNaN, parseInt, encodeURI, JSON.parse, typed arrays, Map, Blob, DOMParser, Image, XMLHttpRequest and so on). Using a seeded random generator, it calls a random selection of them twice, 8 to 14 calls at init and again at collection. For each call that works it adds the character code of the first character of the result (as a string) to a running sum. It sends the generator's seed and the checksum.

## Why

With the seed, the server can likely replay which built-ins were picked and compare the checksum with what the claimed browser should produce. Missing, throwing or non-native (patched) built-ins change the sum, which exposes spoofed or emulated environments.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `probeSeed` | number | Seed of the random generator that chose the probes; a fraction from 0 up to (not including) 1. |
| `checksum` | number | 2 plus the sum of first-character codes of all successful probe results, modulo 65353; range 2 to 65354. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "checksum": 1846,
  "probeSeed": 0.9550309971885809
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.BuiltinProbeChecksum` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
