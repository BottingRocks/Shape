# shape SDKs

Use named operations to recover a program, inspect signals, compile a decode
artifact, and decode, explain or encode captures. All five SDKs share the Rust
engine and the [portable operation contract](operations.json).

| Language | Guide | Runtime interface |
| --- | --- | --- |
| Rust | [rust/README.md](rust/README.md) | Typed native operations in `shape::sdk` |
| Node.js / TypeScript | [node/README.md](node/README.md) | Named ESM exports, strict TypeScript types, async native work |
| Python | [python/README.md](python/README.md) | Named functions and namespaces over ctypes |
| Go | [go/README.md](go/README.md) | Named functions and namespaces over cgo |
| C | [c/README.md](c/README.md) | Versioned JSON operations and explicit native handles |

Each guide supplies complete runnable recipes, installation, errors, precision
and lifetime rules. Start there for language syntax; this page defines shared
behavior. The [signal walkthrough](signals.md) explains field edits and retained
reports. The committed fixtures provide synthetic inputs.

## Build

From the repository root on macOS or Linux:

```sh
scripts/build-sdk.sh
```

This builds the release C library, installs it for Python/Node/Go and builds
the Node addon. Requirements are Rust 1.92+, a C toolchain, Node 18+ with
Node-API headers, Python 3.9+, and Go 1.22+ with cgo. Go's static library needs
LLVM bitcode stripped; the script handles that using `rust-objcopy` from
`rustup component add llvm-tools`. See the individual guides for manual links,
platform prerequisites and package setup. Packages are installed from this
checkout, rather than a public registry.

Use release builds for raw seed recovery. Avoid concurrent Cargo builds into
the same target directory. Rebuild installed bindings after native changes;
a stale installed library does not acquire new operations from source edits.

## Workflow

Choose the entry point matching the artifact you already have:

```text
raw seed ── trace.program ──► complete traced mini-JS ─┐
initializer ── session.from_initializer ──► session ─┼─ artifact.compile
capture site ID ──────────────────────────────────────┘        │
                                                       loaded artifact
                                                              │
headers ── decode ──► full decoded payload ── explain one signal
   ▲                         │
   └──────── encode ◄── edit named fields
```

1. Trace matching raw JavaScript with thread cleanup enabled, whole-program
   mechanical finishing enabled, and automatic job selection. In wire/Rust
   spelling these are `thread_cleanup=true`, `no_cleanup=false`, `jobs=null`/`None`; named APIs
   use those defaults. Check `complete` and inspect `stopped` before using code.
2. Parse the matching initializer; it produces session token, alphabet,
   shuffle key and cipher keys.
3. Compile the traced program with that session and the capture's site ID.
   Compilation runs the required finders and creates executable recipes.
   Mechanical finishing normalizes parameters/captures and substitutes kernel
   values; semantic naming and renaming are optional investigation operations.
   Explicit `no_cleanup=true` retains diagnostic output and skips the full
   cleanup pipeline.
4. Keep the immutable loaded decode artifact outside request loops. Pass it
   explicitly to `decode`, `explain`, `encode` and `schema`. Loading parses and
   validates once; subsequent operations reuse that prepared state in memory.
5. Persist program, artifact, schema or edited captures only when your caller
   needs them. Use the artifact export/serialization operation and your own
   file I/O. Compilation is not an instruction to save files.

Raw-to-artifact compilation is also available with `input_kind=raw`. Use the
expanded trace step when you need completion metadata or tracing controls.
Strict compilation is the default. Explicit `direct=true` permits source-format
compilation without strict lifecycle/algorithm certification; provenance stays
in the artifact. It does not make unsupported encodings valid.

## Documents

| Item | Producer | Reuse boundary / consumer |
| --- | --- | --- |
| Raw seed | Caller or explicit seed acquisition | Deobfuscation, kernel compilation, raw tracing |
| Deobfuscated JavaScript | `deob` | Readable source; `report` with deobfuscated input |
| Kernel artifact | `report`, optional `deob` artifact | Load once for thread/corpus tracing; whole-program tracing additionally checks matching raw-source identity |
| Traced mini-JS | Complete `trace.program` | Program compilation, signal finders and optional naming; avoids repeating the VM trace |
| Session | `session.from_initializer` | Matching artifact compilation and optional whole-session overrides |
| Pinned layout | `pin` | Structural report slots/session layout; lacks executable signal recipes |
| Detailed schema | `signals.schema` | Inspection of layouts, projections and diagnostics |
| Compact schema | `schema`, `artifact.compact_schema`, `signals.compact_schema` | Descriptive schema consumers; cannot decode |
| Decode artifact | `artifact.compile`, explicit export | Versioned layout, session and recipes; `artifact.load` prepares repeated decode/explain/encode |
| Decoded payload | `decode` | Named fields plus retained report; full-payload edits and exact re-encoding |

Kernel, pinned layout and decode artifacts describe different layers and cannot
replace one another. Only canonical versioned documents are accepted. Traced
source avoids tracing but later source analysis can still parse it. A loaded
kernel avoids kernel parsing/compilation; each trace still prepares runtime
state. A loaded decode artifact reuses validated recipes and constants.

Public starting points:

| Fixture | Contains | Start at |
| --- | --- | --- |
| [Sample32](c/tests/fixtures/README.md) | Matching artifact/capture: number 32, packed storage flags, empty string | Load and decode |
| [Synthetic source](../tests/fixtures/sdk/decode/README.md) | Traced program, session, capture with memory 9 | Compile |

<a id="result-and-cache-migration"></a>

## Explicit cache policy

Named tracing and compilation default to **Disabled**, independent of any
process cache setting. Select policy on each call:

| Policy | Wire form | Behavior |
| --- | --- | --- |
| Disabled | `{"mode":"disabled"}` or omitted | No kernel disk-cache reads or writes |
| Directory | `{"mode":"directory","directory":"kernel-cache"}` | Reuse/publish canonical compiled kernels in this directory, validating source identity |
| Environment | `{"mode":"environment"}` | Explicitly opt into legacy `SHAPE_KERNEL_CACHE` and platform/user cache resolution |

No policy mutates process environment. Independent concurrent callers may
choose different policies. The kernel cache does not cache traced programs,
naming results or ASTs. A cache hit skips kernel compilation; VM tracing still
runs. Invalid explicit entries, identity mismatches and publication failures
are errors. Legacy `.report.json` files are not canonical kernel cache entries.

Disk caching, keeping loaded handles in memory, and deliberately saving output
files are three separate choices. Disabled caching permits caller-requested
artifact exports and file writes. Legacy dispatch retains its earlier defaults
and `report_dir` behavior; do not use that field as an output-report directory.

## Operation matrix

Choose the operation by the input you already have and the result you need.
The usual fast path is raw trace with full mechanical cleanup, then signal
finders and artifact compilation, then repeated decode/encode with a loaded
artifact. Naming is optional; readable function labels are not a prerequisite
for signal recipes.

| Operation | What you provide | What you get | When to use it |
| --- | --- | --- | --- |
| Trace program | Raw seed; optionally a matching kernel artifact plus its exact source | Recovered linked JavaScript and completion/stopped status | Recover the reachable program before extracting signals; check completion before continuing |
| Trace thread | Raw seed or kernel artifact, entry coordinates and walk options | One recovered thread and walk status | Investigate a function or a stopped walk without recovering the whole program |
| Compile artifact | Recovered program, matching session and site ID; raw input is also supported explicitly | A reusable loaded decode artifact, exportable as a document | Move layout analysis and recipe compilation out of the request path |
| Load artifact | Saved decode-artifact bytes or document | A validated loaded artifact to reuse | Start a service or reopen saved analysis without source tracing |
| Decode | Loaded artifact and matching captured headers | Named fields plus retained whole-capture context | Inspect a request or begin a full-payload edit |
| Explain | Loaded artifact, complete decoded payload and signal name | That signal's fields, encoding details and optional verification | Investigate one signal without requesting every signal's metadata |
| Encode | Loaded artifact and edited **full payload** | Rebuilt capture headers | Retain capture context while editing supported fields; named-map encoding instead constructs a fresh payload |
| Inspect artifact schema | Loaded decode artifact, or its saved document through compact-schema export | Descriptive field/schema view | See what the artifact supports; this view cannot replace the executable artifact |
| Extract source signal schema | Recovered program | Signal names, routing, layouts and diagnostics | Investigate recognized and unresolved slots before decoding; distinct from inspecting an artifact's compact schema |
| Deobfuscate / inspect kernel | Raw kernel source; kernel loading also accepts saved kernel facts | Readable VM code, kernel facts or operation-handler analyses | Understand the VM or prepare reusable kernel facts, rather than decode request fields |
| Mechanical cleanup | Emitted thread JavaScript | Cleaned thread JavaScript | Simplify an individual recovered function; program tracing already applies its configured cleanup stages |
| Optional naming | Recovered program | Names, collector roles, proposed mappings or renamed code | Make recovered source easier to navigate |
| Handle seed inputs | Saved launcher/kernel or an explicit fetch request | Parsed seed identity/session, imported files or fetched inputs | Prepare matching inputs; fetching is explicit network I/O |
| Header / payload framing and Superpack | Explicit codec profile with a report, bytes or typed values | Framed headers/payload or serialized values | Work below named signals when you already know the framing or serializer contract |

The table below maps those tasks to exact language entry points. The complete
inventory, requests, options, defaults and handle methods live in
[operations.json](operations.json); the [contract harness](tests/README.md)
checks provider and consumer mappings.

| Task | Rust (`sdk::`) | Node (`shape.`) | Python (`shape.`) | Go (`shape.`) |
| --- | --- | --- | --- | --- |
| Trace program | `trace::program` | `trace.program` | `trace.program` | `Trace.Program` |
| Trace thread | `trace::thread` | `trace.thread` | `trace.thread` | `Trace.Thread` |
| Compile artifact | `artifact::compile` | `artifact.compile` | `artifact.compile` | `Artifact.Compile` |
| Load artifact | `artifact::load` | `artifact.load` | `artifact.load` | `Artifact.Load` |
| Decode | `decode` | `decode` | `decode` | `Decode` |
| Explain | `explain` | `explain` | `explain` | `Explain` |
| Encode full payload | `encode` | `encode` | `encode` | `Encode` |
| Inspect loaded artifact schema | `schema` | `schema` | `schema` | `Schema` |
| Extract source signal schema | `signals::schema` | `signals.schema` | `signals.schema` | `Signals.Schema` |
| Deobfuscate kernel | `deob` | `deob` | `deob` | `Deob` |
| Inspect kernel | `kernel::show` | `kernel.show` | `kernel.show` | `Kernel.Show` |
| Clean one thread | `cleanup::thread` | `cleanup.thread` | `cleanup.thread` | `CleanupOperations.Thread` |
| Name functions | `naming::threads` | `naming.threads` | `naming.threads` | `Naming.Threads` |
| Inspect signal | `signals::describe` | `signals.describe` | `signals.describe` | `Signals.Describe` |
| Classify operations | `ops::classify::*` | `ops.classify.*` | `ops.classify.*` | `Ops.Classify.*` |
| Compare trace corpus | `trace::corpus::*` | `trace.corpus.*` | `trace.corpus.*` | `Trace.Corpus.*` |
| Analyze naming corpus | `naming::corpus::*` | `naming.corpus.*` | `naming.corpus.*` | `Naming.Corpus.*` |
| Seed acquisition/parsing | `seeds::*` | `seeds.*` | `seeds.*` | `Seeds.*` |
| Header / payload framing | `headers::*` / `payload::*` | `headers.*` / `payload.*` | `headers.*` / `payload.*` | `HeaderOperations.*` / `PayloadOperations.*` |
| Typed superpack values | `pack` / `unpack` / `unpack_canonical` | `pack` / `unpack` / `unpackCanonical` | `pack` / `unpack` / `unpack_canonical` | `Pack` / `Unpack` / `UnpackCanonical` |

See each guide for arguments, option types and member spelling. The
[C guide](c/README.md#api) covers its JSON calls and loaded handles.
Seed fetching uses the network; seed import writes files.

## Decode, explain and encode

Capture documents are `{siteId, headers}` with suffix keys such as `a`, `a0`,
`b`, `c`, `d`, `f`, `z`. Values are strings, including empty strings. Artifacts,
sessions and captures must come from the same build/site/session.

`decode` defaults to `verify=false, metadata=false`. It returns named
`positionalSignals` and always retains `metadata.originalReport`. Detailed
`metadata.signals` is lazy by default. `explain` reconstructs one signal's
fields and details from retained bytes; `metadata=true` eagerly builds every
present signal's details. `verify=true` independently re-encodes signals and
headers to require exact equality. `metadata.exactReencode=false` on default
decode means verification did not run.

| Encode input | Guarantee |
| --- | --- |
| Unchanged full decoded payload | Retains channels, seeds, keysets, header context and random prefixes; supported matching inputs preserve the header values |
| Full payload with edited named fields | Retains encoding context and checks the output decodes to exactly the requested fields |
| Bare named signal map | Uses fresh prefixes and artifact layout defaults; preserves field values rather than the original ciphertext |

The retained report contains distinctions lost by the readable projection.
Keep it intact when editing fields. Explain reads retained bytes: encode and
decode an edit before explaining its new value. JSON whitespace/key order are
not covered by header byte preservation. Encoding fails when an edit has no
supported wire representation; it does not invent missing recipes.

A session override replaces the whole session. Pass the same override to
decode, explain and encode. It does not make another build's layout compatible.
See [signals.md](signals.md) for exact fields, edit behavior and runnable recipes.

## Interoperability

Share version-compatible artifacts, captures and full decoded payloads across
SDKs using the matching seed/build, session and site ID. Keep version tags
intact, and keep the full decoded payload for later edits and re-encoding.

Preserve large JSON integers when saving or reopening documents:

- Node: use `parseJSON` and `stringifyJSON` to retain bigint values.
- Python: JSON integers remain exact `int` values; avoid conversion to `float`.
- Go: use `json.Number` and `json.RawMessage` rather than `float64`.

See the precision guidance for [Node/TypeScript](node/README.md#types-and-precision),
[Go](go/README.md#preserve-exact-values),
[Rust](rust/README.md#precision) and [C](c/README.md#precision).
For byte framing, use the
[Python framing recipe](python/README.md#header-and-payload-framing) or your
language guide. The [Superpack guide](../docs/generator/superpack.md) explains
typed values and exact number bits.

## Lifetime, errors and concurrency

Load an artifact once and reuse it across calls. Finish all calls before
closing or freeing it; follow your language guide’s threading and lifetime rules.

Handle errors by code and read the message for diagnosis. Set trace `jobs` and
`steps` for your workload, then check `complete` and inspect `stopped` before
using recovered code: a stopped trace can contain partial output. Your language
guide explains which calls block and which to await.

## Tests

The [five-runtime contract suite](tests/README.md) compares complete lossless
results, error codes, named API coverage, explicit cache/default behavior and
cross-runtime edits/encoding. Build matching native code and bindings first;
checks against an older binary do not prove the new API works. Individual
guides describe their focused language and TypeScript checks.

## Folder layout

Each existing SDK folder has its own README: [C](c/README.md),
[Node/TypeScript](node/README.md), [Python](python/README.md),
[Go](go/README.md), [Rust](rust/README.md), and [contract tests](tests/README.md).
Runnable recipes live in these guides; no separate examples directory is needed.
