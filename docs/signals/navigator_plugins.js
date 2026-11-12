// NavigatorPlugins. Captured example values.
export default {
  "plugin": [
    {
      "detailsHash": -1716625998,
      "namePrefix": "WebKit built"
    },
    {
      "detailsHash": 1387368353,
      "namePrefix": "Microsoft Ed"
    },
    {
      "detailsHash": 2126051627,
      "namePrefix": "Chromium PDF"
    },
    {
      "detailsHash": 1061675194,
      "namePrefix": "Chrome PDF V"
    },
    {
      "detailsHash": 975495497,
      "namePrefix": "PDF Viewer"
    }
  ],
  "pluginsLength": 5
};

// Illustrative: no plugins exposed; the summarised list is empty and the length is 0.
export const noPlugins = {
  "plugin": [],
  "pluginsLength": 0
};

// Illustrative: five plugins exposed but one entry could not be read and was skipped.
export const partialRead = {
  "plugin": [
    {
      "detailsHash": -1716625998,
      "namePrefix": "WebKit built"
    },
    {
      "detailsHash": 2126051627,
      "namePrefix": "Chromium PDF"
    },
    {
      "detailsHash": 1061675194,
      "namePrefix": "Chrome PDF V"
    },
    {
      "detailsHash": 975495497,
      "namePrefix": "PDF Viewer"
    }
  ],
  "pluginsLength": 5
};

// Captured from a second build of the collector; decoded positional payload.
export const capturedSecondBuild = {
  "plugin": [
    {
      "detailsHash": 975495497,
      "namePrefix": "PDF Viewer"
    },
    {
      "detailsHash": 1061675194,
      "namePrefix": "Chrome PDF V"
    },
    {
      "detailsHash": 2126051627,
      "namePrefix": "Chromium PDF"
    },
    {
      "detailsHash": 1387368353,
      "namePrefix": "Microsoft Ed"
    },
    {
      "detailsHash": -1716625998,
      "namePrefix": "WebKit built"
    }
  ],
  "pluginsLength": 5
};
