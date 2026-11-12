# Calling the generator

Two calls define the lifecycle: **pin once** per seed, then **load** the saved
layout for every decode or encode. Both are public `shape::generator` API; the
`shape` CLI wiring lives outside `src/generator/cli.rs` and is shown at the end.

## 1. Pin once

```rust
use shape::generator::extract::{pin, session_from_initializer};

// The shipped `detail.init(token, alphabet, [], keys, ...)` call. Literal
// shapes are matched, never executed: one 64/65-symbol alphabet, an eight-word
// key array and the leading token string.
let session = session_from_initializer(&init_source)?;

// The traced program (`shape trace program` output). Every pinned fact keeps
// its byte span in `pinned.evidence`.
let pinned = pin(&traced_program, &session)?;

// One shape-seed-layout/1 document: cipher, header policy, session literals,
// keysets and the report layout.
std::fs::write("seed.json", pinned.to_json_pretty())?;
```

`pin` fails with a byte span instead of guessing when the cipher state, report
route, descriptor table, keyset mapping or header literals are not the
supported shapes.

## 2. Load and run

```rust
use shape::generator::extract::SeedLayout;

let layout = SeedLayout::from_json(&std::fs::read("seed.json")?)?;   // validate
let codec = layout.generator()?;                                     // build

// Decode: base64 -> ChaCha-style cipher -> container -> superpack.
let report = layout.decode_headers(&headers)?;
let channels = report.channels_owned();       // exactly the four source keys
let payload = layout.decode_payload(&joined_payload)?;

// Encode: superpack -> container -> seed prefix -> cipher -> base64,
// then chunking and the b/f/c/d/z header bundle.
let payload_again = layout.encode_payload(&channels, report.seeds)?;
let headers_again = layout.encode_headers(&channels, report.seeds)?;
```

Seeds are not part of the four channels, so encode takes them explicitly.
Positional signal bodies are caller-supplied `extra` bytes; the generator packs
their `encoded` half with superpack but never interprets the body.

## API surface

### Extraction (`shape::generator::extract`)

| Item | Signature | Purpose |
|---|---|---|
| `session_from_initializer` | `fn(&str) -> Result<Session, PinError>` | Literal session material from one `detail.init(...)` call, matched by shape |
| `pin` | `fn(&str, &Session) -> Result<PinnedSeed, PinError>` | Structural pinning of one traced program |
| `PinnedSeed` | `{ layout: SeedLayout, evidence: Vec<Evidence> }` | Artifact plus per-fact spans; `to_json_pretty()` writes `shape-seed-layout/1` |
| `SeedLayout` | `{ format, source, session, cipher, header, keysets, report }` | The saved artifact |
| `SeedLayout::from_json` | `fn(&[u8]) -> Result<Self, Error>` | Parse and validate format, slots, keysets, profile |
| `SeedLayout::generator` | `fn(&self) -> Result<Generator, Error>` | Build the codec, no source parse |
| `SeedLayout::encode_payload` | `fn(&self, &ChannelReport, [u32; 2]) -> Result<String, Error>` | Bare finalized payload |
| `SeedLayout::encode_headers` | `fn(&self, &ChannelReport, [u32; 2]) -> Result<Headers, Error>` | Chunked header bundle |
| `SeedLayout::decode_payload` | `fn(&self, payload: &str) -> Result<Report, Error>` | Full report from a bare payload |
| `SeedLayout::decode_headers` | `fn(&self, &Headers) -> Result<HeaderReport, Error>` | Full report from a header bundle |
| `ReportLayout` | `{ errors, positionalSignals, signals, keyedSignals }` | Four-channel layout; positional entries carry `slot`/`descriptor`/`ctor`/`priority` |
| `Session`, `Source`, `Evidence` | serde structs | Session literals, program digest, span evidence |

### Codec (`shape::generator`)

| Item | Signature | Purpose |
|---|---|---|
| `ChannelReport` | four fields, `Deserialize` | The owned four-channel document shared by decode output and encode input |
| `ChannelReport::from_json` | `fn(&[u8]) -> Result<Self, Error>` | Bounded typed parse of exactly those four keys |
| `Report::channels_owned` / `HeaderReport::channels_owned` | `fn(&self) -> ChannelReport` | Project a decode result to the four keys |
| `Generator::encode_channels` | `fn(&self, &ChannelReport, [u32; 2], Vec<Vec<String>>) -> Result<String, Error>` | Payload path without a layout |
| `HeaderReport::from_channels` | `fn(&ChannelReport, [u32; 2], Vec<Vec<String>>, Option<HeaderContext>) -> Self` | Header path without a layout |
| `Generator::encode_report_headers` / `decode_report_headers` | report ↔ headers | Existing report-level API |
| `Generator::encode_headers` / `decode_headers` | payload ↔ headers | Existing raw-bytes API |
| `PayloadCodec` | `encode_bytes` / `decode` / `encode_report` / `decode_report` | The layers below headers: cipher + base64 + container + superpack |

## CLI wiring

The generator commands are top-level in `shape.rs`, next to `deob`, `report`,
`program`, `seeds` and `trace`:

```rust,ignore
"pin" => shape::generator::cli::run_pin(args),
"headers" => shape::generator::cli::run_headers(args),
"payload" => shape::generator::cli::run_payload(args),
"encode" => shape::generator::cli::run_encode(args),
"decode" => shape::generator::cli::run_decode(args),
"pack" => shape::generator::cli::run_pack(args),
"unpack" => shape::generator::cli::run_unpack(args),
"gen" => shape::generator::cli::run(args),   // legacy alias
```

Each family owns its own clap parser in `generator::cli`, so `shape headers
--help` reads as `shape headers`, not `shape gen headers`. `shape gen <command>`
still forwards to the same functions and is kept as a deprecated alias.

```sh
# first run: pin the program in-process and save the artifact as a side effect
shape headers decode --program program.js --init init.js --save-seed seed.json \
  --input headers.json --out report.json

# artifact-only shortcut (no payload work)
shape pin --input program.js --init init.js --out seed.json
shape pin --input program.js --session session.json --out seed.json

# every capture: the artifact supplies cipher, header policy, count and keysets
shape headers decode --seed seed.json --input headers.json --out report.json
shape headers decode --seed seed.json --input headers.json --channels --out channels.json
shape headers encode --seed seed.json --seeds W1,W2 --input channels.json --out headers.json
shape payload decode --seed seed.json --input payload.txt --channels --out channels.json
shape payload encode --seed seed.json --seeds W1,W2 --input channels.json --out payload.txt

# raw byte/header boundary also accepts the artifact
shape decode --seed seed.json --input headers.json --out payload.json
shape encode --seed seed.json --input payload.json --out headers.json
```

Every encode/decode command takes exactly one of `--profile`, `--seed` or
`--program`. `--program` pins on that run from the traced program plus
`--init`/`--session` (one is required), and `--save-seed PATH` writes the
artifact so later runs switch to `--seed`. With
`--seed` the layout's positional count and keysets are the defaults. Full profiles
also supply these defaults when their optional `positional_count` and `keysets`
fields are present, independently;
`--positional-count` and `--keysets` still override for another capture of the
same build. `--channels` projects decode output to the four keys, and
`--seeds` makes that same document an encode input (seeds are per capture, so
they are not in the artifact).

A pin, whether from `shape pin` or the first `--program` run, prints one
`pinned <role> at bytes A..B (<sha256>)` line per fact to stderr, so the
artifact's provenance is visible when it is written.
