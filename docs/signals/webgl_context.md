# WebGlContext

[Signals index](README.md) · [data-shape example](webgl_context.js) · [schema format](schema.md)

## What it collects

Creates a WebGL context (in an OffscreenCanvas worker when possible, otherwise on a page canvas) and reads its static capabilities: 20 getParameter values, the unmasked GPU vendor and renderer, antialiasing, max anisotropy, line/point/viewport ranges, 12 shader precision formats, the extension list and a hash of the context's property names. It never draws. The worker path sends up to 8 progressively filled records as replies arrive. A missing value is replaced by a null marker; if a whole group (all parameters, the getParameter set, or the debug-info pair) is missing, one null marker replaces the whole group.

## Why

The GPU renderer string and capability limits are among the strongest hardware fingerprints, and mismatches between them (or software renderers like SwiftShader) likely reveal virtual machines, headless browsers and spoofed GPUs.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `paramsMissing` | null (tag byte) | Null marker sent in place of every parameter field when the whole parameter object is missing; nothing is sent here when it is present. |
| `paramsMissing.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `paramsMissing.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `getParameterMissing` | null (tag byte) | Null marker sent in place of the getParameter values (version, renderer, vendor, bit depths and limits) when that set is missing or null; nothing is sent here when it is present. |
| `getParameterMissing.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `getParameterMissing.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `debugInfoMissing` | null (tag byte) | Null marker sent in place of unmaskedVendor and unmaskedRenderer when the debug-info pair is missing or null; nothing is sent here when it is present. |
| `debugInfoMissing.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `debugInfoMissing.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `version` | string or null (tag byte) | getParameter(VERSION), e.g. "WebGL 1.0 (OpenGL ES 2.0 Chromium)". Null marker if missing. |
| `version.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `version.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `renderer` | string or null (tag byte) | getParameter(RENDERER) (masked renderer, usually "WebKit WebGL"). Null marker if missing. |
| `renderer.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `renderer.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `alphaBits` | number or null (tag byte) | getParameter(ALPHA_BITS), usually 8. Null marker if missing. |
| `alphaBits.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `alphaBits.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `greenBits` | number or null (tag byte) | getParameter(GREEN_BITS), usually 8. Null marker if missing. |
| `greenBits.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `greenBits.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `maxRenderbufferSize` | number or null (tag byte) | getParameter(MAX_RENDERBUFFER_SIZE), e.g. 16384. Null marker if missing. |
| `maxRenderbufferSize.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `maxRenderbufferSize.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `maxCubeMapTextureSize` | number or null (tag byte) | getParameter(MAX_CUBE_MAP_TEXTURE_SIZE), e.g. 16384. Null marker if missing. |
| `maxCubeMapTextureSize.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `maxCubeMapTextureSize.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `stencilBits` | number or null (tag byte) | getParameter(STENCIL_BITS), usually 0 or 8. Null marker if missing. |
| `stencilBits.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `stencilBits.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `shadingLanguageVersion` | string or null (tag byte) | getParameter(SHADING_LANGUAGE_VERSION), e.g. "WebGL GLSL ES 1.0 (...)". Null marker if missing. |
| `shadingLanguageVersion.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `shadingLanguageVersion.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `maxTextureSize` | number or null (tag byte) | getParameter(MAX_TEXTURE_SIZE), e.g. 16384. Null marker if missing. |
| `maxTextureSize.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `maxTextureSize.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `contextProperties` | number or null (tag byte) | 32-bit hash of the WebGL context object's property names joined with '\|'; -1 placeholder before the worker reports it. Null marker if missing. |
| `contextProperties.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `contextProperties.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `unmaskedRenderer` | string or null (tag byte) | WEBGL_debug_renderer_info UNMASKED_RENDERER, the real GPU string, e.g. "ANGLE (NVIDIA, ...)"; null marker if the extension is unavailable. |
| `unmaskedRenderer.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `unmaskedRenderer.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `supportedExtensions` | number or null (tag byte) | Number of supported WebGL extensions that follow, or a null marker if missing. |
| `supportedExtensions.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `supportedExtensions.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `extension` | string, repeated (once per supported extension) | One extension name, e.g. "OES_texture_float". |
| `vendor` | string or null (tag byte) | getParameter(VENDOR) (masked vendor, usually "WebKit"). Null marker if missing. |
| `vendor.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `vendor.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `maxVertexUniformVectors` | number or null (tag byte) | getParameter(MAX_VERTEX_UNIFORM_VECTORS), e.g. 4096. Null marker if missing. |
| `maxVertexUniformVectors.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `maxVertexUniformVectors.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `antialiasMissing` | null (tag byte) | Null marker sent when the antialias attribute is missing or null (the flags byte then has antialias 0); nothing is sent here when it is present. |
| `antialiasMissing.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `antialiasMissing.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `flags` | flags (byte) | Packed flags: antialias (context was created with antialiasing), collectedUsingWebWorker (values came from an OffscreenCanvas worker rather than the page). |
| `dimensions` | number or null (tag byte) | Number of range strings that follow (3), or a null marker if missing. |
| `dimensions.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `dimensions.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `dimension` | string, repeated (once per range (3 entries)) | ALIASED_LINE_WIDTH_RANGE, ALIASED_POINT_SIZE_RANGE and MAX_VIEWPORT_DIMS, each array joined with no separator (e.g. "11", "11024", "3276832768"); empty string if not an array. |
| `maxAnisotropy` | number or null (tag byte) | MAX_TEXTURE_MAX_ANISOTROPY_EXT (e.g. 16), -1 if no anisotropic-filtering extension (NaN on the worker path in that case), or a null marker if missing. |
| `maxAnisotropy.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `maxAnisotropy.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `maxTextureImageUnits` | number or null (tag byte) | getParameter(MAX_TEXTURE_IMAGE_UNITS), e.g. 16. Null marker if missing. |
| `maxTextureImageUnits.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `maxTextureImageUnits.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `unmaskedVendor` | string or null (tag byte) | WEBGL_debug_renderer_info UNMASKED_VENDOR, e.g. "Google Inc. (NVIDIA)"; null marker if unavailable. |
| `unmaskedVendor.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `unmaskedVendor.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `depthBits` | number or null (tag byte) | getParameter(DEPTH_BITS), usually 24. Null marker if missing. |
| `depthBits.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `depthBits.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `blueBits` | number or null (tag byte) | getParameter(BLUE_BITS), usually 8. Null marker if missing. |
| `blueBits.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `blueBits.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `maxVaryingVectors` | number or null (tag byte) | getParameter(MAX_VARYING_VECTORS), e.g. 30. Null marker if missing. |
| `maxVaryingVectors.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `maxVaryingVectors.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `maxVertexTextureImageUnits` | number or null (tag byte) | getParameter(MAX_VERTEX_TEXTURE_IMAGE_UNITS), e.g. 16. Null marker if missing. |
| `maxVertexTextureImageUnits.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `maxVertexTextureImageUnits.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `redBits` | number or null (tag byte) | getParameter(RED_BITS), usually 8. Null marker if missing. |
| `redBits.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `redBits.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `shaderPrecisions` | number or null (tag byte) | Number of shader precision strings that follow (12), or a null marker if missing or getShaderPrecisionFormat is unavailable. |
| `shaderPrecisions.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `shaderPrecisions.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `shaderPrecision` | string, repeated (once per shader/precision pair (12 entries)) | precision + rangeMin + rangeMax concatenated (e.g. "23127127"), empty if null. The 12 entries are vertex then fragment shader, each for HIGH/MEDIUM/LOW_FLOAT, then vertex then fragment for HIGH/MEDIUM/LOW_INT. |
| `maxVertexAttribs` | number or null (tag byte) | getParameter(MAX_VERTEX_ATTRIBS), e.g. 16. Null marker if missing. |
| `maxVertexAttribs.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `maxVertexAttribs.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `maxFragmentUniformVectors` | number or null (tag byte) | getParameter(MAX_FRAGMENT_UNIFORM_VECTORS), e.g. 1024. Null marker if missing. |
| `maxFragmentUniformVectors.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `maxFragmentUniformVectors.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `maxCombinedTextureImageUnits` | number or null (tag byte) | getParameter(MAX_COMBINED_TEXTURE_IMAGE_UNITS), e.g. 32. Null marker if missing. |
| `maxCombinedTextureImageUnits.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `maxCombinedTextureImageUnits.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `nullMarker` | null (tag byte) | First byte of a two-byte null marker, sent in place of a missing value for the later fields of the record (marker numbers 32 and up). |
| `nullFlag` | null (tag byte) | Second byte of a two-byte null marker (the high part of the marker number, 1 for these fields). |

Flag names of `flags` (card order; the bit of each flag is per build, see the schema's `bits`):

- `antialias`
- `collectedUsingWebWorker`

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "alphaBits": 8,
  "blueBits": 8,
  "contextProperties": 747546536,
  "depthBits": 24,
  "dimensions": [
    "11",
    "1511",
    "1638416384"
  ],
  "flags": {
    "antialias": true,
    "collectedUsingWebWorker": true
  },
  "greenBits": 8,
  "maxAnisotropy": 16,
  "maxCombinedTextureImageUnits": 32,
  "maxCubeMapTextureSize": 16384,
  "maxFragmentUniformVectors": 1024,
  "maxRenderbufferSize": 16384,
  "maxTextureImageUnits": 16,
  "maxTextureSize": 16384,
  "maxVaryingVectors": 30,
  "maxVertexAttribs": 16,
  "maxVertexTextureImageUnits": 16,
  "maxVertexUniformVectors": 1024,
  "redBits": 8,
  "renderer": "WebKit WebGL",
  "shaderPrecisions": [
    "23127127",
    "23127127",
    "23127127",
    "23127127",
    "23127127",
    "23127127",
    "03130",
    "03130",
    "03130",
    "03130",
    "03130",
    "03130"
  ],
  "shadingLanguageVersion": "WebGL GLSL ES 1.0 (OpenGL ES GLSL ES 1.0 Chromium)",
  "stencilBits": 0,
  "supportedExtensions": [
    "ANGLE_instanced_arrays",
    "EXT_blend_minmax",
    "EXT_clip_control",
    "EXT_color_buffer_half_float",
    "EXT_depth_clamp",
    "EXT_disjoint_timer_query",
    "EXT_float_blend",
    "EXT_frag_depth",
    "EXT_polygon_offset_clamp",
    "EXT_shader_texture_lod",
    "EXT_texture_compression_bptc",
    "EXT_texture_compression_rgtc",
    "EXT_texture_filter_anisotropic",
    "EXT_texture_mirror_clamp_to_edge",
    "EXT_sRGB",
    "KHR_parallel_shader_compile",
    "OES_element_index_uint",
    "OES_fbo_render_mipmap",
    "OES_standard_derivatives",
    "OES_texture_float",
    "OES_texture_float_linear",
    "OES_texture_half_float",
    "OES_texture_half_float_linear",
    "OES_vertex_array_object",
    "WEBGL_blend_func_extended",
    "WEBGL_color_buffer_float",
    "WEBGL_compressed_texture_astc",
    "WEBGL_compressed_texture_etc",
    "WEBGL_compressed_texture_etc1",
    "WEBGL_compressed_texture_pvrtc",
    "WEBGL_compressed_texture_s3tc",
    "WEBGL_compressed_texture_s3tc_srgb",
    "WEBGL_debug_renderer_info",
    "WEBGL_debug_shaders",
    "WEBGL_depth_texture",
    "WEBGL_draw_buffers",
    "WEBGL_lose_context",
    "WEBGL_multi_draw",
    "WEBGL_polygon_mode"
  ],
  "unmaskedRenderer": "ANGLE (Apple, ANGLE Metal Renderer: Apple M5 Pro, Unspecified Version)",
  "unmaskedVendor": "Google Inc. (Apple)",
  "vendor": "WebKit",
  "version": "WebGL 1.0 (OpenGL ES 2.0 Chromium)"
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.WebGlContext` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Repeated fields are `type: "array"` with an `item`.
- Flag bytes are `type: "flags"` with `bits` mapping each flag name above to its bit (0 = least significant) in this build. If the bits cannot all be named the field stays `type: "byte"` and is listed under `diagnostics.flags_fallback`.
