// FunctionToStringComparison: captured example values.
export default {
  "probeFlags": {
    "equal": true,
    "threw": false
  },
  "toStringGetterCalls": 1
};

// Illustrative: valueOf/Symbol.toPrimitive is patched and the comparison throws before reading the getter.
export const patchedPrimitive = {
  "probeFlags": {
    "equal": false,
    "threw": true
  },
  "toStringGetterCalls": 0
};

// Illustrative: a hooked toString reads the getter twice, but both strings still match.
export const hookedToString = {
  "probeFlags": {
    "equal": true,
    "threw": false
  },
  "toStringGetterCalls": 2
};

// Captured from a second build of the collector; decoded positional payload.
export const capturedSecondBuild = {
  "probeFlags": {
    "equal": true,
    "threw": false
  },
  "toStringGetterCalls": 1
};
