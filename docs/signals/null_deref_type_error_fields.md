# NullDerefTypeErrorFields

[Signals index](README.md) · [data-shape example](null_deref_type_error_fields.js) · [schema format](schema.md)

## What it collects

Deliberately runs null[0]() to throw a TypeError, catches it, and sends six properties of that error object: stacktrace, message, name, number, a cleaned-up stack, and description.

## Why

Error message wording, stack format and the presence of legacy properties (stacktrace, number, description) differ by JavaScript engine and version, so this likely fingerprints the engine and exposes user-agent spoofing or a patched Error object.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `stacktrace` | string or null (tag byte) | The error's stacktrace property (old Opera only), or a null marker if it is missing. |
| `message` | string | The error message text, e.g. "Cannot read properties of null (reading '0')" (Chrome) or "null has no properties" (Firefox). Always present. |
| `name` | string | The error name, normally "TypeError". Always present. |
| `number` | number or null (tag byte) | The error's number property (old IE/JScript error code), or a null marker if it is missing. |
| `stack` | string or null (tag byte) | The first two lines of the stack, with every URL replaced by "URL" and cut to at most 1000 characters, or a null marker if there is no stack. |
| `description` | string or null (tag byte) | The error's description property (old IE/legacy Edge), or a null marker if it is missing. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "description": null,
  "message": "Cannot read properties of null (reading '0')",
  "name": "TypeError",
  "number": null,
  "stack": "TypeError: Cannot read properties of null (reading '0')\n    at URL",
  "stacktrace": null
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.NullDerefTypeErrorFields` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
