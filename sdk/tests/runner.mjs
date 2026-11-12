import fs from 'node:fs';
import { pathToFileURL } from 'node:url';
import { invoke, loadedCall, resolve } from './named.mjs';

// Import the actual ESM binding, including its exact-integer JSON codec.
const shape = await import(pathToFileURL(process.env.CONTRACT_NODE_MODULE));
const [artifact, scope, method, request] = process.argv.slice(2);
let generator;
try {
  const environment = { ...process.env };
  const text = fs.readFileSync(request, 'utf8');
  const body = process.argv[6] === 'raw' ? text : shape.parseJSON(text);
  let value;
  if (scope === 'surface') {
    for (const id of body) resolve(shape, id);
    for (const id of ['decode', 'encode', 'encodeSignals', 'explain', 'schema']) {
      if (typeof shape[id] !== 'function') throw new Error(`missing direct ${id}`);
    }
    value = body;
  } else if (scope === 'named') {
    value = await invoke(shape, method, body);
  } else if (scope === 'kernel') {
    generator = shape.kernel.load(shape.parseJSON(fs.readFileSync(artifact, 'utf8')));
    value = await shape.trace.programWithArtifact(body.source, generator);
  } else if (scope === 'loaded') {
    generator = shape.artifact.load(shape.parseJSON(fs.readFileSync(artifact, 'utf8')));
    const detached = generator.toJSON();
    detached.siteId = 'mutated-export-must-not-change-loaded-state';
    value = await loadedCall(shape, generator, method, body);
    for (let i = 0; method !== 'encode' && i < 2; i++) {
      if (shape.stringifyJSON(value) !== shape.stringifyJSON(await loadedCall(shape, generator, method, body))) {
        throw new Error('loaded operation changed on reuse');
      }
    }
  } else if (scope === 'v2') {
    value = await shape.operation(method, body);
  } else if (scope === 'generator') {
    generator = shape.Generator.load(shape.parseJSON(fs.readFileSync(artifact, 'utf8')));
    value = generator.call(method, body);
  } else {
    value = shape.callSync(method, body);
  }
  if (JSON.stringify(environment) !== JSON.stringify(process.env)) throw new Error('SDK mutated process environment');
  console.log(shape.stringifyJSON({ ok: true, value }));
} catch (error) {
  if (!error.code) throw error;
  console.log(shape.stringifyJSON({ ok: false, error: { code: error.code, message: error.message } }));
} finally {
  generator?.close();
}
