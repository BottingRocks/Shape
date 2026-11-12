// Compile only: tsc --noEmit --strict --module nodenext --moduleResolution nodenext
//   --target es2022 --lib esnext sdk/node/test/result-contracts.ts
import { Generator, trace, type DecodeMetadata, type DecodedPayload, type JsonObject, type TraceThreadResult } from '../index.js';

const completed: TraceThreadResult = {
  code: 'function thread() { return; }', complete: true, stop: 'return',
  steps: 1, handlers_traced: 1, undeclared: [], self_check: [], source_hash: 'hash',
};
const stopped: TraceThreadResult = {
  ...completed, code: null, complete: false, self_check: ['register mismatch'],
};

// Negative assertions prevent these contracts from silently widening again.
// @ts-expect-error diagnostics are a string list, not the request's boolean flag
const booleanDiagnostics: TraceThreadResult = { ...completed, self_check: true };
// @ts-expect-error each diagnostic is a string
const numericDiagnostics: TraceThreadResult = { ...completed, self_check: [1] };
// @ts-expect-error nullable code requires a guard
const unguardedCode: string = stopped.code;
// @ts-expect-error originalReport is retained even without detailed metadata
const missingReport: DecodeMetadata = { signals: {}, decoded: 0, missing: 0, exactReencode: false, floatByteOrder: 'little' };
// @ts-expect-error signals is present as an empty object when details are disabled
const missingSignals: DecodeMetadata = { originalReport: {}, decoded: 0, missing: 0, exactReencode: false, floatByteOrder: 'little' };

type Assert<T extends true> = T;
type RequiredMetadata = Assert<{} extends Pick<DecodedPayload, 'metadata'> ? false : true>;

async function checkResults(generator: Generator): Promise<void> {
  const thread = await trace.thread(null, { artifact: '{}', pc: 0, state: 0 });
  const diagnostics: string[] = thread.self_check;
  if (thread.code !== null) {
    const source: string = thread.code;
    void source;
  }
  for (const decoded of [
    generator.decode({ siteId: 'site', headers: {} }, { metadata: false }),
    await generator.decodeAsync({ siteId: 'site', headers: {} }, { metadata: false }),
  ]) {
    const metadata: DecodeMetadata = decoded.metadata;
    const original: JsonObject = metadata.originalReport;
    const details: JsonObject = metadata.signals;
    const count: number = metadata.decoded;
    void [original, details, count];
  }
  void diagnostics;
}

void [completed, stopped, booleanDiagnostics, numericDiagnostics, unguardedCode, missingReport, missingSignals, checkResults];
