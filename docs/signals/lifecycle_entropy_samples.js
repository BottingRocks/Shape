// LifecycleEntropySamples. Illustrative: startup, collection, then final lifecycle step.
export default {
  "samples": [
    2419290613,
    3757324891,
    3982134762
  ]
};

// Illustrative: collection did not run; startup and final steps did.
export const collectionSkipped = {
  "samples": [
    2419290613,
    3982134762
  ]
};

// Illustrative: collection ran twice after an error, then the final step.
export const collectionRetried = {
  "samples": [
    2419290613,
    3757324891,
    1849926475,
    3982134762
  ]
};
