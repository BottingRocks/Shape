'use strict';
const assert = require('node:assert/strict');
const direct = 'return x >= a && x <= b || x >= c && x <= d || x === 45;';
const bodies = [
  direct,
  'let f; if (x >= a) f = x <= b; else f = false; return f || x >= c && x <= d || x === 45;',
  'if (x >= a && x <= b || x >= c && x <= d) return true; else return x === 45;',
  'let f = x >= a && x <= b; if (f) return f; let g = x >= c && x <= d; return g || x === 45;',
  'let f; if (x >= a && x <= b) f = true; else if (x >= c) f = x <= d; else f = false; if (f) return f; return x === 45;',
  // Heldout spelling: blocks, a Boolean copy, early returns.
  'let f; { if (x >= a) { f = x <= b; } else { f = false; } } let saved = f; if (saved) return true; if (x >= c && x <= d) return true; return x === 45;',
];
const functions = bodies.map(body => new Function('x', 'a', 'b', 'c', 'd', body));
function run(fn, bits, throwAt = -1) {
  const log = [];
  let event = 0;
  const input = { valueOf() { log.push('input'); if (event === throwAt) throw Error('coercion'); return 10; } };
  const bounds = ['a', 'b', 'c', 'd'].map((name, index) => ({ valueOf() {
    log.push(name);
    const bit = (bits >> event++) & 1;
    // x>=low or x<=high can independently succeed/fail on each event.
    return index % 2 === 0 ? (bit ? 9 : 11) : (bit ? 11 : 9);
  }}));
  try { const result = fn(input, ...bounds); assert.equal(typeof result, 'boolean'); return { result, log }; }
  catch (error) { return { error: error.message, log }; }
}
for (let bits = 0; bits < 32; ++bits) {
  for (let throwAt = -1; throwAt < 4; ++throwAt) {
    const expected = run(functions[0], bits, throwAt);
    for (const fn of functions) assert.deepEqual(run(fn, bits, throwAt), expected);
  }
}
for (const input of [45, 44, undefined, null, NaN, '45', 10n]) {
  for (const bounds of [[undefined, undefined, undefined, undefined], [1, 20, 30, 40]]) {
    for (const fn of functions) assert.equal(fn(input, ...bounds), functions[0](input, ...bounds));
  }
}
const repeated = new Function('x', 'a', 'b', 'c', 'd',
  'let f; if (x >= a && x <= b) f = true; else if (x >= c) f = x <= d; else f = x >= c; return f || x === 45;');
assert.notDeepEqual(run(repeated, 0), run(functions[0], 0), 'repeated failed lower comparison is observable');
const early = new Function('x', 'a', 'b', 'c', 'd', 'let f = x >= c && x <= d; return x >= a && x <= b || f || x === 45;');
assert.notDeepEqual(run(early, 31), run(functions[0], 31), 'eager second range changes valueOf order');
const folded = x => x === 45;
const log = [];
const object = {valueOf() { log.push('input'); return 45; }};
assert.equal(functions[0](object, undefined, undefined, undefined, undefined), false);
assert.equal(log.length, 2, 'undefined captures still coerce input for both lower comparisons');
folded(object);
assert.equal(log.length, 2);
assert.throws(() => { const f = () => functions[1](1, a, b, c, d); f(); let a,b,c,d; }, ReferenceError);
console.log('PASS: self-authored and heldout valueOf traces, throwing continuations, Boolean returns, repeated/eager negatives, undefined captures and TDZ');
