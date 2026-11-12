// vm_source_integrity: Captured measurements after removing numeric padding; counts retain only their low eight bits.
export default {
  "bodyLength": 42,
  "buildMarker": "e5cfce5772068bdc",
  "nonce": 0.515963662070013,
  "punctuationCount": 35,
  "whitespaceCount": 30
};

// Illustrative build variant in which nonce is absent.
export const withoutNonce = {
  "bodyLength": 42,
  "buildMarker": "e5cfce5772068bdc",
  "punctuationCount": 35,
  "whitespaceCount": 30
};

// Illustrative: pretty-printing the same build raised the whitespace and body-length counts.
export const reformattedSource = {
  "bodyLength": 198,
  "buildMarker": "e5cfce5772068bdc",
  "nonce": 0.6318273059616089,
  "punctuationCount": 41,
  "whitespaceCount": 214
};

// Captured from a second build of the collector; decoded positional payload.
export const capturedSecondBuild = {
  "bodyLength": 35783,
  "buildMarker": "457f8f31909501e5",
  "nonce": 0.2513609998182318,
  "punctuationCount": 4035,
  "whitespaceCount": 462
};
