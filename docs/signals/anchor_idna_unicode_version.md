# AnchorIdnaUnicodeVersion

[Signals index](README.md) · [data-shape example](anchor_idna_unicode_version.js) · [schema format](schema.md)

## What it collects

Creates 12 detached &lt;a&gt; elements, each with href set to "http://" plus one Unicode character. The characters were chosen so that each one first appeared in a different Unicode version (1.1 up to 13.0). For each anchor it reads back the href the browser's URL parser produced and records whether that href is free of a '%' sign, i.e. whether the engine did not percent-encode the host. It sends the count of tests and the 12 pass/fail results.

## Why

How the URL/IDNA parser handles characters from newer Unicode versions depends on the engine and on its IDNA/Unicode table version. This likely lets the vendor check the claimed browser engine and version, and spot spoofed user agents or unusual URL-parser stacks.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `testCount` | number | Number of characters tested; always 12. |
| `resultBits1` | flags (byte) | Pass/fail bits for 8 of the 12 characters: 1 if the href has no '%' (host IDNA-converted or left raw), 0 if it was percent-encoded or the test threw. Tested characters in order: U+0061 'a' (control), U+AAE1 (Unicode 6.0), U+20BA (6.2), U+061C (6.3), U+0978 (7.0), U+ABA2 (8.0), U+23FE (9.0), U+0864 (10.0), U+0560 (11.0), U+0C77 (12.0), U+32FF (12.1), U+31BC (13.0). |
| `resultBits2` | flags (byte) | The other 4 pass/fail bits in the low 4 bits (same meaning as resultBits1); the upper 4 bits are 0. |

Flag names of `resultBits1` (card order; the bit of each flag is per build, see the schema's `bits`):

- `U+0061`
- `U+AAE1`
- `U+20BA`
- `U+061C`
- `U+0978`
- `U+ABA2`
- `U+23FE`
- `U+0864`
- `U+0560`
- `U+0C77`
- `U+32FF`
- `U+31BC`

Flag names of `resultBits2` (card order; the bit of each flag is per build, see the schema's `bits`):

- `U+0061`
- `U+AAE1`
- `U+20BA`
- `U+061C`
- `U+0978`
- `U+ABA2`
- `U+23FE`
- `U+0864`
- `U+0560`
- `U+0C77`
- `U+32FF`
- `U+31BC`

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

No decoded sample available.

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.AnchorIdnaUnicodeVersion` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Flag bytes are `type: "flags"` with `bits` mapping each flag name above to its bit (0 = least significant) in this build. If the bits cannot all be named the field stays `type: "byte"` and is listed under `diagnostics.flags_fallback`.
- Fresh schemas/readable payloads group `resultBits1` through `resultBits2` as `resultBits[0..1]`. This is a heterogeneous fixed array of named boolean objects; each index retains its original byte's bit map. Physical field names above identify source writes, not transmitted JSON keys.
