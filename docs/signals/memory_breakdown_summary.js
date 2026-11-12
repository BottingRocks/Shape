// MemoryBreakdownSummary. Illustrative successful measurement; attributions with and without a container.
export default {
  "status": 2,
  "entry": [
    {
      "attribution": [
        {
          "scope": "Window",
          "url": "example.com/"
        },
        {
          "containerSrc": "https://example.com/frame",
          "containerId": "frame",
          "scope": "Window",
          "url": "example.com/frame"
        }
      ],
      "memoryType": [
        "JavaScript",
        "DOM"
      ],
      "entryBytes": 1048576
    }
  ],
  "totalBytes": 2097152,
  "breakdownCount": 3
};

// Captured: measurement API unavailable.
export const apiUnavailable = {
  "status": 3
};

// Illustrative: setup, before collection.
export const startup = {
  "status": 0
};

// Illustrative: measurement requested; pending or failed.
export const pending = {
  "status": 1
};

// Illustrative: measurement completed but no entries were attributable; the entry list is empty.
export const emptyMeasurement = {
  "status": 2,
  "entry": [],
  "totalBytes": 0,
  "breakdownCount": 0
};

// Captured from a second build of the collector; decoded positional payload.
export const capturedSecondBuild = {
  "status": 3
};
