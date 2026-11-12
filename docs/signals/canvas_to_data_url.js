// CanvasToDataUrl: captured example values.
export default {
  "dataUrlHash": -1952225390,
  "dataUrlPrefix": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAASwA",
  "elapsedMs": 20
};

// Illustrative: drawing or export throws.
export const drawingFailed = {
  "dataUrlHash": 92629711,
  "elapsedMs": null,
  "dataUrlPrefix": "data:image/png;base64,____________SAFE____________"
};

// Illustrative: builds omitting elapsedMs.
export const withoutTiming = {
  "dataUrlHash": -1952225390,
  "dataUrlPrefix": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAASwA"
};

// Illustrative: drawing or export throws in builds omitting elapsedMs.
export const drawingFailedWithoutTiming = {
  "dataUrlHash": 92629711,
  "dataUrlPrefix": "data:image/png;base64,____________SAFE____________"
};

// Captured from a second build of the collector; decoded positional payload.
export const capturedSecondBuild = {
  "dataUrlHash": -1952225390,
  "dataUrlPrefix": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAASwA",
  "elapsedMs": 25
};
