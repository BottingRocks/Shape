# Node SDK checks

Runtime tests use real native calls and checked-in synthetic/public fixtures.
The package command runs the legacy suites and the new operation suite:

```sh
npm --prefix sdk/node test
```

Build the addon against the exact native library and matching header before
running this command; see the [addon guide](../src/README.md). The Node build
script invokes a C compiler and does not invoke Cargo.

| File | Coverage |
| --- | --- |
| `index.js` | Entry for the four legacy runtime suites |
| `shape.test.js` | Legacy commands/errors, off-loop work, concurrency, ESM identity |
| `generator.test.js` | Optional matching capture decode/explain/edit/encode/schema and native load precision |
| `validation.test.js` | Request validation, direct opt-in, close during async calls, GC and Node workers |
| `json.test.js` | Native and forced Node 18/20 lossless JSON paths, exact integer/string/key handling |
| `artifact.js` | Minimal synthetic artifact for lifecycle checks |
| `operations.test.mjs` | Every v2 operation mapped to a named API; real compile/load/reuse/edit, caches, kernel identity/thread completion, classification/corpus, acquisition/import, header/payload/pack engines, u64 decoded metadata |
| `types.ts`, `result-contracts.ts` | Legacy compile contracts and nullable/required result members |
| `operations.mts` | Self-package ESM type resolution, typed operation families, invalid-input negative assertions |
| `docs.mjs` | Syntax checks every JavaScript README block and strictly compiles every TypeScript guide block in temporary files |

The operation suite uses `tests/fixtures/sdk/decode`,
`tests/fixtures/sdk/generator`, the small `javascript-vm-prefix.raw.js` topology,
and the public JavaScript kernel fixture. Its HTTP test listens on loopback;
there is no external acquisition. Temporary cache/store directories are removed.
The small VM topology contains no collectors, so it verifies tracing/reuse;
source-to-signal compilation uses the separate public traced-program fixture.
It does not establish raw recovery support for arbitrary private captures.

Enable legacy capture round trips using the committed public pair:

```sh
SHAPE_TEST_ARTIFACT="$PWD/sdk/c/tests/fixtures/artifact.json" \
SHAPE_TEST_HEADERS="$PWD/sdk/c/tests/fixtures/capture.json" \
  npm --prefix sdk/node test
```

Optional `SHAPE_TEST_SESSION` supplies a matching session override. The legacy
artifact-object precision case skips when that artifact contains no unsafe
integers; the new operation suite explicitly verifies real u64 decoded metadata,
and standalone JSON tests cover signed/u64 bounds. Other legacy capture tests
require DeviceMemory value 32 and a nonempty decoded set with matching counts.

Install cached development dependencies and compile declarations/recipes:

```sh
npm --prefix sdk/node ci --ignore-scripts
npm --prefix sdk/node run test:types
npm --prefix sdk/node run test:docs
```

Documentation checks compile complete ESM `.mts` programs through the package's
conditional export declarations with `strict` and `noUncheckedIndexedAccess`.
They do not execute recipes requiring caller-supplied files. The operation
runtime tests exercise their underlying public fixture workflows.

For a focused suite, run `node --test sdk/node/test/operations.test.mjs` or
`node --test sdk/node/test/validation.test.js`. The
[API map](../api-map.json) matches the authoritative shared operation inventory;
[Node](../README.md) and [TypeScript](../TYPESCRIPT.md) guides document usage.
