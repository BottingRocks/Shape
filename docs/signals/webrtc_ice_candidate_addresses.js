// WebRtcIceCandidateAddresses: illustrative data-bearing example.
// Illustrative nonempty candidate list; the captured list was empty.
export default {
  "candidates": [
    {
      "candidateType": "host",
      "ip": "192.168.1.23"
    },
    {
      "candidateType": "host",
      "ip": "fe80:0000:0000:0000:1c2b:3d4e:5f60:7a8b"
    }
  ]
};

// Captured example: no candidates were recorded.
export const capturedEmpty = {
  "candidates": []
};

// Captured from a second build of the collector; decoded positional payload.
export const capturedSecondBuild = {
  "candidates": []
};
