// CurrentScriptAsyncPolicyState: illustrative logical async alias (from scriptFlags.async / scriptAsync null); policy data is absent.
export default {
  "async": true
};

// Illustrative: currentScript exists and is not async.
export const synchronousScript = {
  "async": false
};

// Illustrative: currentScript or its async property is missing; async is unknown.
export const asyncUnknown = {
  "async": null
};
