// LifecyclePhaseTimings. Captured example values.
export default {
  "collectionElapsedMs": 192,
  "initElapsedMs": 0,
  "lastChanceElapsedMs": 49815,
  "startEpochMs": 1794273726998
};

// Illustrative: init ran; collection and final steps have not run.
export const startup = {
  "collectionElapsedMs": -3600001,
  "startEpochMs": 1700000000000,
  "lastChanceElapsedMs": -3600001,
  "initElapsedMs": 0
};

// Illustrative: collection ran; final step has not run.
export const collection = {
  "collectionElapsedMs": 192,
  "startEpochMs": 1700000000000,
  "lastChanceElapsedMs": -3600001,
  "initElapsedMs": 0
};

// Illustrative: final step ran without collection.
export const collectionSkipped = {
  "collectionElapsedMs": -3600001,
  "startEpochMs": 1700000000000,
  "lastChanceElapsedMs": 49815,
  "initElapsedMs": 0
};

// Captured from a second build of the collector; decoded positional payload.
export const capturedSecondBuild = {
  "collectionElapsedMs": 201,
  "initElapsedMs": 0,
  "lastChanceElapsedMs": 24215,
  "startEpochMs": 1794383061296
};
