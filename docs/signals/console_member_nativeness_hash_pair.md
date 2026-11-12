# ConsoleMemberNativenessHashPair

[Signals index](README.md) · [data-shape example](console_member_nativeness_hash_pair.js) · [schema format](schema.md)

## What it collects

Takes the own property names of the page's console object (up to 2000) and checks each member's function (value, else setter) with Function.prototype.toString: if the text does not end in '[native code]' the name goes in the non-native list, otherwise in the native-or-other list. The work is done in small time slices; when finished it sorts each list, joins it with NUL characters and sends one hash per list. If console is missing, both hashes are 0.

## Why

Non-native console methods reveal wrappers added by monitoring libraries, extensions, anti-debugging code or automation tools, while the full name set reflects the browser; this likely helps detect tampered or instrumented environments.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `nonNativeNamesHash` | number | 32-bit hash of the sorted, NUL-joined names of console members whose function is not native code (wrapped, patched or added by scripts); 0 if there are none. |
| `nativeNamesHash` | number | 32-bit hash of the sorted, NUL-joined names of all other console members (native functions and non-function properties); 0 if there are none. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "nativeNamesHash": -930873437,
  "nonNativeNamesHash": 0
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.ConsoleMemberNativenessHashPair` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
