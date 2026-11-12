# Final reports and session headers

`shape headers encode` accepts final report JSON by default and emits a JSON
header map. `shape headers decode` checks session headers and returns the
final report with `errors`, `positionalSignals`, `signals`, `keyedSignals`,
`seeds`, and `keysets` at the root, plus `header_context`. Its output feeds
directly back into encode. These utilities never collect signals or infer
profiles, positional counts, keysets, tokens, or opaque context.

Supply the full session profile and ordered omitted keysets before running:

```sh
shape headers encode --profile headers-profile.json --input tests/fixtures/sdk/generator/report.json --out headers.json
shape headers decode --profile headers-profile.json --input headers.json --positional-count 3 --keysets keysets.json --out header-report.json
shape headers encode --profile headers-profile.json --input header-report.json --out headers-again.json
cmp headers.json headers-again.json
```

Use caller-owned `headers-profile.json` and `keysets.json` (`[]` for the
synthetic fixture). The [report](../../tests/fixtures/sdk/generator/report.json) contains synthetic values. All
crypto fields, the `-a` header name, chunk policy, and optional transaction token
are explicit [Profile](README.md) facts. Decode needs the exact positional count
and ordered omitted keysets, just as [payload decode](report.md) does. Each flag independently overrides
the saved layout's or full profile's corresponding default; missing facts require the corresponding flags.
Four-channel encode with `--seeds` uses those default keysets (or `[]` when absent); full report JSON
keeps its own keysets and seeds authoritative.

An already-finalized custom-base64 payload uses an explicit input format:

```sh
shape headers encode --profile headers-profile.json --input-format payload --input payload.txt --out wrapped.json
shape headers encode --profile headers-profile.json --input header-report.json --output-format text --out headers.txt
shape headers decode --profile headers-profile.json --input-format text --input headers.txt --positional-count 3 --keysets keysets.json --out from-text.json
```

Payload input is validated, chunked, and wrapped; it is never packed, decrypted,
or encrypted again. It accepts exactly one optional final LF or CRLF. Report
input packs generic channels once through the existing report codec; raw extras
bypass generic packing. Typed primitives, special numbers, sentinel distinctions,
ordered duplicate keyed IDs, ordered keysets, and source depth-100 behavior are
unchanged. Source normalization rules are described in the [report contract](report.md).

## Header-report document

This is a separate extension of the report-only schema:

```json
{
  "errors": [],
  "positionalSignals": [],
  "signals": [],
  "keyedSignals": [],
  "seeds": [1, 2],
  "keysets": [],
  "header_context": {"b": true, "f": true, "c": "", "d": "opaque:descriptor==", "z": "synthetic"}
}
```

Encode also accepts report-only input; it then uses the profile's context, or
no siblings if context is absent. Decode always includes context, even
`{"b":false,"f":false}`, so absence is explicit and survives direct reencoding.
The booleans record **presence** of computed checksum `b` and token `f` headers.
Optional `c`/`d`/`z` strings are opaque and retain empty strings. No sibling
values are invented. The checksum covers the token plus the entire joined
payload, using signed UTF-16-pair base36 hashing. It is not authentication.
`f` must equal the profile token; it never supplies missing profile facts.
Profile and document context must agree exactly when both are supplied.
Reencoding always uses the explicit profile chunk policy.

Existing `payload encode/decode` retain their original report-only schema;
remove `header_context` before supplying a header-report document to those
commands. Existing `encode/decode` retain their opaque byte/header API.

## Raw text scope and validation

Decode defaults to `--input-format auto`: an initial JSON `{` after whitespace
selects JSON; otherwise input is text. `json` and `text` may be selected
explicitly. Text is a **configured-family-only `Name: value` block**. Extract
that family from a full request before calling this utility. HTTP request/status
start lines, unrelated headers (including another prefix), bodies, folding,
controls, blank interior lines, and whitespace before the colon are errors.
Names are case insensitive; duplicates including case variants are errors.
Consistent LF or CRLF and an optional terminating empty line are accepted.
Only surrounding HTTP SP/HTAB is trimmed; colons, equals signs, and interior
bytes remain untouched. Base64 interior whitespace is rejected by the canonical
payload validator. Use JSON to preserve meaningful surrounding spaces in opaque
context; HTTP text treats those spaces as optional whitespace.

Same-family unknown suffixes, noncanonical/gapped numeric chunks, wrong chunk
lengths, checksum/token mismatches, and conflicting context fail before report
decoding. Numeric chunk order handles `a`, `a0`, … `a10`, regardless of map order.
JSON names/case/order may normalize; names and values compare case insensitively
for names and exactly for values. The joined payload is byte exact.

The shared 128 MiB bounded CLI input, 1,024 structural JSON depth preflight,
16 MiB payload/report byte budget, 65,536 header entry cap, bounded header bundle,
and 64 KiB context-value bounds apply. Header names also count toward the bundle
budget; text framing has a bounded additional allowance. Profiles and keysets
files are each limited to 64 KiB. Unknown/duplicate JSON fields and trailing JSON
are rejected. Shared stdin/stdout, atomic writes, no-clobber/`--force`, and
symlink/hardlink alias checks apply to input, output, profile, and keysets files.

## Rust API

```rust,no_run
use shape::generator::{Generator, HeaderReport, Headers, Profile};
# fn example(profile: Profile, report_json: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
let generator = Generator::new(profile)?;
let document = HeaderReport::from_json(report_json)?;
let headers = generator.encode_report_headers(&document)?;
let recovered = generator.decode_report_headers(
    &headers, document.positional_signals.len(), document.keysets.clone(),
)?;
let (payload, context) = generator.decode_finalized_headers(&headers)?;
let wrapped = generator.encode_finalized_headers(&payload, Some(&context))?;
assert_eq!(headers, wrapped);
let raw = Headers::from_text(&headers.to_text())?;
# Ok(()) }
```

`HeaderReport::from(report)` adapts an existing `report::Report`; `.report()`
returns report-only data. Deep typed JSON shares the bounded CLI parser. As with
the existing deep serializer API, use sufficient thread stack for deeply nested
library data (the CLI already runs on its large stack). Header helpers and the
legacy byte API share formatting, joining, checksum, context, and canonical
base64 validation. Header checks precede report decryption.

The portable `tests/generator_header_report.rs` reuses all 53 pinned synthetic
source-factory answers, including depth boundaries, special values, raw extras,
sentinels and ordered keysets. It covers JSON/LF/CRLF and bare payload paths,
metadata preservation, more than ten chunks, strict failures, and file aliases.
Private capture verification belongs only under ignored `.agent-scratch`.
