// PasteCountUnfocusedInputFlag: captured example.
export default {
  "pastedInputCount": 1,
  "unfocusedInputSeen": false
};

// Illustrative: no paste or other input activity was observed.
export const noPastes = {
  "pastedInputCount": 0,
  "unfocusedInputSeen": false
};

// Illustrative: scripted input events reached unfocused inputs without any paste.
export const unfocusedInput = {
  "pastedInputCount": 0,
  "unfocusedInputSeen": true
};

// Captured from a second build of the collector; decoded positional payload.
export const capturedSecondBuild = {
  "pastedInputCount": 1,
  "unfocusedInputSeen": false
};
