# Generator: reports and finalized payloads

`shape payload encode` converts generic report JSON directly to a **bare finalized custom-base64 payload**.
`shape payload decode` returns the source report fields at the root: `errors`, `positionalSignals`, `signals`,
and `keyedSignals`. Values are packed/unpacked automatically; callers supply raw extra bytes, crypto configuration,
ordered omitted keysets and two payload seeds. A [pinned seed layout](pin.md) supplies the source-derived settings.
No headers, signal collection, browser hooks or entropy collection are involved. Decoding recovers the seeds; the JSON result directly re-encodes with the same external crypto profile.

The commands below use a caller-supplied `payload-profile.json` and `keysets.json`.

```sh
cargo build --locked --bin shape
target/debug/shape payload encode --profile payload-profile.json --input tests/fixtures/sdk/generator/report.json --out payload.txt
target/debug/shape payload decode --profile payload-profile.json --keysets keysets.json --positional-count 3 --input payload.txt --out report.json
target/debug/shape payload encode --profile payload-profile.json --input report.json --out payload-again.txt
cmp payload.txt payload-again.txt
```

The [report example](../../tests/fixtures/sdk/generator/report.json) exercises all four fields, missing entries, duplicate keyed
IDs, special primitives and raw extras. Supply your own `payload-profile.json`
using the profile contract below and `keysets.json` containing the ordered omitted
keysets (`[]` for this fixture). See the [report contract](report.md) for the exact JSON/API, source comparison and normalization rules.
The payload is unauthenticated; the decoder cannot discover or authenticate omitted keysets or the positional count.

## Pinned seed layouts

A traced seed becomes one `shape-seed-layout/1` artifact, and every later call
loads it without re-parsing the program. The first encode/decode invocation can
build it in-process and save it. In a fresh working directory, supply a matching
`seed.raw.js`, initializer JavaScript `init.js`, and full-name header map
`headers.json`. The numeric seeds below are caller-selected example values:

```sh
shape trace program seed.raw.js --out seed.program.js --thread-cleanup

# first run: pin, save the artifact, and decode
shape headers decode --program seed.program.js --init init.js --save-seed seed.json \
  --input headers.json --out report.json

# later runs: artifact only
shape headers decode --seed seed.json --input headers.json --channels --out channels.json
shape headers encode --seed seed.json --seeds 287454020,1432778632 --input channels.json --out headers-reencoded.json

# artifact-only shortcut when no payload work is needed
shape pin --input seed.program.js --init init.js --out seed-repinned.json
```

The equivalent Rust program reads the same `seed.program.js`, `init.js` and
`headers.json`. Depend on this repository's `shape` crate by path and on
`serde_json`, then run it from that working directory:

```rust
use shape::signals::payload::{pin, session_from_initializer, Headers, SeedLayout};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let init_source = std::fs::read_to_string("init.js")?;
    let program_source = std::fs::read_to_string("seed.program.js")?;
    let headers: Headers = serde_json::from_slice(&std::fs::read("headers.json")?)?;
    let session = session_from_initializer(&init_source)?;
    let pinned = pin(&program_source, &session)?;
    std::fs::write("seed-rust.json", pinned.to_json_pretty())?;

    let layout = SeedLayout::from_json(&std::fs::read("seed-rust.json")?)?;
    let report = layout.decode_headers(&headers)?;
    let same = layout.encode_headers(&report.channels_owned(), report.seeds)?;
    assert_eq!(same.iter().count(), headers.iter().count());
    for (name, value) in headers.iter() {
        assert_eq!(same.get(name), Some(value));
    }
    Ok(())
}
```

The artifact carries the cipher, header policy, session literals, ordered
keysets and the report layout (the four source channels, with each positional
slot and its descriptor). `SeedLayout::{encode_payload, encode_headers,
decode_payload, decode_headers}` run the complete superpack → container →
cipher → base64 → header path. `shape pin` builds the artifact and the framing
encode/decode commands accept `--seed`; `shape gen` is a legacy CLI alias.
The API surface and exact commands are in [calling the generator](api.md); the
structural proofs, spans and artifact fields are in
[pinning one traced seed](pin.md).

The strict whole-body contract extractor and its `contracts/` JavaScript
templates were removed: they could only accept the exact emission they were
written from. `pin` proves the same facts structurally, so a seed that emits the
constants in another statement order still pins.

## Session header utilities

`shape headers encode` converts final report JSON (or `--input-format payload`
finalized text) into session headers. `shape headers decode` accepts JSON maps
or strict LF/CRLF family blocks and returns final report fields plus explicit
`header_context` for direct reencoding. See [header utilities](headers.md) for
commands, the separate header-report document extension, and the public Rust API.

## Generic typed serialization

`shape pack` accepts a `PackRequest` (`value` plus optional `options`) and emits an `UnpackRequest`
(`bytes` plus the same options). `shape unpack` reverses it into a directly re-encodable `PackRequest`.
Neither command needs a cipher profile. See the [superpack guide](superpack.md) for source evidence, options,
supported values, and normalization limits. These commands serialize caller-supplied values; they do not collect
signals. The clean-program adapter validates the fixed serializer contract and supplies source-derived settings.

```sh
target/debug/shape pack --input tests/fixtures/sdk/generator/superpack-values.json --out packed.json
target/debug/shape unpack --input packed.json --out value.json
target/debug/shape pack --input value.json --out packed-again.json
cmp packed.json packed-again.json
```

Values use `{"kind":"undefined"}`, `{"kind":"null"}`, `{"kind":"bool","value":true}`,
`{"kind":"string","value":"text"}`, `{"kind":"array","value":[...]}`, or
`{"kind":"object","value":[["key",{"kind":"null"}]]}`. Binary bytes use
`{"kind":"binary","value":[0,128,255]}`. Numbers carry **16 hexadecimal IEEE-754 binary64 bit digits**
as strings, for example `{"kind":"number","value":"8000000000000000"}` for negative zero:

| Value | Number `value` string |
| --- | --- |
| +0 | `0000000000000000` |
| -0 | `8000000000000000` |
| 1.5 | `3ff8000000000000` |
| NaN (canonical quiet example) | `7ff8000000000000` |
| +Infinity | `7ff0000000000000` |
| -Infinity | `fff0000000000000` |

The [typed example](../../tests/fixtures/sdk/generator/superpack-values.json) keeps these categories distinct without JSON
`NaN` literals or null coercion. Source serialization normalizes NaN payload bits; arbitrary input value identity
is therefore subject to the core's documented normalization. In Rust, `Value::from(1.5)` and
`Value::Number(Number(0x8000000000000000))` are convenient builders under `shape::signals::payload::superpack`.
The [extension example](../../tests/fixtures/sdk/generator/superpack-extensions.json) explicitly enables missing-sentinel
extension 0 and depth extension `0xdeadbeef` (JSON decimal `3735928559`). Library callers use
`SerializerOptions::outer_channel(keysets)` for the outer signals channel; errors and entry encoded channels use
defaults. Raw entry `extra` bytes bypass superpack and are preserved unchanged, without double packing.

Unpack requires canonical source bytes. Noncanonical encodings fail with a diagnostic pointing to the raw/library
`shape::signals::payload::superpack::unpack` API, whose `Decoded` retains `original_bytes` and reports `canonical`; the CLI never
silently discards that provenance. Malformed, truncated, and trailing bytes are errors. Options travel with bytes
and must match the caller's actual serializer settings. JSON rejects unknown fields. The 16 MiB byte limit and
128 MiB JSON input limit apply. Both commands share stdin/stdout defaults, atomic output, `--force`, and input/output
alias rejection with encode/decode; profile checks apply only to encode/decode.

`pack` accepts at most 1,024 simultaneously open JSON objects/arrays, counting structural delimiters
outside strings before typed parsing. Typed object entries cost three JSON levels per semantic level; this
allows the core depth-256 ceiling and the original outer-channel depth-100 behavior. This JSON safety cap is
separate from Superpack's semantic truncation and resource limits, which are unchanged. `unpack` output
can be fed directly back to `pack` at supported depths. `payload encode` shares this bounded typed-input
parser. Other JSON parsers retain their existing limits.

The final-stage library pipeline is caller values → superpack bytes → outer container → cipher → custom base64 →
session header chunks. Supply packed bytes explicitly in `Container.errors`, entry `.encoded`, and `.signals`;
`pack` output is a serializer request document, not an encode `Payload`. Container decoding still requires an
explicit positional count. The synthetic `tests/generator_pipeline.rs` exercises this entire boundary.

## Existing byte/header boundary

`shape encode` and `shape decode` work now with **caller-produced bytes and an explicit profile**. The Rust
`shape::signals::payload` API uses the same implementation. The profile supplies the cipher geometry, alphabet, keys, and one
header name. Payload seeds are required. No captured seed, keys, session, or random values are selected automatically.

Signal collection, collector wiring and `shape build` / standalone seed binaries are future work.
Source-derived configuration is available through the pinned layouts above; this lower-level route starts where the caller already has bytes.
It supports one configured `-a` payload family, optional numeric chunks, computed `-b`/`-f` session headers,
and caller-supplied opaque `-c`/`-d`/`-z` context. Other families remain unsupported.

## Byte/header copy-paste workflow

Build from the repository root, then use the synthetic examples (replace their profile/key/seed material for real use):

```sh
cargo build --bin shape
mkdir -p reports/generator

target/debug/shape encode --profile tests/fixtures/sdk/generator/profile.json \
  --input tests/fixtures/sdk/generator/raw.json --out reports/generator/headers.json

target/debug/shape decode --profile tests/fixtures/sdk/generator/profile.json \
  --input reports/generator/headers.json --out reports/generator/decoded.json

target/debug/shape encode --profile tests/fixtures/sdk/generator/profile.json \
  --input reports/generator/decoded.json --out reports/generator/reencoded.json

cmp reports/generator/headers.json reports/generator/reencoded.json
```

Files are not overwritten unless `--force` is supplied. `--input -` and `--out -` mean stdin and stdout and are the
defaults. The profile must be a file. JSON results go to stdout (or the output file); diagnostics go to stderr, with a
nonzero exit on failure. For example:

```sh
cat tests/fixtures/sdk/generator/raw.json | target/debug/shape encode \
  --profile tests/fixtures/sdk/generator/profile.json > reports/generator/headers.json
```

Input, profile, and output must be distinct paths, including canonical/symlink aliases and, on Unix, hard links.
Aliases are rejected even with `--force`. Output files are published atomically; output symlinks and directories are
refused. Parent directories must exist. These checks protect ordinary local file use, not hostile concurrent changes
to the filesystem. Do not redirect stdout over an input/profile file: the shell truncates redirected files before
`shape` can check them; use `--out` for checked file output.

## Explicit profile

[`tests/fixtures/sdk/generator/profile.json`](../../tests/fixtures/sdk/generator/profile.json) is a synthetic ten-double-round example:

```json
{
  "format": "shape-byte-codec/1",
  "cipher": {
    "ivs": [1634760805, 857760878, 2036477234, 1797285236],
    "counter": 0,
    "double_rounds": 10
  },
  "alphabet": "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_",
  "keys": [0, 1, 2, 3, 4, 5, 6, 7],
  "header_name": "X-example-a"
}
```

Every field shown above is required. Unknown fields, duplicate struct fields, wrong array lengths, negative/fractional integers,
and integers outside their widths are errors. JSON numbers are decimal. Convert captured signed int32 literals to
their unsigned `u32` representation explicitly; the generator does not wrap them for you.

| Field | Contract |
| --- | --- |
| `format` | Exactly `shape-byte-codec/1`; fixes the supported wire/counter policy |
| `cipher.ivs` | Exactly four `u32` words |
| `cipher.counter` | Integer `0 <= counter < 2^53`; increments modulo `2^53` per 64-byte block |
| `cipher.double_rounds` | Explicit integer `1..=20`; **double** rounds, not individual rounds |
| `keys` | Exactly eight `u32` words |
| `alphabet` | 64 or 65 unique visible ASCII symbols, without whitespace. Only the first 64 encode data; the optional 65th is unused |
| `header_name` | HTTP token, 1–256 ASCII bytes, ending in `-a` (case-insensitive) |
| `transaction_token` | Optional exact session token; required for emitting/checking `-b` or `-f`. Never used by the outer cipher |
| `chunk_size` | Optional explicit characters per `-a` chunk, `1..=MAX_HEADER_BYTES`; absent means unsplit only. Chunked base names are limited to 251 bytes |
| `header_context` | Optional typed sibling context/presence; described below |

The ChaCha-style state is `[ivs[0..4], keys[0..8], counter_low, counter_high, seed1, seed2]`. Each double round runs four
column and four diagonal quarter-rounds, with rotations 16/12/8/7; add-back and output words are little-endian.
Four-, five-, and six-double-round builds have independent captured known answers. Ten double rounds gives the
ChaCha20 round schedule, with this format's original-style 64-bit counter / 64-bit seed state and modulo-`2^53`
counter progression; it is not the IETF 96-bit nonce interface. Choose the geometry for the caller's actual build.

The outer cipher does **not** use the transaction token. Header formatting uses it only for `-b`/`-f` when present.
There is no implicit session fallback. Reusing keys and seeds reuses a stream; fixed seeds in tests are for
reproducibility. This cipher has **no authentication tag**. Wrong keys decrypt to bytes too. Success never verifies
keys, signal contents, or authenticity.

## Input bytes and output headers

[`raw.json`](../../tests/fixtures/sdk/generator/raw.json) is the minimal plaintext contract:

```json
{
  "seeds": [287454020, 1432778632],
  "data": {"kind": "raw", "bytes": [0, 255, 128, 65, 0]}
}
```

`seeds` is exactly two `u32` words; `bytes` is an array of integers `0..=255`, including empty arrays and non-UTF-8
bytes. Raw mode encrypts exactly these bytes and adds no container or per-signal framing. Decode returns this same
schema, preserving the seeds, ready to feed straight back to encode.

Output is a JSON header map, for example `{ "X-example-a": "<value>" }`. Header names are compared case-insensitively.
Without optional policy/context, the original single-header profile behavior is unchanged. Duplicate names (including
exact duplicate JSON keys and case aliases), unknown suffixes, and unrelated prefixes are errors.

Wire format:

```text
custom_base64(be32(seed1) || be32(seed2) || crypt(plaintext))
```

Generator decode accepts **canonical unpadded** custom base64 only: no whitespace, external padding, dangling symbol,
or nonzero unused tail bits. `=` is ordinary data when in the first 64 alphabet symbols, even at the end of a value.
Every accepted value has exact `decode -> encode` identity with the same profile. This strict generator policy does
not change the existing low-level `shape::signals::codec::base64_decode`, which normalizes whitespace and external padding.
No HTTP header name/value can contain control characters through this interface.

## Session headers, chunks, and opaque context

[`session-profile.json`](../../tests/fixtures/sdk/generator/session-profile.json) is a **synthetic** complete example. It adds
these optional fields to the base profile:

```json
{
  "transaction_token": "example-transaction-token",
  "chunk_size": 12,
  "header_context": {
    "b": true,
    "f": true,
    "c": "caller-build-metadata",
    "d": "caller-opaque-descriptor",
    "z": "caller-marker"
  }
}
```

This fragment is not a full profile; keep the required cipher/alphabet/key fields. The small example chunk size makes
splitting visible; the library has no seed-specific chunk-size default. To run the complete synthetic profile:

```sh
target/debug/shape encode --profile tests/fixtures/sdk/generator/session-profile.json \
  --input tests/fixtures/sdk/generator/raw.json --out reports/generator/bundle.json
target/debug/shape decode --profile tests/fixtures/sdk/generator/session-profile.json \
  --input reports/generator/bundle.json --out reports/generator/bundle-bytes.json
target/debug/shape encode --profile tests/fixtures/sdk/generator/session-profile.json \
  --input reports/generator/bundle-bytes.json --out reports/generator/bundle-again.json
```

The supported suffixes share the configured header prefix:

| Suffix | Behavior |
| --- | --- |
| `a`, `a0`, `a1`, ... | Encrypt once, then split the canonical ASCII payload. Decode joins in **numeric** order before base64/cipher processing |
| `b` | `stringBufferHash(transaction_token + FULL_UNCHUNKED_PAYLOAD).toString(36)`: signed wrapping `i32`, over UTF-16 pairs, lowercase signed base36 |
| `f` | Exact supplied `transaction_token` |
| `c`, `d`, `z` | Preserve exact opaque strings without interpretation; explicit profiles use caller context, while extracted configs carry proven source fallback defaults |

`b`/`f` booleans in `HeaderContext` select their presence; they are not pass-through checksum/token values. Decode
checks any supplied `b`/`f`, then records their presence and the exact `c`/`d`/`z` values in `Payload.header_context`.
Encode recomputes `b` and copies the supplied token to `f`. Missing siblings are not generated unless explicitly
selected by profile/payload context. A profile context, when present, must agree with the decoded or input payload
context, including header presence. Conflicts, uppercase context-field aliases, duplicate fields, and unknown fields
are rejected. `c`/`d`/`z` may be supplied without any session token. Their strings and tokens are bounded to 64 KiB;
emitted header values allow ASCII spaces but never control characters.

For a new byte payload, keep the same explicit session/profile context and provide new `data.bytes` and required
`seeds`. Alternatively, carry `header_context` in the payload while omitting it from the profile. No old encrypted
`a` value is retained as a shortcut: bytes always pass through the cipher again. Raw library `encode_bytes`/`decode`
remain usable independently of named-header policy. `encode(Payload)` also rejects invalid/conflicting context.

Chunks require an explicit `chunk_size`. Except the last chunk, every chunk must have exactly that size; the last
must be nonempty and no larger. A base `a` is required; suffixes must be contiguous decimal `0,1,...` without signs,
leading-zero aliases, holes, or case duplicates. Joining cannot exceed `MAX_HEADER_BYTES`; bundles are bounded to
65,536 entries and `MAX_HEADER_BYTES + 5 * 64 KiB` total value bytes. Removing a final chunk may be indistinguishable
from a shorter payload without a supplied checksum; `b` is checked when present, but it is still **not authentication**.
Header-map equality, including every supplied value, is the round-trip contract; JSON insertion order is irrelevant.

The audited copied runtime `reports/full-naming-samples/target-seed-1.latest.renamed.js` supplies the evidence:
`chunkHeaderValue` around lines 14339–14356 and its call around 15429–15430 split at an explicit 7900 characters;
`createEncodingOptions` around 2576–2597 emits `f = transactionToken` and hashes `transactionToken + full a` for `b`.
Its `c`/`d`/`z` literals/descriptor override are context, not additional outer-cipher inputs. This implementation
requires those values from the caller instead of adopting the copied literals for another session/build.

Initializer script/style nonce arguments have CSP roles: assignments around 16465–16469, style nonce reads around
11192, 11260, and 15606. A supplied fallback expression/string is not proof of the resolved runtime nonce. The payload
seeds are separate words on the cipher path around 14800–14835. CSP nonce provenance stays with caller acquisition
metadata; it is neither a payload seed nor a crypto IV here. No initializer parsing or execution is performed.

## Optional opaque container framing

[`container.json`](../../tests/fixtures/sdk/generator/container.json) supplies opaque sections instead of already-packed bytes:

```json
{
  "seeds": [287454020, 1432778632],
  "data": {
    "kind": "container",
    "container": {
      "errors": [0, 255],
      "positional": [{"encoded": [42], "extra": [0, 128, 255]}, null],
      "signals": [129, 0],
      "keyed": [{"name": 7, "encoded": [], "extra": [255, 0]}]
    }
  }
}
```

All four container fields and both fields of each entry are required (`name` is also required for keyed entries).
Sections contain opaque bytes; the generator does not interpret or manufacture their contents. In particular, it
adds no seed prefix to `extra`. A positional `null` is the two-zero-byte sentinel. An entry with both byte arrays empty
would have the same wire representation, so encode rejects it; use `null`. Keyed names are `u32`; keyed records retain
array order and repeated names, with no sorting/deduplication.

Packing uses the existing codec:

```text
uleb(len(errors)) || errors
for each positional: uleb(len(encoded)) || encoded || uleb(len(extra)) || extra
uleb(len(signals)) || signals
for each keyed: uleb(name) || uleb(len(encoded)) || encoded || uleb(len(extra)) || extra
```

The positional count is not stored on the wire. Opaque decode uses a saved-layout or full-profile count when present,
otherwise it defaults to raw bytes. Request or override a structured result with the caller's
explicit count:

```sh
target/debug/shape encode --profile tests/fixtures/sdk/generator/profile.json \
  --input tests/fixtures/sdk/generator/container.json --out reports/generator/container-headers.json

target/debug/shape decode --profile tests/fixtures/sdk/generator/profile.json \
  --input reports/generator/container-headers.json --positional-count 2 \
  --out reports/generator/container-decoded.json
```

This checks framing, length/count bounds, numeric widths, complete consumption, and canonical repacking. A count
that happens to parse is not evidence of correct keys or signal semantics. Noncanonical/overlong framing is refused
in structured mode; raw mode preserves it exactly. Raw and container forms cannot be combined. No accepted structured
result loses bytes on repacking.

Plaintext is limited to 16 MiB; combined positional/keyed entries to 65,536. No untrusted length/count reserves an
unchecked allocation. CLI profile JSON is limited to 64 KiB and input JSON to 128 MiB. Large byte arrays are serialized
as compact JSON. Invalid input is fully checked before file output begins.

## Rust API

Depend on this repository's `shape` crate by path and on `serde_json`. Run from
the repository root so the public profile fixture resolves:

```rust
use shape::signals::payload::{Generator, Payload, Profile};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let profile: Profile = serde_json::from_str(&std::fs::read_to_string(
        "tests/fixtures/sdk/generator/profile.json",
    )?)?;
    let codec = Generator::new(profile)?; // validates all profile material
    let input = Payload::raw([0x11223344, 0x55667788], vec![0, 255, 128]);
    let headers = codec.encode_headers(&input)?;
    let decoded = codec.decode_headers(&headers)?; // Payload with Data::Raw
    assert_eq!(decoded, input);

    // Raw header-value API, without named-header JSON:
    let value = codec.encode_bytes(&[0, 255, 128], input.seeds)?;
    assert_eq!(codec.encode(&codec.decode(&value)?)?, value);
    Ok(())
}
```

`Payload { seeds, data: Data::Container { container }, header_context: None }` packs a caller-supplied `Container`.
`Payload::plaintext()` exposes the bytes, and `Payload::parse_container(count)` returns a separate structured payload
or an error, leaving the raw payload available. `Container`, `PositionalEntry`, and `KeyedEntry` are reexports of the
existing low-level types. Existing signal/layout helpers and examples remain available separately; this generator
does not invoke them.

`HeaderContext` exposes `b`/`f` presence booleans and optional `c`/`d`/`z` strings. `Headers::from_entries` constructs a
bundle with duplicate/size checks; `Headers::get` looks up a name case-insensitively, `iter` exposes all entries, and
`as_parts()` returns `Some((name, value))` only for single-entry maps. `header_checksum` exposes the captured hash
formatting helper. `parse_container` preserves header context and still requires an explicit positional count.

## Known-answer evidence and checks

[`tests/fixtures/generator/known-answers.json`](../../tests/fixtures/generator/known-answers.json) copies literal
known answers from the original Python reference codec (`shape_a_header.py`, no longer in the repo) (classic JS at four double rounds and traced wasm at six) and the existing
Rust JS-oracle payload test at five. No external Python/JS is executed by the generator or these tests. An additional
all-zero key/nonce ChaCha20 block checks the ten-double-round option. Tests cover binary/empty/multiblock bytes,
sentinel/keyed sections, exact header re-encoding, strict JSON, malformed framing, bounded counts, and CLI/file safety.

```sh
cargo check --lib --bin shape
cargo test --test generator_codec --test generator_cli --test generator_headers
cargo test --lib signals::codec::
```

[Pinned seed layouts](pin.md) now supply proven build configuration. Signal value generation,
semantic signal layouts, browser collection, other header families, and native per-seed builds require their own
implementation and evidence. The [program guide](../naming/README.md) describes the analysis side.

Permanent session/bundle tests use synthetic values, including more than ten chunks, signed UTF-16 checksum literals,
missing/invalid chunks, checksum/token mismatches, context conflicts, and CLI round trips. Private captured bundles
and decrypted browser bytes belong under ignored `.agent-scratch`, never in tracked fixtures.
