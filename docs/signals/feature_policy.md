# FeaturePolicy

[Signals index](README.md) · [data-shape example](feature_policy.js) · [schema format](schema.md)

## What it collects

Reads document.featurePolicy (Chromium only). It sends the number of features the browser knows, the number allowed on this page, a hash of both feature lists, and the first allowlist entry (an origin such as '*' or 'https://...') of one randomly chosen allowed feature. If there is no featurePolicy object, nothing is sent.

## Why

The feature lists differ by browser, version and embedding context, so they help verify the claimed browser and detect engines or headless builds that fake being Chrome (likely).

## Fields

This signal sends one of 2 message shapes.

### Message 1: family A (puma builds, including the main build): the feature is picked with Math.random.

| Field | Type | Meaning |
| --- | --- | --- |
| `allowedFeatureCount` | number | Number of features allowedFeatures() returns (0 if the method is missing). |
| `featureListHash` | number | Signed 32-bit hash of features() joined with '\|', then '\|\|', then allowedFeatures() joined with '\|'. |
| `featureCount` | number | Number of features features() returns (0 if the method is missing). |
| `allowlistEntry` | string or null (tag byte) | First allowlist entry (origin string) for one randomly picked allowed feature, or a null marker if there are no allowed features or no entry. |

### Message 2: family B (all other vendors): the feature is picked from a shared random pool, and the pool value used is also sent.

| Field | Type | Meaning |
| --- | --- | --- |
| `allowedFeatureCount` | number | Number of features allowedFeatures() returns (0 if the method is missing). |
| `featureListHash` | number | Signed 32-bit hash of features() joined with '\|', then '\|\|', then allowedFeatures() joined with '\|'. |
| `featureCount` | number | Number of features features() returns (0 if the method is missing). |
| `allowlistEntry` | string or null (tag byte) | First allowlist entry (origin string) for the picked allowed feature, or a null marker if there are no allowed features or no entry. |
| `samplingSeed` | number | Random fraction between 0 and 1 that chose the feature, so the receiver can tell which feature the entry belongs to (likely). |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "allowedFeatureCount": 76,
  "allowlistEntry": "https://www.target.com",
  "featureCount": 77,
  "featureListHash": -667729372,
  "samplingSeed": 0.5477980646993165
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.FeaturePolicy` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- It has 2 message shapes, so the entry carries `messages` (`when`, `mask`, `order`, `fields` each) instead of top-level `order`/`fields`.
