'use strict';
// shape: Node.js binding of libshape (the C ABI in sdk/c/include/shape.h).
//
// Every call sends one JSON request and gets back one JSON envelope,
// {"ok":true,"value":…} or {"ok":false,"error":{"code","message"}}. This
// module parses envelopes, returns the value and throws ShapeError otherwise.

const path = require('path');

function loadAddon() {
  const file = process.env.SHAPE_ADDON_PATH || path.join(__dirname, 'build', 'shape.node');
  try {
    return require(file);
  } catch (error) {
    const hint = new Error(
      `shape: cannot load the native addon ${file} (${error.message}). ` +
        'Build it with sdk/node/build.sh.'
    );
    hint.cause = error;
    throw hint;
  }
}

const native = loadAddon();

/** A failed shape call: `code` is invalid_request, unknown_command, failed or panic. */
class ShapeError extends Error {
  constructor(code, message) {
    super(message);
    this.name = 'ShapeError';
    this.code = code;
  }
}

// ---- lossless JSON ------------------------------------------------------------
//
// Artifacts and decoded payloads carry 64-bit integers (e.g. f64 bit patterns)
// that a JS number cannot hold. Integers outside the safe range parse as
// BigInt and serialize back as the same digits, so an object from
// compileArtifact() or decode() round-trips unchanged into Generator.load /
// explain / encode. New runtimes use JSON.parse source access and JSON.rawJSON;
// Node 18/20 use a token-preserving fallback with the same public behavior.

const LOSSLESS = (() => {
  try {
    if (typeof JSON.rawJSON !== 'function') return false;
    let source;
    JSON.parse('[9007199254740993]', (key, value, context) => {
      if (key === '0') source = context && context.source;
      return value;
    });
    return source === '9007199254740993' && JSON.stringify([JSON.rawJSON('9007199254740993')]) === '[9007199254740993]';
  } catch {
    return false;
  }
})();

// A number token of 16+ digits: the only kind that can be an unsafe integer.
// A JSON value starts at the document start or after ':', ',' or '[', so those
// are the only places the scan can miss a large integer.
const LONG_NUMBER = /(?:^|[:,[])\s*-?\d{16}/;
const INTEGER = /^-?\d+$/;
// Match entire strings first so digits inside them are never treated as numbers.
const JSON_TOKEN = /"(?:[^"\\]|\\.)*"|-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?/g;
const BIGINT_MARKER = '\u0000shape.bigint:';

function escapeMarker(value) {
  return typeof value === 'string' && value.startsWith('\u0000') ? '\u0000' + value : value;
}

function restoreMarker(value) {
  if (typeof value !== 'string') return value;
  if (value.startsWith('\u0000\u0000')) return value.slice(1);
  if (value.startsWith(BIGINT_MARKER)) return BigInt(value.slice(BIGINT_MARKER.length));
  return value;
}

// Transform value tokens only: object keys must retain their exact spelling.
function mapTokens(json, visit) {
  return json.replace(JSON_TOKEN, (token, offset) => {
    if (/^\s*:/.test(json.slice(offset + token.length))) return token;
    return visit(token);
  });
}

function reviveBig(key, value, context) {
  if (typeof value === 'number' && !Number.isSafeInteger(value) && context && INTEGER.test(context.source)) {
    return BigInt(context.source);
  }
  return value;
}

function replaceBig(key, value) {
  return typeof value === 'bigint' ? JSON.rawJSON(value.toString()) : value;
}

function parseJSON(text) {
  text = doc(text);
  if (typeof text !== 'string') throw new TypeError('JSON text must be a string, Buffer, Uint8Array or ArrayBuffer');
  if (!LONG_NUMBER.test(text)) return JSON.parse(text);
  if (LOSSLESS) return JSON.parse(text, reviveBig);
  // Validate before replacing tokens, including illegal leading zeroes.
  JSON.parse(text);
  const marked = mapTokens(text, (token) => {
    if (token[0] === '"') return JSON.stringify(escapeMarker(JSON.parse(token)));
    return INTEGER.test(token) && !Number.isSafeInteger(Number(token))
      ? JSON.stringify(BIGINT_MARKER + token) : token;
  });
  return JSON.parse(marked, (key, value) => restoreMarker(value));
}

function stringifyJSON(value) {
  if (LOSSLESS) return JSON.stringify(value, replaceBig);
  const json = JSON.stringify(value, (key, item) =>
    typeof item === 'bigint' ? BIGINT_MARKER + item : escapeMarker(item));
  if (json === undefined) return undefined;
  return mapTokens(json, (token) => {
    if (token[0] !== '"') return token;
    const item = restoreMarker(JSON.parse(token));
    return typeof item === 'bigint' ? item.toString() : JSON.stringify(item);
  });
}

function unwrap(text) {
  let envelope;
  try {
    envelope = parseJSON(text);
  } catch (error) {
    throw new ShapeError('failed', `libshape returned invalid JSON: ${error.message}`);
  }
  if (envelope && envelope.ok === true) return envelope.value;
  const error = (envelope && envelope.error) || {};
  throw new ShapeError(error.code || 'failed', error.message || 'unknown error');
}

// ---- request building ---------------------------------------------------------

function snake(key) {
  return key.replace(/[A-Z]/g, (c) => '_' + c.toLowerCase());
}

/** A document argument: Buffer/Uint8Array → JSON text; string and objects pass through. */
function doc(value) {
  if (value instanceof Uint8Array) return Buffer.from(value.buffer, value.byteOffset, value.byteLength).toString('utf8');
  if (value instanceof ArrayBuffer) return Buffer.from(value).toString('utf8');
  return value;
}

/** Source text: string, Buffer or Uint8Array. */
function text(value, what) {
  if (typeof value === 'string') return value;
  if (value instanceof Uint8Array) return doc(value);
  throw new TypeError(`${what} must be a string, Buffer or Uint8Array`);
}

/** Options → snake_case request fields; undefined values are dropped. */
function requestValue(value, name) {
  if (typeof value === 'number' && !Number.isFinite(value)) {
    throw new TypeError(`${name} must be finite`);
  }
  return doc(value);
}

function request(fields, options) {
  const out = {};
  for (const [key, value] of Object.entries(fields)) {
    if (value !== undefined) out[key] = requestValue(value, key);
  }
  if (options != null) {
    if (Object.prototype.toString.call(options) !== '[object Object]') {
      throw new TypeError('options must be an object with named fields');
    }
    for (const [key, value] of Object.entries(options)) {
      const field = snake(key);
      if (value !== undefined) {
        if (Object.hasOwn(fields, field)) throw new TypeError(`options cannot override ${field}`);
        if (Object.hasOwn(out, field)) throw new TypeError(`duplicate option ${field}`);
        Object.defineProperty(out, field, { value: requestValue(value, key), enumerable: true, configurable: true });
      }
    }
  }
  return out;
}

function encodeRequest(req) {
  if (req == null) return '';
  if (typeof req === 'string') return req;
  if (req instanceof Uint8Array || req instanceof ArrayBuffer) return doc(req);
  const encoded = stringifyJSON(req);
  if (encoded === undefined) throw new TypeError('request must be a JSON document');
  return encoded;
}

// ---- stateless commands ---------------------------------------------------------

/** The SDK version string, e.g. "0.1.0". */
function version() {
  return native.version();
}

/** Discover the commands supported by the loaded native library. */
async function commands() {
  return (await call('version')).commands;
}
function commandsSync() {
  return callSync('version').commands;
}

/** Run any command asynchronously (on the libuv pool). Resolves to the value. */
async function call(command, req) {
  if (typeof command !== 'string') throw new TypeError('command must be a string');
  return unwrap(await native.callAsync(command, encodeRequest(req)));
}

/** Run any command synchronously on the calling thread. */
function callSync(command, req) {
  if (typeof command !== 'string') throw new TypeError('command must be a string');
  return unwrap(native.call(command, encodeRequest(req)));
}

async function operation(command, req) {
  if (typeof command !== 'string') throw new TypeError('operation must be a string');
  return unwrap(await native.callV2Async(command, encodeRequest(req)));
}
function operationSync(command, req) {
  if (typeof command !== 'string') throw new TypeError('operation must be a string');
  return unwrap(native.callV2(command, encodeRequest(req)));
}
const run = (command, fields, options) => operation(command, request(fields, options));

async function deob(source, options) {
  return run('deob', { source: text(source, 'source') }, options);
}
async function report(source, options) {
  return run('report', { source: text(source, 'source') }, options);
}

const kernel = {
  async show(source, options) {
    return run('kernel.show', { source: text(source, 'source') }, options);
  },
};

const ops = {
  async list(source) {
    return run('ops.list', { source: text(source, 'source') });
  },
  async show(source, op) {
    return run('ops.show', { source: text(source, 'source'), op });
  },
};

const trace = {
  async program(source, options) {
    const req = request({ source: text(source, 'source') }, options);
    // Preserve the older named option while sending only the v2 cache shape.
    if (Object.hasOwn(req, 'report_dir')) {
      if (Object.hasOwn(req, 'cache')) throw new TypeError('choose cache or legacy reportDir');
      req.cache = { mode: 'directory', directory: req.report_dir };
      delete req.report_dir;
    }
    return operation('trace.program', req);
  },
  async thread(source, options) {
    const fields = source == null ? {} : { source: text(source, 'source') };
    return run('trace.thread', fields, options);
  },
};

const cleanup = {
  async thread(source) {
    return run('cleanup.thread', { source: text(source, 'source') });
  },
};

const naming = {
  async threads(source, options) {
    return run('naming.threads', { source: text(source, 'source') }, options);
  },
  async collectors(source, options) {
    return run('naming.collectors', { source: text(source, 'source') }, options);
  },
  async plan(source) {
    return run('naming.plan', { source: text(source, 'source') });
  },
  async inventory(source) {
    return run('naming.inventory', { source: text(source, 'source') });
  },
};

const signals = {
  async schema(source, options) {
    return run('signals.schema', { source: text(source, 'source') }, options);
  },
  async show(source) {
    return run('signals.show', { source: text(source, 'source') });
  },
  async inputs(source) {
    return run('signals.inputs', { source: text(source, 'source') });
  },
  async contracts(source) {
    return run('signals.contracts', { source: text(source, 'source') });
  },
};

async function pin(source, options) {
  return run('pin', { source: text(source, 'source') }, options);
}
async function sessionFromInitializer(init) {
  return run('session.from_initializer', { init: text(init, 'init') });
}
async function compileArtifact(source, options) {
  if (options != null && Object.hasOwn(options, 'direct') &&
      options.direct !== undefined && typeof options.direct !== 'boolean') {
    throw new TypeError('direct must be a boolean');
  }
  return run('artifact.compile', { source: text(source, 'source') }, options);
}
async function compactSchema(artifact) {
  return run('artifact.compact_schema', { artifact: doc(artifact) });
}

// ---- generator -------------------------------------------------------------------

function artifactBytes(artifact) {
  if (artifact instanceof Uint8Array || artifact instanceof ArrayBuffer) return artifact;
  if (typeof artifact === 'string') return Buffer.from(artifact, 'utf8');
  if (artifact && typeof artifact === 'object') return Buffer.from(stringifyJSON(artifact), 'utf8');
  throw new TypeError('artifact must be an object, JSON string, Buffer, Uint8Array or ArrayBuffer');
}

function loaded(result, Type = Generator) {
  if (typeof result === 'string') unwrap(result); // error envelope: throws
  return new Type(result, CONSTRUCT);
}

const CONSTRUCT = Symbol('construct');

/** One loaded, validated decode artifact for repeated decode / explain / encode. */
class Generator {
  #nativeHandle;
  /** @private Use Generator.load / Generator.loadAsync. */
  constructor(handle, token) {
    if (token !== CONSTRUCT || handle === null || typeof handle !== 'object') {
      throw new TypeError('use Generator.load(artifact)');
    }
    this.#nativeHandle = handle;
  }

  /** Parse and validate a decode artifact synchronously. */
  static load(artifact) {
    return loaded(native.generatorLoad(artifactBytes(artifact)), this);
  }

  /** Parse and validate a decode artifact on the libuv pool. */
  static async loadAsync(artifact) {
    return loaded(await native.generatorLoadAsync(artifactBytes(artifact)), this);
  }

  get closed() {
    return this.#nativeHandle === null;
  }

  #handle() {
    const handle = this.#nativeHandle;
    if (handle === null) throw new ShapeError('invalid_request', 'generator is closed');
    return handle;
  }

  #call(method, req) {
    if (typeof method !== 'string') throw new TypeError('method must be a string');
    return unwrap(native.generatorCall(this.#handle(), method, encodeRequest(req)));
  }

  async #callAsync(method, req) {
    if (typeof method !== 'string') throw new TypeError('method must be a string');
    return unwrap(await native.generatorCallAsync(this.#handle(), method, encodeRequest(req)));
  }

  static #decodeRequest(capture, options) {
    return request({ capture: doc(capture) }, options);
  }
  static #explainRequest(decoded, signal, options) {
    if (typeof signal !== 'string') throw new TypeError('signal must be a string');
    return request({ decoded: doc(decoded), signal }, options);
  }
  static #encodeRequest(input) {
    if (input == null || typeof input !== 'object') {
      throw new TypeError('encode takes {signals} or {decoded}, plus an optional session');
    }
    return request({}, input);
  }

  /** Run any generator method with an object or exact JSON text request. */
  call(method, req) {
    return this.#call(method, req);
  }
  async callAsync(method, req) {
    return this.#callAsync(method, req);
  }

  /** Headers → named signals. */
  decode(capture, options) {
    return this.#call('decode', Generator.#decodeRequest(capture, options));
  }
  async decodeAsync(capture, options) {
    return this.#callAsync('decode', Generator.#decodeRequest(capture, options));
  }

  /** One signal's full metadata from a decoded payload. */
  explain(decoded, signal, options) {
    return this.#call('explain', Generator.#explainRequest(decoded, signal, options));
  }
  async explainAsync(decoded, signal, options) {
    return this.#callAsync('explain', Generator.#explainRequest(decoded, signal, options));
  }

  /** Named signals → headers. */
  encode(input) {
    return this.#call('encode', Generator.#encodeRequest(input));
  }
  async encodeAsync(input) {
    return this.#callAsync('encode', Generator.#encodeRequest(input));
  }

  /** The artifact's compact schema. */
  schema() {
    return this.#call('schema', {});
  }
  schemaAsync() {
    return this.#callAsync('schema', {});
  }

  /** Export an independent document only when the caller requests persistence. */
  toJSON() { return this.#call('artifact', {}); }
  toJSONAsync() { return this.#callAsync('artifact', {}); }

  /** Release the artifact now. Safe to call twice; in-flight async calls finish first. */
  close() {
    const handle = this.#nativeHandle;
    if (handle === null) return;
    native.generatorFree(handle);
    this.#nativeHandle = null;
  }
}

if (typeof Symbol.dispose === 'symbol') {
  Generator.prototype[Symbol.dispose] = function dispose() {
    this.close();
  };
}

/** Primary name for an immutable, validated executable artifact. */
class LoadedArtifact extends Generator {}

/** A parsed kernel reused in memory, independently of a disk cache policy. */
class LoadedKernel {
  #handle;
  constructor(handle, token) {
    if (token !== CONSTRUCT) throw new TypeError('use kernel.load(artifact)');
    this.#handle = handle;
  }
  static load(input) { return loaded(native.kernelLoad(artifactBytes(input)), this); }
  static async loadAsync(input) { return loaded(await native.kernelLoadAsync(artifactBytes(input)), this); }
  get closed() { return this.#handle === null; }
  #live() {
    if (this.closed) throw new ShapeError('invalid_request', 'kernel is closed');
    return this.#handle;
  }
  async call(operation, fields, options) {
    return unwrap(await native.generatorCallAsync(this.#live(), operation, encodeRequest(request(fields, options))));
  }
  program(source, options) { return this.call('trace.program', { source: text(source, 'source') }, options); }
  thread(options) { return this.call('trace.thread', {}, options); }
  reachable(options) { return this.call('trace.corpus.reachable', {}, options); }
  toJSON() { return parseJSON(unwrap(native.generatorCall(this.#live(), 'kernel.to_json', '{}'))); }
  async toJSONAsync() { return parseJSON(await this.call('kernel.to_json', {})); }
  close() {
    if (this.closed) return;
    native.generatorFree(this.#handle);
    this.#handle = null;
  }
}
if (typeof Symbol.dispose === 'symbol') {
  LoadedKernel.prototype[Symbol.dispose] = function dispose() { this.close(); };
}

async function compileLoaded(source, options) {
  const req = request({ source: text(source, 'source') }, options);
  return loaded(await native.artifactCompileAsync(encodeRequest(req)), LoadedArtifact);
}

const extensions = require('./operations.js')({ run, request, text, doc, parseJSON, compileLoaded, LoadedArtifact, LoadedKernel });
Object.assign(kernel, extensions.kernel);
Object.assign(ops, extensions.ops);
Object.assign(trace, extensions.trace);
Object.assign(naming, extensions.naming);
Object.assign(signals, extensions.signals);

module.exports = {
  ShapeError,
  Generator,
  LoadedArtifact,
  LoadedKernel,
  operation,
  operationSync,
  ...extensions.direct,
  version,
  commands,
  commandsSync,
  call,
  callSync,
  deob,
  report,
  kernel,
  ops,
  trace,
  cleanup,
  naming,
  signals,
  pin,
  sessionFromInitializer,
  compileArtifact,
  compactSchema,
  parseJSON,
  stringifyJSON,
};

// `import shape from "shape"` under TypeScript esModuleInterop / bundlers.
module.exports.default = module.exports;
