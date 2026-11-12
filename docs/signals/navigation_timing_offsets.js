// NavigationTimingOffsets. Illustrative: navigationStart is absolute epoch milliseconds; other milestones are offsets, with 0 for absent milestones.
export default {
  "domLoading": 95,
  "connectStart": 5,
  "domainLookupEnd": 5,
  "unloadEventEnd": 0,
  "unloadEventStart": 0,
  "navigationStart": 1794268689347,
  "connectEnd": 30,
  "fetchStart": 1,
  "loadEventStart": 240,
  "loadEventEnd": 245,
  "domContentLoadedEventEnd": 185,
  "redirectEnd": 0,
  "responseStart": 80,
  "requestStart": 31,
  "domComplete": 235,
  "domContentLoadedEventStart": 180,
  "domainLookupStart": 2,
  "domInteractive": 175,
  "redirectStart": 0,
  "secureConnectionStart": 10,
  "responseEnd": 90
};

// Illustrative: a redirected navigation that also unloaded the previous page, followed by a slower load.
export const redirectedNavigation = {
  "domLoading": 140,
  "connectStart": 24,
  "domainLookupEnd": 24,
  "unloadEventEnd": 134,
  "unloadEventStart": 131,
  "navigationStart": 1794268621043,
  "connectEnd": 55,
  "fetchStart": 19,
  "loadEventStart": 318,
  "loadEventEnd": 324,
  "domContentLoadedEventEnd": 246,
  "redirectEnd": 18,
  "responseStart": 112,
  "requestStart": 56,
  "domComplete": 312,
  "domContentLoadedEventStart": 240,
  "domainLookupStart": 20,
  "domInteractive": 235,
  "redirectStart": 2,
  "secureConnectionStart": 32,
  "responseEnd": 128
};
