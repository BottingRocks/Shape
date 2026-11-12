// Type definitions for the `shape` Node binding (hand-written; mirrors
// src/sdk/mod.rs and sdk/c/include/shape.h).
//
// Integers outside Number.MAX_SAFE_INTEGER in any result parse as `bigint`
// (artifacts and decoded payloads carry 64-bit values), so numeric fields of
// opaque documents are `number | bigint`.

/// <reference types="node" />
export * from './domain.js';
import type { PackValue, PackDecoded, SerializerOptions, PayloadProfile, HeaderProfile, Payload, PayloadReport, HeaderReport, Headers, HeaderContext, Classification, ClassifiedOp, KindSummary, SeedScript, Launcher, KernelSummary, Site, ClientConfig, ClientOptions, FetchOptions, FetchedSeed, StoreOutcome, ThreadSnapshot, ReportEntry, CipherProfile, U32Pair } from './domain.js';

// ---- JSON --------------------------------------------------------------------

export type JsonPrimitive = string | number | bigint | boolean | null;
export type JsonValue = JsonPrimitive | JsonValue[] | { [key: string]: JsonValue };
export type JsonObject = { [key: string]: JsonValue };

/**
 * A JSON document argument: a parsed object, or its JSON text as a string,
 * Buffer, Uint8Array or ArrayBuffer (text is forwarded unparsed, keeping it exact).
 */
export type Document<T = JsonObject> = T | string | Buffer | Uint8Array | ArrayBuffer;

/** Source text: a string, or UTF-8 bytes. */
export type Source = string | Buffer | Uint8Array;
/** Exact serialized engine document; validated by the native domain parser. */
export type SerializedDocument = Source | ArrayBuffer;

/**
 * Parse JSON keeping integers beyond 2^53 exact (as bigint), including a
 * document that is a bare integer, on all supported Node versions.
 */
export function parseJSON(text: string | Buffer | Uint8Array | ArrayBuffer): JsonValue;
/** Stringify JSON writing bigint values back as exact integer literals. */
export function stringifyJSON(value: object | JsonValue): string;
export function stringifyJSON(value: object | JsonValue | undefined): string | undefined;

// ---- errors ------------------------------------------------------------------

export type ShapeErrorCode = 'invalid_request' | 'unknown_command' | 'failed' | 'panic';

/** Every failed call throws (sync) or rejects (async) with a ShapeError. */
export class ShapeError extends Error {
  constructor(code: ShapeErrorCode | string, message: string);
  readonly name: 'ShapeError';
  /** Stable machine-readable code. */
  code: ShapeErrorCode;
}

// ---- shared shapes -----------------------------------------------------------

/** Per-transaction session material (as `shape decode --session`). */
export interface Session {
  transactionToken: string;
  alphabet: string;
  shuffleKey: number[];
  keys: [number, number, number, number, number, number, number, number];
  nonce?: string;
}

/** A captured request: the site id and its signal headers. */
export interface CaptureHeaders {
  siteId: string;
  headers: Record<string, string>;
}

/** A source-free kernel artifact (`deob --report` / `report`). */
export type KernelArtifact = JsonObject;

/** A reusable decode artifact (`artifact.compile` / `shape decode --save-artifact`). */
export interface DecodeArtifact {
  /** Persisted source-format opt-in; absent in legacy/default artifacts. */
  direct?: boolean;
  format: string;
  siteId: string;
  layout: SeedLayout;
  schema: JsonObject;
  floatByteOrder?: FloatByteOrder;
}

export type FloatByteOrder = 'little' | 'big';

/** `"pc/state"` positions forced fake (with the branch taken) or real. */
export interface BranchOverrides {
  fake?: Record<string, boolean>;
  real?: string[];
}

export type InputKind = 'raw' | 'deobfuscated';

// ---- results -----------------------------------------------------------------

export interface VersionInfo {
  version: string;
  commands: string[];
}

export interface DeobResult {
  code: string;
  /** Present with `{ artifact: true }`. */
  artifact?: KernelArtifact;
}

export interface TraceProgramResult {
  code: string;
  /** True when no function stopped. */
  complete: boolean;
  traced: number;
  stopped: { function: string; reason: string }[];
  kernel_globals: string[];
}

export interface TraceThreadResult {
  /** Null when the walk stopped and partial output was not requested. */
  code: string | null;
  complete: boolean;
  stop: string;
  steps: number;
  handlers_traced: number;
  undeclared: string[];
  /** Self-check diagnostics; empty when none were reported. */
  self_check: string[];
  source_hash: string;
}

export interface CleanupThreadResult {
  code: string;
  cleanup: {
    inlined_bindings: number;
    inlined_snapshots: number;
    rounds: number;
    inlined_helpers: number;
    removed_helpers: number;
    remaining_helpers: number;
    property_writes: number;
    arrays: number;
    methods: number;
    loops: number;
    flattened_blocks: number;
  };
}

export interface NamedSourceResult {
  source: string;
  report: JsonObject;
}

export interface NamingInventory {
  function_index: JsonObject;
  role_graph: JsonObject;
}

export interface DecodeMetadata {
  originalReport: HeaderReport & JsonObject;
  /** Empty unless decoding requested `{ metadata: true }`. */
  signals: Record<string, NamedSignal> & JsonObject;
  decoded: number;
  missing: number;
  exactReencode: boolean;
  floatByteOrder: FloatByteOrder | string;
}

/** Decoded headers: named signals and retained decode metadata. */
export interface DecodedPayload {
  siteId: string;
  errors: PackValue[];
  /** Signal name → field name → value. */
  positionalSignals: Record<string, Record<string, JsonValue>>;
  signals: PackValue[];
  keyedSignals: [number, ReportEntry | null][];
  seeds: U32Pair;
  keysets: string[][];
  /** Always present, including when decoding with `{ metadata: false }`. */
  metadata: DecodeMetadata;
}

/** One signal's full metadata (`explain`). */
export interface NamedSignal {
  status: 'decoded' | string;
  fields: JsonValue;
  metadata: JsonValue;
}

/** An artifact's compact schema (`schema` / `artifact.compact_schema`). */
export interface CompactSchema {
  format: JsonValue;
  encoding: JsonValue;
  seedId: JsonValue;
  signals: JsonValue;
  keysets: JsonValue;
  reportRoutes: JsonValue;
  unroutedSignals: JsonValue;
}

// ---- options -----------------------------------------------------------------
// Option keys are camelCase and are sent as snake_case request fields.

export interface DeobOptions {
  /** Also compile the source-free kernel artifact. */
  artifact?: boolean;
}

export interface ReportOptions {
  inputKind?: InputKind;
}

export interface KernelShowOptions {
  inputKind?: InputKind;
  /** Operation names to select. */
  operations?: string[];
}

export interface TraceProgramOptions {
  /** Seed file name used in reports (default "seed.js"). */
  name?: string;
  /** Step budget (default 1,000,000). */
  steps?: number;
  /** Parallel per-thread cleanup (default true). */
  threadCleanup?: boolean;
  /** Skip the full mechanical pipeline for diagnostics (default false). Semantic naming is optional. */
  noCleanup?: boolean;
  checkReuse?: boolean;
  overrides?: BranchOverrides;
  /** Worker threads for tracing and thread cleanup; omitted/null uses available cores. */
  jobs?: number | null;
  /** Explicit per-call kernel cache; disabled by default. */
  cache?: Cache;
  /** @deprecated Use cache: CachePolicy.Directory(path). Legacy generic calls accept report_dir. */
  reportDir?: string;
  /** Default true. */
  quiet?: boolean;
}

export interface TraceThreadOptions {
  /** A kernel artifact instead of `source` (pass `null` as source). */
  artifact?: Document<KernelArtifact>;
  pc: number;
  state: number;
  descriptor?: number;
  initialSlots?: Document<number[]>;
  inputs?: Document<ThreadSnapshot>;
  steps?: number;
  selfCheck?: boolean;
  /** Default true. */
  outline?: boolean;
  reverseSides?: boolean;
  partial?: boolean;
  overrides?: BranchOverrides;
}

export interface NamingThreadsOptions {
  rename?: boolean;
  labels?: Document;
}

export interface NamingCollectorsOptions {
  rename?: boolean;
}

export interface SignalsSchemaOptions {
  /** Seed name; its file stem is reported as `seed`. */
  name?: string;
  layoutsOnly?: boolean;
  /** Default true. */
  finderRoles?: boolean;
}

/** Exactly one of `session` or `init`. */
export type PinOptions =
  | { session: Document<Session>; init?: undefined }
  | { init: string; session?: undefined };

export interface CompileArtifactOptions {
  cache?: Cache;
  session: Document<Session>;
  siteId: string;
  /** Default false. Source-format decoding without lifecycle certification; persisted in artifacts. */
  direct?: boolean;
  floatByteOrder?: FloatByteOrder;
  /** "program" (traced program, default) or "raw" (trace the seed first). */
  inputKind?: 'program' | 'raw';
  /** Seed name when `inputKind` is "raw". */
  name?: string;
}

export interface DecodeOptions {
  session?: Document<Session>;
  verify?: boolean;
  /** Populate `metadata.signals` with per-signal details (otherwise empty). */
  metadata?: boolean;
}

export interface ExplainOptions {
  session?: Document<Session>;
  verify?: boolean;
}

export type EncodeInput =
  | { signals: Document<DecodedPayload['positionalSignals']>; decoded?: undefined; session?: Document<Session> }
  | { decoded: Document<DecodedPayload>; signals?: undefined; session?: Document<Session> };

export type ShapeCommand =
  | 'version'
  | 'deob'
  | 'report'
  | 'kernel.show'
  | 'ops.list'
  | 'ops.show'
  | 'trace.program'
  | 'trace.thread'
  | 'cleanup.thread'
  | 'naming.threads'
  | 'naming.collectors'
  | 'naming.plan'
  | 'naming.inventory'
  | 'signals.schema'
  | 'signals.show'
  | 'signals.inputs'
  | 'signals.contracts'
  | 'pin'
  | 'session.from_initializer'
  | 'artifact.compile'
  | 'artifact.compact_schema';

// ---- functions ---------------------------------------------------------------

/** The SDK version, e.g. "0.1.0". */
export function version(): string;
/** Commands supported by the loaded native library. */
export function commands(): Promise<string[]>;
export function commandsSync(): string[];

/**
 * Run any command on the libuv thread pool. `request` uses the snake_case
 * field names of src/sdk/mod.rs, as an object or JSON text.
 */
export function call<C extends keyof LegacyResults>(command: C, request?: Document | null): Promise<LegacyResults[C]>;
export function call(command: ShapeCommand | (string & {}), request?: Document | null): Promise<JsonValue>;
/** Run any command synchronously on the calling thread (blocks the event loop). */
export function callSync<C extends keyof LegacyResults>(command: C, request?: Document | null): LegacyResults[C];
export function callSync(command: ShapeCommand | (string & {}), request?: Document | null): JsonValue;

export function deob(source: Source, options: DeobOptions & { artifact: true }): Promise<Required<DeobResult>>;
export function deob(source: Source, options?: DeobOptions): Promise<DeobResult>;
/** Compile the source-free kernel artifact. */
export function report(source: Source, options?: ReportOptions): Promise<KernelArtifact>;

export const kernel: {
  show(source: Source, options?: KernelShowOptions): Promise<JsonObject>;
  load(artifact: Document<KernelArtifact>): LoadedKernel;
  loadAsync(artifact: Document<KernelArtifact>): Promise<LoadedKernel>;
  toJSON(artifact: LoadedKernel | Document<KernelArtifact>): Promise<KernelArtifact>;
};

export const ops: {
  classify: {
    source(source: Source): Promise<Classification>;
    report(source: Source): Promise<Classification>;
    reportDocument(report: SerializedDocument): Promise<Classification>;
    template(source: Source, handlerIndex: number): Promise<ClassifiedOp>;
    templateDocument(handlerSource: Source, template: SerializedDocument): Promise<ClassifiedOp>;
    seeds(seeds: Classification[]): Promise<KindSummary[]>;
  };
  list(source: Source): Promise<JsonObject>;
  show(source: Source, op: number): Promise<JsonObject>;
};

export const trace: {
  corpus: TraceCorpus;
  programWithArtifact(source: Source, artifact: LoadedKernel | Document<KernelArtifact>, options?: TraceProgramOptions): Promise<TraceProgramResult>;
  program(source: Source, options?: TraceProgramOptions): Promise<TraceProgramResult>;
  thread(source: Source | null, options: TraceThreadOptions): Promise<TraceThreadResult>;
};

export const cleanup: {
  thread(source: Source): Promise<CleanupThreadResult>;
};

export const naming: {
  corpus: {
    analyze(sources: NamedSource[]): Promise<NamingAnalysis[]>;
    compare(baseline: NamedSource[], current: NamedSource[]): Promise<NamingDifference[]>;
  };
  threads(source: Source, options?: NamingThreadsOptions): Promise<NamedSourceResult>;
  collectors(source: Source, options?: NamingCollectorsOptions): Promise<NamedSourceResult>;
  plan(source: Source): Promise<JsonObject>;
  inventory(source: Source): Promise<NamingInventory>;
};

export const signals: {
  catalog(): Promise<CatalogCard[]>;
  describe(name: string): Promise<CatalogCard>;
  compactSchema(source: Source, options?: { direct?: boolean }): Promise<CompactSchema>;
  schema(source: Source, options?: SignalsSchemaOptions): Promise<JsonObject>;
  show(source: Source): Promise<JsonObject>;
  inputs(source: Source): Promise<JsonObject>;
  contracts(source: Source): Promise<JsonObject>;
};

/** Pin a traced program's payload layout to one session. */
export function pin(source: Source, options: PinOptions): Promise<SeedLayout>;
/** Derive a session from an initializer string. */
export function sessionFromInitializer(init: Source): Promise<Session>;
/** Compile a reusable decode artifact. */
export function compileArtifact(source: Source, options: CompileArtifactOptions): Promise<DecodeArtifact>;
/** The compact schema of a decode artifact. */
export function compactSchema(artifact: Document<DecodeArtifact>): Promise<CompactSchema>;

// ---- generator ---------------------------------------------------------------

/**
 * One loaded, validated decode artifact for repeated decode / explain / encode.
 * The *Async forms run on the libuv pool. Release it
 * with close() (or `using`); the GC releases it otherwise.
 */
export class Generator {
  protected constructor(handle: never);
  static load(artifact: Document<DecodeArtifact>): Generator;
  static loadAsync(artifact: Document<DecodeArtifact>): Promise<Generator>;

  /** True after close(). */
  readonly closed: boolean;

  /** Generic escape hatch. Request field names are snake_case. */
  call<M extends keyof LoadedResults>(method: M, request?: Document | null): LoadedResults[M];
  call(method: string, request?: Document | null): JsonValue;
  callAsync<M extends keyof LoadedResults>(method: M, request?: Document | null): Promise<LoadedResults[M]>;
  callAsync(method: string, request?: Document | null): Promise<JsonValue>;

  /** Headers → named signals. */
  decode(capture: Document<CaptureHeaders>, options?: DecodeOptions): DecodedPayload;
  decodeAsync(capture: Document<CaptureHeaders>, options?: DecodeOptions): Promise<DecodedPayload>;

  /** One signal's full metadata from a decoded payload. */
  explain(decoded: Document<DecodedPayload>, signal: string, options?: ExplainOptions): NamedSignal;
  explainAsync(decoded: Document<DecodedPayload>, signal: string, options?: ExplainOptions): Promise<NamedSignal>;

  /** Named signals → headers. */
  encode(input: EncodeInput): CaptureHeaders;
  encodeAsync(input: EncodeInput): Promise<CaptureHeaders>;

  /** The artifact's compact schema. */
  schema(): CompactSchema;
  schemaAsync(): Promise<CompactSchema>;

  /** Explicit export, detached from the immutable native state. */
  toJSON(): DecodeArtifact;
  toJSONAsync(): Promise<DecodeArtifact>;

  /** Release the artifact now; idempotent. In-flight async calls finish first. */
  close(): void;
  [Symbol.dispose](): void;
}

declare const shape: {
  LoadedArtifact: typeof LoadedArtifact;
  LoadedKernel: typeof LoadedKernel;
  operation: typeof operation;
  operationSync: typeof operationSync;
  CachePolicy: typeof CachePolicy;
  artifact: typeof artifact;
  session: typeof session;
  seeds: typeof seeds;
  headers: typeof headers;
  payload: typeof payload;
  decode: typeof decode;
  decodeAsync: typeof decodeAsync;
  explain: typeof explain;
  explainAsync: typeof explainAsync;
  encode: typeof encode;
  encodeAsync: typeof encodeAsync;
  encodeSignals: typeof encodeSignals;
  encodeSignalsAsync: typeof encodeSignalsAsync;
  schema: typeof schema;
  schemaAsync: typeof schemaAsync;
  pack: typeof pack;
  unpack: typeof unpack;
  unpackCanonical: typeof unpackCanonical;
  ShapeError: typeof ShapeError;
  Generator: typeof Generator;
  version: typeof version;
  commands: typeof commands;
  commandsSync: typeof commandsSync;
  call: typeof call;
  callSync: typeof callSync;
  deob: typeof deob;
  report: typeof report;
  kernel: typeof kernel;
  ops: typeof ops;
  trace: typeof trace;
  cleanup: typeof cleanup;
  naming: typeof naming;
  signals: typeof signals;
  pin: typeof pin;
  sessionFromInitializer: typeof sessionFromInitializer;
  compileArtifact: typeof compileArtifact;
  compactSchema: typeof compactSchema;
  parseJSON: typeof parseJSON;
  stringifyJSON: typeof stringifyJSON;
};
export default shape;

export type Cache = { mode: 'disabled' } | { mode: 'environment' } | { mode: 'directory'; directory: string };
export const CachePolicy: {
  readonly Disabled: Readonly<{ mode: 'disabled' }>;
  readonly Environment: Readonly<{ mode: 'environment' }>;
  Directory(directory: string): Readonly<{ mode: 'directory'; directory: string }>;
};
export type Bytes = Uint8Array | ArrayBuffer | number[];

/** Parse/validate once; repeated operations reuse immutable native state. */
export class LoadedArtifact extends Generator {
  private constructor(handle: never);
  static load(artifact: Document<DecodeArtifact>): LoadedArtifact;
  static loadAsync(artifact: Document<DecodeArtifact>): Promise<LoadedArtifact>;
}
export const artifact: {
  load(input: Document<DecodeArtifact>): LoadedArtifact;
  loadAsync(input: Document<DecodeArtifact>): Promise<LoadedArtifact>;
  fromArtifact(input: DecodeArtifact): LoadedArtifact;
  compile(source: Source, options: CompileArtifactOptions): Promise<LoadedArtifact>;
  compactSchema(input: Document<DecodeArtifact>): Promise<CompactSchema>;
};
export const session: { fromInitializer(init: Source): Promise<Session> };
export function decode(artifact: LoadedArtifact, capture: Document<CaptureHeaders>, options?: DecodeOptions): DecodedPayload;
export function decodeAsync(artifact: LoadedArtifact, capture: Document<CaptureHeaders>, options?: DecodeOptions): Promise<DecodedPayload>;
export function explain(artifact: LoadedArtifact, decoded: Document<DecodedPayload>, signal: string, options?: ExplainOptions): NamedSignal;
export function explainAsync(artifact: LoadedArtifact, decoded: Document<DecodedPayload>, signal: string, options?: ExplainOptions): Promise<NamedSignal>;
export function encode(artifact: LoadedArtifact, decoded: Document<DecodedPayload>, options?: { session?: Document<Session> }): CaptureHeaders;
export function encodeAsync(artifact: LoadedArtifact, decoded: Document<DecodedPayload>, options?: { session?: Document<Session> }): Promise<CaptureHeaders>;
export function encodeSignals(artifact: LoadedArtifact, signals: Document<DecodedPayload['positionalSignals']>, options?: { session?: Document<Session> }): CaptureHeaders;
export function encodeSignalsAsync(artifact: LoadedArtifact, signals: Document<DecodedPayload['positionalSignals']>, options?: { session?: Document<Session> }): Promise<CaptureHeaders>;
export function schema(artifact: LoadedArtifact): CompactSchema;
export function schemaAsync(artifact: LoadedArtifact): Promise<CompactSchema>;

export interface CorpusOptions {
  steps?: number; jobs?: number; limit?: number | null; cleanup?: boolean;
}
export interface CorpusSourceOptions extends CorpusOptions { name?: string; cache?: Cache }
export interface WalkSeedOptions extends CorpusSourceOptions { selfCheck?: boolean; audit?: boolean; branches?: string }
export interface TraceFunction { descriptor: number | null; pc: number; state: number }
export interface Fingerprint { print: JsonObject; code: string | null }
export interface Fingerprinted extends Fingerprint { function: TraceFunction; created: TraceFunction[] }
export type CorpusDocument = Record<string, Record<string, JsonObject>>;
export interface CorpusComparison { regressions: string[]; changes: string[] }
export interface FingerprintOptions { pc: number; state: number; descriptor?: number; steps?: number; parameters?: string[]; cleanup?: boolean }
export interface TraceCorpus {
  load(source: Source, options?: { name?: string; cache?: Cache }): Promise<KernelArtifact>;
  source(source: Source, options?: CorpusSourceOptions): Promise<Fingerprinted[]>;
  reachable(artifact: LoadedKernel | Document<KernelArtifact>, options?: CorpusOptions): Promise<Fingerprinted[]>;
  walkSeed(seed: string, options?: WalkSeedOptions): Promise<Record<string, JsonObject>>;
  fingerprint(artifact: Document<KernelArtifact>, options: FingerprintOptions): Promise<Fingerprint>;
  compare(baseline: Document<CorpusDocument>, current: Document<CorpusDocument>): Promise<CorpusComparison>;
  compareInline(outlined: Document<JsonObject>, inline: Document<JsonObject>): Promise<string[]>;
}
export class LoadedKernel {
  private constructor(handle: never);
  static load(input: Document<KernelArtifact>): LoadedKernel;
  static loadAsync(input: Document<KernelArtifact>): Promise<LoadedKernel>;
  readonly closed: boolean;
  program(source: Source, options?: TraceProgramOptions): Promise<TraceProgramResult>;
  thread(options: Omit<TraceThreadOptions, 'artifact'>): Promise<TraceThreadResult>;
  reachable(options?: CorpusOptions): Promise<Fingerprinted[]>;
  toJSON(): KernelArtifact;
  toJSONAsync(): Promise<KernelArtifact>;
  close(): void;
  [Symbol.dispose](): void;
}
export interface NamedSource { name: string; source: string }
export interface NamingAnalysis {
  name: string; plan: JsonObject;
  rename: { source: string; source_sha256: string; transformed_sha256: string; applied: number; skipped: number; ambiguous: number; plan: JsonObject };
}
export interface NamingDifference { name: string; baseline_present: boolean; current_present: boolean; missing_roles: string[]; added_roles: string[]; name_divergences: string[] }
export interface CatalogCard {
  signal: string; summary: string; why: string; status: string; provenance: string;
  logical_fields: { path: string; types: string[]; description: string }[];
  data: JsonValue; examples: { name: string; provenance: string; note: string; data: JsonValue; special_values: JsonValue[] }[];
  size: { bytes: number | null; kind: string; basis: string };
}
export const seeds: {
  parse(bytes: Bytes): Promise<SeedScript>;
  parseLauncher(bytes: Bytes): Promise<Launcher>;
  parseKernel(bytes: Bytes): Promise<KernelSummary>;
  parseUrl(input: string): Promise<string>;
  resolveUrl(reference: string, base?: string): Promise<string>;
  token(reference: string, param?: string): Promise<string>;
  sites(): Promise<Site[]>;
  site(name: string): Promise<Site>;
  client(options?: ClientOptions): Promise<ClientConfig>;
  fetch(url: string, options?: FetchOptions): Promise<FetchedSeed>;
  fetchDirect(url: string, options?: FetchOptions): Promise<FetchedSeed>;
  import(directory: string, seed: string, kernel: Bytes, launcher?: Bytes): Promise<StoreOutcome>;
};
export const headers: {
  codec(profile: Document<HeaderProfile>): Promise<HeaderProfile>;
  parse(text: Source): Promise<Headers>;
  encode(profile: Document<HeaderProfile>, report: Document<HeaderReport>): Promise<Headers>;
  decode(profile: Document<HeaderProfile>, headers: Document<Headers>, positionalCount: number, keysets?: string[][]): Promise<HeaderReport>;
  encodeBytes(profile: Document<HeaderProfile>, payload: Document<Payload>): Promise<Headers>;
  decodeBytes(profile: Document<HeaderProfile>, headers: Document<Headers>): Promise<Payload>;
  encodeFinalized(profile: Document<HeaderProfile>, text: Source, context?: HeaderContext): Promise<Headers>;
  decodeFinalized(profile: Document<HeaderProfile>, headers: Document<Headers>): Promise<{ text: string; context: HeaderContext }>;
};
export const payload: {
  codec(profile: Document<PayloadProfile>): Promise<PayloadProfile>;
  encode(profile: Document<PayloadProfile>, report: Document<PayloadReport>): Promise<string>;
  decode(profile: Document<PayloadProfile>, text: Source, positionalCount: number, keysets?: string[][]): Promise<PayloadReport>;
  encodeBytes(profile: Document<PayloadProfile>, bytes: Bytes, seeds: U32Pair): Promise<string>;
  decodeBytes(profile: Document<PayloadProfile>, text: Source): Promise<Payload>;
};
export function pack(value: PackValue, options?: SerializerOptions): Promise<number[]>;
export function unpack(bytes: Bytes, options?: SerializerOptions): Promise<PackDecoded>;
export function unpackCanonical(bytes: Bytes, options?: SerializerOptions): Promise<PackValue>;
/** Advanced v2 JSON dispatch. Prefer the typed named operations above. */
export function operation(command: 'version', request?: Document | null): Promise<VersionInfo & { contract_version: 2 }>;
export function operation(command: string, request?: Document | null): Promise<JsonValue>;
export function operationSync(command: 'version', request?: Document | null): VersionInfo & { contract_version: 2 };
export function operationSync(command: string, request?: Document | null): JsonValue;

export interface SourceIdentity { sha256: string; bytes: number }
export interface SeedLayout {
  format: string; source: SourceIdentity; session: Session; cipher: CipherProfile;
  header: { name: string; prefix: string; chunkSize?: number; checksum: string; b: boolean; f: boolean; c: string; d: string; z: string };
  keysets: string[][];
  report: {
    errors: null[];
    positionalSignals: { slot: number; descriptor: number; ctor: boolean; priority: boolean }[];
    signals: { descriptor: number; name: string }[];
    keyedSignals: { descriptor: number; name: number }[];
  };
}

/** Typed legacy results; generic requests retain the original wire contract. */
export interface LegacyResults {
  version: VersionInfo;
  deob: DeobResult;
  report: KernelArtifact;
  'kernel.show': JsonObject;
  'ops.list': JsonObject;
  'ops.show': JsonObject;
  'trace.program': TraceProgramResult;
  'trace.thread': TraceThreadResult;
  'cleanup.thread': CleanupThreadResult;
  'naming.threads': NamedSourceResult;
  'naming.collectors': NamedSourceResult;
  'naming.plan': JsonObject;
  'naming.inventory': NamingInventory;
  'signals.schema': JsonObject;
  'signals.show': JsonObject;
  'signals.inputs': JsonObject;
  'signals.contracts': JsonObject;
  pin: SeedLayout;
  'session.from_initializer': Session;
  'artifact.compile': DecodeArtifact;
  'artifact.compact_schema': CompactSchema;
}
export interface LoadedResults {
  decode: DecodedPayload; encode: CaptureHeaders; explain: NamedSignal;
  schema: CompactSchema; artifact: DecodeArtifact;
}
