# TypeScript

Install the built local SDK and development types in your consumer directory:

```sh
npm install --install-links /absolute/path/to/repository/sdk/node
npm install --save-dev typescript@~5.7.3 @types/node@^22
```

Each TypeScript block below is a standalone `recipe.mts`. Compile and run one
from that directory with:

```sh
npx tsc recipe.mts --strict --noUncheckedIndexedAccess --target ES2022 \
  --module NodeNext --moduleResolution NodeNext --outDir dist
node dist/recipe.mjs
```

Supply matching input files.
See the [JavaScript guide](README.md) for installation and defaults. Pass source
and JSON content, not filenames; recipes read files explicitly.

## Typed decode and targeted explanation

Supply `decode-artifact.json` and `headers.json` for the same site/build/session.
The typed map remains heterogeneous, so narrow individual fields before use.
Keep the loaded artifact outside loops and close it after the last call.

```ts
import { readFileSync } from 'node:fs';
import { artifact, decode, explain, schema } from 'shape';
import type { DecodedPayload, NamedSignal, CompactSchema } from 'shape';

const loaded = artifact.load(readFileSync('decode-artifact.json'));
try {
  const capture = readFileSync('headers.json');
  for (let i = 0; i < 3; i++) {
    const result: DecodedPayload = decode(loaded, capture);
    const memory = result.positionalSignals.DeviceMemory;
    if (memory && typeof memory.deviceMemory === 'number') {
      const detail: NamedSignal = explain(loaded, result, 'DeviceMemory');
      console.log(memory.deviceMemory, detail.fields);
    }
  }
  const description: CompactSchema = schema(loaded);
  console.log(description.format);
} finally {
  loaded.close();
}
```

## Edit a supported field and verify

The same matching files must contain a supported numeric DeviceMemory field.
Encode the whole payload, retaining `metadata.originalReport` and its channels,
seeds and prefixes. `CaptureHeaders` uses suffix keys. Await calls inside `try`
before `finally` closes the artifact.

```ts
import { readFileSync, writeFileSync } from 'node:fs';
import { artifact, decodeAsync, encodeAsync, stringifyJSON } from 'shape';
import type { CaptureHeaders, DecodedPayload } from 'shape';

const loaded = await artifact.loadAsync(readFileSync('decode-artifact.json'));
try {
  const decoded: DecodedPayload = await decodeAsync(loaded, readFileSync('headers.json'));
  const fields = decoded.positionalSignals.DeviceMemory;
  if (!fields || typeof fields.deviceMemory !== 'number') {
    throw new Error('Capture needs a numeric DeviceMemory.deviceMemory');
  }
  fields.deviceMemory = 16;
  const edited: CaptureHeaders = await encodeAsync(loaded, decoded);
  const verified = await decodeAsync(loaded, edited, { verify: true });
  if (verified.positionalSignals.DeviceMemory?.deviceMemory !== 16) {
    throw new Error('Edited field did not round trip');
  }
  writeFileSync('edited-headers.json', stringifyJSON(edited));
} finally {
  loaded.close();
}
```

## Raw source, explicit cache, compile to a loaded artifact

Supply raw `seed.js`, `session.json`, `headers.json` and the matching site ID as
`node dist/recipe.mjs SITE_ID`. Use a release native library. Omit `jobs` to use
available cores; this recipe bounds it to four. No naming is needed for finders.

```ts
import { readFileSync, writeFileSync } from 'node:fs';
import { trace, artifact, decode, CachePolicy, stringifyJSON } from 'shape';
import type { TraceProgramOptions, CompileArtifactOptions, LoadedArtifact } from 'shape';

const siteId = process.argv[2];
if (!siteId) throw new Error('Pass the matching site ID');
const traceOptions: TraceProgramOptions = {
  cache: CachePolicy.Disabled, jobs: 4, threadCleanup: true, noCleanup: false,
};
const traced = await trace.program(readFileSync('seed.js'), traceOptions);
if (!traced.complete) throw new Error(stringifyJSON(traced.stopped));
writeFileSync('program.js', traced.code);
const compileOptions: CompileArtifactOptions = {
  session: readFileSync('session.json'), siteId, cache: CachePolicy.Disabled,
};
const loaded: LoadedArtifact = await artifact.compile(traced.code, compileOptions);
try {
  console.log(decode(loaded, readFileSync('headers.json')).positionalSignals);
  writeFileSync('decode-artifact.json', stringifyJSON(loaded.toJSON()));
} finally {
  loaded.close();
}
```

Use `CachePolicy.Directory('kernel-cache')` to opt into disk cache access or
`CachePolicy.Environment` to explicitly consult the legacy environment policy.
Caching defaults to disabled. The SDK never sets environment variables or
automatically saves programs/artifacts.

## Kernel reuse and thread completion

Supply saved `kernel.json` and known start coordinates as
`node dist/recipe.mjs PC STATE`. Kernel load validates once. Repeated thread
calls reuse its parsed state; disk caching is unrelated to this reuse.
Inspect `complete`, `stop`, `self_check` and nullable `code`; incomplete output
is diagnostic, not a finished program.

```ts
import { readFileSync } from 'node:fs';
import { kernel } from 'shape';
import type { TraceThreadOptions, TraceThreadResult } from 'shape';

const pc = Number(process.argv[2]);
const state = Number(process.argv[3]);
if (!Number.isSafeInteger(pc) || pc < 0 || !Number.isSafeInteger(state) || state < 0) {
  throw new Error('Pass nonnegative PC and STATE');
}
const loaded = await kernel.loadAsync(readFileSync('kernel.json'));
try {
  const options: Omit<TraceThreadOptions, 'artifact'> = { pc, state, partial: true };
  const result: TraceThreadResult = await loaded.thread(options);
  console.log(result.complete, result.stop, result.self_check);
  if (result.code !== null) console.log(result.code);
} finally {
  loaded.close();
}
```

## Static catalog, source schema and optional naming

Supply complete traced `program.js`. Catalog cards describe documented signals;
they do not claim observations. Naming is a separate optional rewrite.

```ts
import { readFileSync, writeFileSync } from 'node:fs';
import { signals, naming, stringifyJSON } from 'shape';
import type { CatalogCard, NamedSourceResult } from 'shape';

const program = readFileSync('program.js');
const card: CatalogCard = await signals.describe('DeviceMemory');
console.log(card.signal, card.summary, card.logical_fields);
console.log(await signals.show(program));
writeFileSync('schema.json', stringifyJSON(await signals.schema(program)));
const named: NamedSourceResult = await naming.threads(program, { rename: true });
writeFileSync('program.named.js', named.source);
```

## Typed framing and exact pack values

This self-contained synthetic recipe exercises raw payload framing and
Superpack, independent of named signal interpretation. Profile constants are
synthetic, not captured keys. IEEE bits are strings so negative zero survives.

```ts
import assert from 'node:assert/strict';
import { payload, pack, unpackCanonical } from 'shape';
import type { PayloadProfile, Payload, PackValue } from 'shape';

const profile: PayloadProfile = {
  format: 'shape-byte-codec/1',
  cipher: { ivs: [1, 2, 3, 4], counter: 0, double_rounds: 4 },
  alphabet: 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/',
  keys: [1, 2, 3, 4, 5, 6, 7, 8],
};
const codec = await payload.codec(profile);
const input = new TextEncoder().encode('synthetic framing sample');
const text: string = await payload.encodeBytes(codec, input, [1, 2]);
const recovered: Payload = await payload.decodeBytes(codec, text);
if (recovered.data.kind !== 'raw') throw new Error('Expected raw payload');
assert.deepEqual(recovered.data.bytes, Array.from(input));
const value: PackValue = { kind: 'array', value: [
  { kind: 'number', value: '8000000000000000' }, { kind: 'string', value: 'sample' },
] };
assert.deepEqual(await unpackCanonical(await pack(value)), value);
```

Report framing uses typed `PayloadReport`/`HeaderReport` and requires
source-derived positional count and ordered keysets. Full headers are distinct
from the suffix-keyed named decode capture. Keep seeds and header context when
re-encoding reports. Codec factories return validated profiles; pass those
profiles explicitly to subsequent operations.

## Seed URL handling without acquisition

Pass the actual matching captured reference as `node dist/recipe.mjs URL`.
This recipe validates a reference and client settings.

```ts
import { seeds } from 'shape';
import type { ClientConfig } from 'shape';

const reference = process.argv[2];
if (!reference) throw new Error('Pass the matching kernel URL');
const url: string = await seeds.parseUrl(reference);
const token: string = await seeds.token(url, 'seed');
const config: ClientConfig = await seeds.client({ timeoutMs: 10000, maxRedirects: 2 });
console.log(token, config.max_redirects);
```

Next, [parse launchers, resolve URLs or inspect the site registry](README.md#parse-fetch-and-store-seeds).
`seeds.fetch` / `seeds.fetchDirect` download bytes; `seeds.import` saves them.
Keep the URL/token/source association matched.

## Preserve exact integers

Pass file bytes directly to typed SDK operations. For arbitrary JSON,
`parseJSON` returns `JsonValue`; narrow before use. Unsafe integer tokens become
`bigint`, and `stringifyJSON` writes them exactly. Ordinary JSON helpers lose
this contract; never convert exact integers or hexadecimal number bits through
`Number`.
