// GrecaptchaAccessorHookFlags. Captured example values.
export default {
  "hookChecks": [
    {
      "compressionStream": true,
      "descriptorPresent": false,
      "getterHasBasMarker": false,
      "getterIsFunction": false,
      "layoutShiftAttribution": true,
      "ownFound": false,
      "setterHasToString": false
    },
    {
      "featurePolicy": true,
      "getterHasToString": false,
      "setterHasBasMarker": false,
      "setterIsFunction": false
    }
  ]
};

// Illustrative: a plain-value global with a descriptor and own property found, and no automation markers.
export const normalValue = {
  "hookChecks": [
    {
      "compressionStream": true,
      "descriptorPresent": true,
      "getterHasBasMarker": false,
      "getterIsFunction": false,
      "layoutShiftAttribution": true,
      "ownFound": true,
      "setterHasToString": false
    },
    {
      "featurePolicy": true,
      "getterHasToString": false,
      "setterHasBasMarker": false,
      "setterIsFunction": false
    }
  ]
};

// Captured from a second build of the collector; decoded positional payload.
export const capturedSecondBuild = {
  "hookChecks": [
    {
      "compressionStream": true,
      "descriptorPresent": false,
      "getterHasBasMarker": false,
      "getterIsFunction": false,
      "layoutShiftAttribution": true,
      "ownFound": false,
      "setterHasToString": false
    },
    {
      "featurePolicy": false,
      "getterHasToString": false,
      "setterHasBasMarker": false,
      "setterIsFunction": false
    }
  ]
};
