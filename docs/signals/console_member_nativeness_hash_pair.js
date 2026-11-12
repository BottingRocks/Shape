// ConsoleMemberNativenessHashPair: captured example values.
export default {
  "nativeNamesHash": -930873437,
  "nonNativeNamesHash": 0
};

// Illustrative: console is missing, so both classified name lists are empty and hash to 0.
export const consoleMissing = {
  "nativeNamesHash": 0,
  "nonNativeNamesHash": 0
};

// Illustrative: a page-level wrapper replaced one console method, so that name lands in the non-native list.
export const wrappedConsole = {
  "nativeNamesHash": 1337015221,
  "nonNativeNamesHash": -1712894673
};

// Captured from a second build of the collector; decoded positional payload.
export const capturedSecondBuild = {
  "nativeNamesHash": -930873437,
  "nonNativeNamesHash": 0
};
