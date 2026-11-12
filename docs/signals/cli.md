# Signal catalogue CLI

`shape show signals [CanonicalName] [--json]` works from any directory using
compiled card descriptions and clean data examples for all 93 signals. Names are
case sensitive. All-card output is sorted by canonical name; JSON is an array for
all cards and an object for a single card.

Human cards show the purpose, logical paths and types, clean examples and size
availability. Types describe the documented variants, not a complete runtime
schema. Arrays can vary in length, including pending and error variants.
Examples carry their source evidence statement and distinguish captured samples
from illustrative data. Missing capture data is never filled from an example.

For an actual capture use:

```sh
shape show signals DeviceMemory --artifact artifact.json --headers headers.json
shape show signals --artifact artifact.json --headers headers.json --session session.json --json
```

Artifact and headers must be supplied together. Session material is optional and
requires that pair. The capture backend validates the supplied material. Argument
errors exit 2; read/decode errors exit 1 with empty stdout. Closed output pipes
succeed.

An observation's `size.bytes` is reported exactly as supplied by the capture
backend: positional raw signal blob bytes include the four-byte prefix. A header
bundle is never a signal byte measurement. Static examples have unavailable size.
Capture statuses are `captured`, `missing`, `unavailable`, `no_data`; a card absent
from the observation map has status `absent`.

JSON preserves the legacy `signal`, `summary`, `why`, `by_name`, `no_data`,
`messages` and `sample_payload` keys, including precise integer sample values.
Additions are `logical_fields`, `examples`, `example_source`, `status`, `data`,
`provenance`, `special_values`, `size`, and an optional backend `note`.
`sample_payload` remains the original documentary wire/readable sample;
`data` and `examples[].data` are the logical presentation.

Examples use JSON null plus explicit `special_values` entries for `undefined`,
`NaN`, infinities and negative zero. Each entry contains a JSON `pointer` and
`value`; an empty pointer refers to the whole value. Ordinary null has no special
entry. Captured special values use explicit `$special` objects (including
`undefined`, `NaN`, `Infinity`, `-Infinity`, and `-0`) instead of exposing numeric
wire bits or conflating them with null. Opaque binary and extension values are
labeled `binary` and `extension`; their wire bytes are omitted. No-data cards emit
no payload.

The embedded manifest records source paths and SHA-256 digests. Regenerate it
with Node 22 or later using `node scripts/generate-signal-view.mjs`, then check it
with `node scripts/generate-signal-view.mjs --check`. Run this separately from the
documentation data gate, `node scripts/signal-data.test.mjs`. The shared parser
accepts only literal data modules; generation never executes example JavaScript.

The CLI integration expects a public `shape::signals::inspect` module with:

```rust
pub fn inspect(
    artifact: &[u8],
    headers: &[u8],
    session: Option<&[u8]>,
) -> Result<std::collections::BTreeMap<String, serde_json::Value>, String>;
```

Each canonical-name observation contains `status`, `data`, and
`size: { bytes: number | null, kind: "exact" | "unavailable", basis: string }`,
plus an optional string `note`. Presentation preserves this size and status,
normalizes only certified controls and logical aliases in `data`, retains unknown
keys, and distinguishes a missing map entry from an observation with `missing`
status. Documentary examples remain separately labeled in JSON.
