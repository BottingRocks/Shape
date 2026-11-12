# Generic report payload contract

This boundary follows the unchanged user-pasted `createSignalPayloadEncoder` factory. Production Rust never
executes JavaScript. It accepts generic values and already-produced raw bytes, not detector layouts or callbacks.
It always uses an explicitly configured cipher and eight-byte seed prefix; the source's no-cipher/no-key fallback,
implicit alphabet and entropy acquisition are outside this contract.

## JSON and CLI

```json
{
  "errors": [{"kind":"string","value":"synthetic error"}],
  "positionalSignals": [null, [[{"kind":"undefined"},{"kind":"null"}], [0,128,255]]],
  "signals": [{"kind":"number","value":"8000000000000000"}],
  "keyedSignals": [[7,null],[7,[[{"kind":"bool","value":true}],[]]]],
  "seeds": [1200151530,1200151531],
  "keysets": []
}
```

All six fields are required. The four source fields keep their original names. Each value channel is a JSON array
of [typed Superpack values](superpack.md), including nested arrays/objects, undefined, null, bit-exact number inputs,
restricted Dates and Binary. Other root channel types are rejected; JS coercions, sparse arrays and exotic objects
are unsupported. An entry is either `null` or `[values, rawExtraBytes]`; keyed entries are `[u32, entry]` pairs.
Positional and keyed order, including duplicate numeric IDs, are preserved. Names must be 0..4294967295 and raw
bytes 0..255, without JS wrapping/coercion.

```sh
shape payload encode --profile payload-profile.json --input report.json --out payload.txt
shape payload decode --profile payload-profile.json --positional-count 2 --keysets keysets.json --input payload.txt --out report-again.json
shape payload encode --profile payload-profile.json --input report-again.json --out payload-again.txt
cmp payload.txt payload-again.txt
```

Encode emits only base64 text and a final LF, without JSON quotes, objects or headers. Decode accepts the exact
base64 string with either no newline or one final LF/CRLF. Spaces, embedded/repeated line breaks, lone CR,
external padding and noncanonical tail bits are rejected. `=` can be a data symbol in a custom alphabet.
Both routes default to stdin/stdout. Existing file output needs `--force`; output is atomic and input/profile/keyset
aliases, including hard links and symlinks, are rejected. Diagnostics go to stderr and errors do not write output.

A minimal payload profile contains exactly `format`, `cipher`, `alphabet`, and `keys`, as in the
[profile contract](README.md). `format` is `shape-byte-codec/1`;
`cipher` contains explicit `ivs`, `counter`, `double_rounds`. The existing full header profile schema is also
accepted for convenience: its crypto fields and optional `positional_count`/`keysets` defaults are used. No header names, transaction tokens, chunking
or header context are required, fabricated or emitted by this route. There is no automatic profile extraction.
Unknown/duplicate JSON fields are rejected.

`seeds` are two caller-supplied u32 words on encode and are recovered from the prefix on decode. No timing,
randomness or browser state is acquired. `keysets` is the ordered list of omitted keysets for **signals only**;
key order and keyset order are never changed. Full report JSON requires it even when empty and keeps its own
keysets and seeds authoritative. Four-channel encode with `--seeds` uses the saved layout's or full profile's
keysets, defaulting to `[]` when absent for compatibility. Decode requires both the positional count and keysets:
`--positional-count` and a `--keysets` JSON file independently override saved-layout or full-profile defaults.
Missing facts require the corresponding flags; zero and explicit `[]` are valid facts. A crypto-only profile
supplies neither fact. Neither fact is encoded in the payload. The output
retains the supplied keysets, and positional count is represented by the positional array length. Header context
is outside this payload contract and is not report metadata.

Incorrect keysets that leave unresolved references fail with a diagnostic naming the channel and required facts.
A wrong dictionary of the same shape, or a wrong positional count that happens to yield a valid alternative framing,
cannot always be detected: the wire is unauthenticated and does not store those facts. Supply provenance from the
correct build. The codec never searches counts, fills invented keys or substitutes opaque blobs for decoded values.

## Packing and canonical mapping

The adapter packs nonempty errors and entry value arrays exactly once with default Superpack options. Only the
nonempty signals array uses `outer_channel(keysets)`: semantic array/object depth 100, extension 0 for the
missing-value sentinel and `0xdeadbeef` for depth truncation. The channel array itself starts at depth 0.
Empty arrays bypass Superpack and become zero-length blobs. Raw extras are always copied unchanged, without
adding seeds, interpretation or generic packing. Framing order is errors → positional entries → signals → keyed entries.

Missing entries produce `00 00` after any keyed name. This is also the encoding of a present `[[],[]]` entry:
the wire cannot preserve that identity. Encode rejects present empty/empty entries and requires `null`; decode
maps both positional and keyed zero/zero records to `null`. A present raw-only entry `[[],[0]]` remains distinct.

Canonical payload bytes → report → payload preserves the entire finalized base64 string. Decode rejects
noncanonical outer ULEB framing and noncanonical Superpack channels, including packed empty arrays (which the
factory would bypass), instead of silently changing original bytes. Use `PayloadCodec::decode` to retain raw
plaintext and seeds, or the existing byte commands with the legacy profile, when original noncanonical bytes
must be retained. A generic channel that decodes to a non-array is outside this report model and is rejected.

Report JSON → payload → report preserves source semantics, with source normalization: object keys are sorted by
UTF-16 order, NaNs use the fixed source quiet NaN, and deeper signals containers become a typed depth extension
with null payload. The decoder cannot recover content that the source discarded. Undefined, null, +0, -0,
canonical NaN and ±Infinity remain distinct typed values. Errors/entry arrays have no semantic depth truncation;
their normal resource depth limit still applies.

## Public Rust API and bounds

```rust
use shape::generator::{report::Report, PayloadCodec, PayloadProfile};
# fn example(profile: PayloadProfile, report: Report) -> Result<(), Box<dyn std::error::Error>> {
let codec = PayloadCodec::new(profile)?;
let text = codec.encode_report(&report)?;
let recovered = codec.decode_report(&text, report.positional_signals.len(), report.keysets.clone())?;
assert_eq!(codec.encode_report(&recovered)?, text);
# Ok(()) }
```

`Report::to_payload` exposes the packed container when needed. `PayloadCodec::encode_bytes` and `decode` are
shared with the existing `Generator` implementation, so report and byte/header routes use the same cipher,
seed prefix and custom base64. No header policy is consulted by `PayloadCodec`.

Report JSON uses the same 1,024 structural-level preflight as `pack`, on the binary's existing 256 MiB
reserved stack. Strings/escapes cannot hide structural delimiters; syntax, typed fields and exact consumption
are then checked by serde. Other parsers retain their existing recursion policy. Input is bounded to 128 MiB;
profiles and decode keyset files to 64 KiB. Native library callers handling deep typed values should use an
appropriately sized stack, as for the generic API.

Plaintext/framing stays within 16 MiB and combined positional/keyed records within 65,536. Existing Superpack
limits (default depth 128, hard maximum 256) remain unchanged. Report operations also bound aggregate generic
value entries to 65,536 and aggregate string/key/binary/raw-extra storage to 16 MiB across channels, preventing
many individually bounded decodes from accumulating unbounded expanded values. This report-wide check includes
encode content that semantic truncation would discard. Library keysets remain subject to Superpack's existing
entry/storage validation; CLI decode settings additionally have the file bound above.

## Permanent source proof

[Factory](../../tests/fixtures/generator/payload-factory.js) is the unmodified user-pasted function (SHA-256
`24e70043d6b8f5aeb4e15ca045e7336eba422ed3c98d2ac6081ecbac9cab2e8a`).
[Provenance](../../tests/fixtures/generator/report-provenance.json) pins its hash and those of the unchanged
cleaned-source cipher and shipped generic packer. The cipher binding is explicit for this synthetic build;
the pasted factory itself does not define its cipher dependency. Fixtures contain synthetic keys/reports only.

The [vector harness](../../tests/fixtures/generator/report-vectors.cjs) pins random, performance, Date,
crypto and counter state, runs the exact factory, and records the resulting two seeds. The 53 fixed answers
always run in Rust, including four root fields, special primitives, omitted keysets, source depth boundaries,
all six section-length sites at 127/128/16383/16384 and numeric name width boundaries through u32::MAX.
Both payload and plaintext hashes are checked, with literal payload strings for small cases. When Node exists,
`node tests/fixtures/generator/report-vectors.cjs --check` regenerates the answers without ignored reports,
private captures or attachments. Production Rust never shells out to this harness.
