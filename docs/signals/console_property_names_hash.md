# ConsolePropertyNamesHash

[Signals index](README.md) · [data-shape example](console_property_names_hash.js) · [schema format](schema.md)

## What it collects

Lists every enumerable property name of the page's console object (own and inherited), sorts them, joins them with NUL characters and sends one 32-bit hash. Property values are never read. If console is missing, the hash is 0.

## Why

The set of console methods differs between browsers, versions and environments (and changes when scripts or tools add members), so it likely helps identify the real browser and spot modified environments.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `consoleNamesHash` | number | 32-bit hash of the sorted, NUL-joined enumerable property names of console; 0 if the list is empty. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "consoleNamesHash": -930873437
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.ConsolePropertyNamesHash` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
