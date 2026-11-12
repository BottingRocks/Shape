# LocalStoragePersistentId

[Signals index](README.md) · [data-shape example](local_storage_persistent_id.js) · [schema format](schema.md)

## What it collects

Checks whether the browser has localStorage and whether the script managed to save its persistent visitor ID there. Earlier, the script saves the current transaction token under a fixed key, unless a value is already stored. If saving worked, this signal reads back what is stored under that key. On a first visit that is the current token; on a return visit it is the ID from an earlier session. Some builds also carry over an older legacy ID.

## Why

A persistent ID in localStorage recognizes the same browser across visits even after cookies are cleared. Missing or failing storage points to privacy modes or locked-down automation browsers (likely).

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `storedId` | string or null (tag byte) | The value stored under the persistent-ID key, or a null marker if storage is missing, saving failed, or the read returned nothing. |
| `storageStatus` | number | 1 = no localStorage property; 2 = localStorage exists but saving failed (access threw, quota exceeded or disabled); 3 = the ID is saved and storedId holds it. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "storageStatus": 3,
  "storedId": "A7dnGmKdAQAAhZWcRWlWhl2h6vRTaGVX6Z9OSxaXqGmLDUfl8GFN27kLCff6AUm92wCucphhwH9eCOfvosJeCA=="
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.LocalStoragePersistentId` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
