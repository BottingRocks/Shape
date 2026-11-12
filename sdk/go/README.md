# Go

The operation-oriented Go API traces source, compiles executable decode artifacts,
and decodes, explains and edits captured signals. Go 1.22+, cgo and a matching
native library are required. The package uses the standard library only.

<a id="installing-the-library-by-hand"></a>

Install from this checkout in your consumer module:

```sh
go mod edit -require=shape@v0.0.0
go mod edit -replace=shape=/absolute/path/to/repository/sdk/go
go mod tidy
```

The module path is `shape`; it is not a published registry module. Build the
native library with the repository's `scripts/build-sdk.sh` (release mode), then
build your consumer with `go build`. The default archive location is
`sdk/go/lib/libshape.a`. Alternatively use `-tags shape_external` with explicit
`CGO_CFLAGS=-I/absolute/path/to/sdk/c/include` and `CGO_LDFLAGS` pointing to the
matching library. Keep the header and library from the same build.

For external static linking, include the platform libraries:

- macOS: `-framework CoreFoundation -framework Security`
- Linux: `-ldl -lpthread -lm -lrt`

Each Go block below is a standalone `main.go`, with complete imports, input
filenames and error handling. Supply your own matching source/session/capture
files or the public synthetic fixtures for decode examples. Compilation checks
types; it does not establish that arbitrary captures match an artifact. No
`examples/` directory or network access is needed to compile these recipes.
Source and JSON arguments are content, not filenames; read files explicitly as
shown below. Operations that read a path, such as `TraceCorpusWalkSeed`, say so.

## Load once, decode repeatedly, explain on demand

Inputs: executable `decode-artifact.json` and capture files passed on the command
line. A capture is the actual suffix-keyed `siteId`/`headers` document, including
any `a0`, `a1` chunks. A descriptive schema or compiled kernel is not executable
decode input. Pass a canonical signal name actually present in the capture as
the first argument, followed by one or more capture filenames.

```go
package main

import (
    "encoding/json"
    "fmt"
    "log"
    "os"
    "shape"
)

func run() error {
    if len(os.Args) < 3 { return fmt.Errorf("usage: consumer SIGNAL capture.json [capture.json ...]") }
    document, err := os.ReadFile("decode-artifact.json")
    if err != nil { return err }
    loaded, err := shape.Artifact.Load(document)
    if err != nil { return err }
    defer loaded.Close()
    for _, path := range os.Args[2:] {
        data, err := os.ReadFile(path)
        if err != nil { return err }
        var capture shape.Capture
        if err := json.Unmarshal(data, &capture); err != nil { return err }
        decoded, err := shape.Decode(loaded, capture, shape.DecodeOptions{})
        if err != nil { return err }
        detail, err := shape.Explain(loaded, decoded, os.Args[1], shape.ExplainOptions{})
        if err != nil { return err }
        fmt.Println(path, decoded.PositionalSignals, detail.Status, detail.Fields)
    }
    return nil
}
func main() { if err := run(); err != nil { log.Fatal(err) } }
```

Load outside the loop for memory reuse; disk caching stays disabled. Decode
retains the original report by default.

Choose the extra work you need:

- `Explain` computes details for one requested signal.
- `DecodeOptions{Metadata: true}` requests all signal details.
- `DecodeOptions{Verify: true}` checks exact re-encoding.

Share loaded artifacts across goroutines; give each editor its own payload.
Defer `Close`: it waits for active calls, is idempotent, and rejects later calls
with `invalid_request`. Do not copy values that own loaded handles.

## Edit, encode, verify and persist

Inputs: matching executable `decode-artifact.json` and `headers.json` containing
`DeviceMemory.deviceMemory`. The public synthetic sample32 fixture supports
this edit. The result is a full capture document written to `headers.edited.json`.

```go
package main

import (
    "encoding/json"
    "fmt"
    "log"
    "os"
    "shape"
)

func run() error {
    artifact, err := os.ReadFile("decode-artifact.json")
    if err != nil { return err }
    loaded, err := shape.Artifact.Load(artifact)
    if err != nil { return err }
    defer loaded.Close()
    data, err := os.ReadFile("headers.json")
    if err != nil { return err }
    var capture shape.Capture
    if err := json.Unmarshal(data, &capture); err != nil { return err }
    decoded, err := shape.Decode(loaded, capture, shape.DecodeOptions{})
    if err != nil { return err }
    fields, ok := decoded.PositionalSignals["DeviceMemory"]
    if !ok { return fmt.Errorf("capture has no DeviceMemory signal") }
    fields["deviceMemory"] = json.Number("8")
    encoded, err := shape.Encode(loaded, decoded, nil)
    if err != nil { return err }
    checked, err := shape.Decode(loaded, *encoded, shape.DecodeOptions{Verify: true})
    if err != nil { return err }
    if checked.PositionalSignals["DeviceMemory"]["deviceMemory"] != json.Number("8") {
        return fmt.Errorf("edited value did not round-trip")
    }
    output, err := json.MarshalIndent(encoded, "", "  ")
    if err != nil { return err }
    return os.WriteFile("headers.edited.json", output, 0600)
}
func main() { if err := run(); err != nil { log.Fatal(err) } }
```

Keep `Metadata.OriginalReport` when editing: it retains channels, seeds and
per-signal prefixes. Encoding sends the current fields, not `Decoded.JSON()`'s
original snapshot.

Use `EncodeSignals` for bare named fields when original channels are unavailable.
It uses fresh prefixes, so it does not promise original header bytes. A session
override replaces the whole artifact session; use the same one for decode,
explain and encode.

## Trace raw source without disk cache, compile, decode

Inputs from one build/session: `seed.js`, `init.js`, `headers.json`. The capture
supplies its actual site ID. Tracing is expensive: use an optimized native
release library. The defaults clean threads in parallel, finish the whole
program and use available cores. Finishing normalizes parameters and captures
and substitutes kernel values. Naming is optional; finders and artifact
compilation can use the traced program directly.

```go
package main

import (
    "encoding/json"
    "fmt"
    "log"
    "os"
    "shape"
)

func run() error {
    source, err := os.ReadFile("seed.js")
    if err != nil { return err }
    initializer, err := os.ReadFile("init.js")
    if err != nil { return err }
    headers, err := os.ReadFile("headers.json")
    if err != nil { return err }
    var capture shape.Capture
    if err := json.Unmarshal(headers, &capture); err != nil { return err }
    traced, err := shape.Trace.Program(string(source), shape.ProgramOptions{})
    if err != nil { return err }
    if !traced.Complete { return fmt.Errorf("incomplete trace: %v", traced.Stopped) }
    session, err := shape.SessionFromInitializer(string(initializer))
    if err != nil { return err }
    loaded, err := shape.Artifact.Compile(traced.Code, shape.ArtifactOptions{
        Session: session, SiteID: capture.SiteID,
    })
    if err != nil { return err }
    defer loaded.Close()
    document, err := loaded.JSON()
    if err != nil { return err }
    if err := os.WriteFile("decode-artifact.json", document, 0600); err != nil { return err }
    if err := os.WriteFile("program.js", []byte(traced.Code), 0600); err != nil { return err }
    decoded, err := shape.Decode(loaded, capture, shape.DecodeOptions{})
    if err != nil { return err }
    fmt.Println(decoded.PositionalSignals)
    return nil
}
func main() { if err := run(); err != nil { log.Fatal(err) } }
```

`ProgramOptions{}` disables caching and uses a million steps, quiet operation,
parallel thread cleanup and whole-program finishing. Omitted `Jobs` uses available
cores. Pointer options keep explicit values: `NoCleanup=true` selects diagnostic
output that skips the full pipeline. Always check `Complete` before compiling.

Choose reuse deliberately:

| Reuse | What it avoids |
| --- | --- |
| Saved `program.js` | Repeating tracing |
| Compiled kernel | Recompiling source |
| Loaded decode artifact | Parsing and validating decode JSON on each call |

Saving outputs is explicit; kernel caching saves neither traces nor naming.
See the [operation mapping](operations.md) and [typed signatures](operation_coverage.json)
for flat functions and namespaces.

<a id="exact-values-errors-and-compatibility"></a>

## Preserve exact values

Dynamic decoded numbers use `json.Number`; large or evolving documents use
`json.RawMessage`. Keep exact integers as `int64`, `uint64`, number digits or
raw JSON. Never convert them through `float64`.

To save edits, use `json.Marshal(decoded)`. `decoded.JSON()` returns a copy of
the original snapshot. Superpack number values preserve IEEE-754 bits.

## Handle errors and expensive calls

Use `errors.As` to find `*shape.Error`, then inspect `Code`; messages are diagnostic.

| Code | Next step |
| --- | --- |
| `invalid_request` | Check required fields, types and whether the handle is closed |
| `unknown_command` | Check the operation name and matching installed library |
| `failed` | Inspect input documents and the diagnostic message; artifact validation uses this code |
| `panic` | Retain diagnostics for investigation; process aborts cannot be caught |

Calls are synchronous and may occupy an OS thread. Bound steps/jobs and
application concurrency. Check incomplete traces' stop facts even without errors.

## Choose disk cache explicitly

Input: `seed.js`. Pass `off`, `directory`, or `environment` as the command-line
argument. This runs one trace with the selected policy. Directory caching opts
into `kernel-cache`; Environment explicitly reads the native environment policy
(`SHAPE_KERNEL_CACHE` and its defaults). The SDK never changes environment
variables. Disabled is the default even when that environment variable is set.

```go
package main

import (
    "fmt"
    "log"
    "os"
    "shape"
)

func run() error {
    if len(os.Args) != 2 { return fmt.Errorf("usage: consumer off|directory|environment") }
    policy := shape.CachePolicy{}
    switch os.Args[1] {
    case "off": policy.Mode = shape.CacheDisabled
    case "directory": policy = shape.CachePolicy{Mode: shape.CacheDirectory, Directory: "kernel-cache"}
    case "environment": policy.Mode = shape.CacheEnvironment
    default: return fmt.Errorf("unknown cache policy %q", os.Args[1])
    }
    source, err := os.ReadFile("seed.js")
    if err != nil { return err }
    program, err := shape.Trace.Program(string(source), shape.ProgramOptions{Cache: policy})
    if err != nil { return err }
    if !program.Complete { return fmt.Errorf("incomplete trace: %v", program.Stopped) }
    return os.WriteFile("program.js", []byte(program.Code), 0600)
}
func main() { if err := run(); err != nil { log.Fatal(err) } }
```

Directory entries validate source identity: an invalid or stale explicitly
selected entry is an error. Cache policy concerns compiled kernels, not saved
programs, decode artifacts or loaded-handle lifetime. Use `ArtifactOptions.Cache`
when compiling directly from `InputRaw`; program input does not need a kernel
cache.

## Reuse an explicit compiled kernel

Inputs: `seed.js` and its matching compiled `kernel.json`. Obtain the latter
with `Report`, `Deob` with `Artifact: true`, or `TraceCorpusLoad`. Keep the raw
source identity: a kernel from another build is rejected. No disk cache lookup
occurs when using the loaded kernel below.

```go
package main

import (
    "fmt"
    "log"
    "os"
    "shape"
)

func run() error {
    document, err := os.ReadFile("kernel.json")
    if err != nil { return err }
    source, err := os.ReadFile("seed.js")
    if err != nil { return err }
    kernel, err := shape.Kernel.Load(document)
    if err != nil { return err }
    defer kernel.Close()
    traced, err := kernel.TraceProgram(string(source), shape.ProgramOptions{})
    if err != nil { return err }
    if !traced.Complete { return fmt.Errorf("incomplete trace: %v", traced.Stopped) }
    serialized, err := kernel.JSON()
    if err != nil { return err }
    if err := os.WriteFile("kernel.saved.json", serialized, 0600); err != nil { return err }
    return os.WriteFile("program.js", []byte(traced.Code), 0600)
}
func main() { if err := run(); err != nil { log.Fatal(err) } }
```

The lineage is raw source → compiled kernel → traced program → executable
decode artifact with matching session/site → captures and named values. A
kernel cannot replace an executable decode artifact; a compact schema is
descriptive and cannot replace either.

For investigations, `LoadedKernel.TraceThread` and `.Reachable` reuse the loaded
kernel. Check completion before using their output:

- Thread results: inspect `Complete`, nullable `Code`, `Stop` and self-check diagnostics.
- Reachable results: inspect every fingerprint's stop facts; limits may leave functions unwalked.
- Whole programs: require completeness before treating generated text as finished.

## Optional naming, catalog and source schema

Input: complete traced `program.js`. Pass a canonical signal name to inspect,
for example `DeviceMemory`. These are source operations: the detailed schema,
properties, inputs and contracts parse/analyze the program, while the catalog
is static. Compact source schema is descriptive and requires no session.
Optional naming writes a separate presentation copy; compilation can use the
original program directly.

```go
package main

import (
    "encoding/json"
    "fmt"
    "log"
    "os"
    "shape"
)

func run() error {
    if len(os.Args) != 2 { return fmt.Errorf("usage: consumer CANONICAL_SIGNAL_NAME") }
    data, err := os.ReadFile("program.js")
    if err != nil { return err }
    source := string(data)
    catalog, err := shape.Signals.Catalog()
    if err != nil { return err }
    card, err := shape.Signals.Describe(os.Args[1])
    if err != nil { return err }
    fmt.Println(string(catalog), string(card))
    outputs := []struct { name string; run func() (json.RawMessage, error) }{
        {"schema.compact.json", func() (json.RawMessage, error) { return shape.Signals.CompactSchema(source, shape.SourceSchemaOptions{}) }},
        {"schema.detailed.json", func() (json.RawMessage, error) { return shape.Signals.Schema(source, shape.SignalsSchemaOptions{}) }},
        {"signals.properties.json", func() (json.RawMessage, error) { return shape.Signals.Show(source) }},
        {"signals.inputs.json", func() (json.RawMessage, error) { return shape.Signals.Inputs(source) }},
        {"signals.contracts.json", func() (json.RawMessage, error) { return shape.Signals.Contracts(source) }},
    }
    for _, output := range outputs {
        result, err := output.run()
        if err != nil { return err }
        if err := os.WriteFile(output.name, result, 0600); err != nil { return err }
    }
    named, err := shape.Naming.Threads(source, shape.NamingThreadsOptions{Rename: true})
    if err != nil { return err }
    return os.WriteFile("program.named.js", []byte(named.Source), 0600)
}
func main() { if err := run(); err != nil { log.Fatal(err) } }
```

For names without rewriting, use `NamingPlan`; for one emitted function, use
`CleanupThread`. `TraceThread` takes exactly one source or kernel artifact,
with source-derived PC/state and slot inputs.

`SessionFromInitializer` extracts a session without compilation; `Pin` takes
exactly one session or initializer. Other naming, source and VM inspection
tools are in the [operation mapping](operations.md).

## Compare tracing and naming corpora

Inputs: `corpus.before.json` and `corpus.after.json`, each an object mapping
seed names to function-key/fingerprint objects. This recipe compares explicit
saved documents; it does not scan directories, retrace, or acquire seeds.

```go
package main

import (
    "fmt"
    "log"
    "os"
    "shape"
)

func run() error {
    before, err := os.ReadFile("corpus.before.json")
    if err != nil { return err }
    after, err := os.ReadFile("corpus.after.json")
    if err != nil { return err }
    changes, err := shape.Trace.Corpus.Compare(before, after)
    if err != nil { return err }
    fmt.Println("regressions", changes.Regressions, "changes", changes.Changes)
    return nil
}
func main() { if err := run(); err != nil { log.Fatal(err) } }
```

Choose corpus inputs:

| Task | Input and behavior |
| --- | --- |
| Trace source with `TraceCorpusSource` | Raw source plus cache/reachability options |
| Walk a kernel with `TraceCorpusReachable` | Kernel document; returns `[]Fingerprint` with function records |
| Walk a file with `TraceCorpusWalkSeed` | Filename; reads it and may write branches when requested; includes convergence/self-checks |
| Compare naming with `NamingCorpusCompare` | Two `[]NamedSource` arrays; reconstructs plans before comparing |

Corpus defaults: one worker, a million steps, no cleanup. Naming source names
must be nonempty and unique. See the [operation mapping](operations.md) for
fingerprint replay, inline comparisons and classification.

## Parse and explicitly import a seed

Input: local fetched `seed.js`; argument: its actual matching kernel URL.
This recipe checks local bytes and explicitly stores the validated kernel in
`seed-store`. It performs no network request. Keep the supplied URL and source
associated with the same seed.

```go
package main

import (
    "fmt"
    "log"
    "os"
    "shape"
)

func run() error {
    if len(os.Args) != 2 { return fmt.Errorf("pass the matching kernel URL") }
    data, err := os.ReadFile("seed.js")
    if err != nil { return err }
    script, err := shape.Seeds.Parse(data)
    if err != nil { return err }
    if script.Kind != "kernel" { return fmt.Errorf("expected kernel, got %s", script.Kind) }
    token, err := shape.Seeds.Token(os.Args[1], "seed")
    if err != nil { return err }
    stored, err := shape.Seeds.Import("seed-store", token, nil, data)
    if err != nil { return err }
    fmt.Println(stored.Status, stored.KernelPath)
    return nil
}
func main() { if err := run(); err != nil { log.Fatal(err) } }
```

Choose each seed step explicitly:

| Task | API and side effects |
| --- | --- |
| Parse a launcher and resolve its reference | `SeedsParseLauncher`, then `SeedsResolveURL` with a base |
| Validate a URL or kernel | `SeedsParseURL`, `SeedsParseKernel` |
| Inspect the site registry | `SeedsSites`, `SeedsSite`; no fetch |
| Acquire bytes | `SeedsFetch`, `SeedsFetchDirect`; network access, no persistence |
| Validate and save bytes | `SeedsImport`; writes to the caller's directory |

Set timeouts, body/redirect limits in `FetchOptions.Client`, and ordered headers
in `FetchOptions.Headers`. `SeedsClient` validates configuration. Custom HTTP
transports can supply bytes to parsing/import. Import validates structure;
you remain responsible for the matching URL/token/source association.

## Frame payload bytes and headers

Input: `profile.json`, a real source-derived header `Profile` document including
format, cipher, keys, alphabet and header name. This synthetic byte round trip
uses actual profile geometry and explicit seeds; it is not a captured signal.
Successful decryption does not establish authenticity or signal meaning.

```go
package main

import (
    "bytes"
    "encoding/json"
    "fmt"
    "log"
    "os"
    "shape"
)

func run() error {
    document, err := os.ReadFile("profile.json")
    if err != nil { return err }
    var profile shape.HeaderProfile
    if err := json.Unmarshal(document, &profile); err != nil { return err }
    checked, err := shape.HeaderOperations.Codec(profile)
    if err != nil { return err }
    plaintext := []byte("synthetic framing sample")
    text, err := shape.PayloadOperations.EncodeBytes(checked.PayloadProfile, plaintext, [2]uint32{1, 2})
    if err != nil { return err }
    headers, err := shape.HeaderOperations.EncodeFinalized(*checked, text, nil)
    if err != nil { return err }
    finalized, err := shape.HeaderOperations.DecodeFinalized(*checked, headers)
    if err != nil { return err }
    payload, err := shape.PayloadOperations.DecodeBytes(checked.PayloadProfile, finalized.Text)
    if err != nil { return err }
    if payload.Data.Kind != "raw" || !bytes.Equal(payload.Data.Bytes, plaintext) {
        return fmt.Errorf("payload bytes changed")
    }
    output, err := json.MarshalIndent(headers, "", "  ")
    if err != nil { return err }
    return os.WriteFile("headers.synthetic.json", output, 0600)
}
func main() { if err := run(); err != nil { log.Fatal(err) } }
```

Keep the framing context when changing payloads:

- `Headers` uses full names; named decode captures use suffix keys.
- Report decoding needs source-derived positional count and ordered keysets in
  `ReportGeometry`. Ciphertext alone cannot supply them.
- Preserve seeds and sibling `HeaderContext` when re-encoding. Finalized
  operations retain that context.

`HeadersParse` reads header text; encode/decode uses `FramedReport`.
`PayloadCodec` validates profiles; `PayloadEncode`/`PayloadDecode` handle generic
reports without interpreting signals. Pass profiles per call. See
[framing types](framing.go) and the [operation mapping](operations.md).

## Pack generic values independently

No input files or cipher profile are needed. Superpack preserves undefined,
null, binary, dates, objects and explicit number bits. This public synthetic
example packs an array and checks canonical decoding.

```go
package main

import (
    "fmt"
    "log"
    "reflect"
    "shape"
)

func run() error {
    value := shape.PackValue{Kind: shape.PackArray, Array: []shape.PackValue{
        {Kind: shape.PackBool, Bool: true},
        {Kind: shape.PackString, Text: "sample"},
        {Kind: shape.PackNumber, NumberBits: 0x8000000000000000}, // negative zero
    }}
    options := shape.SerializerOptions{}
    packed, err := shape.Pack(value, options)
    if err != nil { return err }
    recovered, err := shape.UnpackCanonical(packed, options)
    if err != nil { return err }
    if !reflect.DeepEqual(*recovered, value) { return fmt.Errorf("packed value changed") }
    fmt.Println(len(packed), "packed bytes")
    return nil
}
func main() { if err := run(); err != nil { log.Fatal(err) } }
```

`Unpack` also returns original bytes and canonicality. `SerializerOptions`
controls omitted keysets, depth truncation, extensions and resource limits.
Use matching options for encode/decode. `PackEntry` preserves object entry
order, `NumberBits` is exact `uint64`, and its wire representation is 16 hex
digits. `Bytes` always serializes as a JSON u8 array, including empty bytes.

## Verification and troubleshooting

Run the focused suite with the public synthetic fixtures, from `sdk/go`:

```sh
SHAPE_TEST_ARTIFACT="$PWD/../c/tests/fixtures/artifact.json" \
SHAPE_TEST_HEADERS="$PWD/../c/tests/fixtures/capture.json" \
go test -race ./...
go vet ./...
```

For external linking, pass the same `-tags shape_external`, `CGO_CFLAGS` and
`CGO_LDFLAGS` to both commands. Runtime checks use the public fixtures. Compile
the README recipes in a temporary consumer module with the `replace` directive
above.

| Symptom | Check |
| --- | --- |
| Missing archive or new C symbols | Build a matching release library and select its header/link paths |
| `package shape is not in std` | Add the consumer module's `replace` directive |
| cgo disabled or missing compiler | Enable cgo and install the platform C toolchain |
| Artifact fails to load | Supply executable decode JSON rather than kernel/schema JSON |
| Site/session mismatch | Use inputs from the same build/session and actual site ID |
| Explicit cache load fails | Verify source identity and canonical kernel format |
| Missing thread code/incomplete trace | Inspect stop facts and step budget |
| Edit encoding fails | Keep the original report and choose a supported field/value |
| Lost integer digits | Remove conversions through floating point |
| Calls after Close fail | Load a new artifact; closed handles are not reusable |
| Slow seed recovery | Use optimized native builds, full cleanup defaults and the appropriate reuse boundary |
