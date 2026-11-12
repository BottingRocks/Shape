// WorkerRealmProfile: captured example.
export default {
  "globalPropertyCount": 401,
  "hardwareConcurrency": 18,
  "origin": "https://www.target.com",
  "platform": "MacIntel",
  "userAgent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Safari/537.36"
};

// Illustrative worker whose hardwareConcurrency is missing.
export const missingHardwareConcurrency = {
  "hardwareConcurrency": null,
  "origin": "https://www.example.com",
  "globalPropertyCount": 400,
  "userAgent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Safari/537.36",
  "platform": "MacIntel"
};

// Captured from a second build of the collector; site/session identifier replaced with example.test.
export const capturedSecondBuild = {
  "globalPropertyCount": 402,
  "hardwareConcurrency": 18,
  "origin": "https://www.example.test",
  "platform": "MacIntel",
  "userAgent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/157.0.0.0 Safari/537.36"
};
