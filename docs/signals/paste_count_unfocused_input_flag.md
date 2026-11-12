# PasteCountUnfocusedInputFlag

[Signals index](README.md) · [data-shape example](paste_count_unfocused_input_flag.js) · [schema format](schema.md)

## What it collects

Listens to every input event on &lt;input&gt; elements for the page's lifetime. It counts pastes into inputs and remembers whether any other input event happened on an input that did not have focus. The values are sent at the end of collection.

## Why

Pasting into fields (e.g. credentials) is a behaviour signal, and text arriving in an input that never had focus is a strong likely sign of scripted or automated form filling.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `pastedInputCount` | number | Number of paste insertions into &lt;input&gt; elements, 0 or more. |
| `unfocusedInputSeen` | boolean (tag byte) | One of two fixed tag values: one means a non-paste input event hit an &lt;input&gt; that was not the focused element at least once, the other means it never happened (the tag values vary per build, e.g. 17 = yes, 77 = no). |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "pastedInputCount": 1,
  "unfocusedInputSeen": false
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.PasteCountUnfocusedInputFlag` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
