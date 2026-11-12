# WebGlRenderHash

[Signals index](README.md) · [data-shape example](webgl_render_hash.js) · [schema format](schema.md)

## What it collects

Draws one fixed coloured triangle with a minimal WebGL program on a 150x150 canvas, reads back a rectangle of pixels and sends a 32-bit hash of them. If any step of the WebGL pipeline fails, it sends a status code naming the first failed step instead.

## Why

The exact rendered pixels depend on the GPU, driver and rasteriser, so the hash fingerprints the graphics stack, and the failure step likely flags headless or software-only environments.

## Fields

This signal sends one of 2 message shapes.

### Message 1: a WebGL step failed

| Field | Type | Meaning |
| --- | --- | --- |
| `kind` | byte | Record-kind tag: a fixed value meaning "status record" (27 in the main build). |
| `status` | number | First failed step: 0 = no typed arrays, 1 = no WebGL context, 2 = vertex createShader returned null, 3 = vertex shader failed to compile, 4 = fragment createShader returned null, 5 = fragment shader failed to compile, 6 = createProgram returned null, 7 = link failed. |
| `digest` | null (tag byte) | Always a null marker (no hash in a status record). |

### Message 2: rendering succeeded

| Field | Type | Meaning |
| --- | --- | --- |
| `kind` | byte | Record-kind tag: a fixed value meaning "digest record" (57 in the main build). |
| `status` | null (tag byte) | Always a null marker (no status in a digest record). |
| `digest` | null (tag byte) | Signed 32-bit hash (h*31 + word) of the read-back pixel buffer, or a null marker if the hash came back null. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "digest": 1847286926,
  "kind": 70,
  "status": null
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.WebGlRenderHash` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- It has 2 message shapes, so the entry carries `messages` (`when`, `mask`, `order`, `fields` each) instead of top-level `order`/`fields`.
