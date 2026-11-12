// CrossOriginIsolationFlags: captured example values.
export default {
  "crossOriginIsolated": false,
  "sharedArrayBufferAvailable": false
};

// Illustrative: a cross-origin isolated page; SharedArrayBuffer is exposed.
export const crossOriginIsolated = {
  "crossOriginIsolated": true,
  "sharedArrayBufferAvailable": true
};

// Illustrative: SharedArrayBuffer is exposed without page isolation (legacy or flag exception).
export const sharedArrayBufferWithoutIsolation = {
  "crossOriginIsolated": false,
  "sharedArrayBufferAvailable": true
};

// Captured from a second build of the collector; decoded positional payload.
export const capturedSecondBuild = {
  "crossOriginIsolated": false,
  "sharedArrayBufferAvailable": false
};
