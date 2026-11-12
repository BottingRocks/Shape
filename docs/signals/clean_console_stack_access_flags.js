// CleanConsoleStackAccessFlags: captured example values.
export default {
  "cleanRealmStackRead": false,
  "pageConsoleStackRead": false,
  "sentryOriginalLogCalled": false
};

// Illustrative: DevTools or an automation runtime serializes console arguments, so both realms read the trap.
export const devtoolsOpen = {
  "cleanRealmStackRead": true,
  "pageConsoleStackRead": true,
  "sentryOriginalLogCalled": false
};

// Illustrative: a page-level console wrapper reads the trap, but the fresh iframe console does not.
export const pageConsoleHook = {
  "cleanRealmStackRead": false,
  "pageConsoleStackRead": true,
  "sentryOriginalLogCalled": false
};

// Captured from a second build of the collector; decoded positional payload.
export const capturedSecondBuild = {
  "cleanRealmStackRead": false,
  "pageConsoleStackRead": false,
  "sentryOriginalLogCalled": false
};
