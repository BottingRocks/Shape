'use strict';

// A valid artifact with zero signal routes, for lifetime/API tests. No seed
// compilation or private capture is needed; codec geometry is synthetic.
module.exports = function artifact() {
  const source = { sha256: '0'.repeat(64), bytes: 0 };
  const proof = { codec: null, span: null, sha256: null, reason: 'test' };
  return {
    format: 'shape-decode-artifact/1', siteId: 'test', floatByteOrder: 'little',
    layout: {
      format: 'shape-seed-layout/1', source,
      session: {
        transactionToken: 'test', alphabet: 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/',
        shuffleKey: [], keys: [1, 2, 3, 4, 5, 6, 7, 8],
      },
      cipher: { ivs: [1, 2, 3, 4], counter: 0, double_rounds: 4 },
      header: {
        name: 'x-test-a', prefix: 'x-test-', b: false, f: false, c: 'test', d: 'test', z: 'test',
        checksum: 'signed-paired-utf16-base36-token-plus-full-payload',
      },
      keysets: [], report: { errors: [], positionalSignals: [], signals: [], keyedSignals: [] },
    },
    schema: {
      source, signals: {}, diagnostics: {},
      wire: {
        version: 1, source, slots: [],
        algorithms: { hash: proof, stream: proof, integer: proof, float_prefix: proof, string: proof, float: proof },
        encoding: {
          schema_version: 1, source, number: null, string: null, byte_stream: null,
          seed_link: { hash: null, seed_reads: [], runtime_value: false, unresolved: [] }, unresolved: [],
        },
      },
    },
  };
};
