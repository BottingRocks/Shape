#!/usr/bin/env node
// Dependency-free docs gate. Default: all 93 registered root-level examples.
// Development subset: node scripts/signal-data.test.mjs file.js [file.js ...]
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const docs = path.join(root, 'docs/signals');
const read = name => fs.readFileSync(path.join(root, name), 'utf8');

// Parse the deliberately small data-module grammar before importing anything.
// Calls, property access, methods/getters, templates, computed keys, spreads,
// assignments and executable statements cannot enter through this parser.
function dataModule(source) {
  let pos = 0;
  const fail = message => { throw Error(`${message} at offset ${pos}`); };
  function skip() {
    while (true) {
      const match = /^(?:\s+|\/\/[^\n]*(?:\n|$)|\/\*[\s\S]*?\*\/)/.exec(source.slice(pos));
      if (!match) return;
      pos += match[0].length;
    }
  }
  function take(text) {
    skip();
    if (!source.startsWith(text, pos)) return false;
    pos += text.length;
    return true;
  }
  function expect(text) { if (!take(text)) fail(`expected ${text}`); }
  function word() {
    skip();
    const match = /^[A-Za-z_$][\w$]*/.exec(source.slice(pos));
    if (!match) fail('expected identifier');
    pos += match[0].length;
    return match[0];
  }
  function string() {
    skip();
    const quote = source[pos++];
    let result = '';
    while (pos < source.length) {
      let c = source[pos++];
      if (c === quote) return result;
      if (c === '\n' || c === '\r') fail('unescaped string newline');
      if (c === '\\') {
        c = source[pos++];
        if (c === 'u') {
          const hex = source.slice(pos, pos + 4);
          if (!/^[a-f\d]{4}$/i.test(hex)) fail('invalid Unicode escape');
          result += String.fromCharCode(parseInt(hex, 16));
          pos += 4;
        } else {
          const escapes = { n: '\n', r: '\r', t: '\t', b: '\b', f: '\f', '\\': '\\', '"': '"', "'": "'", '/': '/' };
          if (!Object.hasOwn(escapes, c)) fail('unsupported string escape');
          result += escapes[c];
        }
      } else result += c;
    }
    fail('unterminated string');
  }
  function value() {
    skip();
    if (take('{')) {
      const object = {};
      if (take('}')) return object;
      do {
        skip();
        const key = /["']/.test(source[pos] ?? '') ? string() : word();
        if (key === '__proto__' || Object.hasOwn(object, key)) fail('unsafe or duplicate object key');
        expect(':');
        object[key] = value();
        if (take('}')) return object;
        expect(',');
      } while (!take('}'));
      return object;
    }
    if (take('[')) {
      const array = [];
      if (take(']')) return array;
      do {
        array.push(value());
        if (take(']')) return array;
        expect(',');
      } while (!take(']'));
      return array;
    }
    if (/["']/.test(source[pos] ?? '')) return string();
    const number = /^-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?/.exec(source.slice(pos));
    if (number) { pos += number[0].length; return Number(number[0]); }
    // These constants express numeric measurements, not executable expressions.
    if (take('-Infinity')) return -Infinity;
    const name = word();
    const constants = { true: true, false: false, null: null, undefined, NaN, Infinity };
    if (!Object.hasOwn(constants, name)) fail(`non-data identifier ${name}`);
    return constants[name];
  }
  const exports = {};
  while (true) {
    skip();
    if (pos === source.length) break;
    if (word() !== 'export') fail('only data exports are permitted');
    const kind = word();
    let name;
    if (kind === 'default') name = 'default';
    else {
      if (kind !== 'const') fail('named exports must be const data');
      name = word();
      if (name === 'default') fail('reserved export name');
      expect('=');
    }
    if (Object.hasOwn(exports, name)) fail('duplicate export');
    exports[name] = value();
    expect(';');
  }
  assert.ok(Object.hasOwn(exports, 'default'), 'missing default payload export');
  return exports;
}

// Exercise rejection as well as successful scalars, special numbers and variants.
assert.deepEqual(dataModule('export default [null, true, -0, NaN]; export const absent = undefined;'),
  { default: [null, true, -0, NaN], absent: undefined });
for (const expression of ['run()', '(() => 1)()', 'new Uint8Array(2)', '{ get x() { return 1; } }',
  '{...other}', '{[key]: 1}', 'Math.random()', 'null; globalThis.x = 1', 'undefined; function collect() {}']) {
  assert.throws(() => dataModule(`export default ${expression};`), undefined, expression);
}

const registry = read('src/signals/catalog/mod.rs');
const stems = [...registry.matchAll(/card:\s*&([a-z_]+)::CARD/g)].map(match => match[1]).sort();
assert.equal(stems.length, 93, 'registered signal count changed: review documentation coverage');
assert.equal(new Set(stems).size, 93, 'duplicate registered cards');
const canonical = stems.map(stem => `${stem}.js`);
assert.deepEqual(fs.readdirSync(docs).filter(name => name.endsWith('.js')).sort(), canonical,
  'root-level example files must cover exactly the registered cards');
const requested = process.argv.slice(2);
for (const name of requested) assert.ok(canonical.includes(name), `unknown canonical filename: ${name}`);
const selected = requested.length ? [...new Set(requested)] : canonical;

const forbidden = new Set(['numberBits', 'wireBytes', 'grammarPath', 'staticXor', 'byteMapping',
  'tagByte', 'nullMarker', 'nullFlag', 'rawBytes', 'rawBuffer', 'root_span', 'program_sha256',
  'wow64Placeholder', 'mobilePlaceholder', 'entriesPlaceholder', 'totalBytesPlaceholder',
  'breakdownCountPlaceholder', 'containerMarker']);

// These exact paths are controls, not measured counts. Evidence: the adjacent
// cards describe list framing, literal tags or fixed-list lengths; counted.rs
// removes proven array counts. Never infer this from a Count/Length/Marker suffix:
// entryBytes, breakdownCount, pluginsLength, directNewCount, buildMarker, etc. stay.
const controls = {
  anchor_idna_unicode_version: ['testCount'],
  animation_frame_timings: ['sampleCount'],
  canvas_readback_probe: ['recordCount', 'records[].directColourCount', 'records[].roundTripColourCount',
    'records[].directColours[].pixelLength', 'records[].roundTripColours[].pixelLength'],
  canvas_scene_pixel_samples: ['sampleCount', 'samples[].channelCount', 'terminator'],
  current_script_async_policy_state: ['scriptAsync', 'status1', 'status2', 'policyAbsentTag', 'policyHash', 'scriptFlags', 'asyncState'],
  emoji_channel_sample_halves: ['firstHalfCount', 'secondHalfCount'],
  emoji_channel_samples: ['sampleCount'],
  emoji_glyph_rgba_samples: ['rowCount', 'rows[].sampleCount'],
  font_availability: ['fontCount'],
  font_metrics: ['familyCount'],
  global_own_property_bits: ['probeCount'],
  keyboard_event_samples: ['eventCount'],
  keystroke_dynamics: ['specialKeySlotCount'],
  lifecycle_entropy_samples: ['sampleCount'],
  math_own_property_names: ['nameCount'],
  memory_breakdown_summary: ['entryCount', 'entry[].attributionCount', 'entry[].typeCount'],
  navigator_plugins: ['entryCount'],
  offline_audio_render_samples: ['sampleCount'],
  ponos_xhr_round_trip: ['signalIndexCount'],
  shuffled_date_string: ['zero'],
  ui_event_samples_mouse: ['rawMoveCount', 'throttledMoveCount', 'buttonEventCount'],
  ui_event_samples_touch: ['recordCount', 'records[].pointCount'],
  video_can_play_type: ['count'],
  visibility_event_log: ['recordCount', 'stateNullTag'],
  webgl_context: ['paramsMissing', 'getParameterMissing', 'debugInfoMissing', 'antialiasMissing'],
  webgl_render_hash: ['kind'],
  webrtc_ice_candidate_addresses: ['recordCount'],
};

// Array-valued group items are certified by the cards' count + repeated child
// and named.rs/counted.rs record collapse, not by a blanket array exception.
const arrayGroups = {
  canvas_readback_probe: { 'records[].directColours': ['components', 4], 'records[].roundTripColours': ['components', 4] },
  canvas_scene_pixel_samples: { samples: ['channel', 4] },
  emoji_glyph_rgba_samples: { rows: ['pixels', 2] },
};
const nullableFlags = {
  user_agent_data: ['flags.mobile', 'flags.wow64'],
  webgl_context: ['flags.antialias'],
  visibility_event_log: ['state.currentlyVisible'],
};

function valuesAt(value, fieldPath) {
  let values = [value];
  for (const token of fieldPath.match(/[^.[\]]+|\[\]/g) ?? []) {
    values = values.flatMap(item => token === '[]'
      ? (Array.isArray(item) ? item : [])
      : (item && typeof item === 'object' && Object.hasOwn(item, token) ? [item[token]] : []));
  }
  return values;
}
const groupingSource = read('src/signals/wire/readable.rs');
const grouping = [...groupingSource.matchAll(/signal: "([^"]+)",\s*prefix: "[^"]+",\s*parent: "([^"]+)",\s*count: (\d+)/g)];
function paths(value, prefix = '', out = new Set()) {
  if (Array.isArray(value)) {
    out.add(`${prefix}[]`);
    for (const item of value) paths(item, `${prefix}[]`, out);
  } else if (value && typeof value === 'object') {
    for (const [key, child] of Object.entries(value)) {
      const next = prefix ? `${prefix}.${key}` : key;
      out.add(next);
      paths(child, next, out);
    }
  }
  return out;
}

async function validateModule(file, source = read(`docs/signals/${file}`), importModule = true) {
    const stem = file.slice(0, -3);
    const card = read(`src/signals/catalog/${stem}.rs`).split('pub(crate) const CARD:')[1];
    assert.ok(card, 'missing canonical card');
    assert.match(source, /captured|illustrative|no data|no payload/i, 'label evidence status in a short comment');
    assert.doesNotMatch(source, /\$(?:THREAD|KEY|SCOPE)\b|\$(?:THREAD|KEY|SCOPE)_|root_span|program_sha256/,
      'obfuscation/provenance artifacts remain');
    const exports = dataModule(source);
    // Import only after validating the entire source as literal data.
    if (importModule) {
      const imported = await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);
      assert.deepEqual({ ...imported }, exports, 'ESM values must match literal data');
    }
    const values = Object.values(exports);
    const noData = /body:\s*CardBody::Fields\(&\[\s*\]\)/.test(card);
    if (noData) assert.equal(exports.default, undefined, 'no-data default must be undefined');
    else assert.notEqual(exports.default, undefined, 'an emitting signal needs a representative default');
    const signal = card.match(/signal:\s*"([^"]+)"/)[1];
    const markdown = read(`docs/signals/${stem}.md`);
    const fields = [...markdown.matchAll(/^\| `([^`]+)` \| ([^|]+) \|/gm)];
    const physical = new Set(controls[stem] ?? []);
    // MatchMedia's fixed query lists use one count per packed boolean map.
    if (stem === 'match_media') {
      for (const field of fields.filter(field => field[1].endsWith('Count'))) physical.add(field[1]);
    }
    const isPhysical = key => forbidden.has(key.split('.').at(-1)) || physical.has(key);
    const flags = [...markdown.matchAll(/^- `([^`]+)`$/gm)].map(match => match[1]);
    const allowed = new Set([...fields.flatMap(match => match[1].split('.').map(key => key.replace(/\[\]/g, ''))), ...flags]);
    const group = grouping.find(match => match[1] === signal);
    if (group) allowed.add(group[2]);
    // Tables describe physical fields; captured card paths additionally certify
    // logical array aliases and flattened flags in the readable projection.
    const sample = card.match(/sample_payload: Some\(r#"([\s\S]*?)"#\)/)?.[1];
    const captured = sample ? JSON.parse(sample) : undefined;
    const legalPaths = captured ? paths(captured) : new Set();
    const booleanPaths = new Set();
    function capturedBooleans(value, prefix = '') {
      if (Array.isArray(value)) return value.forEach(item => capturedBooleans(item, prefix + '[]'));
      if (!value || typeof value !== 'object') return;
      for (const [key, child] of Object.entries(value)) {
        const next = prefix ? `${prefix}.${key}` : key;
        if (typeof child === 'boolean') booleanPaths.add(next);
        capturedBooleans(child, next);
      }
    }
    capturedBooleans(captured);
    const flagLists = new Map();
    const aliases = signal === 'UserAgentData' ? { brandEntries: 'brands', fullVersionEntries: 'fullVersionList' } : {};
    const projectPath = key => {
      for (const [from, to] of Object.entries(aliases)) {
        if (key === from || key.startsWith(from + '[]')) return to + key.slice(from.length);
      }
      return key;
    };
    const repeated = new Set();
    for (const field of fields) {
      const projected = projectPath(field[1]);
      legalPaths.add(projected);
      if (field[2].includes('repeated')) {
        legalPaths.add(projected + '[]');
        repeated.add(projected);
      }
    }
    // Single scalar children unwrap to scalar items (e.g. specialKeyCounts[].count
    // and samples[].entropyWord); a multi-child group keeps its record checks.
    const scalarGroups = new Map();
    for (const field of fields.filter(field => field[2].startsWith('group') && field[2].includes('repeated'))) {
      const prefix = field[1] + '[].';
      const children = fields.filter(child => child[1].startsWith(prefix)
        && !/[.\[\]]/.test(child[1].slice(prefix.length)) && !isPhysical(child[1]));
      if (children.length === 1 && /^(number|string|boolean)/.test(children[0][2])
          && !children[0][2].includes('repeated')) scalarGroups.set(projectPath(field[1]), children[0]);
    }
    for (const [prefix, [child]] of Object.entries(arrayGroups[stem] ?? {})) {
      assert.ok(fields.some(field => field[1] === `${prefix}[].${child}` && field[2].includes('repeated')),
        `card no longer certifies unwrapped ${prefix}.${child}`);
      legalPaths.add(prefix + '[][]');
    }
    for (const match of markdown.matchAll(/Flag names of `([^`]+)`[^\n]*:\n\n((?:- `[^`]+`\n)+)/g)) {
      flagLists.set(match[1], [...match[2].matchAll(/- `([^`]+)`/g)].map(flag => flag[1]));
      for (const flag of match[2].matchAll(/- `([^`]+)`/g)) {
        const flagPath = projectPath(match[1] + '.' + flag[1]);
        legalPaths.add(flagPath);
        booleanPaths.add(flagPath);
        if (fields.length === 1) {
          legalPaths.add(flag[1]);
          booleanPaths.add(flag[1]);
        }
        // Grouped flag arrays contain boolean objects, one per physical byte.
        if (group && !match[1].includes('.')) {
          legalPaths.add(group[2] + '[].' + flag[1]);
          booleanPaths.add(group[2] + '[].' + flag[1]);
        }
      }
    }
    if (stem === 'current_script_async_policy_state') {
      // Card: scriptFlags.async when known, scriptAsync null when unknown.
      // Policy is never populated in the analyzed builds; constant statuses
      // and policy absence controls are intentionally outside this logical view.
      legalPaths.add('async');
      booleanPaths.add('async');
    }
    // A by-name array can be the payload itself rather than a field envelope.
    if (card.includes('by_name: true') && fields.length === 1) {
      for (const key of [...legalPaths]) {
        if (key.startsWith(fields[0][1] + '[]')) legalPaths.add(key.slice(fields[0][1].length));
      }
      legalPaths.add('[]');
    }
    const observed = new Set();
    for (const data of values) {
      function check(value, prefix = '') {
        if (Array.isArray(value)) return value.forEach(item => check(item, prefix + '[]'));
        if (!value || typeof value !== 'object') return;
        assert.ok(!Object.hasOwn(value, signal), 'remove the signal-name envelope');
        assert.ok(!Object.hasOwn(value, 'messages'), 'message alternatives belong in named exports');
        for (const [key, child] of Object.entries(value)) {
          const fieldPath = prefix ? `${prefix}.${key}` : key;
          assert.ok(!isPhysical(fieldPath), `encoding-only field ${fieldPath}`);
          assert.ok(allowed.has(key), `field ${key} is absent from card/readable projection`);
          if (booleanPaths.has(fieldPath)) {
            const nullable = (nullableFlags[stem] ?? []).includes(fieldPath)
              || (stem === 'current_script_async_policy_state' && fieldPath === 'async');
            assert.ok(typeof child === 'boolean' || (nullable && child === null), `logical flag ${fieldPath} must be boolean${nullable ? ' or null' : ''}`);
          }
          check(child, fieldPath);
        }
      }
      check(data);
      for (const item of paths(data)) {
        observed.add(item);
        // pairedFields is a build-dependent holder of two named sibling fields.
        const logical = item.replace(/\.pairedFields(?=\.)/g, '');
        assert.ok(legalPaths.has(logical) || [...legalPaths].some(key => key.startsWith(logical + '.') || key.startsWith(logical + '[]')),
          `field nesting ${item} is absent from card/readable projection`);
      }
      for (const prefix of repeated) {
        if (isPhysical(prefix)) continue;
        // Packed repeated flag bytes project to named boolean maps, not arrays
        // of physical bytes (FontAvailability and GlobalOwnPropertyBits).
        if (fields.some(field => projectPath(field[1]) === prefix && field[2].startsWith('flags'))) continue;
        for (const list of valuesAt(data, prefix)) {
          assert.ok(Array.isArray(list) || list === null, `repeated ${prefix} must be an array or documented null`);
        }
      }
    }
    // Require representative data, distinguishing records from certified scalar
    // and array items. A captured empty alternative cannot hide the primary shape.
    for (const field of fields.filter(match => match[2].startsWith('group'))) {
      const sourcePrefix = field[1];
      const prefix = aliases[sourcePrefix] ?? sourcePrefix;
      if (isPhysical(prefix) || prefix === 'pairedFields') continue;
      const lists = values.flatMap(data => valuesAt(data, prefix)).filter(Array.isArray);
      const items = lists.flat();
      const scalar = scalarGroups.get(prefix);
      const array = arrayGroups[stem]?.[prefix];
      const scalarItems = scalar ? items.filter(item => item !== null && typeof item !== 'object') : [];
      const arrayItems = array ? items.filter(Array.isArray) : [];
      for (const item of items) {
        if (scalar && item !== null && typeof item !== 'object') continue;
        if (array && Array.isArray(item)) continue;
        assert.ok(item && typeof item === 'object' && !Array.isArray(item) && Object.keys(item).length > 0,
          `representative record required for ${prefix}`);
        if (scalar) {
          const name = scalar[1].split('.').at(-1);
          assert.ok(Object.hasOwn(item, name), `single-child record ${prefix} must contain ${name}`);
        } else if (array && stem !== 'canvas_readback_probe') {
          assert.ok(Array.isArray(item[array[0]]), `array-child record ${prefix} must contain ${array[0]}`);
        }
      }
      for (const item of scalarItems) {
        assert.equal(typeof item, scalar[2].startsWith('number') ? 'number' : scalar[2].startsWith('string') ? 'string' : 'boolean',
          `wrong scalar item type for ${prefix}`);
      }
      for (const item of arrayItems) {
        assert.ok(item.every(channel => typeof channel === 'number'), `numeric components required for ${prefix}`);
        if (stem === 'emoji_glyph_rgba_samples') assert.ok(item.length <= array[1], `too many pixels for ${prefix}`);
        else assert.equal(item.length, array[1], `fixed component size for ${prefix}`);
      }
      if (array && stem === 'canvas_readback_probe') {
        for (const item of items.filter(item => item && !Array.isArray(item) && typeof item === 'object')) {
          if (Object.hasOwn(item, 'components')) assert.equal(item.components.length, 4);
          else for (const channel of ['red', 'green', 'blue', 'alpha']) assert.equal(typeof item[channel], 'number', `missing colour channel ${channel}`);
        }
      }
      const unwrapped = scalarItems.length > 0 || arrayItems.some(item => item.length > 0);
      assert.ok(unwrapped || [...observed].some(key => key.startsWith(`${prefix}[].`)),
        `show representative item fields for ${prefix}, using an illustrative primary export if uncaptured`);
      for (const list of valuesAt(exports.default, prefix).filter(Array.isArray)) {
        assert.ok(list.length > 0, `primary ${prefix} must show data; retain captured empty lists as named alternatives`);
      }
      const childPrefix = `${sourcePrefix}[].`;
      for (const child of fields.filter(match => match[1].startsWith(childPrefix)
          && !/[.\[\]]/.test(match[1].slice(childPrefix.length)))) {
        if (child[1].includes('pairedFields') || isPhysical(child[1])) continue;
        if (unwrapped && (scalar || child[1] === `${sourcePrefix}[].${array?.[0]}`)) continue;
        if (child[1].endsWith('.nullMarker') || child[1].endsWith('.nullFlag')) continue;
        const projected = prefix + child[1].slice(sourcePrefix.length);
        assert.ok(observed.has(projected), `missing nested field ${projected}`);
      }
    }
    if (card.includes('CardBody::Messages')) {
      const messages = [...card.matchAll(/CardMessage\s*\{/g)].length;
      assert.ok(values.length >= messages, `show all ${messages} supported message alternatives`);
    }
    if (group) {
      assert.ok(values.some(value => Array.isArray(value?.[group[2]]) && value[group[2]].length === Number(group[3])),
        `preserve fixed ${group[2]} cardinality ${group[3]}`);
      const config = groupingSource.match(new RegExp(`signal: "${signal}",\\s*prefix: "([^"]+)",\\s*parent: "[^"]+",\\s*count: \\d+,\\s*alphabetic: (true|false)`));
      assert.ok(config, `missing readable grouping configuration for ${signal}`);
      for (const value of values.filter(value => Array.isArray(value?.[group[2]]))) {
        assert.equal(value[group[2]].length, Number(group[3]), `fixed ${group[2]} cardinality`);
        const known = new Set([...flagLists.values()].flat());
        const present = new Set();
        value[group[2]].forEach((item, index) => {
          const field = config[1] + (config[2] === 'true' ? String.fromCharCode(65 + index) : index + 1);
          // Some cards list the union of all possible flags on the first byte;
          // a captured readable tuple is stronger per-index evidence. Without a
          // capture, require the full card union across the bounded byte groups.
          const capturedItem = captured?.[group[2]]?.[index];
          assert.ok(item && typeof item === 'object' && !Array.isArray(item), `boolean record required for ${field}`);
          assert.ok(Object.keys(item).length > 0 && Object.keys(item).length <= 8, `flag cardinality for ${field}`);
          if (capturedItem) assert.deepEqual(Object.keys(item).sort(), Object.keys(capturedItem).sort(), `complete captured flag names for ${group[2]}[${index}]`);
          for (const [key, flag] of Object.entries(item)) {
            assert.ok(known.has(key), `unknown flag ${key}`);
            assert.equal(typeof flag, 'boolean', `boolean flag ${key}`);
            present.add(key);
          }
        });
        assert.deepEqual([...present].sort(), [...known].sort(), `complete flag union for ${group[2]}`);
      }
    }
    // These fixed vectors are certified in the cards, even if an error path is empty.
    const fixed = {
      offline_audio_render_samples: ['sample', 9], video_can_play_type: ['codes', 34],
      webgl_context: ['shaderPrecisions', 12], keystroke_dynamics: ['specialKeyCounts', 22],
      canvas_readback_probe: ['records', 4],
    }[stem];
    if (fixed) assert.equal(exports.default[fixed[0]].length, fixed[1], `fixed ${fixed[0]} size`);
    if (stem === 'webgl_context') assert.equal(exports.default.dimensions.length, 3);
    if (stem === 'remember_me_checkbox_state') {
      assert.equal(typeof exports.default, 'boolean');
      assert.ok(values.includes(''), 'show the missing-checkbox bare string alternative');
    }
    if (stem === 'current_script_async_policy_state') {
      assert.deepEqual(new Set(values.map(value => value.async)), new Set([true, false, null]),
        'show known async, known synchronous and unknown async values');
      for (const value of values) assert.deepEqual(Object.keys(value), ['async']);
    }
    if (stem === 'user_agent_data' && exports.missingHints) {
      assert.equal(exports.missingHints.flags.wow64, null, 'unavailable wow64 is unknown, not observed false');
      assert.equal(exports.missingHints.flags.mobile, null, 'unavailable mobile is unknown, not observed false');
    }
    if (stem === 'ponos_xhr_round_trip') {
      assert.deepEqual(exports.provisional?.signalIndex, [], 'provisional digest index list is empty');
      assert.equal(exports.provisional.xhrStatus, 0);
      assert.equal(exports.provisional.xhrLoadEndTs, -1);
    }
    const measurements = {
      memory_breakdown_summary: ['totalBytes', 'breakdownCount'],
      navigator_plugins: ['pluginsLength'],
      vm_source_integrity: ['bodyLength', 'whitespaceCount', 'punctuationCount', 'buildMarker'],
    }[stem] ?? [];
    for (const key of measurements) assert.ok(Object.hasOwn(exports.default, key), `preserve measured ${key}`);
    return exports;
}

// Mutate real data shapes in memory: these regressions must fail for semantic
// reasons, without touching the checked-in examples or importing fixture code.
async function regressionChecks() {
  const cases = [
    ['user_agent_data', data => { data.missingHints.wow64Placeholder = null; }, /encoding-only/],
    ['user_agent_data', data => { data.missingHints.flags.wow64 = false; }, /unknown, not observed false/],
    ['ponos_xhr_round_trip', data => { data.provisional.signalIndexCount = 0; }, /encoding-only/],
    ['memory_breakdown_summary', data => { data.apiUnavailable.entriesPlaceholder = null; }, /encoding-only/],
    ['memory_breakdown_summary', data => { data.default.entry[0].attribution[0].containerMarker = null; }, /encoding-only/],
    ['memory_breakdown_summary', data => { delete data.default.entry[0].entryBytes; }, /missing nested field/],
    ['memory_breakdown_summary', data => { delete data.default.breakdownCount; }, /preserve measured/],
    ['memory_breakdown_summary', data => { data.default.entry[0].memoryType = 'JavaScript'; }, /repeated/],
    ['webgl_context', data => { data.valuesMissing.antialiasMissing = null; }, /encoding-only/],
    ['visibility_event_log', data => { data.unknownVisibility.stateNullTag = null; }, /encoding-only/],
    ['match_media', data => { data.default.gridCount = 2; }, /encoding-only/],
    ['global_own_property_bits', data => { data.default.probeCount = 44; }, /encoding-only/],
    ['current_script_async_policy_state', data => { data.default.status1 = 130; }, /encoding-only/],
    ['current_script_async_policy_state', data => { data.asyncUnknown.async = false; }, /unknown async/],
    ['browser_api_proxy_flags', data => { data.default.proxyChecks[0].unknownFlag = true; }, /absent/],
    ['browser_api_proxy_flags', data => { data.default.proxyChecks.pop(); }, /cardinality/],
    ['browser_api_proxy_flags', data => { delete data.default.proxyChecks[0].Proxy; }, /complete captured flag names/],
    ['browser_api_proxy_flags', data => { data.default.proxyChecks[0].Proxy = null; }, /logical flag|boolean flag/],
    ['ui_event_samples_touch', data => { data.default.records = []; }, /representative|primary/],
    ['webrtc_ice_candidate_addresses', data => { data.default.candidates = []; }, /representative|primary/],
    ['ui_event_samples_touch', data => {
      // Every export must lose the field: a complete alternate export must not
      // mask a missing nested field in the primary one.
      for (const value of Object.values(data)) {
        value?.records?.forEach(record => record.points?.forEach(point => { delete point.pageX; }));
      }
    }, /missing nested field/],
    ['ui_event_samples_touch', data => { data.default.time = 3; }, /field nesting/],
    ['canvas_scene_pixel_samples', data => { data.default.samples[0].push(100); }, /fixed component size/],
    ['canvas_readback_probe', data => { data.componentLoopColours.records[0].directColours[0].push(0); }, /fixed component size/],
    ['keystroke_dynamics', data => { data.default.specialKeyCounts[0] = 'bad'; }, /wrong scalar item type/],
    ['keystroke_dynamics', data => { data.default.specialKeyCounts.pop(); }, /fixed specialKeyCounts size/],
    ['lifecycle_entropy_samples', data => { data.default.samples = [{}]; data.collectionSkipped.samples = []; data.collectionRetried.samples = []; }, /representative/],
    ['lifecycle_entropy_samples', data => { data.default.samples = [{}]; }, /representative record/],
    ['webgl_render_hash', data => { data.default.kind = 70; }, /encoding-only/],
    ['shuffled_date_string', data => { data.default.zero = 0; }, /encoding-only/],
    ['vm_source_integrity', data => { delete data.default.buildMarker; }, /preserve measured/],
  ];
  for (const [stem, mutate, reason] of cases) {
    const data = dataModule(read(`docs/signals/${stem}.js`));
    mutate(data);
    const source = '// Illustrative validator regression fixture.\n' + Object.entries(data).map(([key, value]) =>
      `export ${key === 'default' ? 'default' : `const ${key} =`} ${value === undefined ? 'undefined' : JSON.stringify(value)};`).join('\n');
    await assert.rejects(validateModule(`${stem}.js`, source, false), reason, `regression in ${stem}`);
  }
  console.log(`${cases.length} semantic regression fixtures rejected`);
}

let failed = 0;
for (const file of selected) {
  try {
    const exports = await validateModule(file);
    const count = Object.keys(exports).length;
    console.log(`PASS ${file} (${count} data export${count === 1 ? '' : 's'})`);
  } catch (error) {
    failed++;
    console.error(`FAIL ${file}: ${error.message}`);
  }
}
console.log(`${selected.length - failed}/${selected.length} canonical modules passed${requested.length ? ' (explicit development subset)' : ''}`);
if (failed) process.exitCode = 1;
else await regressionChecks();
