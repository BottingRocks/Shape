# KeyboardEventSamples

[Signals index](README.md) · [data-shape example](keyboard_event_samples.js) · [schema format](schema.md)

## What it collects

Listens to every keydown, keyup and keypress on the page and records each one: when it happened, which modifier keys were held, whether the event was real (isTrusted, a UIEvent), the id and name of the element that had focus, and a key code. Typed characters are hidden: any key that produces a character is recorded as code 1, so only special keys (Enter, Tab, arrows, F-keys, Backspace…) and Ctrl/Meta shortcuts keep their real code. It keeps the last 40 events and sends them just before the payload is sent.

## Why

Keyboard timing, the trusted flag and the target fields show whether a human typed into the form or a script filled it with synthetic events (likely).

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `eventCount` | number | Number of keyboard event records that follow (0 to 40). |
| `events` | group, repeated (once per keyboard event, newest first, up to 40) | One entry per repetition with the fields below. |
| `events[].targetName` | string | The name attribute of the focused element. Or an 'unchanged' marker if it is the same as in the previous (older) record, or a null marker if the element has no name or there is no target. |
| `events[].timeMs` | number | For the oldest record, milliseconds since the script started. For every other record, milliseconds since the previous (older) event. Usually fractional. |
| `events[].targetId` | string | The id of the focused element. Or an 'unchanged' marker if it is the same as in the previous (older) record, or a null marker if there is no id or no target. |
| `events[].flags` | flags (byte) | Packed flags: isKeydown, isKeyup (both 0 means keypress), Ctrl held, Alt held, Meta held, event is a UIEvent, event isTrusted. |
| `events[].ownFlag` | flags (byte) | One event boolean written as its own 0/1 byte instead of being packed into flags. Which of the eight booleans (isKeydown, isKeyup, Ctrl, Alt, Meta, Shift held, UIEvent, isTrusted) it is varies per build: Shift in the main build, others in other builds; flags then packs the remaining seven. |
| `events[].keyCode` | number | Hidden key code: 1 for any key that types a character (or an unknown key). Otherwise the real keyCode (e.g. 13 Enter, 9 Tab), or a 32-bit hash of the key name for keypress events with Ctrl/Meta. |

Flag names of `events[].flags` (card order; the bit of each flag is per build, see the schema's `bits`):

- `isKeydown`
- `isKeyup`
- `ctrlKey`
- `altKey`
- `metaKey`
- `shiftKey`
- `instanceOfUIEvent`
- `isTrusted`

Flag names of `events[].ownFlag` (card order; the bit of each flag is per build, see the schema's `bits`):

- `isKeydown`
- `isKeyup`
- `ctrlKey`
- `altKey`
- `metaKey`
- `shiftKey`
- `instanceOfUIEvent`
- `isTrusted`

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "events": [
    {
      "flags": {
        "altKey": false,
        "instanceOfUIEvent": true,
        "isKeydown": true,
        "isKeyup": false,
        "isTrusted": false,
        "metaKey": false,
        "shiftKey": false
      },
      "keyCode": 1,
      "ownFlag": {
        "ctrlKey": false
      },
      "targetId": "username",
      "targetName": "username",
      "timeMs": 693.2999999523163
    },
    {
      "flags": {
        "altKey": false,
        "instanceOfUIEvent": true,
        "isKeydown": false,
        "isKeyup": true,
        "isTrusted": false,
        "metaKey": false,
        "shiftKey": false
      },
      "keyCode": 1,
      "ownFlag": {
        "ctrlKey": false
      },
      "targetId": 132,
      "targetName": 130,
      "timeMs": 0
    },
    {
      "flags": {
        "altKey": false,
        "instanceOfUIEvent": true,
        "isKeydown": true,
        "isKeyup": false,
        "isTrusted": true,
        "metaKey": false,
        "shiftKey": false
      },
      "keyCode": 1,
      "ownFlag": {
        "ctrlKey": false
      },
      "targetId": 132,
      "targetName": 130,
      "timeMs": 8662.100000023842
    },
    {
      "flags": {
        "altKey": false,
        "instanceOfUIEvent": true,
        "isKeydown": false,
        "isKeyup": false,
        "isTrusted": true,
        "metaKey": false,
        "shiftKey": false
      },
      "keyCode": 1,
      "ownFlag": {
        "ctrlKey": false
      },
      "targetId": 132,
      "targetName": 130,
      "timeMs": 0.30000007152557373
    },
    {
      "flags": {
        "altKey": false,
        "instanceOfUIEvent": true,
        "isKeydown": false,
        "isKeyup": true,
        "isTrusted": true,
        "metaKey": false,
        "shiftKey": false
      },
      "keyCode": 1,
      "ownFlag": {
        "ctrlKey": false
      },
      "targetId": 132,
      "targetName": 130,
      "timeMs": 48.699999928474426
    }
  ]
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.KeyboardEventSamples` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Repeated fields are `type: "array"` with an `item`; a repeated group's item is a `record` with its own `order` and `fields`.
- Flag bytes are `type: "flags"` with `bits` mapping each flag name above to its bit (0 = least significant) in this build. If the bits cannot all be named the field stays `type: "byte"` and is listed under `diagnostics.flags_fallback`.
