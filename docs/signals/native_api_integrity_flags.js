// NativeApiIntegrityFlags. Captured example values.
export default {
  "integrityChecks": [
    {
      "createElementNative": true,
      "functionToStringNative": true,
      "mimeTypesGetterNative": true,
      "pluginsGetterNative": true,
      "pluginsTagNotPlain": true,
      "screenHeightGetterNative": true,
      "strokeStyleAccessorNative": true
    },
    {
      "createElementNSNative": true,
      "documentProtoNoOwnCreateElement": true,
      "fillTextNative": true,
      "fontAccessorNative": true,
      "getImageDataNative": true,
      "screenWidthGetterNative": true,
      "toDataURLNative": true
    },
    {
      "documentNoOwnCreateElement": true,
      "documentPrototypeHasOwnCreateElement": true,
      "fillStyleAccessorNative": true,
      "getContextNative": true,
      "mimeTypesTagNotPlain": true,
      "userAgentGetterNative": true
    }
  ]
};

// Illustrative: an injected script overrides the user-agent, plugin and canvas APIs.
export const tamperedApis = {
  "integrityChecks": [
    {
      "createElementNative": true,
      "functionToStringNative": true,
      "mimeTypesGetterNative": true,
      "pluginsGetterNative": false,
      "pluginsTagNotPlain": false,
      "screenHeightGetterNative": true,
      "strokeStyleAccessorNative": true
    },
    {
      "createElementNSNative": true,
      "documentProtoNoOwnCreateElement": true,
      "fillTextNative": true,
      "fontAccessorNative": true,
      "getImageDataNative": false,
      "screenWidthGetterNative": true,
      "toDataURLNative": false
    },
    {
      "documentNoOwnCreateElement": true,
      "documentPrototypeHasOwnCreateElement": true,
      "fillStyleAccessorNative": true,
      "getContextNative": true,
      "mimeTypesTagNotPlain": false,
      "userAgentGetterNative": false
    }
  ]
};

// Captured from a second build of the collector; decoded positional payload.
export const capturedSecondBuild = {
  "integrityChecks": [
    {
      "createElementNative": true,
      "functionToStringNative": false,
      "mimeTypesGetterNative": false,
      "pluginsGetterNative": false,
      "pluginsTagNotPlain": true,
      "screenHeightGetterNative": false,
      "strokeStyleAccessorNative": true
    },
    {
      "createElementNSNative": true,
      "documentProtoNoOwnCreateElement": false,
      "fillTextNative": true,
      "fontAccessorNative": false,
      "getImageDataNative": true,
      "screenWidthGetterNative": false,
      "toDataURLNative": true
    },
    {
      "documentNoOwnCreateElement": false,
      "documentPrototypeHasOwnCreateElement": false,
      "fillStyleAccessorNative": true,
      "getContextNative": true,
      "mimeTypesTagNotPlain": false,
      "userAgentGetterNative": true
    }
  ]
};
