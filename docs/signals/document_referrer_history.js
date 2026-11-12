// DocumentReferrerHistory: captured example values.
export default {
  "historyLength": 2,
  "referrerEmpty": false,
  "referrerPrefix": "https://www.target.com/"
};

// Illustrative: builds that omit the referrer text.
export const withoutReferrerPrefix = {
  "historyLength": 2,
  "referrerEmpty": false
};

// Illustrative: a fresh tab opened directly, so there is no referrer and the prefix is empty.
export const freshTab = {
  "historyLength": 1,
  "referrerEmpty": true,
  "referrerPrefix": ""
};

// Captured from a second build of the collector; site/session identifier replaced with example.test.
export const capturedSecondBuild = {
  "historyLength": 2,
  "referrerEmpty": false,
  "referrerPrefix": "https://example.test/"
};
