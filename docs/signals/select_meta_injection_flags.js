// select_meta_injection_flags: Illustrative example; no captured payload available.
export default {
  "selectMeta": false,
  "animationStart": false,
  "measurementDiv": false,
  "animationStyle": false
};

// Illustrative: the _selectMeta global appears first; the injected extension has not added DOM or style traces yet.
export const selectMetaDetected = {
  "selectMeta": true,
  "animationStart": false,
  "measurementDiv": false,
  "animationStyle": false
};

// Illustrative: all four traces of the injected select customiser are present.
export const allTracesDetected = {
  "selectMeta": true,
  "animationStart": true,
  "measurementDiv": true,
  "animationStyle": true
};
