# SDK contract verification

From the repository root, after building matching native code and bindings:

```sh
uv run --no-project python sdk/tests/contract.py
```

This runs the existing compatibility cases, strict TypeScript ESM checks,
registry-derived public API coverage and operation-oriented runtime cases in
Rust, C, Node, Python and Go. Any mismatch, missing named wrapper, missing tool
or compiler error fails; no runtime is silently omitted. It does not run Cargo,
regenerate artifacts, recover a large seed or access external HTTP services.

## Prerequisites

Use uv/Python 3.10+, Node with Node-API headers, Go 1.22+ with cgo, Rust/rustc
matching the native build, and a C compiler on macOS or Linux. Install the Node
SDK's development dependencies for the strict TypeScript compiler and Node
types. The native build must contain `libshape.dylib`/`libshape.so`, exactly one
native `deps/libshape-*.rlib` and one matching `deps/libserde_json-*.rlib` with
arbitrary precision enabled. The runners link these cached dependencies with
rustc and never rebuild the engine. The header and libraries must implement
contract v2 and the loaded artifact/kernel ABI.

The [provider registry](../operations.json) supplies the complete operation
inventory. Consumer adapters map those IDs to public language names: Python
uses snake_case with `seeds.import_seed`; Node uses camelCase; Go binds public
namespace objects and resolves exported methods. The Go namespaces include
`Ops.Classify`, `Trace.Corpus`, `Naming.Corpus`, `CleanupOperations`,
`HeaderOperations` and `PayloadOperations`. Missing paths fail before runtime
flow tests. There is no generic-call fallback in named adapters.

## External builds and isolated worker sources

Supply the directory containing the optimized library and native dependency
rlibs read-only. Each `--*-root` is a checkout root, not an SDK subdirectory:

```sh
uv run --no-project python sdk/tests/contract.py \
  --release-dir /absolute/provider/target/release \
  --provider-root /absolute/provider-checkout \
  --node-root /absolute/node-checkout \
  --python-root /absolute/python-checkout \
  --go-root /absolute/go-checkout \
  --output /tmp/shape-operation-contract
```

An optimized custom target directory works too. All roots default to this
checkout. `--registry` can override the default
`<provider-root>/sdk/operations.json`; use the version-controlled provider
artifact, rather than a second handwritten operation list. `--legacy-only`
runs just the retained compatibility cases against the new native build;
it does not prove the new operation API.

The output directory must not exist. Without `--output`, the temporary directory
is removed on exit. The harness snapshots the library, copies Node source and
builds its addon, and builds C/Rust/Go drivers under this isolated directory.
It supplies the provider header explicitly, including for the external Go
module. Installed libraries, source checkouts and the supplied build directory
are never modified. Normal compiler/uv caches can be reused. Do not replace
the native build while it is being snapshotted/linked.

The committed [sample32 pair](../c/tests/fixtures/README.md) provides a matching
artifact/capture covering a number, packed flags, a constant string and exact
64-bit metadata. Source compilation uses the committed public
[program/session/capture](../../tests/fixtures/sdk/decode/README.md). The tiny
public `javascript-vm-prefix.raw.js` fixture tests tracing and caching; it does
not perform expensive recovery of a real captured seed. All generated output
and disk caches remain under the temporary directory.

## What passes through each boundary

Legacy cases retain generic dispatch and `Generator`, including complete
version/catalogue, cleanup, schema, default/detailed/verified decode, explain,
object/text documents and sessions, unchanged full headers, DeviceMemory 32
edited to 8, and signals-only encoding followed by all 25 cross-runtime
decodes. Wrong sites, malformed documents, conflicting/missing encode inputs,
invalid flags and shapes retain their machine-readable error checks.

New cases use public named APIs for signal catalogue/description/finders,
optional naming and corpus analysis, classification, trace defaults, cache
policies, seed parsing/URL/client configuration, typed superpack and payload/
header framing. Compilation returns a loaded artifact in the consumer API;
the adapter deliberately exports and closes it for cross-language comparison.
It then loads the same compiled document in every runtime and verifies the
public capture. Required paths are derived from all provider registry IDs;
representative successful flows and validation groups exercise their behavior.
Coverage of a callable path is reported separately from runtime flow coverage.

Direct `decode`, `explain`, `schema` and export checks load once before three
repeated calls. Direct full-payload encode checks exact headers and edits; each
language's `encode_signals` output is decoded by all five runtimes. Mutable exported snapshots are changed before the calls to
confirm they cannot change native state. A loaded kernel traces after its
originating disk cache is removed and rejects mismatched raw source. Default
and explicit Disabled policies leave the configured environment cache absent;
Directory and Environment deliberately create their expected kernel entries.
Python/Node/Go runners also compare their process environment before/after each
call. Caller-owned result files are distinct from library cache behavior.

Strict TypeScript checks compile a standalone `.mts` consumer using ESM value
imports and `import type`, strict NodeNext resolution, verbatim module syntax
and unchecked indexed access. The compiler checks every registry-derived public
path, concrete loaded decode/edit/encode and typed trace/codec results, plus
negative option/handle assertions. Runtime Node imports the actual ESM entry.

## Precision and comparison

Results are compared recursively without excluding fields: object order is
irrelevant, array order matters, booleans differ from numbers, and integers do
not pass through floating point. Node uses `parseJSON`/`stringifyJSON`, Python
uses exact JSON integers, Go uses `json.RawMessage`/`json.Number` and explicit
u8 arrays, Rust links the SDK's arbitrary-precision serde_json, and C transports
JSON text unchanged. Detailed decode must contain an integer above 2^53.
Superpack additionally checks exact IEEE bit strings, special values and bytes.

Error status/code is stable; diagnostic wording is saved but not compared.
Signals-only encryption may choose fresh prefixes: each runtime's capture is
decoded by all five and its complete named map is compared. Retained ciphertext
metadata is still compared when runtimes decode that same capture.

Python's public legacy generic API takes keyword fields rather than raw request
text. The four raw top-level malformed/nonobject compatibility checks explicitly
skip Python. Their nested-document equivalents run in all five. Host argument
TypeErrors before serialization are outside the shared wire error contract.

With `--output`, `<case>.rust.json` and `<case>.<language>.json` retain complete
envelopes. The final RESULT includes checks, individual skips and failures.
Fix discrepancies in the responsible SDK; do not filter fields or relax numeric
comparison to hide them. See [VALIDATION.md](VALIDATION.md) for evidence and
remaining integration requirements.
