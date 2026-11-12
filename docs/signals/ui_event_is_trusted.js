// UiEventIsTrusted: captured example.
export default {
  "isTrusted": false
};

// Illustrative: isTrusted was patched; a script-made event claims to be trusted.
export const spoofedTrusted = {
  "isTrusted": true
};

// Captured from a second build of the collector; decoded positional payload.
export const capturedSecondBuild = {
  "isTrusted": false
};
