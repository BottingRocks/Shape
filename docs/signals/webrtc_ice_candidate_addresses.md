# WebRtcIceCandidateAddresses

[Signals index](README.md) · [data-shape example](webrtc_ice_candidate_addresses.js) · [schema format](schema.md)

## What it collects

Creates a throw-away RTCPeerConnection with no STUN/TURN servers, opens a data channel and creates an offer so the browser gathers its local ICE candidates. From each candidate line it pulls the IPv4 or full IPv6 address and the candidate type, drops duplicate addresses, and sends the list. No remote host is contacted; the result is cached for the page.

## Why

Local IP addresses can reveal the real network behind a proxy or VPN and identify a machine across sessions (likely); mDNS-only or empty results are also informative about the browser.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `recordCount` | number | Number of unique candidate addresses that follow; often 0 on modern browsers, which hide host IPs behind mDNS names. |
| `candidates` | group, repeated (once per unique address (last found first in the main build)) | One entry per repetition with the fields below. |
| `candidates[].candidateType` | string | Candidate type: "host", "srflx", "prflx" or "relay", or empty if none matched; usually "host". |
| `candidates[].ip` | string | IPv4 dotted-quad or full 8-group IPv6 address from the candidate line, e.g. "192.168.1.23". |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "candidates": []
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.WebRtcIceCandidateAddresses` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Repeated fields are `type: "array"` with an `item`; a repeated group's item is a `record` with its own `order` and `fields`.
