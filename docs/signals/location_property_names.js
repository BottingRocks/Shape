// LocationPropertyNames. Captured example values.
export default {
  "locationKeyCodes": "a\u0000g\u0000h\u0000k\u0000e\u0000f\u0000j\u0000i\u0000n\u0000d\u0000b\u0000l\u0000m\u0000o\u0000"
};

// Illustrative: window.location is missing, so no property codes are emitted.
export const locationMissing = {
  "locationKeyCodes": ""
};

// Illustrative: a patched location enumerates one extra, unrecognized property (u).
export const unknownProperty = {
  "locationKeyCodes": "a\u0000g\u0000h\u0000k\u0000e\u0000f\u0000j\u0000i\u0000n\u0000d\u0000b\u0000l\u0000m\u0000o\u0000u\u0000"
};
