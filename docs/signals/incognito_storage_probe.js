// IncognitoStorageProbe. Captured example values.
export default {
  "browserCode": "D",
  "isPrivateMode": false
};

// Illustrative: the private-mode test reported a positive result.
export const privateModeDetected = {
  "browserCode": "D",
  "isPrivateMode": true
};

// Illustrative: the browser family could not be identified, so the probe reports false.
export const unknownBrowser = {
  "browserCode": "UD",
  "isPrivateMode": false
};

// Captured from a second build of the collector; decoded positional payload.
export const capturedSecondBuild = {
  "browserCode": "D",
  "isPrivateMode": false
};
