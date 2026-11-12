// Type-level check of index.d.ts (not run by node --test):
//   tsc --noEmit --strict --module nodenext --moduleResolution nodenext --target es2022 sdk/node/test/types.ts
import shape, {
  Generator,
  ShapeError,
  call,
  cleanup,
  compileArtifact,
  deob,
  trace,
  type CleanupThreadResult,
  type DecodedPayload,
  type TraceProgramResult,
} from '../index.js';

async function main(): Promise<void> {
  const v: string = shape.version();
  const info = await call('version');
  const commands: string[] = info.commands;
  const discovered: string[] = await shape.commands();
  const synchronous: string[] = shape.commandsSync();
  const json: string | undefined = shape.stringifyJSON(undefined);
  const exact: string = shape.stringifyJSON({ bits: 18446744073709551615n });
  shape.parseJSON(new ArrayBuffer(0));

  const cleaned: CleanupThreadResult = await cleanup.thread('function f(a) { let $SSA_1 = a; return $SSA_1; }');
  const inlined: number = cleaned.cleanup.inlined_bindings;

  const traced: TraceProgramResult = await trace.program('...', {
    name: 'seed.js',
    steps: 1_000_000,
    threadCleanup: true,
    overrides: { fake: { '12/3': true }, real: ['40/1'] },
    jobs: 4,
  });
  const stops: string[] = traced.stopped.map((s) => s.reason);

  const thread = await trace.thread(null, { artifact: '{}', pc: 1, state: 0, outline: false });
  const done: boolean = thread.complete;

  const withArtifact = await deob('...', { artifact: true });
  const kernelArtifact = withArtifact.artifact;

  const artifact = await compileArtifact('...', { session: '{}', siteId: 'site', floatByteOrder: 'little' });
  const persistedDirect: boolean | undefined = artifact.direct;
  await compileArtifact('...', { session: '{}', siteId: 'example.test', direct: true });
  await compileArtifact('...', { session: '{}', siteId: 'example.test', direct: false });
  // @ts-expect-error direct requires a boolean
  await compileArtifact('...', { session: '{}', siteId: 'example.test', direct: 'true' });
  // @ts-expect-error null is not an opt-in
  await compileArtifact('...', { session: '{}', siteId: 'example.test', direct: null });
  const generator = Generator.load(artifact);
  try {
    const decoded: DecodedPayload = generator.decode({ siteId: 'site', headers: { a: '' } }, { metadata: true });
    const count: number | undefined = decoded.metadata?.decoded;
    const memory = decoded.positionalSignals['DeviceMemory']?.['deviceMemory'];
    const explained = await generator.explainAsync(decoded, 'DeviceMemory', { verify: true });
    const status: string = explained.status;
    generator.encode({ signals: decoded.positionalSignals });
    generator.encode({ decoded });
    const schema = generator.schema();
    const genericSchema = generator.call('schema', new ArrayBuffer(0));
    const asyncSchema = await generator.callAsync('schema');
    const arbitrary = generator.call('new.method', { option: true });
    void [genericSchema, asyncSchema, arbitrary];
    void [count, memory, status, schema];
  } catch (error) {
    if (error instanceof ShapeError) {
      const code: 'invalid_request' | 'unknown_command' | 'failed' | 'panic' = error.code;
      void code;
    }
  } finally {
    generator.close();
  }

  const loaded = await Generator.loadAsync(Buffer.from('{}'));
  const closed: boolean = loaded.closed;

  // @ts-expect-error pin needs session or init
  await shape.pin('...', {});
  // @ts-expect-error pc and state are required
  await trace.thread('...', {});

  void [v, commands, discovered, synchronous, json, exact, inlined, stops, done, kernelArtifact, closed];
}

void main;
