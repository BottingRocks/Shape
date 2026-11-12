// Compile only: a real ESM consumer, with value/type imports kept distinct.
import { artifact, decode, encode, explain, schema, parseJSON, stringifyJSON,
  trace, CachePolicy, pack, unpackCanonical, signals } from 'shape';
import type { LoadedArtifact, CaptureHeaders, DecodedPayload, DecodeOptions,
  TraceProgramOptions, PackValue } from 'shape';

const options: DecodeOptions = { metadata: false, verify: false };
export function roundtrip(loaded: LoadedArtifact, capture: CaptureHeaders): CaptureHeaders {
  const decoded: DecodedPayload = decode(loaded, capture, options);
  const memory = decoded.positionalSignals.DeviceMemory;
  if (memory) memory.deviceMemory = 8;
  const encoded: CaptureHeaders = encode(loaded, decoded);
  explain(loaded, decoded, 'DeviceMemory', { verify: true });
  schema(loaded);
  const json: string = stringifyJSON(decoded);
  parseJSON(json);
  return encoded;
}
const programOptions: TraceProgramOptions = {
  cache: CachePolicy.Disabled, threadCleanup: true, noCleanup: false,
};
export async function recover(source: string): Promise<string> {
  const result = await trace.program(source, programOptions);
  if (!result.complete) throw new Error('incomplete trace');
  return result.code;
}
export async function typedCodec(value: PackValue): Promise<PackValue> {
  return unpackCanonical(await pack(value));
}
export async function inspect(): Promise<void> {
  await signals.describe('DeviceMemory');
  const loaded: LoadedArtifact = await artifact.loadAsync('{}');
  loaded.close();
}
// Errors must be caught by public declarations rather than broad any options.
// @ts-expect-error a file name cannot replace a loaded artifact
schema('artifact.json');
// @ts-expect-error flags are booleans
const invalidOptions: DecodeOptions = { verify: 'yes' };
void invalidOptions;
