<h1 align="center">Shape</h1>

Shape decodes captured request headers into named signal fields. You can
inspect those fields, change supported values, and encode the full decoded
payload back into headers while retaining its other signals and report channels.

For a new capture, trace its VM seed into readable JavaScript and use that
program with the matching session to compile a decode artifact. Save the
artifact to decode later requests from that build and session without repeating
the source analysis. You can also deobfuscate the kernel and name recovered
functions to investigate how signals are collected.

Use the CLI for one-off work with files, or load an artifact once in the SDK
for repeated calls from your application. Both use the same engine:

| | Use it to | Start here |
| --- | --- | --- |
| **[Shape CLI](#shape-cli)** | Investigate a capture from the terminal: deobfuscate, trace, name functions, extract signal layouts, decode and encode | `cargo build --release --locked --bin shape` |
| **[SDK](#sdk)** | Do the same from your application in Node.js/TypeScript, Python, Go, Rust or C | `scripts/build-sdk.sh` |

Before running any `shape` command, follow the [CLI build and PATH setup](#build-once)
from the repository root.

The table below shows where each operation is available. A decode artifact
saved by the CLI loads in every SDK, so you can prepare a capture in the
terminal and reuse the result in your application.

| Task | CLI | SDK command |
| --- | --- | --- |
| Deobfuscate a kernel | `shape deob` | `deob` |
| Compile a kernel artifact | `shape report` | `report` |
| Inspect VM properties and operations | `shape kernel show`, `shape ops list`, `shape ops show` | `kernel.show`, `ops.list`, `ops.show` |
| Recover the linked program | `shape trace program` | `trace.program` |
| Walk or clean one thread | `shape trace thread`, `shape trace cleanup-thread` | `trace.thread`, `cleanup.thread` |
| Name functions and collectors | `shape naming threads`, `collectors`, `plan`, `inventory` | `naming.*` |
| Compare captures | `shape trace corpus`, `shape naming corpus` | |
| Extract the signal schema | `shape signals` | `signals.schema` |
| Pin a session layout | `shape pin` | `pin` |
| Compile a decode artifact | `shape decode --save-artifact` | `artifact.compile` |
| Decode headers into named fields | `shape decode --artifact` | Generator `decode`, `explain` |
| Encode edited fields back into headers | — (use the SDK; the CLI encodes framed payloads, not named fields) | Generator `encode` |
| Look up what a signal means | `shape show signals` | |
| Report and payload framing | `shape payload`, `shape headers`, `shape pack`, `shape unpack` | |

**Contents:** [Scope and limitations](#scope-and-limitations) ·
[Choose a path](#choose-a-path) · [How it fits together](#how-it-fits-together) ·
[Inputs](#inputs) · [Shape CLI](#shape-cli) · [SDK](#sdk) ·
[Decoded signals and editing](sdk/signals.md) ·
[SDK performance](#performance-in-the-sdk) · [Troubleshooting](#troubleshooting) ·
[Documentation](#documentation)

## Scope and limitations

Shape analyses captured Shape-style VM kernels and the request headers they
produce. Behavior that depends on runtime values may stop a trace.

Check the trace's `complete` or `stopped` status before using its output. The
signal schema lists `unidentified`, `ambiguous` and `refused` slots under
`diagnostics`, so you can see where analysis could not establish a result.
Decoding a field does not guarantee that every edit can be encoded: the
compiled wire recipes must support the requested value.

Coordinates, handler keys, salts and layouts belong to a particular build.
Keep the decode artifact with its matching site, build and session; re-run
analysis for a different build. Unsupported edits are refused, including edits
whose encoding context cannot be reconstructed.

Use `shape seeds fetch` to download launchers and kernels; capturing browser
traffic is a separate step. Tracing large captures can be expensive, so use
release builds, step limits and kernel-artifact caches for repeat work.

## Choose a path

| If you want to… | Start with |
| --- | --- |
| Understand the project from one capture | [Deobfuscate and inspect a kernel](#deobfuscate-and-inspect-a-kernel) |
| Recover readable code from a seed | [Trace the program](#trace-a-whole-program) |
| Decode captured request headers | [Decode headers into named fields](#decode-headers-into-named-fields) |
| Build a decode artifact from your own capture | [Your own capture](#recover-code-and-save-a-decode-artifact) |
| Integrate decoding into an application | [SDK](#sdk), then your [language guide](#documentation) |
| Edit signal values and re-encode headers | [Decoded signals and editing](sdk/signals.md) |
| Investigate a stopped or slow trace | [Tracing](docs/tracing/trace-cli.md) and [Troubleshooting](#troubleshooting) |
| Work on the engine or the CLI | [Architecture](docs/architecture.md) and [cli/README.md](cli/README.md) |

## How it fits together

Shape parses captured JavaScript and WebAssembly, then traces a Rust intermediate representation to recover readable code.

Start with the raw VM seed, the initializer that started the session, and the
request headers from the same capture. The seed supplies the program, the
initializer supplies session settings, and the headers carry the values to
decode:

```text
seed.raw.js ── deob ────────────────► readable kernel JavaScript
            └─ trace program ───────► traced program (linked JavaScript)
init.js ───── session.from_initializer ─► session (token, alphabet, keys)
                                          │
traced program + session + site ID ─ artifact.compile ─► decode artifact
                                                          │
headers.json ──────────────────────────── decode ◄─────────┘
                                           │
                              named fields in positionalSignals
                                           │
                              explain one signal, edit fields
                                           │
headers.json ◄─────────────────────────── encode
```

Stages are independent. You can read a kernel, trace one thread, or extract a
signal schema without ever decoding headers, and you can decode headers
repeatedly from one saved artifact without tracing again.

| Document | Produced by | Contains | Reused by |
| --- | --- | --- | --- |
| Kernel artifact | `report`, `deob --report` | Source-free kernel facts (entry, tables, bytecode, module bytes) | `trace thread --report` |
| Traced program | `trace program` | Linked JavaScript for every reachable thread | `signals`, `naming`, `pin`, `artifact.compile` |
| Session | `session.from_initializer`, `pin` | Transaction token, alphabet, shuffle key, cipher keys, optional nonce | `pin`, `artifact.compile`, every decode |
| Decode artifact | `artifact.compile`, `decode --save-artifact` | Layout, cipher settings, session defaults, compiled signal recipes | Every SDK generator, `shape decode --artifact` |
| Compact schema | `artifact.compact_schema`, generator `schema` | Public descriptive view of a decode artifact | Inspection only; it cannot decode |

Save the decode artifact when you want to use the analysis in another process
or language. The CLI and all five SDKs load the same bytes without needing the
source files.

Each document is versioned (`shape-kernel-artifact/v1`,
`shape-decode-artifact/1`, `shape-seed-schema/1`). A document from a different
version is rejected rather than guessed at.

## Inputs

The workflow below uses four files from one build and session. Save the
launcher and kernel, capture the browser's headers, and parse the session from
the launcher.

| Input | File | Comes from |
| --- | --- | --- |
| Launcher | `seeds/<token>.async.js` | Browser Network panel, or `shape seeds fetch` |
| Kernel | `seeds/<token>.raw.js` | The kernel URL in that launcher; `fetch` stores it |
| Headers | `headers.json` | The captured request headers, reduced to suffix keys |
| Session | `session.json` | The launcher's `detail.init(...)` call |

### Launcher and kernel

`<token>` is the capture token in the launcher's kernel URL. A fetch writes
exactly two sibling files:

```text
seeds/
  <token>.async.js   launcher: kernel URL, detail.init(...), boot code
  <token>.raw.js     kernel: the VM source analysed by shape
```

Use the registry instead of reconstructing URLs:

```sh
shape seeds sites
shape seeds fetch --site NAME --out seeds
```

SDK integrations can fetch bytes with their own HTTP client and pass them to parsing or import operations.

Use `fetch` to download launchers and kernels. `parse` classifies an already
saved file and prints its seed and resolved kernel URL, and `import` stores a
downloaded kernel without a request:

```sh
shape seeds parse saved.async.js \
  --base-url 'https://assets.example.test/launcher.js?async'
shape seeds import saved.raw.js \
  --launcher saved.async.js --out seeds
```

An abbreviated launcher (values replaced with placeholders):

```js
(function () {
  var kernel = "https://assets.example.test/kernel.js?seed=<token>&X-…--z=q";
  addEventListener("<event-name>", function (event) {
    event.stopImmediatePropagation();
    event.detail.init("<transaction-token>", "<alphabet>", [],
      [ /* eight keys */ ], "<nonce>", /* … */);
  });
})();
```

The kernel is minified JavaScript; `shape deob` reads it and
`shape trace program` turns it into the traced program.

```js
(function () {
  var document = window.document; /* … VM tables, handlers, dispatch … */
  /* … */
})();
```

`shape seeds parse` reports the same identity the launcher carries:

```text
saved.async.js: async launcher
  seed        <capture-token>
  kernel url  https://assets.example.test/kernel.js?seed=<capture-token>
```

The launcher is also the initializer. Save it as `init.js`, or pass it directly
to `session.from_initializer`; `shape pin --init` accepts the whole launcher.
No separate init extraction is required.

### Headers

The browser sends one response header per field, prefixed with the capture's
header prefix (often `x-…-`). The normalised file keeps only the suffix and a
site label:

```json
{
  "siteId": "example-site",
  "headers": {
    "a": "ZCcDA5M-Wp6u…",
    "a0": "_kDqP0McfRa…",
    "b": "-upu45s",
    "c": "AIAS-MKh…",
    "d": "ADYAhICB…",
    "f": "A71QDMOh…",
    "z": "q"
  }
}
```

- The real names are the capture's prefix plus the suffix, commonly
  `x-…-a`, `x-…-a0`, and so on. The decode artifact supplies the prefix; `a0`,
  `a1`, … are numeric chunks when a value is split across headers.
- `siteId` is a label for this capture, not an HTTP header. It must match the
  artifact.
- Values are strings, including empty ones; optional fields may be absent.
- `shape seeds` does not produce headers. Copy them from the same browser
  request (Network panel response headers), or use an existing capture.
- `shape decode --artifact … --headers …` reads exactly this shape.

### Session

The launcher's `detail.init(...)` call carries the session: transaction token,
alphabet, shuffle key, eight cipher keys and an optional nonce. The SDK parses
the saved launcher:

```python
import shape
session = shape.session_from_initializer(open("seeds/<token>.async.js").read())
```

The CLI's `shape pin --init seeds/<token>.async.js` parses the same file and
writes a `shape-seed-layout/1` document; its `session` member is the session
document:

```sh
shape pin --input reports/program.js \
  --init seeds/<token>.async.js --out reports/seed.json
python3 -c 'import json; json.dump(json.load(open("reports/seed.json"))["session"], open("reports/session.json", "w"))'
```

The values below are illustrative placeholders; use the parsed session
unchanged:

```json
{
  "transactionToken": "<token>",
  "alphabet": "<at least 64 symbols>",
  "shuffleKey": [],
  "keys": [111111111, 222222222, 333333333, 444444444,
           555555555, 666666666, 777777777, 888888888],
  "nonce": "<optional>"
}
```

The session must belong to the same launcher, kernel and headers. When supplying
a replacement session, pass its complete token and cipher material; individual
fields are not merged with the previous session.

### From inputs to the generator

This sequence prepares an artifact and uses it to decode another request.
Deobfuscation and function naming help with investigation; artifact preparation
needs the complete traced program with mechanical cleanup. You can go straight
to tracing the raw seed when decoding is your goal.

```sh
# 1. Read the kernel (and optionally inspect the VM).
shape deob seeds/<token>.raw.js --output reports/kernel.js

# 2. Trace it. Check reports/trace.json for "status": "complete".
shape trace program seeds/<token>.raw.js --thread-cleanup \
  --json --out reports/program.js --report-dir reports/cache \
  > reports/trace.json

# 3. Parse the launcher and compile the decode artifact (the generator).
shape pin --input reports/program.js \
  --init seeds/<token>.async.js --out reports/seed.json
python3 -c 'import json; json.dump(json.load(open("reports/seed.json"))["session"], open("reports/session.json", "w"))'
shape decode --seed reports/program.js --input-kind program \
  --headers headers.json --session reports/session.json \
  --float-byte-order little --save-artifact reports/artifact.json \
  --out reports/decoded.json

# 4. Inspect the signal schema and, optionally, rename functions.
shape signals reports/program.js --json > reports/schema.json
shape naming threads reports/program.js \
  --rename --out reports/named.js

# 5. Reuse the generator without the seed, session or source files.
shape decode --artifact reports/artifact.json \
  --headers next-headers.json --out reports/decoded-next.json
```

- `reports/schema.json` is the **field schema**: names, routing and layouts for
  this build. It describes signals; it does not decode by itself.
- The **decode artifact** holds the layout, cipher settings, session defaults
  and executable wire recipes. `--save-artifact` and
  `artifact.compile` produce it; `decode --artifact` and every SDK
  `Generator.load` consume it.
- Renaming (`naming threads --rename`) is optional and cosmetic. It does not
  change recipes, artifacts or header bytes.
- In the SDK, `Generator.decode` returns named fields, `explain` shows one
  signal, and `encode` turns edited fields back into headers. The CLI's
  `shape encode` handles framed payloads, not named fields.
- To inspect instead of decode: `kernel show`, `signals`, `signals show` and
  `show signals` print the VM, the schema and the catalogue.

## Shape CLI

One binary, `shape`, covers the whole path from a raw capture to named signal
values. The walkthroughs use files committed to this repository, so most of
them can be tried without a capture of your own; the steps that need a real
capture say so.

<a id="build-once"></a>

### Build

Requires **Rust 1.92+** and a C toolchain for linking. Run from the repository
root:

```sh
cargo build --release --locked --bin shape
export PATH="$PWD/target/release:$PATH"
mkdir -p reports
shape --help
```

This builds `target/release/shape` and adds it to PATH for the current shell and
its child processes; repeat the export in each new shell.
`shape --help` lists every command, and
`shape <command> --help` shows its options.

- **Use a release build.** Debug builds are far slower on real captures.
- **`--locked` builds the exact dependency versions in `Cargo.lock`.** The first
  build resolves and downloads crates; later builds reuse Cargo's cache.
- **Outputs go to `reports/`.** That directory is Git-ignored; committed
  fixtures and examples live under `tests/fixtures/`.
- **File outputs use atomic writes.** Existing outputs are refused unless the
  command supports and receives `--force`; input/output path aliasing is rejected.
  Choose a fresh output path to keep earlier results.

### Deobfuscate and inspect a kernel

```sh
shape deob tests/fixtures/kernels/javascript/kernel.raw.js \
  --output reports/kernel.js
```

Open `reports/kernel.js` to read the dispatcher and VM handlers. To look at the
VM's operation table instead of the whole file:

```sh
shape ops list tests/fixtures/kernels/javascript/kernel.raw.js
shape ops show tests/fixtures/kernels/javascript/kernel.raw.js --op 12
shape kernel show tests/fixtures/kernels/javascript/kernel.raw.js
```

- `ops list` prints the catalogue of operation handlers (241 in this kernel).
- `ops show --op KEY` prints one handler with its source and analyses.
- `kernel show` prints the tracer's input: the entry point, operation table,
  string table and bytecode.

`shape report <source> -o reports/kernel-artifact.json` saves the same facts as
a source-free kernel artifact that `trace thread --report` can reuse. This
document describes the VM for tracing; decoding named signals requires the
separate decode artifact built from the recovered program and session.

To compare handlers across several captures instead of reading one:

```sh
shape ops classify reports/a.raw.js reports/b.raw.js
```

It labels each handler as a JavaScript operation, VM machinery, fused, or
unrecognized across the supplied seeds.

<a id="trace-a-whole-program"></a>

### Trace the program

`trace program` walks every reachable VM thread and links the result into one
JavaScript file:

```sh
shape trace program tests/fixtures/kernels/javascript/kernel.raw.js \
  --thread-cleanup --json --out reports/program.js --report-dir reports/cache \
  > reports/trace.json
```

This takes a few seconds and recovers 602 functions from the fixture.

- **Check `reports/trace.json` for `"status": "complete"`** before continuing.
  A complete result has an empty `result.functions.stopped` list and
  `result.javascript.incomplete` set to `false`.
- A partial trace exits with code `3` and can contain throwing stubs. Inspect
  `result.functions.stopped` to see what needs resolving.
- `--report-dir` is a cache directory for compiled kernel artifacts. Reusing a
  cache makes repeated traces of the same seed faster; it does not change the
  result.
- `--thread-cleanup` removes cleanup-safe helper scaffolding from each thread
  before linking; omit it to retain that per-thread scaffolding. Independently,
  `--no-cleanup` skips whole-program finishing after linking.

To recover a single function, or to clean one up on its own:

```sh
shape trace thread tests/fixtures/kernels/javascript/kernel.raw.js \
  --pc 0 --state 0 --initialslots 2,0,3,1 --out reports/thread.js
shape trace cleanup-thread reports/thread.js --out reports/thread-clean.js
```

The entry `pc`, `state` and initial slots come from `kernel show`. A saved
kernel artifact can replace the raw source: `--report reports/kernel-artifact.json`.

To compare which functions are reachable across captures:

```sh
shape trace corpus reports/a.raw.js reports/b.raw.js --json
```

### Name functions and read the signal schema

Both commands read the traced program:

```sh
shape signals reports/program.js --json > reports/schema.json
shape naming threads reports/program.js \
  --rename --out reports/named.js > reports/naming.json
```

- `reports/schema.json` holds every recognized signal by canonical name, with
  its position, routing and typed fields. The fixture yields 64 signals.
  `--layouts-only` adds projected layouts for unidentified slots.
- `reports/named.js` is the program with recovered functions renamed.
  `naming collectors`, `naming plan` and `naming inventory` report collector
  routing, proposed names and the function index without editing the program.
- `signals show`, `signals inputs` and `signals contracts` read generator-side
  details: descriptor routing, composed inputs, and the typed wire contract per
  emission.

To compare naming across seeds or against a reviewed baseline:

```sh
shape naming corpus reports/a.raw.js reports/b.raw.js --out reports/naming-corpus
```

To see what signals mean, with no program or capture at all, try these four
catalogue cards. Run `shape show signals` without a name to list every card.
The catalogue is source-independent and describes what a signal can collect.
The JSON blocks below show the cards' committed public example data, not values
observed by running these commands against your browser. All four cards report
`size.kind: "unavailable"`: exact encoded sizes require a matching capture;
example data is not a byte measurement.

**DeviceMemory — scalar.** Reads the browser-reported `navigator.deviceMemory`
number, or `null` when the property is missing. This is the catalogue's example
`data` object:

```sh
shape show signals DeviceMemory --json
```

```json
{
  "deviceMemory": 32
}
```

**NavigatorPlugins — array of records.** Summarises up to five readable plugins;
each entry has a name prefix (up to 12 characters) and a hash of its details.
`pluginsLength` is the full plugin count, which may exceed the number of
summarised entries. The complete example `data` object preserves the array:

```sh
shape show signals NavigatorPlugins --json
```

```json
{
  "plugin": [
    {
      "detailsHash": -1716625998,
      "namePrefix": "WebKit built"
    },
    {
      "detailsHash": 1387368353,
      "namePrefix": "Microsoft Ed"
    },
    {
      "detailsHash": 2126051627,
      "namePrefix": "Chromium PDF"
    },
    {
      "detailsHash": 1061675194,
      "namePrefix": "Chrome PDF V"
    },
    {
      "detailsHash": 975495497,
      "namePrefix": "PDF Viewer"
    }
  ],
  "pluginsLength": 5
}
```

**UserAgentData — nested objects.** Requests User-Agent Client Hints, including
brand/version records and a nested `flags` object for `mobile` and `wow64`.
Missing or rejected hints become `null`; browsers without the API emit no
payload. This focused excerpt of example `data` keeps `architecture`,
`brands`, `flags` and `platform`; other hints are omitted:

```sh
shape show signals UserAgentData --json
```

```json
{
  "architecture": "arm",
  "brands": [
    {
      "brand": "Google Chrome",
      "version": "156"
    },
    {
      "brand": "Chromium",
      "version": "156"
    },
    {
      "brand": "Not:A-Brand",
      "version": "8"
    }
  ],
  "flags": {
    "mobile": false,
    "wow64": false
  },
  "platform": "macOS"
}
```

**BrowserApiOwnPropertyFlags — named booleans.** Checks for `clipboard` on
`Navigator.prototype`, `ServiceWorker` and `ApplePaySession` on the global
object, and `WebkitOverflowScrolling` on `CSSStyleDeclaration.prototype`.
Only own properties count. The complete example `data` object exposes each
result as a boolean:

```sh
shape show signals BrowserApiOwnPropertyFlags --json
```

```json
{
  "ApplePaySession": false,
  "ServiceWorker": true,
  "WebkitOverflowScrolling": false,
  "clipboard": true
}
```

### Decode headers into named fields

A decode artifact holds the recipes and session settings for matching captured
headers. Try decoding with the committed synthetic artifact and capture:

```sh
shape decode \
  --artifact sdk/c/tests/fixtures/artifact.json \
  --headers sdk/c/tests/fixtures/capture.json
```

```json
{
  "siteId": "example",
  "positionalSignals": {
    "ConstantEmptyString": { "constant": "" },
    "DeviceMemory": { "deviceMemory": 32 },
    "StorageAccessApiPresence": {
      "hasStorageAccess": true,
      "requestStorageAccess": false
    }
  }
}
```

The output above is trimmed to the named fields; the full document also carries
the report channels, seeds and metadata. Add `--out FILE` to save the full
document for later edits and `--verify` to require byte-identical re-encoding.

- **`shape decode` has two modes.** `--seed` traces and compiles recipes from a
  raw or traced program; `--artifact` loads saved recipes and does no source
  analysis. Only the second mode is cheap enough to run per request.
- **`--float-byte-order` records how the collecting browser wrote native
  floats.** It is required when the seed emits them, and is saved in the
  artifact. `little` and `big` are the only values; guessing produces decode
  failures rather than wrong values.

<a id="recover-code-and-save-a-decode-artifact"></a>

### Your own capture: from seed to artifact

Supply matching files from the same build and session. [Inputs](#inputs) shows
what each file contains and how to obtain it.

| File | Contents |
| --- | --- |
| `seeds/<token>.async.js` | Launcher: kernel URL and the `detail.init(...)` session call |
| `seeds/<token>.raw.js` | Kernel: the captured VM source |
| `session.json` | The session parsed from the launcher, or an existing parsed session |
| `headers.json` | The capture document `{"siteId":"example","headers":{"a":"…","b":"…"}}` |

The [SDK workflow](sdk/README.md#workflow) shows the same path in code, and
[`shape seeds`](docs/deob/seeds.md) covers acquisition and import.

```sh
# 1. Trace the seed. Check reports/trace.json for "status": "complete".
shape trace program seed.raw.js --thread-cleanup \
  --json --out reports/program.js --report-dir reports/cache > reports/trace.json

# 2. Compile recipes, decode named fields, and save the reusable artifact.
shape decode --seed reports/program.js --input-kind program \
  --headers headers.json --session session.json --float-byte-order little \
  --save-artifact reports/artifact.json --out reports/decoded.json

# 3. Decode another request without tracing or loading source again.
shape decode --artifact reports/artifact.json \
  --headers next-headers.json --out reports/decoded-next.json
```

- `reports/decoded.json` contains named `positionalSignals` like the output
  above, along with the full report context needed to preserve the capture
  during edits.
- **The saved artifact works in every SDK.**
- Choose `little` or `big` to match the capturing browser's float byte order.
- `--save-artifact` verifies signal and header re-encoding. An ordinary decode
  verifies only when `--verify` is given.
- For `next-headers.json`, keep the same seed and session pairing, or pass the
  matching replacement with `--session next-session.json`.
- The session and the artifact are matched as a whole. Do not mix a token from
  one capture with keys from another.

For explicit source-format analysis without lifecycle certification, opt into
`--direct`. The same matching program, session and capture headers are required:

```sh
shape decode --direct --seed reports/program.js \
  --session session.json --headers headers.json --float-byte-order little \
  --verify --save-artifact reports/direct-artifact.json --out reports/decoded.json
```

With `--direct`, `--seed` defaults to a traced program; use `--input-kind raw`
to trace a raw seed first. Without `--direct`, the existing raw-seed default
and strict refusal behavior remain. Direct mode bypasses lifecycle and strict
algorithm certification; it does not certify strict consumer support. Unsupported
source constructs can still fail. `--verify` requires exact signal and header
re-encoding, and saving an artifact always performs that verification.

Direct provenance persists in the artifact, so later `--artifact` loads use the
recorded mode automatically. `--direct` requires `--seed` and conflicts with
`--artifact` and `--schema-only`; it cannot reinterpret a saved strict artifact.
See the [decode checkpoint](docs/signals/decode-checkpoint.md) and
[SDK options](sdk/README.md#workflow) for the same opt-in in library callers.

### Report and payload framing

Use this workflow to inspect and rebuild a captured report container. Start with
the recovered program and the initializer from the same capture/session:

```sh
shape pin --input reports/program.js --init init.js --out reports/seed.json
shape headers decode --seed reports/seed.json --input headers.http.json --out reports/report.json
shape headers encode --seed reports/seed.json --input reports/report.json --out reports/rebuilt-headers.json
```

Here `headers.http.json` is a raw captured HTTP header object containing all six
full header names and their values, without the SDK's `{siteId, headers}` wrapper.
Decoding writes the framed report to `reports/report.json`; encoding rebuilds
the headers from that report. For named signal fields, use the earlier
[`shape decode` workflow](#decode-headers-into-named-fields) or the SDK.

The pinned seed layout saves framing and session settings for reuse with this
capture; the compiled decode artifact additionally supports named signal decoding.

| Command | Converts |
| --- | --- |
| `shape payload encode` / `decode` | Report JSON to and from a bare finalized payload |
| `shape headers encode` / `decode` | Report framing to and from a header bundle |
| `shape encode` / `decode --profile` | Opaque payload bytes to and from session headers |
| `shape pack` / `unpack` | Typed values without encryption |

For advanced bare-payload work, run `shape payload encode --help` and
`shape payload decode --help`; the pinned seed also supplies their framing settings.

Commands refuse to overwrite existing output. Choose new output paths, or use
`--force` where it is supported.

### Command reference

Run `shape <command> --help` for the complete option list. The binary exit
status is `0` on success, `2` for usage errors, `3` for an incomplete trace,
and `1` for an operational failure.

**Recovery and inspection**

| Command | Purpose |
| --- | --- |
| `deob <source>` | Deobfuscate one JavaScript source; `--output`, `--report`, `--verbose` |
| `report <source>` | Compile a source-free kernel artifact; `--input-kind`, `--output` |
| `kernel show <source>` | Print the tracer-input properties bundle; `--format json\|markdown` |
| `ops list <source>` | List proven operation handlers |
| `ops show <source> --op KEY` | Show one operation handler; `--format` |
| `ops classify <source>...` | Label handlers across captures |
| `signals <program.js>` | Per-seed signal schema; `--json`, `--layouts-only`, `--no-finder-roles` |
| `signals show\|inputs\|contracts <program.js>` | Generator inputs, composed inputs, typed contracts |
| `show signals [SignalName]` | Source-independent catalogue card; `--json` |

**Tracing**

| Command | Purpose |
| --- | --- |
| `trace program <source>` | Recover and link the reachable program; `--out`, `--json`, `--thread-cleanup`, `--no-cleanup`, `--report-dir`, `--steps` |
| `trace thread <source>` | Walk one thread; `--pc`, `--state`, `--initialslots`, `--report`, `--json`, `--out` |
| `trace cleanup-thread <thread.js>` | Clean one emitted function; `--out`, `--json` |
| `trace corpus <source>...` | Compare reachable-function fingerprints; `--json` |

**Naming**

| Command | Purpose |
| --- | --- |
| `naming threads <program.js>` | Name thread functions; `--rename`, `--out`, `--map-out`, `--labels` |
| `naming collectors <program.js>` | Identify collector routing; `--rename`, `--out`, `--map-out` |
| `naming inventory <program.js>` | Function index and role graph; `--json` |
| `naming plan <program.js>` | Proposed names, ambiguities and refusals (JSON) |
| `naming corpus <source>...` | Run the naming chain per seed and compare baselines |

**Sessions, artifacts and framing**

| Command | Purpose |
| --- | --- |
| `seeds fetch\|parse\|import\|sites` | Acquire launchers and kernels, classify saved files, import local seeds, list the registry |
| `pin --input <program.js> --init\|--session` | Pin a traced program into a `shape-seed-layout/1` document |
| `decode --seed\|--artifact ... --headers ...` | Decode captured headers into named fields |
| `decode --schema-only --seed <program.js> --input-kind program --schema-out <schema.json>` | Export the static schema without a session or capture |
| `headers encode\|decode` | Report framing to and from a header bundle |
| `payload encode\|decode` | Report framing to and from a bare finalized payload |
| `encode --profile` / `decode --profile` | Opaque bytes to and from session headers |
| `pack` / `unpack` | Generic superpack serialization |
| `gen <command>` | Legacy alias for the framing commands above |

`shape seeds fetch` is the only command that uses the network. It downloads
launchers and the kernels they load; `seeds import` stores an already
downloaded kernel without network access, and `seeds sites` lists the registry.

### Performance and limits

Raw-seed recovery does several jobs: compile the kernel facts, trace reachable
threads, mechanically clean the recovered program, then extract layouts and
compile signal recipes. Capture complexity and the number of reachable threads
make this a seconds-to-minutes operation. Use release builds for this stage.

For the shortest preparation path, trace the raw seed with thread cleanup and
whole-program mechanical finishing enabled. The SDK's automatic job selection
uses available cores for tracing. Deobfuscating a separate kernel file and naming
functions are optional investigation steps; neither is needed to compile the
decode artifact. See the [SDK workflow](sdk/README.md#workflow) for the defaults.

Save a decode artifact to reuse the finished analysis. Each invocation of
`shape decode --artifact` still starts a process, reads the artifact and
validates it before decoding headers. For repeated requests, the SDK can keep
that validated artifact in memory and avoid those repeated setup costs.

A kernel cache such as `--report-dir` skips kernel compilation on a later
trace; it does **not** skip tracing. Saving a decode artifact skips the source
analysis stages on reuse. Keeping an SDK artifact loaded also skips repeated
artifact parsing and validation. The SDK's named source operations leave disk
caching disabled unless requested; see the [cache policy](sdk/README.md#explicit-cache-policy).

Use `--steps` to limit each function walk, and the SDK's tracing `jobs` option
to limit concurrent walks. These bound tracing work, not the cost of decoding a
loaded capture. Decode and encode cost instead depends on the captured payload
and signal layouts, requested metadata and verification, and concurrent request
load. Default decode retains round-trip context without building every signal's
details; `explain` can build one later. Verification adds re-encoding work.

## SDK

The SDK puts the same engine inside your application. Decode headers into
fields such as `DeviceMemory.deviceMemory`, inspect how a signal was encoded,
change its values, and encode the full payload back into headers. It also
exposes tracing, cleanup, naming and schema extraction.

The engine is one Rust crate. The C ABI (`libshape`) wraps it, and the Python,
Node.js and Go bindings wrap the C ABI. The Rust SDK is the crate itself, with a
JSON dispatch that matches the C ABI plus typed APIs. All five paths share the
same requests, results and error codes.

| Language | Guide | Interface | Best when |
| --- | --- | --- | --- |
| Node.js 18+ / TypeScript | [sdk/node](sdk/node/README.md) | CommonJS/ESM, Promises for commands, hand-written types | A Node service or tool should keep the event loop free while tracing |
| Python 3.9+ | [sdk/python](sdk/python/README.md) | Standard-library `ctypes`, synchronous | Scripting and data work; no compiler or package dependencies |
| Go 1.22+ with cgo | [sdk/go](sdk/go/README.md) | Synchronous functions returning values and errors | A static Go service or CLI that wants no shared-library deployment |
| Rust | [sdk/rust](sdk/rust/README.md) | Native crate, typed APIs and JSON dispatch | Maximum control, zero FFI, and access to internal modules |
| C | [sdk/c](sdk/c/README.md) | JSON ABI with explicit ownership | Any other language, or a minimal deployment surface |

The [shared SDK guide](sdk/README.md) has the complete workflow, documents and
operation matrix. The [decoded output and editing guide](sdk/signals.md)
explains the named fields, per-signal details and full-payload round trips.

Start with your language guide to load an artifact and decode a capture. It
also covers preparing an artifact from a raw seed, inspecting and editing
fields, and handling errors. Check its precision and lifetime guidance when
passing decoded payloads between processes or sharing loaded artifacts across
calls.

### Set up once

For Node.js, Python, Go and C, run the shared native build from the repository
root on macOS or Linux:

```sh
scripts/build-sdk.sh
mkdir -p reports
```

The script needs Rust 1.92+ with Cargo, a C toolchain, and Node.js with Node-API
headers to build the addon. It also uses Rust's `llvm-tools` component
(`rustup component add llvm-tools`) to prepare Go's static library. It builds
the release C ABI library, installs the libraries for the bindings and builds
the Node addon. Rerun it after native source changes; its outputs are Git-ignored.
`scripts/build-sdk.sh --help` lists environment options, and the
[C guide](sdk/c/README.md#install-and-build) covers manual builds.

Rust applications depend directly on the repository's root crate and do not
need the C ABI build. For package setup and runtime minimums, follow the
[Node.js 18+](sdk/node/README.md), [Python 3.9+](sdk/python/README.md),
[Go 1.22+ with cgo](sdk/go/README.md), or [Rust](sdk/rust/README.md) guide.
The build supports macOS and Linux.

The examples below use the committed synthetic artifact and capture in
`sdk/c/tests/fixtures/`, the same pair as the CLI decode above. Each one
decodes the capture, changes `DeviceMemory.deviceMemory` from 32 to 16, and
encodes the full payload back into headers.

Keep the loaded artifact for as many requests as you need, then release it
using your language's lifetime rules. Each decode returns a separate payload
to inspect or edit. The Node examples use explicit async methods; named Node
`decode` and `encode` calls are synchronous.

For your own data, create an artifact with the
[CLI](#recover-code-and-save-a-decode-artifact) or the
[SDK workflow](sdk/README.md#workflow), then substitute its path and your
captured headers. The artifact, headers and session must match. Other captures
may carry different signals.

### Node.js / TypeScript

#### JavaScript (ES modules)

From the repository root, run `node --input-type=module` and paste this code on
standard input (finish with Ctrl-D). Installed applications import from
`'shape'` instead of the checkout's `index.mjs`.

```js
import { readFileSync, writeFileSync } from 'node:fs';
import { Generator, stringifyJSON } from './sdk/node/index.mjs';

const generator = await Generator.loadAsync(readFileSync('sdk/c/tests/fixtures/artifact.json'));
try {
  const decoded = await generator.decodeAsync(readFileSync('sdk/c/tests/fixtures/capture.json'));
  console.log(decoded.positionalSignals);
  const memory = decoded.positionalSignals.DeviceMemory;
  if (!memory || typeof memory.deviceMemory !== 'number') {
    throw new Error('Capture has no numeric DeviceMemory field');
  }
  memory.deviceMemory = 16;
  const encoded = await generator.encodeAsync({ decoded });
  const json = stringifyJSON(encoded);
  if (json === undefined) throw new Error('Capture could not be serialized');
  writeFileSync('reports/node-headers.json', json);
} finally {
  generator.close();
}
```

#### TypeScript (ES modules)

Install the local package and compiler in an ignored demo workspace:

```sh
npm install --prefix reports/sdk-demo --install-links "$PWD/sdk/node"
npm install --prefix reports/sdk-demo --save-dev typescript@~5.7.3 @types/node@^22
```

Save this as `reports/sdk-demo/demo.mts`. The `.mts` extension selects ESM, and
`NodeNext` resolves the installed package's exports and declarations.
`--install-links` installs a package copy so its types resolve in the demo
workspace.

```ts
import { readFileSync, writeFileSync } from 'node:fs';
import { Generator, stringifyJSON } from 'shape';
import type { CaptureHeaders, DecodedPayload } from 'shape';

const generator = await Generator.loadAsync(readFileSync('sdk/c/tests/fixtures/artifact.json'));
try {
  const decoded: DecodedPayload = await generator.decodeAsync(
    readFileSync('sdk/c/tests/fixtures/capture.json'),
  );
  console.log(decoded.positionalSignals);
  const memory = decoded.positionalSignals.DeviceMemory;
  if (!memory || typeof memory.deviceMemory !== 'number') {
    throw new Error('Capture has no numeric DeviceMemory field');
  }
  memory.deviceMemory = 16;
  const encoded: CaptureHeaders = await generator.encodeAsync({ decoded });
  const json = stringifyJSON(encoded);
  if (json === undefined) throw new Error('Capture could not be serialized');
  writeFileSync('reports/typescript-headers.json', json);
} finally {
  generator.close();
}
```

Compile and run from the repository root:

```sh
reports/sdk-demo/node_modules/.bin/tsc reports/sdk-demo/demo.mts \
  --strict --noUncheckedIndexedAccess --target ES2022 \
  --module NodeNext --moduleResolution NodeNext --outDir reports/sdk-demo/dist
node reports/sdk-demo/dist/demo.mjs
```

Pass artifact and capture bytes directly. For saved decoded objects, use
`shape.parseJSON` and `shape.stringifyJSON`: they preserve large integers as
`bigint` across Node 18+, including runtimes without native lossless JSON
support. Ordinary `JSON.parse` can round integers needed for exact encoding.

### Python

From the repository root, run `PYTHONPATH=sdk/python python3` and supply this
code on standard input:

```python
import json
from pathlib import Path
import shape

artifact = Path("sdk/c/tests/fixtures/artifact.json")
headers = Path("sdk/c/tests/fixtures/capture.json").read_bytes()
with shape.Generator.load(artifact) as generator:
    decoded = generator.decode(headers)
    print(decoded["positionalSignals"])
    decoded["positionalSignals"]["DeviceMemory"]["deviceMemory"] = 16
    encoded = generator.encode(decoded=decoded)
    Path("reports/python-headers.json").write_text(
        json.dumps(encoded), encoding="utf-8"
    )
```

### Go

In a Go module configured with `require shape v0.0.0` and
`replace shape => /absolute/path/to/repo/sdk/go`, use this function. Pass the
artifact and capture JSON bytes read from the two fixture files. It returns the
complete edited capture, ready to marshal with `encoding/json`.

```go
package demo

import (
	"encoding/json"
	"fmt"

	"shape"
)

func EditCapture(artifact, headers []byte) (*shape.Capture, error) {
	generator, err := shape.Load(artifact)
	if err != nil {
		return nil, err
	}
	defer generator.Close()

	decoded, err := generator.Decode(headers, shape.DecodeOptions{})
	if err != nil {
		return nil, err
	}
	fmt.Println(decoded.PositionalSignals)
	memory, ok := decoded.PositionalSignals["DeviceMemory"]
	if !ok {
		return nil, fmt.Errorf("capture has no DeviceMemory signal")
	}
	memory["deviceMemory"] = json.Number("16")
	return generator.Encode(shape.EncodeRequest{Decoded: decoded})
}
```

Edits to Go's exported `Decoded` fields are encoded. Keep numbers as
`json.Number`, integers or `json.RawMessage` to preserve precision.

### Rust and C

Rust callers depend on this repository's root crate by path and use
`shape::Generator` or the typed APIs directly. C callers link `libshape` and
exchange JSON text. See the [Rust guide](sdk/rust/README.md) and the
[C guide](sdk/c/README.md) for complete examples.

```rust
use shape::Generator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let generator = Generator::load(&std::fs::read("artifact.json")?)?;
    let capture: shape::signals::decode::CaptureHeaders =
        serde_json::from_slice(&std::fs::read("headers.json")?)?;
    let mut decoded = generator.decode(&capture, None, Default::default())?;
    let memory = decoded.positional_signals.get_mut("DeviceMemory")
        .ok_or("capture has no DeviceMemory signal")?;
    memory["deviceMemory"] = serde_json::json!(16);
    let headers = generator.encode(&serde_json::to_value(&decoded)?, None)?;
    println!("{} {}", headers.site_id, headers.headers.iter().count());
    Ok(())
}
```

### Preserve the full payload

Decode returns both readable `positionalSignals` and the context needed to
rebuild the capture. Keep the **whole decoded object**, edit the specific named
fields, and pass that whole object to encode. Persist the entire document when
passing it to another pipeline stage, including `metadata.originalReport`;
saving only the named map loses the original encoding context.

That retained report carries raw signal bytes, other report channels, seeds,
keysets, header context and per-signal prefixes. For supported encodings with a
*matching artifact and session*, encoding the unchanged full object reproduces
**all six captured header values exactly**. Editing fields retains the logical
unedited signals and other channels, and reuses original bytes and context
where the codec supports it. A changed or reordered repeated row can require
canonical encoding of the entire row. Shared framing and encryption can also
change header bytes beyond the edited field; an edit is not a promise of
unchanged ciphertext elsewhere.

`metadata=false` still retains this context. It only avoids eagerly building
details for every signal; use `explain` when you need one signal's fields,
encoding details or verification. Explain reads the retained bytes, so **encode
and decode an edit** before explaining its new value. The loaded artifact is
immutable and reusable; each decoded object is mutable request data. **Copy that
object** if you need to keep both the original and edited versions.

Encoding the **named map alone** deliberately constructs a new payload with
**fresh signal prefixes** and layout defaults for other channels. Choose it when
you want new construction from field values; use full-payload encoding when
you want to retain a capture. Both paths require supported wire recipes and
check that the result decodes to exactly the requested fields. Literal and
count constraints still apply, and unsupported edits refuse. Some encodings
depend on the retained report and will refuse an edit if that context cannot
be reconstructed.

A session override replaces the **whole** artifact session. Use the **same
consistent replacement** for decode, explain and encode, paired with headers
from that session and the matching site/build layout. Replacing a session does
not make a different capture's layout compatible. See the complete
[editing walkthrough](sdk/signals.md) for retained fields and supported edits.

To check the distinction yourself, run this complete Python program from the
repository root with the binding installed. It uses the same public sample32
pair as the examples above and creates no output files:

```python
import copy
import json
from pathlib import Path
import shape

fixtures = Path("sdk/c/tests/fixtures")
capture = json.loads((fixtures / "capture.json").read_text())
with shape.artifact.load((fixtures / "artifact.json").read_bytes()) as loaded:
    decoded = shape.decode(loaded, capture)  # metadata=False retains context
    assert shape.encode(loaded, decoded) == capture
    assert decoded["positionalSignals"]["DeviceMemory"]["deviceMemory"] == 32

    edited = copy.deepcopy(decoded)
    edited["positionalSignals"]["DeviceMemory"]["deviceMemory"] = 16
    encoded = shape.encode(loaded, edited)
    again = shape.decode(loaded, encoded, verify=True)
    assert again["positionalSignals"] == edited["positionalSignals"]
    for key in ("errors", "signals", "keyedSignals", "seeds", "keysets"):
        assert again[key] == decoded[key]
    for name, fields in decoded["positionalSignals"].items():
        if name != "DeviceMemory":
            assert again["positionalSignals"][name] == fields
    assert decoded["positionalSignals"]["DeviceMemory"]["deviceMemory"] == 32
    print("Unchanged capture exact; edited value and untouched channels verified.")
```

### Performance in the SDK

Compile from source once, then keep one loaded artifact outside request loops.
Loading parses and validates the saved document; repeated decode, explain and
encode calls reuse that immutable native state. This removes source tracing,
recipe compilation and repeated artifact parsing/validation from the request
path. Each call still processes its own capture and returns its own result.

Default `metadata=false` leaves per-signal detail work until `explain` is
needed. `verify=true` on decode additionally rebuilds signals and headers to
check exact equality; enable it when that check is useful. Encode independently
checks the requested fields' round trip. Neither option replaces the need for
a matching artifact/session. For host concurrency and handle lifetime, follow
your [language guide](#documentation); Node services can use async operations.

Measure source preparation, artifact loading, repeated decode and repeated
encode separately. Record the capture's signal count, options and request
concurrency, warm up the loaded handle, then report iteration count, median and
p95 for each operation. Raw-seed walkthrough timings include the preparation
pipeline and do not measure loaded decode throughput. Capture complexity,
metadata, verification and competing work on the host all affect the result;
there is no single latency figure that describes every artifact.

A local example using the public [sample32 capture](sdk/c/tests/fixtures/capture.json) and matching [artifact](sdk/c/tests/fixtures/artifact.json): three signals, six headers, on macOS arm64 / Apple M5 Pro, Node 22.22.2.
Named Node calls in a warmed process, concurrency 1, one reused loaded artifact; default decode `metadata=false` / `verify=false` (encode includes its own verification).

| Operation | Median | p95 |
| --- | ---: | ---: |
| Artifact load + validation | 0.313 ms | 0.415 ms |
| Loaded decode | 0.053 ms | 0.063 ms |
| Full-payload encode | 0.062 ms | 0.075 ms |

Hot calls: 20 warmups then 200 samples each; fresh loads: 10 warmups then 50 samples, excluding file I/O. These timings represent this small fixture; capture complexity and options change timing. Follow the measurement procedure above and the [Node guide](sdk/node/README.md) for your inputs.

### Cross-language interoperability

Compile a decode artifact once with the CLI or Rust, save it, and load the
**same bytes** in Node.js/TypeScript, Python, Go or C. The languages expose the
same operations, signal names and supported encodings. Save the artifact
document to share it; each process loads its own instance. Kernel artifacts,
traced programs and decode artifacts are different inputs, and their
format versions and source identity matter. Keep the artifact with its matching
site/build and session/capture pair rather than swapping documents from another
capture.

For example, the CLI can prepare an artifact, a Python script can inspect a
capture and save its **full decoded payload**, and a Node service can load that
artifact, read the saved payload, edit a named field and encode it. Saving the
artifact carries the compiled analysis between stages; saving the entire
decoded object separately carries that request's original channels and encoding
context. Saving just `positionalSignals` selects new construction instead.

Exchange JSON text or bytes through a lossless path. Node provides
`parseJSON` / `stringifyJSON` for exact integers; Python's standard JSON
integers are exact. Go uses `json.Number` / `json.RawMessage`, Rust uses the
SDK's configured `serde_json`, and C can pass the raw JSON text through.
Ordinary JavaScript numbers or Go `float64` can round large integers in
artifacts and metadata. A JSON handoff needs those language-specific readers
and writers; it does not automatically convert arbitrary BigInt objects between
languages.

The [contract harness](sdk/tests/README.md) compares artifact loading, decoding,
edits, exact numeric data and errors across the SDKs. Use compatible SDK
versions and artifact formats when passing documents between deployments.

## Troubleshooting

Start with the command's `--help` and the error message. Check which CLI binary
or installed native SDK library is actually running: source changes do not
update an older installed build. Then check that the seed, whole session,
headers and artifact describe the same site/build/capture. Use the
[CLI build and PATH setup](#build-once) when checking a checkout's binary.

| Symptom | Likely cause | Fix |
| --- | --- | --- |
| `shape: command not found` | The repository binary is not on `PATH` | From the repository root, run `export PATH="$PWD/target/release:$PATH"` after the [CLI build](#build-once) |
| Exit status `3`, empty `result.javascript` | Tracing stopped before producing a partial program | Inspect `result.functions.stopped`; raise `--steps`, retry without `--thread-cleanup`, or use branch overrides |
| `capture/artifact siteId mismatch` | Headers and artifact come from different sites | Use the artifact compiled for that site, or recompile with the capture's `siteId` |
| `native float byte order must be explicitly configured` | The seed emits native floats and the artifact has no byte order | Re-run `decode --seed` or `artifact.compile` with `--float-byte-order little\|big` |
| `reconstructed report differs` under `--verify` | Artifact, session and headers do not describe one capture | Re-extract the matching session and recompile the artifact |
| `unsupported decode artifact format` | A compact schema or a kernel artifact was passed where a decode artifact was expected | Pass the `--save-artifact`/`artifact.compile` output, not `--schema-out` or `report` |
| `an existing output file` | Commands refuse to overwrite by default | Choose a new output path, or add `--force` where supported |
| `too many open files` during a large trace | Many concurrent walks | Raise the shell's file limit; SDK callers can also lower the tracing `jobs` option |
| Python `library_not_found` | `libshape` is not installed or not discoverable | Run `scripts/build-sdk.sh`, or set `SHAPE_LIBRARY` to the absolute library path |
| Node cannot load `shape.node` | Addon not built, wrong architecture, or missing library beside it | Re-run `sdk/node/build.sh`; confirm `libshape` is in `sdk/node/build/`; check `process.arch` |
| Go `disallowed LTO section __LLVM,__bitcode` | The static library still contains LLVM bitcode | Run `scripts/build-sdk.sh`, or strip it (see the [Go guide](sdk/go/README.md#installing-the-library-by-hand)) |
| C link error `mis-aligned LINKEDIT string pool` | A stripped Rust dylib was linked | Rebuild with `scripts/build-sdk.sh`; see the [C guide](sdk/c/README.md#install-and-build) |
| `artifact.json` decodes but edits fail | The requested edit has no supported encoding | Check `describe`/`explain` for the signal; unsupported leaves cannot be synthesized |
| Large integers change value | A JSON layer parsed them as doubles | Keep documents as text, or use the language's exact-integer path (see each guide's precision section) |

Successful decoding does not mean every edit can be encoded. Inspect the
signal with `explain`, check its supported shape and literal/count constraints,
and retain the full decoded object. If values changed while moving through
JSON, check integer precision before diagnosing a codec failure.

If a failure looks like an internal panic, the SDK returns a `panic` error
code rather than crashing your process. Include the operation or command,
CLI/native version, options, error `code` and a minimal reproduction in a report.

## Repository layout

| Path | Contents |
| --- | --- |
| `src/` | The engine: deobfuscation, tracing, naming, signals, wire codec, SDK dispatch |
| `shape.rs`, `cli/` | The `shape` binary and its command implementations |
| `sdk/c/` | The C ABI crate (`shape-c`), header, samples and consumer tests |
| `sdk/python/`, `sdk/node/`, `sdk/go/` | Language bindings and their guides |
| `sdk/rust/` | Guide to the root crate's Rust SDK (no separate binding) |
| `sdk/tests/` | The five-language contract harness |
| `tests/` | Rust integration tests and committed fixtures |
| `docs/` | Contracts, workflows and source references by area |
| `scripts/` | `build-sdk.sh` and the documentation link check |
| `reports/` | Git-ignored output for local runs |

## Documentation

| Topic | Where |
| --- | --- |
| SDK workflow, documents, operation matrix and contract | [sdk/README.md](sdk/README.md) |
| Decoded signals and editing | [sdk/signals.md](sdk/signals.md) |
| Node.js / TypeScript SDK | [sdk/node/README.md](sdk/node/README.md) |
| Python SDK | [sdk/python/README.md](sdk/python/README.md) |
| Go SDK | [sdk/go/README.md](sdk/go/README.md) |
| Rust SDK | [sdk/rust/README.md](sdk/rust/README.md) |
| C ABI and request reference | [sdk/c/README.md](sdk/c/README.md) |
| Five-language contract harness | [sdk/tests/README.md](sdk/tests/README.md) |
| Tracing | [docs/tracing/trace-cli.md](docs/tracing/trace-cli.md) |
| Signal schema | [docs/signals/schema.md](docs/signals/schema.md) |
| What each signal means | [shape show signals](#name-functions-and-read-the-signal-schema) or [docs/signals/](docs/signals/README.md) |
| Payloads and headers | [docs/generator/README.md](docs/generator/README.md) |
| Capture acquisition | [docs/deob/seeds.md](docs/deob/seeds.md) |
| WebAssembly boundaries | [docs/reference/wasm-boundary.md](docs/reference/wasm-boundary.md) |
| Architecture | [docs/architecture.md](docs/architecture.md) |
| CLI module ownership | [cli/README.md](cli/README.md) |
| Everything else | [docs/README.md](docs/README.md) |
