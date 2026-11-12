'use strict';
// node --test sdk/node/test   (build first: sdk/node/build.sh)
const test = require('node:test');
const assert = require('node:assert/strict');
const shape = require('..');

const { ShapeError } = shape;

test('version', async () => {
  assert.match(shape.version(), /^\d+\.\d+\.\d+/);
  const info = await shape.call('version');
  assert.equal(info.version, shape.version());
  assert.ok(info.commands.includes('trace.program'));
  assert.deepEqual(shape.callSync('version', {}), info);
});

test('unknown command rejects with unknown_command', async () => {
  await assert.rejects(shape.call('no.such.command', {}), (error) => {
    assert.ok(error instanceof ShapeError);
    assert.ok(error instanceof Error);
    assert.equal(error.name, 'ShapeError');
    assert.equal(error.code, 'unknown_command');
    assert.match(error.message, /no\.such\.command/);
    return true;
  });
  assert.throws(() => shape.callSync('no.such.command'), { code: 'unknown_command' });
});

test('missing field and bad JSON reject with invalid_request', async () => {
  await assert.rejects(shape.call('deob', {}), { name: 'ShapeError', code: 'invalid_request', message: /source/ });
  await assert.rejects(shape.call('deob', '{'), { code: 'invalid_request' });
  await assert.rejects(shape.call('trace.thread', { source: 'x', artifact: '{}' }), { code: 'invalid_request' });
  await assert.rejects(shape.pin('function f() {}', {}), { code: 'invalid_request' });
  await assert.rejects(shape.trace.program('function f() {}', { steps: -1 }), { code: 'invalid_request' });
});

test('argument type errors throw TypeError', async () => {
  await assert.rejects(shape.deob(42), TypeError);
  assert.throws(() => shape.Generator.load(42), TypeError);
});

test('cleanup.thread inlines an SSA binding', async () => {
  const out = await shape.cleanup.thread('function f(a) { let $SSA_1 = a; return $SSA_1; }');
  assert.match(out.code, /return a;/);
  assert.doesNotMatch(out.code, /\$SSA_1/);
  assert.equal(out.cleanup.inlined_bindings, 1);
  // Buffers are accepted as source text.
  const again = await shape.cleanup.thread(Buffer.from('function f(a) { let $SSA_1 = a; return $SSA_1; }'));
  assert.equal(again.code, out.code);
});

test('async calls run off the event loop', async () => {
  const lines = [];
  for (let i = 1; i <= 1000; i++) lines.push(`let $SSA_${i} = a + ${i}; g($SSA_${i});`);
  const source = `function f(a) { ${lines.join(' ')} return 0; }`;
  let ticks = 0;
  const timer = setInterval(() => ticks++, 1);
  try {
    const started = process.hrtime.bigint();
    const pending = shape.cleanup.thread(source);
    const returnedAfterMs = Number(process.hrtime.bigint() - started) / 1e6;
    const out = await pending;
    const totalMs = Number(process.hrtime.bigint() - started) / 1e6;
    assert.match(out.code, /a \+ 999;/);
    assert.ok(ticks > 0, `timer never fired during a ${totalMs.toFixed(0)} ms call`);
    assert.ok(returnedAfterMs < totalMs, 'the call returned only after the work finished');
  } finally {
    clearInterval(timer);
  }
});

test('concurrent async calls all resolve', async () => {
  const results = await Promise.all(
    Array.from({ length: 8 }, (_, i) =>
      shape.cleanup.thread(`function f(a) { let $SSA_1 = a + ${i}; return $SSA_1; }`)
    )
  );
  results.forEach((out, i) => assert.match(out.code, new RegExp(`return a \\+ ${i};`)));
});

test('generator load errors throw ShapeError', async () => {
  assert.throws(() => shape.Generator.load('{}'), (error) => error instanceof ShapeError && error.code === 'failed');
  await assert.rejects(shape.Generator.loadAsync(Buffer.from('not json')), ShapeError);
});

test('64-bit integers round-trip exactly', () => {
  const text = '{"bits":4607182418800017408,"neg":-9223372036854775808,"small":7,"float":0.1}';
  const value = shape.parseJSON(text);
  assert.equal(value.bits, 4607182418800017408n);
  assert.equal(value.neg, -9223372036854775808n);
  assert.equal(value.small, 7);
  assert.equal(value.float, 0.1);
  assert.equal(shape.stringifyJSON(value), text);
  assert.deepEqual(shape.parseJSON(Buffer.from(text)), value);

  // A document whose top level is a large integer (no ':'/','/'[' before it),
  // and values nested in arrays.
  assert.equal(shape.parseJSON('9007199254740993'), 9007199254740993n);
  assert.equal(shape.parseJSON('  -9223372036854775808  '), -9223372036854775808n);
  assert.deepEqual(
    shape.parseJSON('[0,9007199254740993,[18446744073709551615]]'),
    [0, 9007199254740993n, [18446744073709551615n]]
  );

  // Requests are built with stringifyJSON, so nested bigints write exact digits.
  assert.equal(
    shape.stringifyJSON({ big: 18446744073709551615n, list: [9007199254740993n] }),
    '{"big":18446744073709551615,"list":[9007199254740993]}'
  );
});

test('ESM entry exports the same API', async () => {
  const esm = await import('../index.mjs');
  for (const key of Object.keys(shape)) assert.equal(esm[key], shape[key], key);
  assert.equal(esm.default, shape);
});
