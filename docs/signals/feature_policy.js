// FeaturePolicy: captured example values.
export default {
  "allowedFeatureCount": 76,
  "allowlistEntry": "https://www.target.com",
  "featureCount": 77,
  "featureListHash": -667729372,
  "samplingSeed": 0.5477980646993165
};

// Illustrative family A: random selection without a shared-pool samplingSeed.
export const randomSelection = {
  "allowedFeatureCount": 76,
  "allowlistEntry": "https://www.target.com",
  "featureCount": 77,
  "featureListHash": -667729372
};

// Illustrative family B: chosen feature has no allowlist entry.
export const noAllowlistEntry = {
  "allowedFeatureCount": 76,
  "allowlistEntry": null,
  "featureCount": 77,
  "featureListHash": -667729372,
  "samplingSeed": 0.5477980646993165
};

// Illustrative family A: chosen feature has no allowlist entry.
export const randomSelectionWithoutEntry = {
  "allowedFeatureCount": 76,
  "allowlistEntry": null,
  "featureCount": 77,
  "featureListHash": -667729372
};

// Illustrative: document.featurePolicy is absent, so the signal emits no payload.
export const featurePolicyUnavailable = undefined;

// Captured from a second build of the collector; site/session identifier replaced with example.test.
export const capturedSecondBuild = {
  "allowedFeatureCount": 76,
  "allowlistEntry": "https://example.test",
  "featureCount": 77,
  "featureListHash": -667729372,
  "samplingSeed": 0.7282635515660257
};
