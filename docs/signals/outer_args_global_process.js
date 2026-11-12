// OuterArgsGlobalProcess: captured example.
export default {
  "flags": {
    "globalIsSelf": false,
    "outerArgsDefined": false,
    "processDefined": false,
    "stringifyThrew": false
  },
  "outerArgsJson": null
};

// Illustrative: loader arguments are defined and stringify succeeds.
export const definedArguments = {
  "outerArgsJson": "{\"example\":true}",
  "flags": {
    "globalIsSelf": false,
    "outerArgsDefined": true,
    "processDefined": false,
    "stringifyThrew": false
  }
};

// Illustrative: serializing defined loader arguments throws.
export const stringifyFailed = {
  "outerArgsJson": null,
  "flags": {
    "globalIsSelf": false,
    "outerArgsDefined": true,
    "processDefined": false,
    "stringifyThrew": true
  }
};

// Illustrative: a server-runtime global where global.global is self and process exists.
export const serverSideGlobal = {
  "outerArgsJson": "{\"wrap\":true}",
  "flags": {
    "globalIsSelf": true,
    "outerArgsDefined": true,
    "processDefined": true,
    "stringifyThrew": false
  }
};

// Captured from a second build of the collector; decoded positional payload.
export const capturedSecondBuild = {
  "flags": {
    "globalIsSelf": false,
    "outerArgsDefined": false,
    "processDefined": false,
    "stringifyThrew": false
  },
  "outerArgsJson": null
};
