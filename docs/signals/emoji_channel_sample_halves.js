// EmojiChannelSampleHalves: captured example values.
export default {
  "firstHalfSamples": [
    0,
    0,
    232,
    162
  ],
  "secondHalfSamples": [
    255,
    31,
    142,
    9
  ]
};

// Illustrative: cleanup stopped collection after the first four emoji, so the second half is empty.
export const partialCapture = {
  "firstHalfSamples": [
    0,
    0,
    230,
    160
  ],
  "secondHalfSamples": []
};

// Illustrative: every draw or read failed, so the collected samples are all -1 error markers.
export const readbackFailed = {
  "firstHalfSamples": [
    -1,
    -1,
    -1,
    -1
  ],
  "secondHalfSamples": [
    -1,
    -1,
    -1,
    -1
  ]
};

// Captured from a second build of the collector; decoded positional payload.
export const capturedSecondBuild = {
  "firstHalfSamples": [
    0,
    0,
    163,
    195
  ],
  "secondHalfSamples": [
    0,
    0,
    85,
    203
  ]
};
