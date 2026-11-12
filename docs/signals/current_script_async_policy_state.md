# CurrentScriptAsyncPolicyState

[Signals index](README.md) · [data-shape example](current_script_async_policy_state.js) · [schema format](schema.md)

## What it collects

Records whether the &lt;script&gt; element that loaded the collector (document.currentScript) has its async flag set. The message also has room for a script policy record and its policyHash, but in all analyzed builds that record is always empty, so the signal always reports it as missing. The rest is constant status bytes.

## Why

How the vendor script was loaded (async or not, or with no currentScript at all) likely shows whether it was injected or loaded in an unusual way, for example by automation, a proxy or a script re-hosting the collector.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `scriptAsync` | null (tag byte) | Only present as a null marker when there is no currentScript or its async property is missing; when async is known, nothing is written here and the value goes into scriptFlags instead. |
| `status1` | byte | Constant status tag, always the same value; meaning unknown (likely a field that is always missing in these builds). |
| `status2` | byte | Second constant status tag; meaning unknown (likely another always-missing field). |
| `policyAbsentTag` | null (tag byte) | Tag byte meaning the script policy record is absent (135 in the main build). Always written in the analyzed builds, because the record is never set. |
| `policyHash` | string or null (tag byte) | Hash string of the script policy record (or a policyHash-absent marker), written only when the record is present instead of policyAbsentTag; never reached in the analyzed builds. |
| `scriptFlags` | flags (byte) | Packed byte holding the async flag (1 = async script, 0 = not async; 0 when unknown) plus fixed constant bits whose meaning is unknown. |
| `asyncState` | group | One entry per repetition with the fields below. |
| `asyncState.scriptAsync` | null (tag byte) | Null when currentScript.async is unknown. |
| `asyncState.scriptFlags` | flags (byte) | Packed async flag and fixed bits, as above. |

Flag names of `scriptFlags` (card order; the bit of each flag is per build, see the schema's `bits`):

- `async`

Flag names of `asyncState.scriptFlags` (card order; the bit of each flag is per build, see the schema's `bits`):

- `async`

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

No decoded sample available.

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.CurrentScriptAsyncPolicyState` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Flag bytes are `type: "flags"` with `bits` mapping each flag name above to its bit (0 = least significant) in this build. If the bits cannot all be named the field stays `type: "byte"` and is listed under `diagnostics.flags_fallback`.
