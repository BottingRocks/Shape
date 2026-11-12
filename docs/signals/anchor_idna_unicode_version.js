// AnchorIdnaUnicodeVersion: illustrative values; twelve tests in two ordered result groups.
export default {
  "resultBits": [
    {
      "U+0061": true,
      "U+AAE1": true,
      "U+20BA": true,
      "U+061C": true,
      "U+0978": true,
      "U+ABA2": true,
      "U+23FE": true,
      "U+0864": true
    },
    {
      "U+0560": true,
      "U+0C77": true,
      "U+32FF": true,
      "U+31BC": true
    }
  ]
};

// Illustrative: an engine with Unicode 11.0 IDNA tables; characters introduced in Unicode 12.0 and later percent-encode the host.
export const olderIdnaTables = {
  "resultBits": [
    {
      "U+0061": true,
      "U+AAE1": true,
      "U+20BA": true,
      "U+061C": true,
      "U+0978": true,
      "U+ABA2": true,
      "U+23FE": true,
      "U+0864": true
    },
    {
      "U+0560": true,
      "U+0C77": false,
      "U+32FF": false,
      "U+31BC": false
    }
  ]
};
