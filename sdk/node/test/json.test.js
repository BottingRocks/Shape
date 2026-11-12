'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const shape = require('..');

// Exercise Node 18/20's fallback even when the test runner has JSON.rawJSON.
const context = vm.createContext({ Buffer, Uint8Array, ArrayBuffer, module: { exports: {} }, __dirname: __dirname });
context.require = name => name === 'path' ? require(name)
  : name === './operations.js' ? require('../operations.js') : {};
context.process = { env: {} };
vm.runInContext('JSON.rawJSON = undefined', context);
vm.runInContext(fs.readFileSync(require.resolve('..'), 'utf8'), context);
const fallback = context.module.exports;

for (const [name, sdk] of [['native JSON', shape], ['fallback JSON', fallback]]) {
  test(`${name}: exact integers, strings, keys, and invalid syntax`, () => {
    const text = '{"bits":18446744073709551615,"neg":-9223372036854775808,"text":"9007199254740993","float":1.25,"exp":1e20,"array":[9007199254740993]}';
    const parsed = sdk.parseJSON(text);
    assert.equal(parsed.bits, 18446744073709551615n);
    assert.equal(parsed.neg, -9223372036854775808n);
    assert.equal(parsed.text, '9007199254740993');
    assert.equal(parsed.exp, 1e20);
    assert.equal(parsed.array[0], 9007199254740993n);
    assert.equal(sdk.parseJSON(sdk.stringifyJSON(parsed)).bits, parsed.bits);
    assert.equal(sdk.parseJSON('  -9223372036854775808  '), parsed.neg);
    const bytes = new TextEncoder().encode(text);
    assert.equal(sdk.parseJSON(bytes.buffer).bits, parsed.bits);
    const special = { '\0shape.bigint:1': '\0shape.bigint:123', more: '\0\0literal', value: 9007199254740993n };
    const roundtrip = sdk.parseJSON(sdk.stringifyJSON(special));
    assert.equal(roundtrip['\0shape.bigint:1'], special['\0shape.bigint:1']);
    assert.equal(roundtrip.more, special.more);
    assert.equal(roundtrip.value, special.value);
    assert.equal(sdk.parseJSON('"\\u0000shape.bigint:9007199254740993"'), '\0shape.bigint:9007199254740993');
    for (const invalid of ['09007199254740993', '[9007199254740993,]', '{"a":9007199254740993} garbage']) {
      assert.throws(() => sdk.parseJSON(invalid), { name: 'SyntaxError' });
    }
    assert.throws(() => sdk.parseJSON(42), { name: 'TypeError' });
    assert.equal(sdk.stringifyJSON(undefined), undefined);
  });
}
