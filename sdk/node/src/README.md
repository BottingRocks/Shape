# Native Node-API bridge

[`addon.c`](addon.c) transports exact JSON text and artifact bytes between
JavaScript and the public C ABI. [`../index.js`](../index.js) parses envelopes
losslessly and raises `ShapeError`; [`../operations.js`](../operations.js)
adapts named operations to the authoritative [v2 contract](../../operations.json).

The addon exports legacy stateless dispatch and separate v2 dispatch.
`artifactCompileAsync` transfers `shape_artifact_compile` output directly into
a retained holder, without serializing/loading an intermediate artifact. Artifact
and kernel holders retain immutable native handles. Each queued call increments
a pending count on the JavaScript thread; close/finalization delays native free
until every pending call finishes. Worker environments create their own holders.
C ABI result strings are always released using `shape_free`.

## Build

Use a matching optimized native library/header pair. This script compiles only
the C addon and copies its library and header into ignored `build/`:

```sh
SHAPE_LIBRARY_DIR=/absolute/path/to/native/library-directory \
SHAPE_INCLUDE_DIR=/absolute/path/to/native/include \
  sh sdk/node/build.sh
```

A C compiler, Node 18+ and `node_api.h` are required. Set `NODE_INCLUDE_DIR` if
Node's headers are elsewhere. `SHAPE_ADDON_PATH` selects the addon at load time.
The default library search is `SHAPE_LIBRARY_DIR`, `sdk/node/lib`, repository
`target/release`, `target/iter`, then `build`. Set both explicit paths when
validating a new ABI so a stale repository header cannot be selected.

## Verification

```sh
npm --prefix sdk/node test
```

The [test guide](../test/README.md) describes focused checks, fixture selection,
GC/concurrency coverage and public operation parity. See the
[Node guide](../README.md) for usage and [TypeScript guide](../TYPESCRIPT.md)
for typed ESM programs.
