// Restricted literal parser shared by validation and manifest generation. Never executes JS.
import assert from "node:assert/strict";
export function dataModule(source, parseNumber = Number) {
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
    if (number) { pos += number[0].length; return parseNumber(number[0]); }
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
