'use strict';
// Opt-in: needs a local decode artifact and a capture that matches it.
//   SHAPE_TEST_ARTIFACT=/path/artifact.json SHAPE_TEST_HEADERS=/path/headers.json node --test sdk/node/test
// Optional: SHAPE_TEST_SESSION=/path/session.json is passed to decode/explain.
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const shape = require('..');

const { SHAPE_TEST_ARTIFACT, SHAPE_TEST_HEADERS, SHAPE_TEST_SESSION } = process.env;
const skip = SHAPE_TEST_ARTIFACT && SHAPE_TEST_HEADERS ? false : 'set SHAPE_TEST_ARTIFACT and SHAPE_TEST_HEADERS';

// A session document can carry large integers too, so parse it losslessly.
const session = () => (SHAPE_TEST_SESSION ? shape.parseJSON(fs.readFileSync(SHAPE_TEST_SESSION, 'utf8')) : undefined);

/** Call visit for every bigint anywhere in a parsed JSON value. */
function visitBigInts(value, visit) {
  if (typeof value === 'bigint') visit(value);
  else if (Array.isArray(value)) value.forEach((item) => visitBigInts(item, visit));
  else if (value && typeof value === 'object') Object.values(value).forEach((item) => visitBigInts(item, visit));
}

test('generator decode / explain / schema (sync)', { skip }, () => {
  const generator = shape.Generator.load(fs.readFileSync(SHAPE_TEST_ARTIFACT));
  try {
    const decoded = generator.decode(fs.readFileSync(SHAPE_TEST_HEADERS), { metadata: true, session: session() });
    assert.equal(decoded.positionalSignals.DeviceMemory.deviceMemory, 32);
    assert.ok(decoded.metadata.decoded > 0);
    assert.equal(decoded.metadata.decoded, Object.keys(decoded.positionalSignals).length);

    const explained = generator.explain(decoded, 'DeviceMemory', { session: session() });
    assert.equal(explained.status, 'decoded');

    const schema = generator.schema();
    assert.equal(typeof schema, 'object');
    assert.ok(schema.signals);

    assert.throws(() => generator.explain(decoded, 'NoSuchSignal'), shape.ShapeError);

    // Re-encoding the full payload reproduces the capture byte for byte, and
    // an edit decodes back.
    assert.deepEqual(
      generator.encode({ decoded, session: session() }),
      shape.parseJSON(fs.readFileSync(SHAPE_TEST_HEADERS, 'utf8')),
    );
    const edited = shape.parseJSON(shape.stringifyJSON(decoded));
    edited.positionalSignals.DeviceMemory.deviceMemory = 8;
    const editedHeaders = generator.encode({ decoded: edited, session: session() });
    const decodedEdited = generator.decode(editedHeaders, { session: session() });
    assert.equal(decodedEdited.positionalSignals.DeviceMemory.deviceMemory, 8);
  } finally {
    generator.close();
  }
  assert.equal(generator.closed, true);
  assert.throws(() => generator.schema(), { code: 'invalid_request' });
  generator.close(); // idempotent
});

test('generator async forms and close during a call', { skip }, async () => {
  // shape.parseJSON keeps 64-bit integers exact (plain JSON.parse would not).
  const artifact = shape.parseJSON(fs.readFileSync(SHAPE_TEST_ARTIFACT, 'utf8'));
  const headers = fs.readFileSync(SHAPE_TEST_HEADERS, 'utf8');
  const generator = await shape.Generator.loadAsync(artifact);
  const decoded = await generator.decodeAsync(headers, { metadata: true });
  assert.equal(decoded.positionalSignals.DeviceMemory.deviceMemory, 32);
  const explained = await generator.explainAsync(decoded, 'DeviceMemory');
  assert.equal(explained.status, 'decoded');
  assert.deepEqual(await generator.schemaAsync(), generator.schema());
  assert.deepEqual(
    await generator.encodeAsync({ decoded }),
    shape.parseJSON(headers),
  );

  // In-flight calls finish even when close() runs right after they start.
  const pending = generator.decodeAsync(headers);
  generator.close();
  assert.equal((await pending).positionalSignals.DeviceMemory.deviceMemory, 32);
  await assert.rejects(generator.decodeAsync(headers), { code: 'invalid_request' });
});

test('artifact object keeps 64-bit integers through load', { skip }, (t) => {
  // An artifact parsed with shape.parseJSON contains bigints. Loading it as an
  // object serializes those bigints as exact integer literals; the decoded
  // payload must equal the one produced from the artifact bytes.
  const text = fs.readFileSync(SHAPE_TEST_ARTIFACT, 'utf8');
  const artifact = shape.parseJSON(text);
  let artifactBig = 0;
  visitBigInts(artifact, () => artifactBig++);
  if (artifactBig === 0) return t.skip('artifact holds no integer beyond 2^53');

  const fromBytes = shape.Generator.load(Buffer.from(text));
  const fromObject = shape.Generator.load(artifact);
  try {
    const expected = fromBytes.decode(fs.readFileSync(SHAPE_TEST_HEADERS), { metadata: true });
    const actual = fromObject.decode(fs.readFileSync(SHAPE_TEST_HEADERS), { metadata: true });
    assert.deepEqual(actual, expected);
    let decodedBig = 0;
    visitBigInts(actual, () => decodedBig++);
    assert.ok(decodedBig > 0, 'decoded payload lost its 64-bit integers');
  } finally {
    fromBytes.close();
    fromObject.close();
  }
});

test('compactSchema matches generator.schema', { skip }, async () => {
  const artifact = fs.readFileSync(SHAPE_TEST_ARTIFACT);
  const generator = shape.Generator.load(artifact);
  try {
    assert.deepEqual(await shape.compactSchema(artifact), generator.schema());
  } finally {
    generator.close();
  }
});
