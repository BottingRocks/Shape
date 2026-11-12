// Test adapter only: every entry resolves a public named function, never call().
const camel = text => text.replace(/_([a-z])/g, (_, char) => char.toUpperCase());
const aliases = { 'kernel.to_json': 'kernel.toJSON' };
export function publicPath(id) { return aliases[id] ?? id.split('.').map(camel).join('.'); }
export function resolve(shape, id) {
  const path = publicPath(id);
  const parts = path.split('.');
  const name = parts.pop();
  const owner = parts.reduce((value, key) => value?.[key], shape);
  const method = owner?.[name];
  if (typeof method !== 'function') throw new Error(`missing named API ${id}: ${path}`);
  return method.bind(owner);
}

export async function invoke(shape, id, body) {
  const fn = resolve(shape, id);
  const { source, ...options } = body;
  switch (id) {
    case 'version': return fn();
    case 'artifact.compact_schema': return fn(body.artifact);
    case 'signals.catalog': case 'seeds.sites': return fn();
    case 'signals.describe': case 'seeds.site': return fn(body.name);
    case 'ops.classify.seeds': return fn(body.seeds);
    case 'ops.classify.template': return fn(body.source, body.handler_index);
    case 'trace.corpus.compare': case 'naming.corpus.compare': return fn(body.baseline, body.current);
    case 'trace.corpus.compare_inline': return fn(body.outlined, body.inline);
    case 'naming.corpus.analyze': return fn(body.sources);
    case 'seeds.parse': case 'seeds.parse_launcher': case 'seeds.parse_kernel': return fn(body.bytes);
    case 'seeds.parse_url': return fn(body.input);
    case 'seeds.resolve_url': return fn(body.reference, body.base);
    case 'seeds.token': return fn(body.reference, body.param);
    case 'seeds.client': return fn(body);
    case 'headers.codec': case 'payload.codec': return fn(body.profile);
    case 'headers.parse': return fn(body.text);
    case 'headers.encode': case 'payload.encode': return fn(body.profile, body.report);
    case 'headers.decode': return fn(body.profile, body.headers, body.positional_count, body.keysets);
    case 'payload.decode': return fn(body.profile, body.text, body.positional_count, body.keysets);
    case 'headers.encode_bytes': return fn(body.profile, body.payload);
    case 'payload.encode_bytes': return fn(body.profile, body.bytes, body.seeds);
    case 'headers.decode_bytes': case 'headers.decode_finalized': return fn(body.profile, body.headers);
    case 'payload.decode_bytes': return fn(body.profile, body.text);
    case 'headers.encode_finalized': return fn(body.profile, body.text, body.context);
    case 'pack': return fn(body.value, body.options);
    case 'unpack': case 'unpack_canonical': return fn(body.bytes, body.options);
    default: {
      // Source-family option names are idiomatic camelCase at the public API.
      const converted = Object.fromEntries(Object.entries(options).map(([key, value]) => [camel(key), value]));
      if (id === 'artifact.compile') {
        const loaded = await fn(source, converted);
        try { return loaded.toJSON(); } finally { loaded.close(); }
      }
      return fn(source, converted);
    }
  }
}

export async function loadedCall(shape, loaded, id, body) {
  const { capture, decoded, signal, signals, ...options } = body;
  switch (id) {
    case 'decode': return shape.decode(loaded, capture, options);
    case 'explain': return shape.explain(loaded, decoded, signal, options);
    case 'encode': return 'signals' in body ? shape.encodeSignals(loaded, signals, options) : shape.encode(loaded, decoded, options);
    case 'schema': return shape.schema(loaded);
    case 'artifact': return loaded.toJSON();
    default: throw new Error(`unmapped loaded operation ${id}`);
  }
}
