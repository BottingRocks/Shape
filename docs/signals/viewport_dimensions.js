// viewport_dimensions: Illustrative example; no captured payload available.
export default {
  "innerWidth": 1920,
  "innerHeight": 949,
  "outerHeight": 1050,
  "outerWidth": 1920,
  "screenWidth": 1920,
  "screenHeight": 1080,
  "screenAvailWidth": 1920,
  "screenAvailHeight": 1050,
  "visualViewportWidth": 1920,
  "visualViewportHeight": 949,
  "visualViewportScale": 1
};

// Illustrative: a viewport property read throws; no payload is emitted.
export const readFailed = undefined;

// Illustrative: the page is pinch-zoomed to 150%, so the visual viewport is smaller but the layout viewport is unchanged.
export const pinchZoomed = {
  "innerWidth": 1920,
  "innerHeight": 949,
  "outerHeight": 1050,
  "outerWidth": 1920,
  "screenWidth": 1920,
  "screenHeight": 1080,
  "screenAvailWidth": 1920,
  "screenAvailHeight": 1050,
  "visualViewportWidth": 1280,
  "visualViewportHeight": 632.6666666666666,
  "visualViewportScale": 1.5
};
