# HostEnvironment

[Signals index](README.md) · [data-shape example](host_environment.js) · [schema format](schema.md)

## What it collects

If window.external has a getHostEnvironmentValue method (a non-standard host API of legacy Microsoft Edge / Windows web hosts), calls it once with 16 keys: browser-name, browser-platform-version, browser-version, device-sku, device-dpi, device-logical-processors, device-max-touch-points, device-pixel-height, device-pixel-width, device-role, os-architecture, os-build, os-language, os-manufacturer, os-mode, os-sku. It sends the returned text unchanged, or the error message if the call throws. On browsers without that method nothing is sent.

## Why

Likely a detector for legacy Edge and Windows-hosted web views, and where available a rich device/OS fingerprint (OS build, device model, screen, CPU count) that is hard to fake consistently.

## Fields

This signal sends one of 2 message shapes.

### Message 1: getHostEnvironmentValue returned a value

| Field | Type | Meaning |
| --- | --- | --- |
| `hostEnvironmentValue` | string | The value returned by getHostEnvironmentValue for the 16 keys, as-is (normally a JSON-formatted string). |
| `errorMessage` | null (tag byte) | Always a null marker in this message (no error). |

### Message 2: the call (or writing its result) threw

| Field | Type | Meaning |
| --- | --- | --- |
| `hostEnvironmentValue` | null (tag byte) | Always a null marker in this message (no value). |
| `errorMessage` | string | The exception's message, or 'No message' if it had none. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

No decoded sample available.

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.HostEnvironment` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- It has 2 message shapes, so the entry carries `messages` (`when`, `mask`, `order`, `fields` each) instead of top-level `order`/`fields`.
