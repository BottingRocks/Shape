// SeleniumIdeArtifactFlags: captured example.
export default {
  "promptScript": false,
  "sideexPlaying": false
};

// Illustrative: SideeX is playing, but no extension prompt script was seen.
export const sideexRunning = {
  "promptScript": false,
  "sideexPlaying": true
};

// Illustrative: an extension-served prompt.js was seen, but SideeX is not playing.
export const promptScriptDetected = {
  "promptScript": true,
  "sideexPlaying": false
};

// Captured from a second build of the collector; decoded positional payload.
export const capturedSecondBuild = {
  "promptScript": false,
  "sideexPlaying": false
};
