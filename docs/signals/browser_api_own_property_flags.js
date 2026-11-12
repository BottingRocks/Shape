// BrowserApiOwnPropertyFlags: captured example values.
export default {
  "ApplePaySession": false,
  "ServiceWorker": true,
  "WebkitOverflowScrolling": false,
  "clipboard": true
};

// Illustrative: alternate engine layout where both engine-specific own properties are present.
export const alternateOwnPropertyFlags = {
  "ApplePaySession": true,
  "ServiceWorker": true,
  "WebkitOverflowScrolling": true,
  "clipboard": true
};

// Captured from a second build of the collector; decoded positional payload.
export const capturedSecondBuild = {
  "ApplePaySession": false,
  "ServiceWorker": true,
  "WebkitOverflowScrolling": false,
  "clipboard": true
};
