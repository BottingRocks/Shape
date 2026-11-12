# NodeJS

Use named operations to analyze source, compile artifacts, and decode or edit
captured fields. Load an executable artifact once and keep it outside capture
loops. Node 18+ is supported, with ESM imports, strict TypeScript declarations,
and compatible CommonJS consumers. No runtime npm dependencies are needed.

## Load once, decode repeatedly

Install the built local package as described below. Save each JavaScript recipe
as a separate `.mjs` file and run it with `node recipe.mjs`. Recipes using files
require your own matching inputs; they do not acquire captures. This recipe uses
`decode-artifact.json` and `headers.json` from the same build/site/session.
Pass JSON content or bytes to document operations; a string filename is not
loaded automatically.

```js
import { readFileSync } from 'node:fs';
import { artifact, decode, explain, schema } from 'shape';

const loaded = artifact.load(readFileSync('decode-artifact.json'));
try {
  const capture = readFileSync('headers.json');
  for (let i = 0; i < 3; i++) {
    const decoded = decode(loaded, capture);
    console.log(decoded.positionalSignals);
    if (decoded.positionalSignals.DeviceMemory) {
      console.log(explain(loaded, decoded, 'DeviceMemory').fields);
    }
  }
  console.log(schema(loaded));
} finally {
  loaded.close();
}
```

Load outside the loop; close when finished. Decoded payloads are independent
editable documents. Keep `metadata.originalReport` for encoding; decode retains
it by default.

Choose how much work each decode does:

- `explain(loaded, decoded, signal)` computes details for one signal on demand.
- `metadata: true` requests all signal details.
- `verify: true` checks exact re-encoding independently.

Both decode options default to false.
See the [typed TypeScript guide](TYPESCRIPT.md) for complete `.mts` programs.

## Edit, encode, verify

This independent recipe needs the same two files and a capture containing a
supported numeric `DeviceMemory.deviceMemory` field.

```js
import { readFileSync, writeFileSync } from 'node:fs';
import { artifact, decode, encode, stringifyJSON } from 'shape';

const loaded = artifact.load(readFileSync('decode-artifact.json'));
try {
  const decoded = decode(loaded, readFileSync('headers.json'));
  const memory = decoded.positionalSignals.DeviceMemory;
  if (!memory || typeof memory.deviceMemory !== 'number') {
    throw new Error('Capture needs numeric DeviceMemory.deviceMemory');
  }
  memory.deviceMemory = 16;
  const edited = encode(loaded, decoded);
  const checked = decode(loaded, edited, { verify: true });
  if (checked.positionalSignals.DeviceMemory?.deviceMemory !== 16) {
    throw new Error('Edited value did not survive encoding');
  }
  writeFileSync('edited-headers.json', stringifyJSON(edited));
} finally {
  loaded.close();
}
```

Encoding an unchanged full payload preserves retained report channels, seeds,
keysets and random prefixes when the inputs match. Encoding supported edits
checks field round trips. `encodeSignals(loaded, namedFields)` uses fresh random
prefixes and layout defaults for other channels; it does not reproduce capture
bytes.

An optional `session` replaces the artifact's whole session. Use that same
replacement for decode, explain and encode.

## Raw seed to executable artifact

Supply matching `seed.js`, `session.json`, and `headers.json`. Pass the site's
actual ID as `node recipe.mjs SITE_ID`. Use a release native library for this
expensive workflow. Tracing uses available cores when `jobs` is omitted/null;
`jobs: 4` below deliberately bounds the request. Complete tracing is required.

```js
import { readFileSync, writeFileSync } from 'node:fs';
import { trace, artifact, decode, CachePolicy, stringifyJSON } from 'shape';

const siteId = process.argv[2];
if (!siteId) throw new Error('Pass the matching site ID');
const traced = await trace.program(readFileSync('seed.js'), {
  cache: CachePolicy.Disabled, jobs: 4,
  threadCleanup: true, noCleanup: false,
});
if (!traced.complete) throw new Error(stringifyJSON(traced.stopped));
writeFileSync('program.js', traced.code);
const loaded = await artifact.compile(traced.code, {
  session: readFileSync('session.json'), siteId,
});
try {
  console.log(decode(loaded, readFileSync('headers.json')).positionalSignals);
  writeFileSync('decode-artifact.json', stringifyJSON(loaded.toJSON()));
} finally {
  loaded.close();
}
```

`artifact.compile(seed, { inputKind: 'raw', session, siteId })` traces and
compiles directly to a loaded artifact. Export `loaded.toJSON()` to save it.

The default pipeline disables disk caching, cleans threads in parallel, and
finishes the whole program (`noCleanup: false`). Finishing normalizes parameters
and captures and substitutes kernel values. Naming is optional.

For diagnostic output, explicitly set `noCleanup: true` to skip the full pipeline.
Compilation otherwise uses strict checking (`direct: false`) and little-endian
encoding. `direct: true` allows source-format decoding while bypassing lifecycle
checks; unsupported source constructs can still fail.

## Cache choices and kernel reuse

A disk cache avoids compiling a kernel again. A loaded kernel reuses parsed
state in memory; neither implies saved traces or naming results. Only the
caller persists outputs. This complete recipe reads `seed.js`, explicitly
populates `kernel-cache`, then loads the returned kernel for repeated tracing.

```js
import { readFileSync, writeFileSync } from 'node:fs';
import { trace, kernel, CachePolicy, stringifyJSON } from 'shape';

const source = readFileSync('seed.js');
const document = await trace.corpus.load(source, {
  name: 'seed.js', cache: CachePolicy.Directory('kernel-cache'),
});
const loaded = kernel.load(document);
try {
  const result = await trace.programWithArtifact(source, loaded);
  if (!result.complete) throw new Error(stringifyJSON(result.stopped));
  writeFileSync('program.js', result.code);
  writeFileSync('kernel.json', stringifyJSON(loaded.toJSON()));
} finally {
  loaded.close();
}
```

| Policy | Effect on this call |
| --- | --- |
| Omitted / `CachePolicy.Disabled` | No disk kernel cache access, even if an environment cache is configured |
| `CachePolicy.Directory(path)` | Explicit kernel cache directory; validates source identity |
| `CachePolicy.Environment` | Explicitly consult legacy `SHAPE_KERNEL_CACHE`/default cache policy |

The SDK never changes process environment variables. The deprecated `reportDir`
named option translates to Directory and cannot be combined with `cache`.

Pass source text or bytes to `trace.corpus.load` and `trace.corpus.source`.
Their `name` option labels that content; it is not a filename to open.
`trace.corpus.walkSeed(path, options)` explicitly reads a file and may write
branch output if `branches` is supplied. Loaded kernels require matching raw
source identity for program tracing.

## Choose the artifact boundary

| Input/output | Purpose and persistence |
| --- | --- |
| Raw `seed.js` | Input to deobfuscation or tracing |
| Kernel artifact (`shape-kernel-artifact/v1`) | Compiled VM facts; `kernel.load` reuses in memory |
| Traced `program.js` | Reuse to skip tracing; input to finders and compilation |
| Session document | Matching transaction token/alphabet/keys from capture |
| Executable decode artifact (`shape-decode-artifact/1`) | Load once with `artifact.load`; export deliberately with `loaded.toJSON()` |
| Compact schema (`shape-seed-schema/1`) | Descriptive export; cannot execute decoding |
| Capture headers | Site ID plus suffix-keyed header map |
| Decoded payload | Editable named fields and retained raw report |

Optional naming is for presentation and analysis after tracing. This independent
recipe reads a complete `program.js` and writes an explicitly requested rewrite.

```js
import { readFileSync, writeFileSync } from 'node:fs';
import { naming, stringifyJSON } from 'shape';

const program = readFileSync('program.js');
const named = await naming.threads(program, { rename: true });
writeFileSync('program.named.js', named.source);
writeFileSync('naming.json', stringifyJSON(await naming.plan(program)));
```

## Signal catalog and schemas

Catalog descriptions and illustrative values are static documentation, not
observations of your capture. Supply `program.js` for this complete recipe.

```js
import { readFileSync, writeFileSync } from 'node:fs';
import { signals, stringifyJSON } from 'shape';

const program = readFileSync('program.js');
console.log((await signals.catalog()).map(card => card.signal));
console.log(await signals.describe('DeviceMemory'));
console.log(await signals.show(program));
writeFileSync('signal-schema.json', stringifyJSON(await signals.schema(program)));
writeFileSync('compact-schema.json', stringifyJSON(await signals.compactSchema(program)));
```

`signals.inputs` and `signals.contracts` expose source-derived reports.
`artifact.compactSchema(document)` exports a schema from an executable document;
`schema(loaded)` exports it from the already loaded state.

## Deobfuscation, kernel and operations

This independent recipe reads raw `seed.js`. Kernel/operation inspection needs a
supported VM seed, rather than arbitrary JavaScript.

```js
import { readFileSync, writeFileSync } from 'node:fs';
import { deob, kernel, ops, stringifyJSON } from 'shape';

const source = readFileSync('seed.js');
const result = await deob(source, { artifact: true });
writeFileSync('deobfuscated.js', result.code);
writeFileSync('kernel.json', stringifyJSON(result.artifact));
console.log(await kernel.show(source));
console.log(await ops.list(source));
console.log(await ops.classify.source(source));
```

For further inspection, select the task you need:

| Task | API |
| --- | --- |
| Compile a kernel from raw or deobfuscated source | `report(source, { inputKind })` |
| Inspect an operation key returned by `ops.list` | `ops.show(source, operationKey)` (u32 key) |
| Classify a report from raw source | `ops.classify.report(source)` |
| Classify one handler by index | `ops.classify.template(source, handlerIndex)` |
| Compare classifications across seed variants | `ops.classify.seeds(classifications)` |

Already have serialized analysis documents? The
[declarations](index.d.ts) also describe `ops.classify.reportDocument` and
`ops.classify.templateDocument`, which validate those documents.

## Investigate a thread

Supply `kernel.json` and known start coordinates as
`node recipe.mjs PC STATE`. Incomplete results are diagnostic output; check
completion before treating emitted code as a finished program.

```js
import { readFileSync } from 'node:fs';
import { kernel } from 'shape';

const [pc, state] = process.argv.slice(2).map(Number);
if (!Number.isSafeInteger(pc) || pc < 0 || !Number.isSafeInteger(state) || state < 0) {
  throw new Error('Pass nonnegative PC and STATE');
}
const loaded = kernel.load(readFileSync('kernel.json'));
try {
  const result = await loaded.thread({ pc, state, steps: 1000000, partial: true });
  console.log({ complete: result.complete, stop: result.stop, diagnostics: result.self_check });
  if (result.code !== null) console.log(result.code);
} finally {
  loaded.close();
}
```

`trace.thread(source, options)` compiles a kernel from source, while
`trace.thread(null, { artifact: document, pc, state })` accepts a document.
`descriptor`, `initialSlots`, `inputs`, branch overrides, outlining and self
checks are explicit options. `cleanup.thread(emittedSource)` cleans one emitted
thread independently.

## Advanced operation groups

Use the [API map](api-map.json) for the full operation list and
[domain declarations](domain.d.ts) for document types. Wrapper options use
camelCase; returned documents keep their serialized field names.

For corpus comparisons and optional naming, see the [API signatures](index.d.ts).
For header/payload framing and generic packed values, see below.

### Parse, fetch and store seeds

- Parse bytes with `seeds.parse`, or extract a launcher reference with
  `seeds.parseLauncher` and resolve it with `seeds.resolveUrl(reference, base)`.
- Look up configured sites with `seeds.sites()` or `seeds.site(name)`; neither fetches.
- Fetch bytes with `seeds.fetch` or `seeds.fetchDirect`. Use `seeds.client` to
  validate timeout, body-size and redirect limits; fetching does not save files.
- Save validated bytes with `seeds.import` to an explicit directory. Keep the
  URL, token and source matched. Custom HTTP clients can also supply bytes.

### Frame full headers or payloads

Header operations use full header names; named decoding uses suffix-keyed
captures. Decode reports with the source-derived positional count and ordered
keysets. Retain seeds and sibling header context when editing and re-encoding.
Codec factories validate profiles, which you pass to each later call.

### Pack generic values

This independent synthetic pack example needs no input files.

```js
import assert from 'node:assert/strict';
import { pack, unpackCanonical } from 'shape';

const value = { kind: 'array', value: [
  { kind: 'bool', value: true }, { kind: 'string', value: 'sample' },
  { kind: 'number', value: '8000000000000000' },
] };
assert.deepEqual(await unpackCanonical(await pack(value)), value);
```

Framing is unauthenticated and does not establish signal meaning. Low-level
report channels use tagged `PackValue` values; named signal fields use ordinary
JSON values. Number bit strings have exactly sixteen hexadecimal digits.

## Install and build

This package is local to the checkout. Build `libshape` in release mode using
the repository's SDK build process, or provide an already built matching
library/header pair. The Node-only build invokes `cc`, never Cargo:

```sh
SHAPE_LIBRARY_DIR=/absolute/path/to/release-library \
SHAPE_INCLUDE_DIR=/absolute/path/to/matching/include \
  sh sdk/node/build.sh
```

Then install from your application directory:

```sh
npm install --install-links /absolute/path/to/repository/sdk/node
```

The build needs a C compiler and Node's `node_api.h` headers (override with
`NODE_INCLUDE_DIR`). It copies the library beside `build/shape.node` and uses a
relative loader path. Ship both for the target platform/architecture.
`SHAPE_ADDON_PATH` explicitly overrides the addon loaded at startup.

| Setup failure | Action |
| --- | --- |
| `node_api.h` missing | Point `NODE_INCLUDE_DIR` to the Node headers |
| `libshape` or `shape.h` missing | Set the library/include paths above to a matching release pair |
| Addon cannot load | Check `build/shape.node`, its adjacent library, and the target platform/architecture |

## Types and precision

TypeScript uses supported ESM imports with NodeNext resolution. The package
has distinct ESM and CommonJS declaration entries; [TYPESCRIPT.md](TYPESCRIPT.md)
contains complete typed workflows and a compilation command.

Never route 64-bit integers through JavaScript `Number`. Pass artifact/capture
bytes directly, or use `parseJSON` / `stringifyJSON`. Unsafe integer tokens
become `bigint` and serialize back as exact JSON integers, including on Node
18/20. Plain `JSON.parse`/`JSON.stringify` cannot preserve this contract.

## Errors and lifetime

Inspect `ShapeError.code`; messages are diagnostic. Wrong JavaScript argument
types can throw `TypeError`.

| Code | Next step |
| --- | --- |
| `invalid_request` | Check required fields, option types and whether the handle is closed |
| `unknown_command` | Check the operation name and matching installed library |
| `failed` | Check input documents and the diagnostic message |
| `panic` | Retain diagnostics for investigation; process aborts cannot be caught |

Check trace completion and stop facts before using emitted code as finished.

### Finish work before closing

Command calls and `*Async` methods use libuv's pool: await them inside `try`
before `finally` closes the handle. Named synchronous calls such as `decode`
block JavaScript.

Concurrent reads share immutable state. Idempotent `close()` rejects new calls
and releases state after pending calls finish; GC also frees forgotten handles.
Load handles separately in each Node worker; they are not transferable.

## Checks

```sh
npm --prefix sdk/node ci --ignore-scripts
npm --prefix sdk/node test
npm --prefix sdk/node run test:types
npm --prefix sdk/node run test:docs
```

The [test guide](test/README.md) documents public fixtures, lifecycle/precision
coverage and recipe compilation. The [addon guide](src/README.md) explains
native retention and building against matching release libraries. Recipe syntax
and type checks do not establish that arbitrary captures match their artifacts.
