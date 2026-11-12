// AnimationFrameTimings: captured example values.
export default {
  "frameTime": [
    9,
    8,
    9,
    8,
    9,
    8,
    8,
    8,
    9,
    8,
    9,
    8,
    8,
    8,
    8,
    9,
    8,
    8,
    10,
    1056
  ],
  "timeQuantum": 1
};

// Illustrative: no frame callbacks sampled.
export const noFrames = {
  "frameTime": [],
  "timeQuantum": 1
};

// Illustrative: only one frame sampled; the single entry is just its timestamp.
export const singleSample = {
  "frameTime": [
    7
  ],
  "timeQuantum": 1
};

// Captured from a second build of the collector; decoded positional payload.
export const capturedSecondBuild = {
  "frameTime": [
    897,
    17,
    8,
    17,
    8,
    8,
    9,
    8,
    9,
    8,
    8,
    9,
    8,
    8,
    8,
    9,
    8,
    9,
    8,
    8
  ],
  "timeQuantum": 1
};
