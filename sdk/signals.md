# From capture headers to editable signals

Direct decode, explain and encode operations take one immutable loaded artifact.
They turn a capture into named fields, inspect one signal and encode edited fields:

```text
{siteId, headers} → decode → {siteId, positionalSignals, metadata, …}
                             ↓ inspect / edit named fields
{siteId, headers} ← encode ← full decoded payload
```

Start with the [SDK setup and artifact workflow](README.md), the root
[Inputs](../README.md#inputs) section for the seed, session and header files,
and your [Node](node/README.md), [Python](python/README.md), [Go](go/README.md),
[Rust](rust/README.md), or [C](c/README.md) installation guide.
A saved **decode artifact** contains the build's layout, signal recipes and
session defaults. Load it once; subsequent calls need no source analysis.
The capture and artifact must describe the same build and session, with matching
`siteId`. A compact schema is descriptive; it cannot replace the artifact.

## Input and actual decoded output

Capture keys are header **suffixes**, including numeric chunks such as `a0`.
The artifact supplies the shared header prefix. Values are strings, including
empty strings. Here is an actual synthetic capture generated with the release
native library, with no ciphertext abbreviation:

```json
{
  "siteId": "example",
  "headers": {
    "a": "AAAAEwAAABdUs4B1XKMfGA9pxzxsyEPOya7TLSkT140twoI4wQgT",
    "b": "-2m4fh7",
    "c": "",
    "d": "",
    "f": "ab",
    "z": ""
  }
}
```

Its decoded payload excerpt is:

```json
{
  "siteId": "example",
  "errors": [{"kind": "null"}],
  "positionalSignals": {
    "ConstantEmptyString": {"constant": ""},
    "DeviceMemory": {"deviceMemory": 32},
    "StorageAccessApiPresence": {
      "hasStorageAccess": true,
      "requestStorageAccess": false
    }
  },
  "signals": [{"kind": "bool", "value": true}],
  "keyedSignals": [[17, null]],
  "seeds": [19, 23],
  "keysets": [],
  "metadata": {
    "decoded": 3,
    "missing": 0,
    "exactReencode": false,
    "floatByteOrder": "little",
    "signals": {}
  }
}
```

**Excerpt only:** `metadata.originalReport` is omitted here for readability.
The actual default result retains it, including raw positional bytes and header
context. Keep the complete result for editing and encoding.
`positionalSignals` is an object keyed by signal name; `deviceMemory` is a JSON
number (32 here), the two storage API presence fields are JSON booleans, and
`constant` is the empty string. The top-level `signals` and `keyedSignals` are
separate report channels, retained from this capture. `metadata.decoded: 3` counts
decoded positional signals;
`missing: 0` counts absent positional slots. These counts describe this small
supported fixture, not coverage of an arbitrary browser capture.

Field names and shapes come from the artifact's projection. They can include
arrays, nested objects, and null markers in other captures. Inspect actual keys
or the compact schema rather than assuming every signal is a scalar. See the
[readable projection contract](../docs/signals/schema.md#readable-projection-version-2)
for those shapes.

## Reproduce, inspect and edit

Run from the repository root after [building the SDKs](README.md#build).
The committed [sample32 pair](c/tests/fixtures/README.md) contains a matching
executable artifact and capture: memory 32, storage flags and an empty string.
No fixture generation or raw seed recovery is needed. The ciphertext shown here
is from that committed snapshot. The separate source fixture has memory 9.

Create only the output directory deliberately:

```sh
mkdir -p reports/sdk-signals
```

This complete Python program loads the artifact once, edits named fields and
saves caller-chosen output files. It uses the artifact's session throughout:

```python
import copy
import json
from pathlib import Path
import shape

out = Path("reports/sdk-signals")
fixtures = Path("sdk/c/tests/fixtures")
capture = json.loads((fixtures / "capture.json").read_text())
with shape.artifact.load((fixtures / "artifact.json").read_bytes()) as loaded:
    decoded = shape.decode(loaded, capture)  # verify=False, metadata=False
    (out / "decoded-32.json").write_text(json.dumps(decoded))
    print(decoded["positionalSignals"])
    assert shape.encode(loaded, decoded) == capture

    detail = shape.explain(loaded, decoded, "DeviceMemory", verify=True)
    print(detail["fields"], detail["status"])

    edited = copy.deepcopy(decoded)  # retain the complete originalReport
    edited["positionalSignals"]["DeviceMemory"]["deviceMemory"] = 8
    encoded = shape.encode(loaded, edited)
    again = shape.decode(loaded, encoded, verify=True)
    assert again["positionalSignals"] == edited["positionalSignals"]
    for key in ("errors", "signals", "keyedSignals", "seeds", "keysets"):
        assert again[key] == decoded[key]
    (out / "edited-capture.json").write_text(json.dumps(encoded))
    print(encoded)

    fresh = shape.encode_signals(loaded, edited["positionalSignals"])
    fresh_decoded = shape.decode(loaded, fresh, verify=True)
    assert fresh_decoded["positionalSignals"] == edited["positionalSignals"]
```

Editing the named field while passing the full decoded document preserves the
other channels and the retained encoding context. There is no need to change
`metadata.originalReport`. An example edit result is:

```json
{
  "siteId": "example",
  "headers": {
    "a": "AAAAEwAAABdUs4B1XKMfGA9pxz9syEPO4ebWKSoS0CuNUZOp0Ag",
    "b": "-ehr16t",
    "c": "",
    "d": "",
    "f": "ab",
    "z": ""
  }
}
```

Decoding it returned `DeviceMemory.deviceMemory: 8`, the same two storage
booleans, `metadata.decoded: 3`, `missing: 0`, and `exactReencode: true`.
The memory bytes changed from `[1,2,3,4,130,72]` to
`[1,2,3,4,170]`; their four-byte prefix stayed intact. Seeds remained
`[19,23]`; errors, other channels, keysets, header context and storage bytes were
retained. Encoding the untouched full payload reproduced the entire original
capture. These are checks on this supported pair, not a promise that arbitrary
edits to any artifact can be emitted.

`encode` returns **`{siteId, headers}`**, ready for the next decode or your capture
adapter. It does not return a decoded payload or a success-only flag. Supported
edits must decode back to exactly the requested fields; otherwise encoding fails.
Pass exactly one of `decoded` or `signals` to JSON/binding encode methods.

Signals-only encoding takes the bare named map, uses fresh per-signal prefixes,
and uses layout defaults for the other channels. The fresh memory prefix can
differ from `[1,2,3,4]`, while its named value remains 8. Fresh encoding preserves
requested field values; use the full decoded document when retaining capture
context matters.

## Lazy explain, details and verification

The language-neutral explain request contains the complete `decoded` result:

```text
{"decoded": decoded, "signal": "DeviceMemory", "verify": true}
```

The actual result excerpt from the original memory-32 payload was:

```json
{
  "fields": {"deviceMemory": 32},
  "status": "decoded",
  "metadata": {
    "canonicalName": "DeviceMemory",
    "schemaKey": "DeviceMemory",
    "address": "direct:1",
    "descriptor": 1,
    "position": 1,
    "capturedSeed": 16909060,
    "bytes": [1, 2, 3, 4, 130, 72],
    "routing": {"positional": 1, "type": "positional"},
    "streamSalts": [17]
  }
}
```

This excerpt omits `projection`, `projectionCandidates`, `selection` and
`sourcePlan`. Those carry the field mappings, leaves, occurrences and source
choices for deeper inspection. For example, this memory projection's field is
named `deviceMemory`, and its candidate includes
`numberBits: 4629700416936869888`. Preserve such integers exactly.
Explain derives facts from the **retained report bytes**. After editing named
fields, encode and decode again before asking explain about the new value.

Default decode retains `metadata.originalReport` but leaves
`metadata.signals` empty. `metadata=true` eagerly builds details for every
present positional signal; explain builds just one later. Independently,
`verify=true` checks byte-identical per-signal and whole-header re-encoding.
`metadata.exactReencode` records whether decode verification ran; `false` on a
default decode does not indicate lost bytes or an encoding failure. Encode
checks the requested fields' roundtrip independently of that option.

Absent slots are omitted from the named map and counted in `metadata.missing`.
The named record status vocabulary is `decoded`, `missing`, `unsupported`,
`invalid`, with nullable `fields` for non-decoded records. The public capture
decode path rejects unsupported or invalid **present** signals rather than
returning partially decoded named fields. Explain rejects an unknown signal or
an absent signal; it does not synthesize a `missing` record. For example,
explaining `NotASignal` returned
`invalid_request: unknown signal NotASignal` in this run. Do not confuse the
report's `errors` channel (collected data) with an SDK operation error.
See each language guide for exceptions/errors; C returns an `ok` envelope.

A session override replaces the **whole** artifact session. Pass that same
replacement to decode, explain and encode. None of this walkthrough overrides
it. Keep artifacts with their matching session/capture; artifacts contain tokens
and keys. See [header framing](../docs/generator/headers.md) for adapter details.

## The same edit in each language

The complete runnable recipes in the [Node/TypeScript](node/README.md),
[Python](python/README.md), [Go](go/README.md), [Rust](rust/README.md), and
[C](c/README.md) guides load once, decode, inspect actual fields, explain one
signal, change `DeviceMemory.deviceMemory`, and encode the full payload.
Keep the loaded artifact outside loops and pass it to each direct operation.
`Generator` methods remain available for existing callers.

TypeScript recipes use ESM `import` and `import type`, strict NodeNext settings
and exact-integer JSON helpers. When saving or reloading artifacts/decoded
payloads in Node, use `parseJSON` / `stringifyJSON`; ordinary JSON parsing can
round integer metadata. Go uses `json.Number` / `json.RawMessage`; Python's
standard JSON integers are exact. In C, free result strings with `shape_free`
and wait for all calls before releasing a shared loaded artifact handle.

## Existing CLI payload codec

The CLI can inspect and rebuild a report container below the named signal
fields used in the SDK examples above. Follow the [CLI build and PATH
setup](../README.md#build-once) first; `scripts/build-sdk.sh` alone does not
build the CLI. Run from the repository root with `reports/` created by that
setup, a recovered program, and its matching capture/session initializer:

```sh
shape pin --input reports/program.js --init init.js --out reports/seed.json
shape headers decode --seed reports/seed.json --input headers.http.json --out reports/report.json
shape headers encode --seed reports/seed.json --input reports/report.json --out reports/rebuilt-headers.json
```

Here `headers.http.json` is a raw captured HTTP header object containing all six
full header names and their values, without the SDK's `{siteId, headers}` wrapper.
The decoded report exposes the framed report channels; use the SDK examples
above or the CLI's [named decode workflow](../README.md#decode-headers-into-named-fields)
to work with named fields. The pinned seed layout stores reusable framing and
session settings for this capture; it is distinct from a compiled decode artifact
that supports named signal decoding.

Commands refuse to overwrite existing output. Choose new output paths, or use
`--force` where supported. For bare finalized payload text, use
`shape payload encode --help` and `shape payload decode --help` with the same
pinned seed. See the [report format](../docs/generator/report.md),
[typed JSON format](../docs/generator/README.md), and
[Generator API](../docs/generator/api.md) for lower-level work.
