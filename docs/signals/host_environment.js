// HostEnvironment. Illustrative: host API returned a string; no emission when the API is absent.
export default {
  "hostEnvironmentValue": "{\"browser-name\":\"Microsoft Edge\",\"browser-platform-version\":\"10.0.0\",\"browser-version\":\"44.19041.1266.0\",\"device-sku\":\"\",\"device-dpi\":\"96\",\"device-logical-processors\":\"8\",\"device-max-touch-points\":\"0\",\"device-pixel-height\":\"1080\",\"device-pixel-width\":\"1920\",\"device-role\":\"desktop\",\"os-architecture\":\"x64\",\"os-build\":\"19041\",\"os-language\":\"en-US\",\"os-manufacturer\":\"Microsoft Corporation\",\"os-mode\":\"desktop\",\"os-sku\":\"\"}",
  "errorMessage": null
};

// Illustrative: the host API call threw without an error message.
export const callFailed = {
  "hostEnvironmentValue": null,
  "errorMessage": "No message"
};

// Illustrative: window.external has no getHostEnvironmentValue method; no payload is emitted.
export const apiUnavailable = undefined;
