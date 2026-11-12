// VisibilityEventLog: captured example.
export default {
  "records": [
    {
      "timestamp": 36164.39999997616,
      "visible": false
    },
    {
      "timestamp": 47865.39999997616,
      "visible": true
    }
  ],
  "state": {
    "currentlyVisible": true,
    "initiallyVisible": true
  },
  "visibilityChangeCount": 2
};

// Illustrative dormant unknown-current-visibility branch; observed builds re-sample a boolean instead.
export const unknownVisibility = {
  "records": [],
  "visibilityChangeCount": 0,
  "state": {
    "currentlyVisible": null,
    "initiallyVisible": true
  }
};

// Illustrative: the page was hidden from the start and never became visible.
export const neverVisible = {
  "records": [],
  "state": {
    "currentlyVisible": false,
    "initiallyVisible": false
  },
  "visibilityChangeCount": 0
};

// Illustrative: the page loaded hidden, then became visible once.
export const startedHidden = {
  "records": [
    {
      "timestamp": 1245.6999999284744,
      "visible": true
    }
  ],
  "state": {
    "currentlyVisible": true,
    "initiallyVisible": false
  },
  "visibilityChangeCount": 1
};

// Captured from a second build of the collector; decoded positional payload.
export const capturedSecondBuild = {
  "records": [
    {
      "timestamp": 21043.5,
      "visible": true
    },
    {
      "timestamp": 10527.900000002235,
      "visible": false
    }
  ],
  "state": {
    "currentlyVisible": true,
    "initiallyVisible": true
  },
  "visibilityChangeCount": 2
};
