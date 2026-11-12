import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, mkdtempSync, readdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer } from 'node:http';
import { execFileSync } from 'node:child_process';
import shape from '../index.mjs';

const fixture = name => readFileSync(new URL(`../../../tests/fixtures/${name}`, import.meta.url));
const json = name => shape.parseJSON(fixture(name));
const raw = fixture('javascript-vm-prefix.raw.js').toString();
const program = fixture('sdk/decode/program.js');
const session = fixture('sdk/decode/session.json');
const capture = fixture('sdk/decode/headers.json');
const bigints = value => typeof value === 'bigint' ? [value]
  : Array.isArray(value) ? value.flatMap(bigints)
  : value && typeof value === 'object' ? Object.values(value).flatMap(bigints) : [];
const mapping = JSON.parse(readFileSync(new URL('../api-map.json', import.meta.url)));

// Every named family runs against the actual newly built native library.
test('v2 discovery covers every named portable operation; legacy discovery remains compatible', async () => {
  const info = await shape.operation('version');
  assert.equal(info.contract_version, 2);
  assert.deepEqual(info, shape.operationSync('version'));
  assert.deepEqual([...info.commands].sort(), Object.keys(mapping.operations).sort());
  for (const [id, path] of Object.entries(mapping.operations)) {
    const fn = path.split('.').reduce((value, key) => value[key], shape);
    assert.equal(typeof fn, 'function', `${id} -> ${path}`);
  }
  assert.equal((await shape.commands()).length, 21);
  await assert.rejects(shape.operation('no.such.operation'), { code: 'unknown_command' });
});

test('compiled loaded artifacts decode/edit/encode/explain without reloading; export is detached', async () => {
  const loaded = await shape.artifact.compile(program, { session, siteId: 'example' });
  assert.ok(loaded instanceof shape.LoadedArtifact);
  const document = loaded.toJSON();
  assert.deepEqual(document, await loaded.toJSONAsync());
  const reload = shape.artifact.fromArtifact(document);
  try {
    const decoded = shape.decode(loaded, capture, { metadata: true, verify: true });
    assert.equal(decoded.metadata.exactReencode, true);
    assert.ok(bigints(decoded).length > 0, 'real decoded metadata exercises exact u64 bits');
    assert.deepEqual(bigints(shape.parseJSON(shape.stringifyJSON(decoded))), bigints(decoded));
    assert.equal(typeof decoded.positionalSignals.DeviceMemory.deviceMemory, 'number');
    assert.deepEqual(shape.encode(loaded, decoded), shape.parseJSON(capture));
    assert.throws(() => shape.encode(loaded, decoded, []), TypeError);
    assert.throws(() => shape.encode(loaded, decoded, { decoded: {} }), TypeError);
    assert.equal(shape.explain(loaded, decoded, 'DeviceMemory').status, 'decoded');
    assert.deepEqual(shape.schema(loaded), shape.schema(reload));
    assert.deepEqual(await shape.artifact.compactSchema(document), shape.schema(loaded));
    // Export/edit must not mutate native immutable state.
    document.siteId = 'changed';
    assert.equal(loaded.toJSON().siteId, 'example');
    decoded.positionalSignals.DeviceMemory.deviceMemory = 16;
    const edited = await shape.encodeAsync(loaded, decoded);
    const checked = await shape.decodeAsync(loaded, edited, { verify: true });
    assert.equal(checked.positionalSignals.DeviceMemory.deviceMemory, 16);
    assert.equal((await shape.explainAsync(loaded, checked, 'DeviceMemory')).status, 'decoded');
    const pending = Array.from({ length: 16 }, () => shape.schemaAsync(loaded));
    loaded.close(); loaded.close();
    for (const value of await Promise.all(pending)) assert.deepEqual(value, reload.schema());
    assert.throws(() => shape.decode(loaded, capture), { code: 'invalid_request' });
    assert.throws(() => loaded.toJSON(), { code: 'invalid_request' });
  } finally {
    loaded.close(); reload.close();
  }
});

test('named full cleanup defaults, disk policy, kernel reuse, identity, and thread completion', async () => {
  const directory = mkdtempSync(join(tmpdir(), 'shape-node-cache-'));
  const environmentBefore = { ...process.env };
  try {
    const uncached = await shape.trace.corpus.load(raw, { name: 'synthetic.js' });
    assert.deepEqual(readdirSync(directory), []);
    const cached = await shape.trace.corpus.load(raw, { name: 'synthetic.js', cache: shape.CachePolicy.Directory(directory) });
    assert.deepEqual(cached, uncached);
    assert.ok(readdirSync(directory).length > 0);
    const options = { steps: 16, jobs: 2, cache: shape.CachePolicy.Disabled };
    const implicit = await shape.trace.program(raw, options);
    const explicit = await shape.trace.program(raw, { ...options, threadCleanup: true, noCleanup: false });
    assert.deepEqual(implicit, explicit);
    const loaded = await shape.kernel.loadAsync(cached);
    try {
      assert.deepEqual(await shape.trace.programWithArtifact(raw, loaded, options), implicit);
      assert.deepEqual(await shape.trace.programWithArtifact(raw, uncached, options), implicit);
      assert.deepEqual(await shape.kernel.toJSON(loaded), cached);
      assert.deepEqual(await shape.kernel.toJSON(cached), cached);
      assert.deepEqual(await loaded.toJSONAsync(), cached);
      await assert.rejects(loaded.program(raw + '\n// different', options), { code: 'failed' });
      const threadOptions = { pc: 0, state: 0, steps: 16, partial: true };
      const thread = await loaded.thread(threadOptions);
      assert.equal(typeof thread.complete, 'boolean');
      assert.ok(Array.isArray(thread.self_check));
      assert.deepEqual(await shape.trace.thread(null, { artifact: cached, ...threadOptions }), thread);
      const reachable = await shape.trace.corpus.reachable(loaded, { steps: 16, jobs: 2, limit: 4 });
      assert.ok(Array.isArray(reachable));
      const pending = [loaded.thread(threadOptions), loaded.reachable({ steps: 16, limit: 2 })];
      loaded.close();
      await Promise.all(pending);
      await assert.rejects(loaded.thread(threadOptions), { code: 'invalid_request' });
      assert.throws(() => loaded.toJSON(), { code: 'invalid_request' });
    } finally { loaded.close(); }
    const before = readdirSync(directory);
    await shape.trace.corpus.load(raw, { cache: shape.CachePolicy.Disabled });
    assert.deepEqual(readdirSync(directory), before);
    assert.ok(Object.keys(process.env).length === Object.keys(environmentBefore).length &&
      Object.keys(environmentBefore).every(key => process.env[key] === environmentBefore[key]),
      'SDK must leave environment unchanged');
    const old = { seed: { entry: { completed: true, stop: 'return' } } };
    assert.deepEqual(await shape.trace.corpus.compare(old, old), { regressions: [], changes: [] });
    assert.deepEqual(await shape.trace.corpus.compareInline({}, {}), []);
    await shape.trace.corpus.source(raw, { steps: 16, jobs: 2, limit: 2 });
    await shape.trace.corpus.fingerprint(cached, { pc: 0, state: 0, steps: 16 });
    const walk = await shape.trace.corpus.walkSeed(fileURLToPath(new URL('../../../tests/fixtures/javascript-vm-prefix.raw.js', import.meta.url)), { steps: 16, jobs: 2 });
    assert.ok(Object.keys(walk).length);
  } finally { rmSync(directory, { recursive: true, force: true }); }
});

test('environment caching is opt-in even when a cache path is configured', () => {
  const directory = mkdtempSync(join(tmpdir(), 'shape-node-environment-'));
  const cache = join(directory, 'cache');
  try {
    execFileSync(process.execPath, ['--input-type=module', '-e', `
      import assert from 'node:assert/strict';
      import { readFileSync, existsSync } from 'node:fs';
      import { trace, CachePolicy } from ${JSON.stringify(new URL('../index.mjs', import.meta.url).href)};
      const source = readFileSync(${JSON.stringify(fileURLToPath(new URL('../../../tests/fixtures/javascript-vm-prefix.raw.js', import.meta.url)))});
      await trace.corpus.load(source);
      assert.equal(existsSync(process.env.SHAPE_KERNEL_CACHE), false);
      await trace.corpus.load(source, { cache: CachePolicy.Disabled });
      assert.equal(existsSync(process.env.SHAPE_KERNEL_CACHE), false);
      await trace.corpus.load(source, { cache: CachePolicy.Environment });
      assert.equal(existsSync(process.env.SHAPE_KERNEL_CACHE), true);
    `], { env: { ...process.env, SHAPE_KERNEL_CACHE: cache }, stdio: 'pipe' });
  } finally { rmSync(directory, { recursive: true, force: true }); }
});

test('source operations, naming corpus, catalog and static schemas use native engines', async () => {
  const deob = await shape.deob(raw, { artifact: true });
  assert.ok(deob.artifact);
  assert.ok(await shape.report(raw));
  assert.ok(await shape.kernel.show(raw));
  assert.ok(await shape.ops.list(raw));
  assert.ok(await shape.ops.show(raw, 0));
  const classified = await shape.ops.classify.source(raw);
  assert.deepEqual(await shape.ops.classify.report(raw), classified);
  assert.ok(classified.ops.length);
  assert.equal((await shape.ops.classify.template(raw, classified.ops[0].handler_index)).handler_index, classified.ops[0].handler_index);
  assert.ok((await shape.ops.classify.seeds([classified])).length);
  const source = [{ name: 'synthetic', source: '(function main(){ var registry=[1,2,3]; function entry(){return registry[0];} return {init:entry}; })();' }];
  const analysis = (await shape.naming.corpus.analyze(source))[0];
  assert.equal(analysis.name, 'synthetic');
  assert.equal(typeof analysis.rename.source, 'string');
  assert.match(analysis.rename.source_sha256, /^[a-f0-9]{64}$/);
  assert.match(analysis.rename.transformed_sha256, /^[a-f0-9]{64}$/);
  for (const field of ['applied', 'skipped', 'ambiguous']) assert.equal(typeof analysis.rename[field], 'number');
  assert.deepEqual(analysis.rename.plan, analysis.plan);
  const difference = (await shape.naming.corpus.compare(source, source))[0];
  assert.deepEqual(difference.missing_roles, []);
  assert.deepEqual(difference.name_divergences, []);
  assert.ok(await shape.naming.threads(program));
  assert.ok(await shape.naming.collectors(program));
  assert.ok(await shape.naming.plan(program));
  assert.ok(await shape.naming.inventory(program));
  assert.ok((await shape.signals.catalog()).some(card => card.signal === 'DeviceMemory'));
  assert.equal((await shape.signals.describe('DeviceMemory')).signal, 'DeviceMemory');
  assert.ok(await shape.signals.schema(program));
  assert.ok(await shape.signals.show(program));
  assert.ok(await shape.signals.inputs(program));
  assert.ok(await shape.signals.contracts(program));
  assert.ok(await shape.signals.compactSchema(program));
  assert.ok(await shape.pin(program, { session }));
  assert.ok(await shape.session.fromInitializer(fixture('generator/seed-pinned-init.js')));
});

test('header/payload framing and tagged pack values preserve exact data', async () => {
  const exactConfig = await shape.seeds.client({ maxBodyBytes: 18446744073709551615n });
  assert.equal(exactConfig.max_body_bytes, 18446744073709551615n);
  const profile = json('sdk/generator/profile.json');
  const report = json('sdk/generator/report.json');
  const p = { format: profile.format, cipher: profile.cipher, alphabet: profile.alphabet, keys: profile.keys };
  const payloadProfile = await shape.payload.codec(p);
  const text = await shape.payload.encode(payloadProfile, report);
  const decodedReport = await shape.payload.decode(payloadProfile, text, report.positionalSignals.length, report.keysets);
  // Object member order is normalized by pack; compare exact encoded payload.
  assert.equal(await shape.payload.encode(payloadProfile, decodedReport), text);
  const framedProfile = await shape.headers.codec(profile);
  const framed = await shape.headers.encode(framedProfile, report);
  const back = await shape.headers.decode(framedProfile, framed, report.positionalSignals.length, report.keysets);
  assert.deepEqual(await shape.headers.encode(framedProfile, back), framed);
  assert.deepEqual(await shape.headers.parse(Object.entries(framed).map(([key, value]) => `${key}: ${value}`).join('\n')), framed);
  const opaque = await shape.headers.decodeBytes(framedProfile, framed);
  assert.deepEqual(await shape.headers.encodeBytes(framedProfile, opaque), framed);
  const finalized = await shape.headers.decodeFinalized(framedProfile, framed);
  assert.deepEqual(await shape.headers.encodeFinalized(framedProfile, finalized.text, finalized.context), framed);
  const rawText = await shape.payload.encodeBytes(payloadProfile, Uint8Array.from([0, 128, 255]), [1, 2]);
  assert.deepEqual((await shape.payload.decodeBytes(payloadProfile, rawText)).data, { kind: 'raw', bytes: [0, 128, 255] });
  const value = { kind: 'array', value: [
    { kind: 'number', value: '8000000000000000' },
    { kind: 'binary', value: [0, 255] }, { kind: 'date', value: 281474976710655 },
  ] };
  const packed = await shape.pack(value);
  assert.deepEqual(await shape.unpackCanonical(packed), value);
  const unpacked = await shape.unpack(packed);
  assert.deepEqual(unpacked.value, value);
  assert.deepEqual(unpacked.original_bytes, packed);
  assert.equal(unpacked.canonical, true);
  const exact = { captured_memory: { 0: { kind: 'object', identity: 18446744073709551615n } } };
  assert.equal(shape.parseJSON(shape.stringifyJSON(exact)).captured_memory[0].identity, 18446744073709551615n);
});

test('seed parsing, registry, explicit local acquisition/import use real public fixtures', async () => {
  const source = fixture('kernels/javascript/kernel.raw.js');
  const summary = await shape.seeds.parseKernel(source);
  assert.equal((await shape.seeds.parse(source)).kind, 'kernel');
  const launcher = Buffer.from('var script = document.createElement("script"); script.src = "/kernel.js?seed=public_token"; document.head.appendChild(script);');
  assert.equal(typeof (await shape.seeds.parseLauncher(launcher)).kernel_ref, 'string');
  const url = await shape.seeds.resolveUrl('../kernel.js?seed=public_token', 'https://example.invalid/path/launcher.js');
  assert.equal(await shape.seeds.parseUrl(url), url);
  assert.equal(await shape.seeds.token(url), 'public_token');
  const sites = await shape.seeds.sites();
  assert.ok(sites.length);
  assert.equal((await shape.seeds.site(sites[0].name)).name, sites[0].name);
  const directory = mkdtempSync(join(tmpdir(), 'shape-node-seeds-'));
  const server = createServer((request, response) => response.end(request.url.startsWith('/launcher') ? launcher : source));
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  try {
    const base = `http://127.0.0.1:${server.address().port}`;
    const client = await shape.seeds.client({ timeoutMs: 5000, maxRedirects: 2 });
    const fetched = await shape.seeds.fetch(`${base}/launcher.js`, { client });
    assert.equal(fetched.kernel_summary.sha256, summary.sha256);
    assert.ok((await shape.seeds.fetchDirect(`${base}/kernel.js`, { client })).kernel.length);
    const stored = await shape.seeds.import(directory, 'public_token', source, launcher);
    assert.equal(stored.status, 'stored');
    assert.equal((await shape.seeds.import(directory, 'public_token', source, launcher)).status, 'duplicate');
  } finally {
    await new Promise(resolve => server.close(resolve));
    rmSync(directory, { recursive: true, force: true });
  }
});

test('v2 validates presence, null booleans, cache shapes, numeric and byte constraints', async () => {
  for (const steps of [0, -1, 0.5, '1']) {
    await assert.rejects(shape.trace.program(raw, { steps }), { code: 'invalid_request' });
  }
  for (const cache of [true, 'disabled', { mode: 'directory' }, { mode: 'disabled', directory: 'ignored' }]) {
    await assert.rejects(shape.trace.program(raw, { cache }), { code: 'invalid_request' });
  }
  await assert.rejects(shape.artifact.compile(program, {}), { code: 'invalid_request' });
  await assert.rejects(shape.artifact.compile('function {', { session, siteId: 'example' }), { code: 'failed' });
  for (const direct of [null, 'true', 1]) {
    await assert.rejects(shape.artifact.compile(program, { session, siteId: 'example', direct }), { code: 'invalid_request' });
  }
  await assert.rejects(shape.trace.thread(raw, { pc: -1, state: 0 }), { code: 'invalid_request' });
  await assert.rejects(shape.signals.describe('no-such-signal'), { code: 'invalid_request' });
  assert.throws(() => shape.unpack([256]), TypeError);
  assert.throws(() => shape.CachePolicy.Directory(''), TypeError);
  await assert.rejects(shape.trace.corpus.source(raw, { jobs: 0 }), { code: 'invalid_request' });
  await assert.rejects(shape.seeds.parseUrl('file:///private'), { code: 'failed' });
});

test('serialized classification documents use explicit typed native document boundaries', async () => {
  const template = {
    handler_index: 9, table_key: 9, semantic_identity: 'synthetic-return',
    source_span: { start: 0, end: 9 }, status: 'Executable',
    steps: [{ order: 0, span: { start: 0, end: 9 }, operation: { Return: { value: { Literal: { Number: '1' } } } } }],
    unsupported: [],
  };
  const classified = await shape.ops.classify.templateDocument('return 1;', shape.stringifyJSON(template));
  assert.equal(classified.handler_index, 9);
  assert.equal(classified.source, 'return 1;');
  await assert.rejects(shape.ops.classify.reportDocument('{}'), { code: 'invalid_request' });
});
