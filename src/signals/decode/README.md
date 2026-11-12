# Capture decode workflow

`DecodeArtifact` combines the existing pinned payload layout with the per-seed
signal schema and compiled source-format recipes. `compile` runs the existing
finders and source compiler once; `from_json` and `decode` require no original
JavaScript source file or program analysis. Saved tag expressions are revalidated
against their mappings at load. This is the explicit-session source-format adapter, not a
claim of certification by the separate strict wire loader.

`CaptureHeaders` is the input boundary for future capture adapters:

```json
{
  "siteId": "example",
  "headers": { "a": "encoded payload", "b": "checksum" }
}
```

The site ID is a caller-supplied grouping identity, not a header prefix or a
cryptographic build identity. It must match the artifact. Header suffixes may
be a–z or numeric a chunks (`a0`, `a1`, …). The bounded `Headers` type rejects
duplicate keys, case collisions and control characters. The pinned source
supplies the complete prefix; no `x-` convention is assumed. Payload codecs
validate the actual supported header bundle.

`DecodeArtifact` contains `format`, `siteId`, `layout`, `schema` and
`floatByteOrder`. The layout retains cipher settings, ordered keysets,
positional slots, header policy and session defaults. The schema retains
canonical field names, masks, routing and source-compiled programs. Source
hashes connect these documents. A session override replaces the complete
`Session` (token, alphabet, shuffle key, eight keys, optional nonce), leaving
build facts intact. Different seed builds need different artifacts.

`DecodedPayload.positional_signals` is the slim name-to-fields map. Its
`DecodeMetadata` holds original report bytes, detailed `NamedSignal` records,
missing/decoded counts and exact writer verification. Every present positional
signal must have an unambiguous named projection and an exact codec re-encode.
The reconstructed report and full header bundle must also match. Unsupported
present signals fail; missing signals are omitted from the slim map. An unknown
float byte order fails when a native float field is encountered.

Source-proven boolean tags appear as JSON `true`/`false`, and nullable marker
arms as `null`; genuine byte counters and number/string payloads retain their
data values. Each mapped wire leaf saves its build-specific `byte_mapping`
(concrete byte, logical value, condition and source span). Mapping applies only
to that leaf and selected arm, including within repeated records. Verification
inverts the projected logical tags and encodes the rebuilt wire tree before
checking the captured bytes. This is verification of the projection; editing
the output JSON is not an input to an encoder. Older artifacts without these
optional mappings retain their original numeric projection.

`source_compiler` and `source_counts` implement source-format compilation and
count verification.
The [CLI](../../../cli/decode.rs) traces raw seeds in process, handles files and
publishes output only after successful decoding. See the
[synthetic regression inputs](../../../tests/fixtures/sdk/decode/README.md).

Fresh naming version 2 groups packed-byte families into heterogeneous fixed
arrays and flattens whole single-byte flag messages. No-write fields are omitted;
written null markers remain null. Certified scalar two-component null branches
collapse to null while metadata retains both inverses. See the
[projection contract](../../../docs/signals/schema.md#readable-projection-version-2)
for migration paths and legacy artifact behavior.
