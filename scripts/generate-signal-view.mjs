#!/usr/bin/env node
// Generate/check the compiled examples without importing or evaluating source JS.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { dataModule } from './signal-data-parser.mjs';
import { forbidden, controls } from './signal-data-rules.mjs';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const registry = fs.readFileSync(path.join(root, 'src/signals/catalog/mod.rs'), 'utf8');
const stems = [...registry.matchAll(/card:\s*&([a-z_]+)::CARD/g)].map(m => m[1]).sort();
const cards = {};
for (const stem of stems) {
  const sourcePath = `docs/signals/${stem}.js`;
  const source = fs.readFileSync(path.join(root, sourcePath), 'utf8');
  const examples = Object.entries(dataModule(source, text => /^-?\d+$/.test(text) && !Number.isSafeInteger(Number(text)) ? BigInt(text) : Number(text))).map(([name, value]) => {
    const special_values = [];
    function encode(v, pointer = '') {
      if (typeof v === 'bigint') return JSON.rawJSON(v.toString());
      const special = v === undefined ? 'undefined' : typeof v === 'number' && !Number.isFinite(v) ? String(v) : Object.is(v, -0) ? '-0' : null;
      if (special) { special_values.push({pointer, value: special}); return null; }
      if (Array.isArray(v)) return v.map((x, i) => encode(x, `${pointer}/${i}`));
      if (v && typeof v === 'object') return Object.fromEntries(Object.entries(v).map(([k,x]) => [k, encode(x, `${pointer}/${k.replaceAll('~','~0').replaceAll('/','~1')}`)]));
      return v;
    }
    // Keep the source's evidence statement with each alternative, conservatively
    // classify mixed/unspecified evidence as illustrative rather than captured.
    const declaration = name === 'default' ? 'export default' : `export const ${name}`;
    const preceding = source.slice(0, source.indexOf(declaration));
    const note = (preceding.match(/(?:^|\n)((?:[ \t]*\/\/[^\n]*(?:\n|$))+)\s*$/)?.[1] ?? '').replace(/\/\//g, '').trim();
    return {name, provenance: /\bcaptured\b/i.test(note) && !/illustrative|not captured|no capture/i.test(note) ? 'captured_sample' : /no data|no payload/i.test(note) ? 'no_data' : 'illustrative', note, data: encode(value), special_values};
  });
  cards[stem] = {source: sourcePath, sha256: createHash('sha256').update(source).digest('hex'), controls: controls[stem] ?? [], examples};
}
const output = JSON.stringify({version: 1, forbidden: [...forbidden].sort(), cards}, null, 2) + '\n';
const destination = path.join(root, 'src/signals/catalog/view-data.json');
if (process.argv.includes('--check')) {
  if (fs.readFileSync(destination, 'utf8') !== output) throw Error('compiled signal examples drifted: run node scripts/generate-signal-view.mjs');
  console.log(`compiled examples match all ${stems.length} cards`);
} else fs.writeFileSync(destination, output);
