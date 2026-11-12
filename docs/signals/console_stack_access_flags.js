// ConsoleStackAccessFlags: captured example values.
export default {
  "iframeConsoleStackRead": false,
  "pageConsoleStackRead": false,
  "sentryOriginalLogCalled": false
};

// Illustrative: a page-level console.log hook reads the probe's stack while the iframe realm stays unobserved.
export const pageConsoleHook = {
  "iframeConsoleStackRead": false,
  "pageConsoleStackRead": true,
  "sentryOriginalLogCalled": false
};

// Captured from a second build of the collector; decoded positional payload.
export const capturedSecondBuild = {
  "iframeConsoleStackRead": false,
  "pageConsoleStackRead": false,
  "sentryOriginalLogCalled": false
};
