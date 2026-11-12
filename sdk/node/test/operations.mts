// Self-package imports exercise conditional ESM declarations under strict NodeNext.
import shape, {
  artifact, decode, decodeAsync, encode, encodeSignals, explain, schema,
  kernel, trace, signals, ops, naming, seeds, headers, payload, pack, unpack,
  unpackCanonical, CachePolicy, operation, type Cache, type DecodedPayload,
  type CaptureHeaders, type LoadedArtifact, type LoadedKernel, type PackValue,
  type PayloadProfile, type HeaderProfile, type HeaderReport, type Classification,
} from 'shape';

async function check(loaded: LoadedArtifact, vm: LoadedKernel, decoded: DecodedPayload,
  profile: PayloadProfile, headerProfile: HeaderProfile, report: HeaderReport,
  classifications: Classification[]): Promise<void> {
  const cache: Cache = CachePolicy.Directory('cache');
  const document = loaded.toJSON();
  const same: LoadedArtifact = artifact.fromArtifact(document);
  const compiled: LoadedArtifact = await artifact.compile('program', { session: '{}', siteId: 'site', cache });
  const result: DecodedPayload = decode(loaded, { siteId: 'site', headers: {} });
  const next: DecodedPayload = await decodeAsync(loaded, '{}');
  const captured: CaptureHeaders = encode(loaded, decoded);
  encodeSignals(loaded, decoded.positionalSignals);
  explain(loaded, decoded, 'DeviceMemory');
  schema(loaded);
  const fromDefault: LoadedArtifact = shape.artifact.load('{}');
  await trace.program('source', { cache, jobs: null });
  await trace.programWithArtifact('source', vm, { cache: CachePolicy.Disabled });
  const thread = await vm.thread({ pc: 0, state: 1, inputs: { arguments: [{ kind: 'object', identity: 18446744073709551615n }] } });
  const diagnostics: string[] = thread.self_check;
  await trace.corpus.source('source', { cache: CachePolicy.Environment, limit: 10 });
  await trace.corpus.reachable(vm, { jobs: 2 });
  await trace.corpus.walkSeed('seed.js', { cache });
  await trace.corpus.fingerprint(vm.toJSON(), { pc: 0, state: 1 });
  await trace.corpus.compare({ seed: { entry: {} } }, {});
  await trace.corpus.compareInline({}, {});
  await kernel.toJSON(vm);
  const described = await signals.describe('DeviceMemory');
  const title: string = described.signal;
  const bits: PackValue = { kind: 'number', value: 'ffffffffffffffff' };
  const encoded: number[] = await pack(bits);
  const canonical: PackValue = await unpackCanonical(encoded);
  const stable: boolean = (await unpack(encoded)).canonical;
  await ops.classify.template('source', 1);
  await ops.classify.report('source');
  await ops.classify.reportDocument(Buffer.from('{}'));
  await ops.classify.templateDocument('return 1;', Buffer.from('{}'));
  await ops.classify.seeds(classifications);
  await naming.corpus.analyze([{ name: 'a', source: 'function f(){}' }]);
  const analyses = await naming.corpus.analyze([{ name: 'a', source: 'function f(){}' }]);
  const count: number | undefined = analyses[0]?.rename.applied;
  // @ts-expect-error native rename projection has counts/hashes, no report property
  const invented = analyses[0]?.rename.report;
  const differences = await naming.corpus.compare([], []);
  void [count, invented];
  const urls: string = await seeds.resolveUrl('kernel.js', 'https://example.test/');
  const config = await seeds.client({ timeoutMs: 1n });
  await seeds.fetch('https://example.test/', { client: config });
  await seeds.fetchDirect('https://example.test/', { headers: [['x-test', 'synthetic']] });
  await seeds.import('store', 'synthetic', new Uint8Array());
  await seeds.parseLauncher(new Uint8Array());
  const framed = await headers.encode(headerProfile, report);
  await headers.decode(headerProfile, framed, 0, []);
  const finalized = await headers.decodeFinalized(headerProfile, framed);
  await headers.encodeFinalized(headerProfile, finalized.text, finalized.context);
  const bytes = await headers.decodeBytes(headerProfile, framed);
  await headers.encodeBytes(headerProfile, bytes);
  const text: string = await payload.encodeBytes(profile, new Uint8Array(), [1, 2]);
  await payload.decode(profile, text, 0);
  const v2: 2 = (await operation('version')).contract_version;
  void [same, compiled, result, next, captured, fromDefault, diagnostics, title, canonical, stable, differences, urls, v2];

  // @ts-expect-error cache is an explicit tagged policy
  await trace.program('source', { cache: true });
  // @ts-expect-error directory policy requires its directory
  await trace.program('source', { cache: { mode: 'directory' } });
  // @ts-expect-error no implicit artifact reload in direct decode
  decode(document, '{}');
  // @ts-expect-error tagged pack number requires hexadecimal bits, not a rounded number
  await pack({ kind: 'number', value: 18446744073709551615 });
  // @ts-expect-error payload seeds are exactly two values
  await payload.encodeBytes(profile, [], [1, 2, 3]);
  // @ts-expect-error operation result is concrete; signal name cannot be numeric
  const badSignal: number = described.signal;
  // @ts-expect-error thread code may be null
  const code: string = thread.code;
  void [badSignal, code];
}
void check;
