'use strict';
// Operation adapters use only the provider's v2 contract. Nested engine
// documents retain their serialized field names; public option keys are camelCase.
module.exports = ({ run, request, text, doc, parseJSON, compileLoaded, LoadedArtifact, LoadedKernel }) => {
  const source = input => ({ source: text(input, 'source') });
  const bytes = input => {
    if (input instanceof Uint8Array) return Array.from(input);
    if (input instanceof ArrayBuffer) return Array.from(new Uint8Array(input));
    if (Array.isArray(input) && input.every(x => Number.isInteger(x) && x >= 0 && x <= 255)) return input;
    throw new TypeError('bytes must be a Uint8Array, ArrayBuffer or u8 array');
  };
  const CachePolicy = Object.freeze({
    Disabled: Object.freeze({ mode: 'disabled' }),
    Environment: Object.freeze({ mode: 'environment' }),
    Directory(directory) {
      if (typeof directory !== 'string' || !directory.trim()) throw new TypeError('directory must be a nonempty string');
      return Object.freeze({ mode: 'directory', directory });
    },
  });
  const artifact = {
    load: input => LoadedArtifact.load(input),
    loadAsync: input => LoadedArtifact.loadAsync(input),
    fromArtifact: input => LoadedArtifact.load(input),
    compile: compileLoaded,
    compactSchema: input => run('artifact.compact_schema', { artifact: doc(input) }),
  };
  const session = { fromInitializer: init => run('session.from_initializer', { init: text(init, 'init') }) };
  const classify = {
    source: input => run('ops.classify.source', source(input)),
    report: input => run('ops.classify.report', source(input)),
    reportDocument: report => run('ops.classify.report', { report: doc(report) }),
    template: (input, handlerIndex) => run('ops.classify.template', { ...source(input), handler_index: handlerIndex }),
    templateDocument: (input, template) => run('ops.classify.template', { ...source(input), template: doc(template) }),
    seeds: input => run('ops.classify.seeds', { seeds: input }),
  };
  const corpus = {
    load: (input, options) => run('trace.corpus.load', source(input), options),
    source: (input, options) => run('trace.corpus.source', source(input), options),
    reachable: (input, options) => input instanceof LoadedKernel ? input.reachable(options)
      : run('trace.corpus.reachable', { artifact: doc(input) }, options),
    walkSeed: (seed, options) => run('trace.corpus.walk_seed', { seed }, options),
    fingerprint: (input, options) => run('trace.corpus.fingerprint', { artifact: doc(input) }, options),
    compare: (baseline, current) => run('trace.corpus.compare', { baseline: doc(baseline), current: doc(current) }),
    compareInline: (outlined, inline) => run('trace.corpus.compare_inline', { outlined: doc(outlined), inline: doc(inline) }),
  };
  const seeds = {
    parse: input => run('seeds.parse', { bytes: bytes(input) }),
    parseLauncher: input => run('seeds.parse_launcher', { bytes: bytes(input) }),
    parseKernel: input => run('seeds.parse_kernel', { bytes: bytes(input) }),
    parseUrl: input => run('seeds.parse_url', { input }),
    resolveUrl: (reference, base) => run('seeds.resolve_url', { reference, base }),
    token: (reference, param) => run('seeds.token', { reference, param }),
    sites: () => run('seeds.sites', {}),
    site: name => run('seeds.site', { name }),
    client: options => run('seeds.client', {}, options),
    fetch: (url, options) => run('seeds.fetch', { url }, options),
    fetchDirect: (url, options) => run('seeds.fetch_direct', { url }, options),
    import: (directory, seed, input, launcher) => run('seeds.import', {
      directory, seed, kernel: bytes(input), launcher: launcher === undefined ? undefined : bytes(launcher),
    }),
  };
  const headers = {
    codec: profile => run('headers.codec', { profile: doc(profile) }),
    parse: input => run('headers.parse', { text: text(input, 'text') }),
    encode: (profile, report) => run('headers.encode', { profile: doc(profile), report: doc(report) }),
    decode: (profile, input, positionalCount, keysets) => run('headers.decode', { profile: doc(profile), headers: doc(input), positional_count: positionalCount, keysets }),
    encodeBytes: (profile, payload) => run('headers.encode_bytes', { profile: doc(profile), payload: doc(payload) }),
    decodeBytes: (profile, input) => run('headers.decode_bytes', { profile: doc(profile), headers: doc(input) }),
    encodeFinalized: (profile, input, context) => run('headers.encode_finalized', { profile: doc(profile), text: text(input, 'text'), context }),
    decodeFinalized: (profile, input) => run('headers.decode_finalized', { profile: doc(profile), headers: doc(input) }),
  };
  const payload = {
    codec: profile => run('payload.codec', { profile: doc(profile) }),
    encode: (profile, report) => run('payload.encode', { profile: doc(profile), report: doc(report) }),
    decode: (profile, input, positionalCount, keysets) => run('payload.decode', { profile: doc(profile), text: text(input, 'text'), positional_count: positionalCount, keysets }),
    encodeBytes: (profile, input, seeds) => run('payload.encode_bytes', { profile: doc(profile), bytes: bytes(input), seeds }),
    decodeBytes: (profile, input) => run('payload.decode_bytes', { profile: doc(profile), text: text(input, 'text') }),
  };
  return {
    kernel: {
      load: input => LoadedKernel.load(input),
      loadAsync: input => LoadedKernel.loadAsync(input),
      async toJSON(input) {
        return input instanceof LoadedKernel ? input.toJSON()
          : parseJSON(await run('kernel.to_json', { artifact: doc(input) }));
      },
    },
    ops: { classify },
    trace: {
      corpus,
      programWithArtifact: (input, kernel, options) => kernel instanceof LoadedKernel ? kernel.program(input, options)
        : run('trace.program', { ...source(input), artifact: doc(kernel) }, options),
    },
    naming: { corpus: {
      analyze: sources => run('naming.corpus.analyze', { sources }),
      compare: (baseline, current) => run('naming.corpus.compare', { baseline, current }),
    } },
    signals: {
      catalog: () => run('signals.catalog', {}),
      describe: name => run('signals.describe', { name }),
      compactSchema: (input, options) => run('signals.compact_schema', source(input), options),
    },
    direct: {
      CachePolicy, artifact, session, seeds, headers, payload,
      decode: (loaded, capture, options) => loaded.decode(capture, options),
      decodeAsync: (loaded, capture, options) => loaded.decodeAsync(capture, options),
      explain: (loaded, decoded, signal, options) => loaded.explain(decoded, signal, options),
      explainAsync: (loaded, decoded, signal, options) => loaded.explainAsync(decoded, signal, options),
      encode: (loaded, decoded, options) => loaded.encode(request({ decoded: doc(decoded) }, options)),
      encodeAsync: (loaded, decoded, options) => loaded.encodeAsync(request({ decoded: doc(decoded) }, options)),
      encodeSignals: (loaded, signals, options) => loaded.encode(request({ signals: doc(signals) }, options)),
      encodeSignalsAsync: (loaded, signals, options) => loaded.encodeAsync(request({ signals: doc(signals) }, options)),
      schema: loaded => loaded.schema(),
      schemaAsync: loaded => loaded.schemaAsync(),
      pack: (value, options) => run('pack', { value, options }),
      unpack: (input, options) => run('unpack', { bytes: bytes(input), options }),
      unpackCanonical: (input, options) => run('unpack_canonical', { bytes: bytes(input), options }),
    },
  };
};
