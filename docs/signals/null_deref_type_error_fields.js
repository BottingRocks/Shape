// NullDerefTypeErrorFields: captured example.
export default {
  "description": null,
  "message": "Cannot read properties of null (reading '0')",
  "name": "TypeError",
  "number": null,
  "stack": "TypeError: Cannot read properties of null (reading '0')\n    at URL",
  "stacktrace": null
};

// Illustrative legacy error properties with an unavailable stack.
export const legacyErrorProperties = {
  "stacktrace": "TypeError at example.js:1",
  "message": "null has no properties",
  "name": "TypeError",
  "number": -1,
  "stack": null,
  "description": "null has no properties"
};

// Illustrative: a modern Firefox-worded TypeError, with the same shape and no legacy properties.
export const firefoxMessage = {
  "description": null,
  "message": "null has no properties",
  "name": "TypeError",
  "number": null,
  "stack": "TypeError: null has no properties\n    at URL",
  "stacktrace": null
};

// Captured from a second build of the collector; decoded positional payload.
export const capturedSecondBuild = {
  "description": null,
  "message": "Cannot read properties of null (reading '0')",
  "name": "TypeError",
  "number": null,
  "stack": "TypeError: Cannot read properties of null (reading '0')\n    at URL",
  "stacktrace": null
};
