// scroll_offsets: Illustrative example; no captured payload available.
export default {
  "initialScrollY": 0,
  "currentScrollY": 240
};

// Illustrative: pageYOffset is missing, so both numeric readings are NaN.
export const missingOffsets = {
  "initialScrollY": NaN,
  "currentScrollY": NaN
};

// Illustrative: the page was never scrolled between setup and report.
export const unscrolled = {
  "initialScrollY": 0,
  "currentScrollY": 0
};

// Illustrative: rubber-band overscroll at setup, back to the top when the report is built.
export const overscroll = {
  "initialScrollY": -37,
  "currentScrollY": 0
};
