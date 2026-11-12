/** Typed engine documents. Field spelling matches serialized Rust documents. */
export type U32Pair = [number, number];
export type U32Four = [number, number, number, number];
export type U32Eight = [number, number, number, number, number, number, number, number];

/** IEEE-754 bits as exactly sixteen hexadecimal digits, never a JSON float. */
export type NumberBits = string;
export type PackValue =
  | { kind: 'undefined' | 'null' | 'missing_sentinel' }
  | { kind: 'bool'; value: boolean }
  | { kind: 'number'; value: NumberBits }
  | { kind: 'string'; value: string }
  | { kind: 'array'; value: PackValue[] }
  | { kind: 'object'; value: [string, PackValue][] }
  | { kind: 'binary'; value: number[] }
  | { kind: 'date'; value: number | bigint }
  | { kind: 'extension'; value: { id: number; payload: PackValue } };
export interface PackLimits {
  max_bytes?: number;
  max_entries?: number;
  max_depth?: number;
}
export interface SerializerOptions {
  keysets_to_omit?: string[][];
  depth_bound?: number | null;
  missing_sentinel_extension?: boolean;
  opaque_extensions?: boolean;
  limits?: PackLimits;
}
export interface PackDecoded {
  value: PackValue;
  original_bytes: number[];
  canonical: boolean;
}
export interface CipherProfile {
  ivs: U32Four;
  counter: number | bigint;
  double_rounds: number;
}
export interface PayloadProfile {
  format: string;
  cipher: CipherProfile;
  alphabet: string;
  keys: U32Eight;
}
export interface HeaderContext {
  b?: boolean;
  f?: boolean;
  c?: string;
  d?: string;
  z?: string;
}
export interface HeaderProfile extends PayloadProfile {
  header_name: string;
  transaction_token?: string;
  chunk_size?: number;
  header_context?: HeaderContext;
  positional_count?: number;
  keysets?: string[][];
}
export interface PositionalEntry { encoded: number[]; extra: number[] }
export interface KeyedEntry extends PositionalEntry { name: number }
export interface Container {
  errors: number[];
  positional: (PositionalEntry | null)[];
  signals: number[];
  keyed: KeyedEntry[];
}
export interface Payload {
  seeds: U32Pair;
  data: { kind: 'raw'; bytes: number[] } | { kind: 'container'; container: Container };
  header_context?: HeaderContext;
}
export type ReportEntry = [PackValue[], number[]];
export interface ChannelReport {
  errors: PackValue[];
  positionalSignals: (ReportEntry | null)[];
  signals: PackValue[];
  keyedSignals: [number, ReportEntry | null][];
}
export interface PayloadReport extends ChannelReport { seeds: U32Pair; keysets: string[][] }
export interface HeaderReport extends PayloadReport { header_context?: HeaderContext }
export type Headers = Record<string, string>;

export type OpNature = 'javascript' | 'vm' | 'fused' | 'unrecognized';
export interface ClassifiedOp {
  handler_index: number; kind: string; nature: OpNature; pops: number; pushes: number; source: string;
}
export interface Classification { variant: 'javascript-vm' | 'webassembly-vm'; ops: ClassifiedOp[] }
export interface KindSummary {
  kind: string; nature: OpNature;
  scope: 'common' | 'javascript-vm-only' | 'webassembly-vm-only';
  handlers: [number, number][];
}
export interface KernelSummary { bytes: number; sha256: string; statements: number; functions: number }
export interface Launcher { kernel_ref: string }
export type SeedScript = { kind: 'launcher'; launcher: Launcher } | { kind: 'kernel'; kernel: KernelSummary };
export interface Site {
  name: string; launcher_url: string; base_url: string | null; seed_param: string;
  protocol: 'async_launcher' | 'single_file' | 'direct_kernel';
  status: 'validated' | 'reference'; headers: [string, string][];
}
export interface ClientConfig {
  timeout_ms: number | bigint; connect_timeout_ms: number | bigint; max_body_bytes: number | bigint; max_redirects: number;
}
export interface ClientOptions { timeoutMs?: number | bigint; connectTimeoutMs?: number | bigint; maxBodyBytes?: number | bigint; maxRedirects?: number }
export interface FetchOptions { client?: Partial<ClientConfig>; headers?: [string, string][]; baseUrl?: string; seedParam?: string }
export interface FetchedSeed {
  seed: string; launcher_url: string; kernel_url: string; launcher: number[] | null; kernel: number[]; kernel_summary: KernelSummary;
}
export interface StoreOutcome {
  status: 'stored' | 'duplicate' | 'variant'; kernel_path: string; launcher_path: string | null; launcher_differs: boolean;
}
export type TraceValue =
  | { kind: 'undefined' | 'null' }
  | { kind: 'boolean'; value: boolean }
  | { kind: 'number'; value: number; encoding?: null }
  | { kind: 'number'; value?: null; encoding: 'negative-zero' | 'nan' | 'positive-infinity' | 'negative-infinity' }
  | { kind: 'string'; value: string }
  | { kind: 'object'; identity: number | bigint }
  | { kind: 'external'; name: string };
export interface ThreadSnapshot {
  descriptor?: {
    initial_slots?: number[]; captured_slots?: number[]; argument_slots?: number[];
    arguments_slot?: number | null; result_slot?: number | null;
  };
  captured_memory?: Record<string, TraceValue>; incoming_stack?: (TraceValue | null)[];
  arguments_object?: TraceValue; self_value?: TraceValue; receiver?: TraceValue; arguments?: TraceValue[];
}
