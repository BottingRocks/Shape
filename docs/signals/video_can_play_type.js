// VideoCanPlayType: captured example.
export default {
  "codes": [
    0,
    2,
    2,
    0,
    0,
    0,
    0,
    0,
    0,
    1,
    0,
    0,
    1,
    2,
    2,
    2,
    2,
    2,
    2,
    0,
    0,
    0,
    2,
    2,
    2,
    2,
    2,
    2,
    2,
    1,
    2,
    2,
    0,
    1
  ]
};

// Illustrative: probing fails before any of the usual 34 results.
export const probeFailed = {
  "codes": []
};

// Illustrative: a spoofed canPlayType answers "probably" (2) to every string, including the nonsense entries.
export const spoofedMediaSupport = {
  "codes": [
    2,
    2,
    2,
    2,
    2,
    2,
    2,
    2,
    2,
    2,
    2,
    2,
    2,
    2,
    2,
    2,
    2,
    2,
    2,
    2,
    2,
    2,
    2,
    2,
    2,
    2,
    2,
    2,
    2,
    2,
    2,
    2,
    2,
    2
  ]
};

// Captured from a second build of the collector; decoded positional payload.
export const capturedSecondBuild = {
  "codes": [
    1,
    0,
    2,
    2,
    1,
    2,
    2,
    2,
    2,
    2,
    2,
    2,
    0,
    0,
    0,
    2,
    2,
    2,
    2,
    2,
    2,
    1,
    0,
    0,
    1,
    0,
    0,
    0,
    0,
    0,
    0,
    2,
    2,
    0
  ]
};
