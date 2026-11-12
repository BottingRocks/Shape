// ProxyTamperFlags: captured example.
export default {
  "globalLooksLikeProxy": false,
  "navigatorLooksLikeProxy": false
};

// Illustrative: navigator is wrapped in a Proxy but the global object is not.
export const navigatorWrapped = {
  "globalLooksLikeProxy": false,
  "navigatorLooksLikeProxy": true
};

// Illustrative: both the global object and navigator are wrapped in a Proxy.
export const bothWrapped = {
  "globalLooksLikeProxy": true,
  "navigatorLooksLikeProxy": true
};

// Captured from a second build of the collector; decoded positional payload.
export const capturedSecondBuild = {
  "globalLooksLikeProxy": false,
  "navigatorLooksLikeProxy": false
};
