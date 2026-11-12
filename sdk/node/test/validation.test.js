'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const { Worker } = require('node:worker_threads');
const { execFileSync } = require('node:child_process');
const shape = require('..');
const artifact = require('./artifact');

test('compileArtifact forwards direct and preserves the strict default', async () => {
  const { readFileSync } = require('node:fs');
  const path = require('node:path');
  const fixture = name => readFileSync(path.join(__dirname, '../../../tests/fixtures/sdk/decode', name), 'utf8');
  const source = fixture('program.js');
  const options = { session: fixture('session.json'), siteId: 'example' };
  const strict = await shape.compileArtifact(source, options);
  assert.deepEqual(await shape.compileArtifact(source, { ...options, direct: false }), strict);
  const direct = await shape.compileArtifact(source, { ...options, direct: true });
  assert.notDeepEqual(direct, strict, 'direct artifacts retain their provenance');
  const expected = await shape.call('artifact.compile', {
    source, session: options.session, site_id: options.siteId, direct: true,
  });
  assert.deepEqual(direct, expected);
  const generator = shape.Generator.load(direct);
  try {
    const decoded = generator.decode(fixture('headers.json'), { verify: true, metadata: true });
    assert.equal(decoded.metadata.exactReencode, true);
  } finally {
    generator.close();
  }
});

test('compileArtifact rejects nonboolean direct options', async () => {
  for (const direct of [null, 0, 1, 'true', [], {}]) {
    await assert.rejects(shape.compileArtifact('', { direct }), { name: 'TypeError', message: /direct/ });
    await assert.rejects(shape.call('artifact.compile', { direct }), { code: 'invalid_request', message: /direct/ });
  }
});

test('command discovery matches the native version command', async () => {
  const expected = shape.callSync('version').commands;
  assert.deepEqual(await shape.commands(), expected);
  assert.deepEqual(shape.commandsSync(), expected);
});

test('C strings reject NUL instead of truncating commands or JSON', async () => {
  for (const [command, request] of [['version\0hidden', {}], ['version', '{}\0invalid'], ['version', Buffer.from('{}\0invalid')]]) {
    assert.throws(() => shape.callSync(command, request), { name: 'TypeError', message: /NUL/ });
    await assert.rejects(shape.call(command, request), { name: 'TypeError', message: /NUL/ });
  }
  assert.equal(shape.callSync('version', { value: 'escaped\0NUL' }).version, shape.version());
});

test('convenience options reject arrays, byte buffers, and field collisions', async () => {
  for (const options of [[], Buffer.from('{}'), new Date(), { source: 'override' }]) {
    await assert.rejects(shape.deob('function f() {}', options), TypeError);
  }
  await assert.rejects(shape.trace.program('', { reportDir: 'a', report_dir: 'b' }), /duplicate option/);
  for (const steps of [NaN, Infinity, -Infinity]) {
    await assert.rejects(shape.trace.program('', { steps }), { name: 'TypeError', message: /finite/ });
  }
  await assert.rejects(shape.ops.show('', NaN), { name: 'TypeError', message: /finite/ });
  class Options { constructor() { this.steps = -1; } }
  await assert.rejects(shape.trace.program('', new Options()), { code: 'invalid_request' });
  assert.throws(() => new shape.Generator(null), TypeError);
  assert.throws(() => new shape.Generator({}), TypeError);
  await assert.rejects(shape.call('version', () => {}), /JSON document/);
});

test('generic generator calls and async validation use the public error contract', async () => {
  const generator = shape.Generator.load(artifact());
  try {
    assert.deepEqual(generator.call('schema'), generator.schema());
    assert.deepEqual(await generator.callAsync('schema', Buffer.from('{}')), generator.schema());
    assert.throws(() => generator.call('no.such.method'), { code: 'unknown_command' });
    await assert.rejects(generator.callAsync('no.such.method'), { code: 'unknown_command' });
    assert.throws(() => generator.call('schema\0hidden'), /NUL/);
    await assert.rejects(generator.callAsync('schema', '{}\0invalid'), /NUL/);
    await assert.rejects(generator.callAsync(42), TypeError);
    await assert.rejects(generator.explainAsync({}, 42), TypeError);
    await assert.rejects(generator.encodeAsync(null), TypeError);
    await assert.rejects(generator.decodeAsync({}, []), TypeError);
    assert.throws(() => generator.encode({}), { code: 'invalid_request' });
    assert.throws(() => generator.encode({ signals: {}, decoded: {} }), { code: 'invalid_request' });
  } finally {
    generator.close();
  }
});

test('concurrent generator calls finish after close and new calls reject', async () => {
  const generator = await shape.Generator.loadAsync(artifact());
  const expected = generator.schema();
  const pending = Array.from({ length: 32 }, () => generator.schemaAsync());
  generator.close();
  generator.close();
  assert.equal(generator.closed, true);
  for (const result of await Promise.all(pending)) assert.deepEqual(result, expected);
  assert.throws(() => generator.call('schema'), { code: 'invalid_request' });
  await assert.rejects(generator.callAsync('schema'), { code: 'invalid_request' });
});

test('garbage collection keeps pending native generator calls alive', () => {
  execFileSync(process.execPath, ['--expose-gc', '-e', `
    const assert = require('node:assert/strict');
    const shape = require(${JSON.stringify(require.resolve('..'))});
    const artifact = require(${JSON.stringify(require.resolve('./artifact'))});
    (async () => {
      let generator = shape.Generator.load(artifact());
      const pending = Array.from({ length: 32 }, () => generator.schemaAsync());
      generator.close();
      generator = null;
      global.gc();
      for (const result of await Promise.all(pending)) assert.ok(result.signals);
      global.gc();
    })().catch(error => { console.error(error); process.exitCode = 1; });
  `], { stdio: 'pipe' });
});

test('generators work in separate Node worker environments', async () => {
  const worker = new Worker(`
    const { parentPort, workerData } = require('node:worker_threads');
    const shape = require(workerData.entry);
    (async () => {
      const generator = await shape.Generator.loadAsync(workerData.artifact);
      const pending = generator.schemaAsync();
      generator.close();
      parentPort.postMessage(await pending);
    })().catch(error => { throw error; });
  `, { eval: true, workerData: { entry: require.resolve('..'), artifact: artifact() } });
  const result = await new Promise((resolve, reject) => {
    worker.once('message', resolve);
    worker.once('error', reject);
    worker.once('exit', code => { if (code !== 0) reject(new Error(`worker exited ${code}`)); });
  });
  assert.ok(result.signals);
});
