# ConsoleStackAccessFlags

[Signals index](README.md) · [data-shape example](console_stack_access_flags.js) · [schema format](schema.md)

## What it collects

Logs a probe Error whose 'stack' property is a trap (a getter) and records whether anything read it. One probe goes through the page's console.log; a second goes through the console of a directly created hidden iframe. It also checks for Sentry's saved original console.log and calls it. It sends a snapshot of the flags at the start and after the Sentry check, plus a new message every time a trap fires, so one run can send several messages, possibly late. If the page has no console, it sends nothing.

## Why

Error.stack is normally only read inside console.log when DevTools is open or a hook/automation tool formats console arguments, so this likely detects open DevTools or console instrumentation; the page-vs-iframe pair helps tell a page-level console hook from DevTools.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `consoleStackFlags` | flags (byte) | Three flags, other bits 0: pageConsoleStackRead (logging through the page's console read the probe's stack), sentryOriginalLogCalled (console.log.__sentry_original__ exists, is a function, and the call returned normally), iframeConsoleStackRead (logging through the new iframe's console read the probe's stack). Each bit is 1 if true, 0 if false. |

Flag names of `consoleStackFlags` (card order; the bit of each flag is per build, see the schema's `bits`):

- `pageConsoleStackRead`
- `sentryOriginalLogCalled`
- `iframeConsoleStackRead`

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "iframeConsoleStackRead": false,
  "pageConsoleStackRead": false,
  "sentryOriginalLogCalled": false
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.ConsoleStackAccessFlags` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Flag bytes are `type: "flags"` with `bits` mapping each flag name above to its bit (0 = least significant) in this build. If the bits cannot all be named the field stays `type: "byte"` and is listed under `diagnostics.flags_fallback`.
- When the entire resolved message is one packed flag byte, fresh schemas/readable payloads expose its named booleans at the signal root. The schema retains the original byte under `physicalFlags`; the wrapper is a projection name, not a transmitted object.
