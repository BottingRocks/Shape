# Superpack value serialization

Superpack converts caller-supplied typed values into compact serialized bytes and
decodes those bytes back into values. Use it to prepare individual report
subfields or to work directly with their encoding. It does not collect signals
or require a browser, JavaScript runtime, seed, or cipher profile.

The core Rust API is `shape::signals::payload::superpack`. The SDK also exposes
`shape::sdk::{pack, unpack, unpack_canonical, PackValue, SerializerOptions}`.
For complete reports and headers, see the [generator guide](README.md).
Superpack bytes feed the container, cipher, base64, and header layers; they are
not a complete report or header. Already packed entry `extra` bytes pass through
unchanged and must not be packed again. An absent channel is also distinct from
a serialized empty array.

## Rust quickstart

Build a typed value, pack it, and inspect the decoded result:

```rust
use shape::signals::payload::superpack::{pack, unpack, Error, SerializerOptions, Value};

fn main() -> Result<(), Error> {
    let value = Value::Array(vec![
        Value::String("ready".into()),
        Value::Bool(true),
        Value::Undefined,
    ]);
    let options = SerializerOptions::default();

    let bytes = pack(&value, &options)?;
    let decoded = unpack(&bytes, &options)?;
    assert_eq!(decoded.value, value);
    assert!(decoded.canonical);
    assert_eq!(pack(&decoded.value, &options)?, bytes);
    Ok(())
}
```

`pack(&Value, &SerializerOptions)` returns `Vec<u8>`. `unpack` returns a
`Decoded` containing `value`, `original_bytes`, and `canonical`.
`unpack_canonical` returns a `Value` and rejects bytes that would change when
repacked. All three functions return `Result<_, Error>`.

Typed values distinguish undefined from null and preserve binary data and
special numbers that ordinary JSON cannot express. In Rust, `Value::from(1.5)`
builds a binary64 number; `Number::from(f64)` and `Number(pub u64)` allow explicit
control over its bits. Object entries use `Vec<(String, Value)>` so their order
and duplicate keys can be handled explicitly.

## CLI quickstart

With `shape` available as described in [CLI setup](../../README.md#build-once),
run this example in a directory where the output files do not yet exist:

```sh
cat > value.json <<'JSON'
{
  "value": {
    "kind": "array",
    "value": [
      {"kind": "string", "value": "ready"},
      {"kind": "bool", "value": true},
      {"kind": "undefined"}
    ]
  },
  "options": {}
}
JSON
shape pack --input value.json --out packed.json
shape unpack --input packed.json --out restored.json
shape pack --input restored.json --out packed-again.json
cmp packed.json packed-again.json
```

`pack` reads a `PackRequest` with `value` and optional `options`. It writes an
`UnpackRequest`: a JSON `bytes` array and the same serializer options. This file
is a request document, not a raw binary file or a report `Payload`.
`unpack` accepts that document and writes a directly reusable `PackRequest`.
Default options are expanded in the output, so `restored.json` need not be
textually identical to `value.json`.

The CLI requires canonical bytes on unpack. Use Rust `unpack` when you need to
retain an alternate encoding. Both commands default to stdin/stdout, accept
`--input` and `--out`, and require `--force` to replace an existing output file.
They reject input/output aliases and validate input before writing output.
Library callers can use `PackRequest::pack()` and `UnpackRequest::unpack()` for
the same serde request types; the latter retains the library's permissive
noncanonical decode behavior.

## Supported values

JSON values always use an explicit `kind`; arbitrary untyped JSON is not accepted.
Unknown kinds and fields are rejected.

| Rust `Value` variant | JSON representation / constraint |
| --- | --- |
| `Undefined`, `Null` | `{"kind":"undefined"}` or `{"kind":"null"}` |
| `Bool(bool)` | `{"kind":"bool","value":true}` |
| `Number(Number)` | `{"kind":"number","value":"3ff8000000000000"}` for 1.5 |
| `String(String)` | `{"kind":"string","value":"text"}`; valid Unicode, including NUL |
| `Array(Vec<Value>)` | `{"kind":"array","value":[{"kind":"null"}]}` |
| `Object(Vec<(String, Value)>)` | `{"kind":"object","value":[["key",{"kind":"null"}]]}`; unique keys |
| `Binary(Vec<u8>)` | `{"kind":"binary","value":[0,128,255]}` |
| `Date(u64)` | `{"kind":"date","value":123456789}`; integer milliseconds in `0..=2^48-1` |
| `MissingSentinel` | `{"kind":"missing_sentinel"}`; requires its extension option |
| `Extension { id, payload }` | `{"kind":"extension","value":{"id":7,"payload":{"kind":"null"}}}`; requires matching options |

A number's JSON `value` is exactly 16 hexadecimal IEEE-754 binary64 bit digits,
not a decimal number or a hexadecimal numeric magnitude. This avoids JSON
number precision loss and represents signed zero, infinities, and NaNs without
coercing them to null. For example, negative zero is `8000000000000000` and
positive infinity is `7ff0000000000000`. NaN payloads survive the typed JSON
boundary but normalize during packing.

Numbers have binary64 semantics, not arbitrary `i64`/`u64` precision. Dates are
limited to the nonnegative range above; negative, fractional, invalid, and
out-of-range dates are unsupported. The format's six timestamp bytes do not
recover an original negative or invalid JavaScript Date identity.

The typed model has no functions, symbols, BigInts, sparse arrays, JavaScript
prototypes, or detector callbacks. Strings cannot contain lone UTF-16
surrogates. Convert application values explicitly rather than silently replacing
unsupported values with null or ordinary objects.

## Serializer options

Use the same options to pack and unpack, especially when keysets are omitted.

| `SerializerOptions` field | Default | Behavior |
| --- | --- | --- |
| `keysets_to_omit` | `[]` | Preloads ordered keysets without transmitting them; the decoder needs the same ordered list |
| `depth_bound` | `None` / JSON `null` | Replaces arrays/objects at the semantic bound with a depth extension |
| `missing_sentinel_extension` | `false` | Enables `MissingSentinel`, extension ID 0 with undefined payload |
| `opaque_extensions` | `false` | Allows explicit extensions with other IDs and typed value payloads |
| `limits` | See below | Bounds bytes, entries, and recursive processing independently of semantic truncation |

`SerializerOptions::outer_channel(keysets)` sets depth 100, enables the missing
sentinel, and supplies omitted keysets. Use this preset for the outer signals
channel. Errors and individual entry `encoded` values use default options;
independent calls do not inherit the outer channel's depth bound.

Each preloaded keyset must contain unique strings. Keyset arrays and their
containing list are not automatically sorted or deduplicated; lookup reuses the
first exact ordered match. Supply keysets in the order expected by the data.

The semantic depth bound counts entry into arrays and objects. Primitives,
dates, binary values, and extension dispatch do not themselves trigger semantic
truncation, though resource limits still apply. Truncation emits extension
`0xDEADBEEF` with a null payload; unpacking returns that marker and cannot
reconstruct the omitted value. This ID is a marker, not encryption or a hash.
The bound also affects generated keyset metadata: a small bound such as 1 can
truncate that metadata, producing bytes that `pack` emits but `unpack` rejects
as unrecoverable.

Known extension IDs always require their matching option and exact payload,
even with `opaque_extensions` enabled. ID 0 requires undefined;
`0xDEADBEEF` requires null and a configured depth bound. Other IDs are `u32`
values with a recursively encoded `Value` payload, not a raw byte-length field.
An explicit ID-0 extension decodes as `MissingSentinel`.

## Canonical decoding and normalization

`unpack` consumes exactly one complete document and retains its exact bytes in
`original_bytes`. Its `canonical` flag is true only when repacking the decoded
value with the supplied options succeeds and produces identical bytes. Use
`unpack_canonical` or CLI `unpack` to require that property.

Packing normalizes object keys to lexical UTF-16 code-unit order, including
numeric-looking keys. Decoded objects carry the wire order. Every NaN packs as
quiet float32 NaN (`ec 7f c0 00 00`); signed zero remains distinct. Depth truncation
and the ID-0 extension conversion described above can also change typed value
identity. Round trips preserve values subject to these normalization rules.

Alternate integer, float, or string forms, nonzero unused boolean padding,
duplicate or unused string lookup entries, and unused keysets can decode but
fail the canonical check. Their original bytes remain available through
`unpack`; repacking the value chooses the canonical encoding when it succeeds.
Resource limits can prevent repacking even after a successful decode.

Malformed or truncated values, invalid references or UTF-8, duplicate object
keys, oversized counts or extension IDs, unsupported/reserved tags, nested
lookup preambles, and trailing bytes are errors. Integer encodings outside the
supported binary64 integer domain are rejected rather than rounded. The decoder
supports the forms below; this does not establish compatibility with an
external receiver or make the format interchangeable with standard MessagePack.

## Resource limits

Set `SerializerOptions::limits` to lower defaults or raise them within the hard
ceilings:

| Limit | Default | Hard ceiling |
| --- | --- | --- |
| `max_bytes` | 16 MiB | 16 MiB |
| `max_entries` | 65,536 | 1,000,000 |
| `max_depth` | 128 | 256 |

Entry accounting includes metadata, binary and boolean elements, and extension
payloads. Byte limits cover input/output and temporary string, key, and encoding
storage; a small final encoding does not guarantee acceptance. Resource failures
return errors rather than silently truncating data. A semantic `depth_bound`
does not override these limits.

The CLI additionally limits JSON input to 128 MiB. `pack` accepts at most 1,024
simultaneously open JSON objects/arrays, including request and typed-value
wrappers. These JSON levels differ from semantic depth: an object entry adds
three JSON levels per nested typed object. The JSON cap accommodates the core's
depth-256 ceiling but does not raise any serializer limit.

## Advanced: wire format

Lengths and indices inside Superpack use **type-tagged unsigned integers**.
Outer container section lengths use separate ULEB framing. Multibyte numeric
payloads below are big-endian.

| Form | Tag and payload |
| --- | --- |
| Unsigned integer | `00..3f`; `40..7f` + low byte; `e4/e5/e6/e7` + 2/3/4/8 bytes |
| Negative integer | `81..8f` for magnitudes 1–15; `e8/e9/ea/eb` + 1/2/4/8 magnitude bytes |
| Floating point | `ec` + float32 if exact, otherwise `ed` + float64 |
| False / true / null / undefined | `e0` / `e1` / `e2` / `e3` |
| Short UTF-8 string | `c0..df`, low five bits give byte count below 32 |
| Longer string without NUL | `f0`, UTF-8 bytes, zero terminator |
| Longer string with NUL | `f1` + one-byte length, or `f2` + tagged unsigned length, then UTF-8 bytes |
| String reference | `f3` + one-byte lookup index |
| Array | `a0..bf`, or `f4` + one-byte count, or `f5` + tagged count, then values |
| Nonempty boolean array | `91..9f`, or `f6` + one-byte count, or `f7` + tagged count; packed bits, MSB first |
| Object / boolean object | `f8/f9` + tagged keyset index; values in key order / MSB-first bits |
| Date | `ee` + six timestamp bytes |
| Binary | `ef` + tagged unsigned count + a tagged unsigned encoding of each byte |
| Lookup preamble | `fe`, one-byte string count, literal strings, encoded local-keyset array, then value |
| Extension | `ff`, tagged unsigned ID, encoded value payload |

Integral numbers use integer forms only for magnitude `< 0xfffffffffffff800`;
larger doubles use a floating form. Negative zero uses `ec 80 00 00 00`.
Infinities and NaN use float32 forms. Empty arrays use the ordinary array form;
empty objects use the boolean object form. Canonical boolean padding bits are
zero. Tags `fa/fb` (MAPL/BMAPL) are unsupported; `fc/fd` are reserved.

The encoder chooses repeated-string lookup entries automatically, considering
both values and new keysets. Candidates need at least two occurrences and
UTF-16 length × count ≥ 8; the lookup holds at most 255 strings. Object keysets
reuse the first exact ordered key-array match. Callers do not need to construct
lookup metadata themselves.
