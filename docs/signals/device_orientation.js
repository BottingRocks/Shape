// DeviceOrientation: captured example values.
export default {
  "eventCount": 2,
  "meanAlpha": 0,
  "meanAlphaSquared": 0,
  "meanBeta": 0,
  "meanBetaSquared": 0,
  "meanGamma": 0,
  "meanGammaSquared": 0,
  "meanIntervalMs": 47880,
  "meanSquaredIntervalMs": 2292494400
};

// Illustrative: a handheld device reports a noisy ~16 ms stream of real alpha/beta/gamma angles.
export const handheldDevice = {
  "eventCount": 96,
  "meanAlpha": 184.7,
  "meanAlphaSquared": 40218.3,
  "meanBeta": -12.4,
  "meanBetaSquared": 421.8,
  "meanGamma": 3.2,
  "meanGammaSquared": 188.6,
  "meanIntervalMs": 16,
  "meanSquaredIntervalMs": 288
};

// Captured from a second build of the collector; decoded positional payload.
export const capturedSecondBuild = {
  "eventCount": 2,
  "meanAlpha": 0,
  "meanAlphaSquared": 0,
  "meanBeta": 0,
  "meanBetaSquared": 0,
  "meanGamma": 0,
  "meanGammaSquared": 0,
  "meanIntervalMs": 21019,
  "meanSquaredIntervalMs": 441798361
};
