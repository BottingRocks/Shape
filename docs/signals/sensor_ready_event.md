# SensorReadyEvent

[Signals index](README.md) · [data-shape example](sensor_ready_event.js) · [schema format](schema.md)

## What it collects

Sends no data. When the collector starts, this named slot completes at once without a value, so it is always reported as not collected. Its only effect is on the page: it sets a property on window to true and, on the next tick, dispatches a CustomEvent on window (in every build seen so far both are named f5cd836ea16e4a77). The report lists the property (windowFlag) and the event type (event) read from the build.

## Why

Likely a readiness hook for the protected site's own scripts: the page can check the window flag or listen for the event to know the sensor script has loaded and started collecting (the listener is not in the collector source).

## Fields

No data: the slot completes without ever collecting a value, so nothing of it reaches the payload but the not-collected marker.

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

No payload emitted.

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.SensorReadyEvent` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- No data: `type: "no_data"`, the slot's routing `name` (or `positional`), `fields: {}` and no `order` or `mask`; `effect` names what it does on the page in that build.
