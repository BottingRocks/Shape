# PonosXhrRoundTrip

[Signals index](README.md) · [data-shape example](ponos_xhr_round_trip.js) · [schema format](schema.md)

## What it collects

Hashes the property names of the console object, then 100 ms later sends a GET request to https://ponos.zeronaught.com carrying the site key, the transaction token, and either that console hash or a SHA-256 digest of other signals' outputs. It reports the request's round trip: status, response body, network error, and send and load-end times. A provisional message is sent right after send() and replaced by the final one when the request finishes.

## Why

A real network round trip likely checks that the client can reach the vendor's server and ties the timing, response and signal digest to the transaction; the console hash likely detects devtools or patched console objects.

## Fields

This signal sends one of 2 message shapes.

### Message 1: provisional, sent immediately after the request is sent

| Field | Type | Meaning |
| --- | --- | --- |
| `cpHash` | number | 32-bit hash of the sorted property names of console. |
| `xhrStatus` | number | xhr.status right after send(), normally 0. |
| `xhrResponse` | string | Response text, empty at this point. |
| `signalIndexCount` | number | Always 0 here. |
| `hasNetworkError` | boolean (tag byte) | Tag value: 50 if the request's error handler fired, 94 otherwise (values per build). |
| `xhrSendTs` | number | Time in ms (Date) just before send(). |
| `xhrLoadEndTs` | number | -1, the request has not finished yet. |

### Message 2: final, sent when the request finishes (loadend)

| Field | Type | Meaning |
| --- | --- | --- |
| `cpHash` | number | 32-bit hash of the sorted property names of console. |
| `xhrStatus` | number | HTTP status of the response, or the status at the time of a network error (0). |
| `xhrResponse` | string | Response body text, or empty if the load did not succeed. |
| `signalIndexCount` | number | How many signal indices follow (signals whose outputs were digested into the request); 0 if no digest was made. |
| `signalIndex` | number, repeated (once per digested signal, in order) | Index of a signal whose output was included in the digest. |
| `hasNetworkError` | boolean (tag byte) | Tag value: 50 if the request's error handler fired, 94 otherwise (values per build). |
| `xhrSendTs` | number | Time in ms (Date) just before send(). |
| `xhrLoadEndTs` | number | Time in ms (Date) when the request finished or failed. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "cpHash": -930873437,
  "hasNetworkError": false,
  "signalIndex": [
    49,
    2,
    46,
    18,
    17,
    42,
    69,
    53,
    30,
    3,
    51,
    74,
    66,
    58,
    35,
    33,
    76,
    56,
    26,
    75,
    70,
    44,
    11,
    16,
    20,
    61,
    64,
    48,
    22
  ],
  "xhrLoadEndTs": 1794273727273,
  "xhrResponse": " 7JeluLeQw1PdTB+9n6ScfzhKETP7Mv0ghQgfW5+qbdH1e3dRVQiXcnO0z4nGP1Dr8FAR4wT8Qtgkpk2MmihT0havbdU81Syl4Pv7rxFfbs4nuaog1EOPhU1/lTAet+DryF/vv/xOYMrJSZ9oBEYftahSOXmLIhJAL/zk/As3lp58f9BiklKilT4770dZhU1peb60XkIPgUPRVcRD2R9rQ6diTk/CkgDeQBv2tQLfdUz1LMuIDEyCtlxdlmWSDhXHiiMmOC1OtnjAn0jfhfk2CwkgcwpunQPfl7RGEjfFFyiLOkiyZB72bTnn29LgEFXjOkr0jr10OJMNjVTbFLgoOUli9dy/UUaxbS/XOqbEal/vb0XfR5N4aDt35jONYrEkjvm+0th2UE6pdzyi3fMOid8Ky99ExU1TM7jkfgEbDUsZUcsLMPdXUiQPdKwVa+F4ZWn+qXrzMa4lX5A9xJHFvCXZu5WGrBihWQEL8ogA9F2uYv0441sor9ef+nwo8BJWMI0cMvwakrxT0I66y9CyxSprtFt4uyrkcMGvjTS870vPA8sUQDdHKyQFKLJAwQT9RbZgKPeCuevAKsRTvO7Ptls4sPkBbV3TvuPZDvh9r+LX7uc+Wm4WzZ2bB0idVm+gz5st5xsCh0aABdNa9N/IItuVEhDU5E4AiebylGAr55qW8ndublugsmlQ7cz+ngQVduWaKWwH+WcNEToCmdfwUtU+dQoLKg+j72VREWXYuyy1hZn9TuSQRbaodQ7kiTen4RbYeYpWkHpgcKRntwl2YjqO8olQmwPN2BFAzmbWDofeGiP1CNpyce6cHWWZdAQTbCn+tRxH6F+3yynBllZwfFNqug0cRtf8GORANE4S4QvyEgdUYWRAj5G92Du6f915pRDMk1+ODuL97n/vobsong==",
  "xhrSendTs": 1794273727104,
  "xhrStatus": 200
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.PonosXhrRoundTrip` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- It has 2 message shapes, so the entry carries `messages` (`when`, `mask`, `order`, `fields` each) instead of top-level `order`/`fields`.
- Repeated fields are `type: "array"` with an `item`.
