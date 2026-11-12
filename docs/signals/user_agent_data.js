// UserAgentData: captured example.
export default {
  "architecture": "arm",
  "bitness": "64",
  "brands": [
    {
      "brand": "Google Chrome",
      "version": "156"
    },
    {
      "brand": "Chromium",
      "version": "156"
    },
    {
      "brand": "Not:A-Brand",
      "version": "8"
    }
  ],
  "flags": {
    "mobile": false,
    "wow64": false
  },
  "fullVersionList": [
    {
      "brand": "Google Chrome",
      "version": "156.0.8078.81"
    },
    {
      "brand": "Chromium",
      "version": "156.0.8078.81"
    },
    {
      "brand": "Not:A-Brand",
      "version": "8.0.0.0"
    }
  ],
  "model": "",
  "platform": "macOS",
  "platformVersion": "27.0.1",
  "uaFullVersion": "156.0.8078.81"
};

// Illustrative logical view: rejected/missing hints are null, including the two boolean hints.
export const missingHints = {
  "brands": null,
  "platformVersion": null,
  "architecture": null,
  "fullVersionList": null,
  "bitness": null,
  "platform": null,
  "flags": {
    "wow64": null,
    "mobile": null
  },
  "uaFullVersion": null,
  "model": null
};

// Illustrative: userAgentData is unavailable; no payload is emitted.
export const apiUnavailable = undefined;

// Illustrative: an Android phone; the mobile hint is true and the device model is populated.
export const mobileDevice = {
  "architecture": "arm",
  "bitness": "64",
  "brands": [
    {
      "brand": "Google Chrome",
      "version": "156"
    },
    {
      "brand": "Chromium",
      "version": "156"
    },
    {
      "brand": "Not:A-Brand",
      "version": "8"
    }
  ],
  "flags": {
    "mobile": true,
    "wow64": false
  },
  "fullVersionList": [
    {
      "brand": "Google Chrome",
      "version": "156.0.8078.81"
    },
    {
      "brand": "Chromium",
      "version": "156.0.8078.81"
    },
    {
      "brand": "Not:A-Brand",
      "version": "8.0.0.0"
    }
  ],
  "model": "Pixel 7",
  "platform": "Android",
  "platformVersion": "16.0.0",
  "uaFullVersion": "156.0.8078.81"
};

// Captured from a second build of the collector; decoded positional payload.
export const capturedSecondBuild = {
  "architecture": "arm",
  "bitness": "64",
  "brands": [
    {
      "brand": "Not-A.Brand",
      "version": "99"
    },
    {
      "brand": "Google Chrome",
      "version": "157"
    },
    {
      "brand": "Chromium",
      "version": "157"
    }
  ],
  "flags": {
    "mobile": false,
    "wow64": false
  },
  "fullVersionList": [
    {
      "brand": "Chromium",
      "version": "157.0.8103.40"
    },
    {
      "brand": "Google Chrome",
      "version": "157.0.8103.40"
    },
    {
      "brand": "Not-A.Brand",
      "version": "99.0.0.0"
    }
  ],
  "model": "",
  "platform": "macOS",
  "platformVersion": "27.0.1",
  "uaFullVersion": "157.0.8103.40"
};
