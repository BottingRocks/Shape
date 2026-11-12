# PersistentStorageState

[Signals index](README.md) · [data-shape example](persistent_storage_state.js) · [schema format](schema.md)

## What it collects

Watches the vendor's persistent-ID entry in localStorage. It reports once at startup whether storage works and whether an ID survived from a previous visit, and again each time another tab clears localStorage or removes the ID, including the removed value.

## Why

Lets the vendor likely detect first visits, storage being blocked, and users or bots that wipe the tracking ID to look like a new visitor.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `removedValue` | string or null (tag byte) | The persistent-ID value that was just removed or cleared, or a null marker if nothing was removed (always null in the startup message). |
| `flags` | flags (byte) | Packed flags (positions vary per build): localStorage access failed or threw; no persistent ID was inherited from an earlier visit; a removal or clear of the ID was seen. Other bits 0. |

Flag names of `flags` (card order; the bit of each flag is per build, see the schema's `bits`):

- `storageAccessFailed`
- `noInheritedId`
- `idRemovalSeen`

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

No decoded sample available.

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.PersistentStorageState` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Flag bytes are `type: "flags"` with `bits` mapping each flag name above to its bit (0 = least significant) in this build. If the bits cannot all be named the field stays `type: "byte"` and is listed under `diagnostics.flags_fallback`.
