# Pinning one traced seed

The removed contract extractor proved a clean program by comparing whole
function bodies against reviewed JavaScript templates. That is too exact for a
captured seed: the tracer emits the same wire format with a different statement
order, hoisted temporaries and split expressions, so a seed fails every contract
with a hash mismatch even though its constants are the ones the capture used.

`shape::generator::extract::pin` takes the other route. Trace the seed once with
`shape trace program`, then pin the handful of places that actually define the
capture. Nothing is compared against a template; each fact is still proved
structurally, and every fact records the byte span it came from.

```sh
# Recover a readable program from the raw seed (no execution of captured code).
shape trace program ~/Desktop/ssx.mod.js --out seed.program.js --thread-cleanup

# First run: pin in-process, save the artifact, and run the operation.
shape headers decode --program seed.program.js --init init.js --save-seed seed.json \
  --input headers.json --out report.json

# Artifact-only shortcut (no payload work).
shape pin --input seed.program.js --init init.js --out seed.json

# Later runs load only the artifact.
shape headers decode --seed seed.json --input headers.json --channels --out channels.json
shape headers encode --seed seed.json --seeds 306906895,3915760865 \
  --input report.json --out headers-again.json
```

A pin, whether from `shape pin` or a first `--program` run, prints one
`pinned <role> at bytes A..B (<sha256>)` line per fact. `shape pin` writes the
`shape-seed-layout/1` document to `--out` (stdout without it); `--program ...
--save-seed seed.json` writes the same artifact beside the operation. `--init`
takes the shipped `detail.init(...)` JavaScript; `--session` takes the
normalized session JSON instead. Exactly one is required.

`--init` compares argument *shapes*, not positions: one 64/65-symbol unique
visible-ASCII string is the alphabet, an eight-word array of `u32` literals is
the key list, and the first remaining string is the token. A nonce, an empty
shuffle list and unrelated arguments are ignored, so a differently ordered
initializer still pins.

## What is pinned, and where

### Source-only seed ID finder

`shape::signals::payload::extract::find_seed_id` (also available through
`shape::generator::extract`) finds the literal emitted in the `-c` header:

```rust,ignore
use shape::signals::payload::extract::find_seed_id;
let found = find_seed_id(&program)?; // no Session, initializer or headers
println!("{}", found.seed_id);
for fact in &found.evidence {
    let literal = &program[fact.start as usize..fact.end as usize];
    println!("{}: {}", fact.role, literal);
}
```

`SourceSeedId` returns the decoded literal string and `Evidence` for the
`header_c_default`, `header_d_default` and `header_z_default` literals. Evidence
uses UTF-8 byte offsets including quotes and SHA-256 of that exact source slice.
The seed ID is distinct from a URL seed token, site ID, source hash, session
transaction token and the per-message seed words.

The finder and `pin` share the same AST context extraction; `DecodeArtifact::compile`
uses it through `pin`. They require exactly one lexical receiver binding with all
three defaults. Unrelated `c` assignments and shadowed bindings cannot supply the
other properties. Dot properties and computed literal string properties, including
parentheses, are accepted. The `d` conditional default must test the same resolved
descriptor returned by its alternate against `void 0`. Conflicting, nonliteral or
compound defaults, dynamic property writes on the candidate, reassigned receiver
bindings, missing contexts and multiple contexts fail with `PinError`.

Unknown/global, nested-member, call-result and alias receivers are not inferred or
pooled together. This is a static source-pattern finder, not execution or general
alias/control-flow analysis. It parses at most 16 MiB; use `shape::run_with_large_stack`
for large traced programs as with the other analysis APIs. It does not verify that
a capture came from the source. Saved artifacts need no new field: their existing
source-pinned `layout.header.c` supplies the same ID on reload.

| Fact | Structural proof |
| --- | --- |
| IVs, counter, double rounds | A 16-element array whose first four entries are numeric literals (masked or not), entries 4–11 are `keys[i] & 0xffffffff`, entries 12–13 are `0`, and entries 14–15 are payload words. The owning function must contain a loop. Rounds are the count of rotate-left-by-16 quarter rounds divided by eight, and must be whole. The counter is the only numeric literal feeding the `state[12]` assignment, below 2<sup>53</sup>. |
| Positional count and keyed names | The report route's `signalFilter === void 0 ? default : signalFilter` branch names the default filter. Its indices select rows of a static descriptor table; a selected row with its own `name` is keyed or named, and a row without one consumes a positional slot. |
| Ordered omitted keysets | The unique `indices.map(index => table[index])` call whose table rows are literal string arrays. Index order and repeats are preserved; the same validation as `extract` applies. |
| Header prefix, chunk size, `c`/`d`/`z` | The one string literal ending in `-` that is concatenated into a call argument; the integer third argument of those calls; and three literal defaults set on one option object. The `d` default may be the supported `descriptor === void 0 ? literal : descriptor` form. |
| Alphabet, keys, token | From `--init` or `--session`, never inferred from browser state. |

The committed capture fixture
(`tests/fixtures/generator/seed-pinned-program.js`) pins at these roles and
spans; they are examples of the shapes above, not magic offsets:

| Role | Bytes |
| --- | ---: |
| `stream_cipher_state` | 99766..100196 |
| `stream_cipher_rounds` | 99663..123817 |
| `signal_filter` | 135739..136505 |
| `descriptor_table` | 481256..485956 |
| `descriptor_routing` | 526909..528248 |
| `keyset_mapping` | 530082..530118 |
| `header_prefix` | 537124..537137 |
| `header_chunk_size` | 543474..543517 |
| `header_c_default` / `header_d_default` / `header_z_default` | 57023..57085 / 57118..57218 / 57240..57243 |

For that seed the pinned geometry is IVs
`[2241456886, 1063953896, 3131140720, 3913805869]`, counter
`3685634438990593`, four double rounds, 77 positional slots, no keyed names, no
omitted keysets, prefix `x-gyjwza5z-` and a 7900-character chunk size.

The same pin, run over four other traced seeds of the same family, reproduces
the cipher geometry those seeds were independently derived with (including 4, 5,
6 and 9 double rounds and both `v & mask` and `mask & v` spellings), while their
positional counts differ (66 and 77) — the layout really is per-seed.

## The artifact and its loader contract

`shape pin` writes one `shape-seed-layout/1` document. It is the static
inventory a loader needs, in the same spirit as the kernel layout bundle: every
fact the program can prove, and no captured payload bytes.

```json
{
  "format": "shape-seed-layout/1",
  "source": {"sha256": "2071ad4b…", "bytes": 568204},
  "session": {"transactionToken": "…", "alphabet": "…64 symbols…",
              "shuffleKey": [], "keys": [1284205162, 1727177461, 1076700704,
              529866104, 628119182, 468536728, 512678569, 746440578]},
  "cipher": {"ivs": [2241456886, 1063953896, 3131140720, 3913805869],
             "counter": 3685634438990593, "double_rounds": 4},
  "header": {"name": "x-gyjwza5z-a", "prefix": "x-gyjwza5z-", "chunkSize": 7900,
             "checksum": "signed-paired-utf16-base36-token-plus-full-payload",
             "b": true, "f": true, "c": "…", "d": "…", "z": "q"},
  "keysets": [],
  "report": {
    "errors": [],
    "positionalSignals": [
      {"slot": 0, "descriptor": 0, "ctor": false, "priority": false},
      …
    ],
    "signals": [],
    "keyedSignals": []
  }
}
```

`report` is the four source channels and nothing else. It is layout, not an
instance: each positional entry names the container slot and the descriptor it
came from, with that descriptor's proven `ctor`/`priority` flags. `errors` is
always `[]` because the errors channel is a generic superpack array with no
per-seed field layout; `signals` and `keyedSignals` list named descriptors when
the build has them. Seeds, per-capture keysets and the opaque `c`/`d`/`z`
context are not part of those four channels.

Decode-time facts live at the root, where a loader can find them without
re-reading the seed: `keysets` (the ordered omitted-keyset table is not on the
wire) and `report.positionalSignals.len()` (the count is not on the wire either).
`session` records the literals the capture was started with; a different page
load can replace it and load the same build facts.

### Loading and the full wire path

```rust,ignore
let layout = SeedLayout::from_json(&saved)?;   // validates format, slots, keysets
let codec = layout.generator()?;               // alphabet, keys, cipher, header

// Decode: custom base64 -> pinned ChaCha-style cipher -> container framing ->
// superpack for errors/signals/encoded values. Positional `extra` bytes stay raw.
let report = layout.decode_headers(&headers)?;
let channels = ChannelReport::from_json(&serde_json::to_vec(&report.channels())?)?;
assert_eq!(layout.decode_payload(&joined_payload)?, report.report());

// Encode: superpack -> ULEB container (errors, positional encoded+extra,
// signals, keyed) -> seed prefix -> cipher -> unpadded custom base64, then
// chunking and the b/f/c/d/z header formatting.
assert_eq!(layout.encode_payload(&channels, report.seeds)?, joined_payload);
assert_eq!(layout.encode_headers(&channels, report.seeds)?, headers);
```

`encode_payload` and `encode_headers` run the same path; the header form adds
splitting at the pinned chunk size and the `b` checksum plus `f`/`c`/`d`/`z`
siblings. Seeds are not part of the four channels, so encode takes them
explicitly; a full report can also be encoded directly with
`Generator::encode_report_headers`.

`SeedLayout::from_json` rejects an unknown format, a noncontiguous slot list, an
over-limit entry count, duplicate preloaded keyset entries, and any profile the
`Generator` would refuse. Loading is a parse plus those checks; there is no Oxc
pass, no contract comparison and no seed re-extraction.

`ChannelReport` is the owned four-channel document (`errors`,
`positionalSignals`, `signals`, `keyedSignals`). `Report::channels()` and
`HeaderReport::channels()` project a decode result to that borrow, so the same
JSON is both a decode output and an encode input.

## Two base64 details the capture makes easy to get wrong

The session alphabet may contain `=`. When it does, `=` is the 32nd *data*
symbol, not padding, and it must be decoded like any other symbol. A captured
payload is unpadded, so a final group of three symbols carries two bytes; a
four-symbol group carries three even when one of the symbols is `=`.

Both properties are visible in the committed capture: the joined `a`+`a0` value
is 9247 symbols, not a multiple of four, and it contains `=`. The checksum header
`b` is the independent witness: it is `hash(token + joined_payload)` rendered as
signed base 36, so a single mistyped or mis-padded symbol fails before the
cipher runs.

## Limits

* Pinning reads one traced program and one session. It does not prove that the
  program is the one that produced a given payload; the `b` checksum and the
  round trip are the evidence.
* Only the shapes above pin. A seed that emits the cipher with a helper call, a
  loop-based quarter round, a computed descriptor table, a non-literal `c`/`d`/`z`,
  or a different base64 alphabet policy is refused with the offending span
  rather than guessed.
* Signal bodies stay opaque. The report keeps each positional entry's raw
  `extra` bytes and empty generic value; decoding the signal values inside them
  needs a per-signal layout, which is separate from this capture-level pin. The
  generator never invents signal values.
* Payloads remain unauthenticated. The checksum and round trip demonstrate
  agreement with this capture, not the origin or meaning of the bytes.
