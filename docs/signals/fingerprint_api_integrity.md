# FingerprintApiIntegrity

[Signals index](README.md) · [data-shape example](fingerprint_api_integrity.js) · [schema format](schema.md)

## What it collects

Checks whether 15 browser functions that fingerprinting scripts commonly read (screen width/height, navigator userAgent/plugins/mimeTypes, createElement/createElementNS, canvas getContext/toDataURL/toBlob, 2D getImageData/fillText and the strokeStyle/fillStyle/font getters) are genuine native code. For each function it tests that it is a function, that its toString text looks like native code, that it has no own prototype, and that deleting the prototype does not throw; it also checks that the properties are not shadowed on the instance objects, that plugins/mimeTypes have a real type tag, and whether a clean iframe toString could be used. That gives 72 true/false checks packed 7 per byte into 11 bytes. It sends one message at init with only the flags, and a second at collection that re-checks everything with a clean iframe's toString, keeps a flag only if it passed both times, and adds a hash of each function's source text.

## Why

To detect anti-detect browsers, bot frameworks and privacy extensions that patch, wrap or proxy the very APIs used for fingerprinting, so spoofed fingerprint values can be distrusted (likely).

## Fields

This signal sends one of 2 message shapes.

### Message 1: init: sent right away; the 15 hash slots are always null markers.

| Field | Type | Meaning |
| --- | --- | --- |
| `integrityFlags1` | flags (byte) | 7 packed true/false checks: userAgent getter deleting prototype did not throw; createElementNS not shadowed on the document object; 2D fillText is a function; plugins getter toString looks native; plugins getter has no own prototype; 2D fillText deleting prototype did not throw; screen.width getter is a function. Each flag is 1 if the check passed at init. |
| `mimeTypesSourceHash` | null (tag byte) | Slot for the mimeTypes getter source-text hash; at init it is always a null marker (no hash sent). |
| `toDataURLSourceHash` | null (tag byte) | Slot for the canvas toDataURL source-text hash; at init it is always a null marker (no hash sent). |
| `getImageDataSourceHash` | null (tag byte) | Slot for the 2D getImageData source-text hash; at init it is always a null marker (no hash sent). |
| `integrityFlags2` | byte | 7 packed true/false checks: screen.width getter has no own prototype; canvas toBlob deleting prototype did not throw; width not shadowed on the screen object; createElement has no own prototype; createElement not shadowed on the document object; screen.width getter toString looks native; userAgent getter toString looks native. Each flag is 1 if the check passed at init. |
| `fillTextSourceHash` | null (tag byte) | Slot for the 2D fillText source-text hash; at init it is always a null marker (no hash sent). |
| `createElementSourceHash` | null (tag byte) | Slot for the createElement source-text hash; at init it is always a null marker (no hash sent). |
| `integrityFlags3` | byte | 7 packed true/false checks: clean iframe toString was used; plugins not shadowed on the navigator object; createElementNS has no own prototype; 2D fillStyle getter toString looks native; 2D fillStyle getter deleting prototype did not throw; canvas toDataURL toString looks native; canvas getContext toString looks native. Each flag is 1 if the check passed at init. |
| `userAgentSourceHash` | null (tag byte) | Slot for the userAgent getter source-text hash; at init it is always a null marker (no hash sent). |
| `fontSourceHash` | null (tag byte) | Slot for the 2D font getter source-text hash; at init it is always a null marker (no hash sent). |
| `integrityFlags4` | byte | 7 packed true/false checks: mimeTypes has a genuine object type tag; userAgent not shadowed on the navigator object; screen.height getter deleting prototype did not throw; 2D strokeStyle getter toString looks native; canvas getContext is a function; plugins getter is a function; canvas toBlob has no own prototype. Each flag is 1 if the check passed at init. |
| `toBlobSourceHash` | null (tag byte) | Slot for the canvas toBlob source-text hash; at init it is always a null marker (no hash sent). |
| `integrityFlags5` | byte | 7 packed true/false checks: canvas getContext has no own prototype; mimeTypes getter deleting prototype did not throw; screen.height getter toString looks native; userAgent getter is a function; 2D fillText has no own prototype; userAgent getter has no own prototype; plugins getter deleting prototype did not throw. Each flag is 1 if the check passed at init. |
| `pluginsSourceHash` | null (tag byte) | Slot for the plugins getter source-text hash; at init it is always a null marker (no hash sent). |
| `integrityFlags6` | byte | 7 packed true/false checks: canvas toDataURL has no own prototype; createElement is a function; height not shadowed on the screen object; 2D font getter has no own prototype; canvas toBlob is a function; 2D getImageData has no own prototype; 2D strokeStyle getter deleting prototype did not throw. Each flag is 1 if the check passed at init. |
| `strokeStyleSourceHash` | null (tag byte) | Slot for the 2D strokeStyle getter source-text hash; at init it is always a null marker (no hash sent). |
| `screenWidthSourceHash` | null (tag byte) | Slot for the screen.width getter source-text hash; at init it is always a null marker (no hash sent). |
| `integrityFlags7` | byte | 7 packed true/false checks: 2D font getter deleting prototype did not throw; createElement not overridden on Document.prototype; 2D font getter is a function; mimeTypes getter has no own prototype; screen.height getter is a function; 2D getImageData deleting prototype did not throw; createElementNS is a function. Each flag is 1 if the check passed at init. |
| `fillStyleSourceHash` | null (tag byte) | Slot for the 2D fillStyle getter source-text hash; at init it is always a null marker (no hash sent). |
| `integrityFlags8` | byte | 7 packed true/false checks: 2D font getter toString looks native; canvas toBlob toString looks native; createElement deleting prototype did not throw; mimeTypes getter is a function; 2D fillStyle getter is a function; mimeTypes getter toString looks native; 2D getImageData is a function. Each flag is 1 if the check passed at init. |
| `screenHeightSourceHash` | null (tag byte) | Slot for the screen.height getter source-text hash; at init it is always a null marker (no hash sent). |
| `createElementNSSourceHash` | null (tag byte) | Slot for the createElementNS source-text hash; at init it is always a null marker (no hash sent). |
| `integrityFlags9` | byte | 7 packed true/false checks: canvas toDataURL is a function; canvas getContext deleting prototype did not throw; mimeTypes not shadowed on the navigator object; plugins has a genuine object type tag; canvas toDataURL deleting prototype did not throw; screen.height getter has no own prototype; createElement toString looks native. Each flag is 1 if the check passed at init. |
| `integrityFlags10` | byte | 7 packed true/false checks: 2D getImageData toString looks native; 2D fillText toString looks native; screen.width getter deleting prototype did not throw; 2D strokeStyle getter is a function; createElementNS deleting prototype did not throw; 2D strokeStyle getter has no own prototype; 2D fillStyle getter has no own prototype. Each flag is 1 if the check passed at init. |
| `integrityFlags11` | byte | 2 packed true/false checks: createElementNS toString looks native; createElementNS not overridden on Document.prototype. Each flag is 1 if the check passed at init. |
| `getContextSourceHash` | null (tag byte) | Slot for the canvas getContext source-text hash; at init it is always a null marker (no hash sent). |

Flag names of `integrityFlags1` (card order; the bit of each flag is per build, see the schema's `bits`):

- `screenWidthIsFunction`
- `screenWidthLooksNative`
- `screenWidthNoOwnPrototype`
- `screenWidthPrototypeDeletable`
- `screenHeightIsFunction`
- `screenHeightLooksNative`
- `screenHeightNoOwnPrototype`
- `screenHeightPrototypeDeletable`
- `userAgentIsFunction`
- `userAgentLooksNative`
- `userAgentNoOwnPrototype`
- `userAgentPrototypeDeletable`
- `pluginsIsFunction`
- `pluginsLooksNative`
- `pluginsNoOwnPrototype`
- `pluginsPrototypeDeletable`
- `mimeTypesIsFunction`
- `mimeTypesLooksNative`
- `mimeTypesNoOwnPrototype`
- `mimeTypesPrototypeDeletable`
- `createElementIsFunction`
- `createElementLooksNative`
- `createElementNoOwnPrototype`
- `createElementPrototypeDeletable`
- `createElementNSIsFunction`
- `createElementNSLooksNative`
- `createElementNSNoOwnPrototype`
- `createElementNSPrototypeDeletable`
- `getContextIsFunction`
- `getContextLooksNative`
- `getContextNoOwnPrototype`
- `getContextPrototypeDeletable`
- `toDataURLIsFunction`
- `toDataURLLooksNative`
- `toDataURLNoOwnPrototype`
- `toDataURLPrototypeDeletable`
- `toBlobIsFunction`
- `toBlobLooksNative`
- `toBlobNoOwnPrototype`
- `toBlobPrototypeDeletable`
- `getImageDataIsFunction`
- `getImageDataLooksNative`
- `getImageDataNoOwnPrototype`
- `getImageDataPrototypeDeletable`
- `fillTextIsFunction`
- `fillTextLooksNative`
- `fillTextNoOwnPrototype`
- `fillTextPrototypeDeletable`
- `strokeStyleIsFunction`
- `strokeStyleLooksNative`
- `strokeStyleNoOwnPrototype`
- `strokeStylePrototypeDeletable`
- `fillStyleIsFunction`
- `fillStyleLooksNative`
- `fillStyleNoOwnPrototype`
- `fillStylePrototypeDeletable`
- `fontIsFunction`
- `fontLooksNative`
- `fontNoOwnPrototype`
- `fontPrototypeDeletable`
- `documentCreateElementNotShadowed`
- `documentCreateElementNSNotShadowed`
- `documentProtoCreateElementNotShadowed`
- `documentProtoCreateElementNSNotShadowed`
- `screenWidthNotShadowed`
- `screenHeightNotShadowed`
- `navigatorUserAgentNotShadowed`
- `navigatorPluginsNotShadowed`
- `navigatorMimeTypesNotShadowed`
- `pluginsTagGenuine`
- `mimeTypesTagGenuine`
- `iframeToStringUsed`

### Message 2: collection: sent after re-checking with a hidden iframe's clean toString; carries the source hashes. Not sent if the collection callback aborts (for example no 2D canvas with measureText).

| Field | Type | Meaning |
| --- | --- | --- |
| `integrityFlags1` | byte | 7 packed true/false checks: userAgent getter deleting prototype did not throw; createElementNS not shadowed on the document object; 2D fillText is a function; plugins getter toString looks native; plugins getter has no own prototype; 2D fillText deleting prototype did not throw; screen.width getter is a function. Each flag is 1 if the check passed both at init and now at collection. |
| `mimeTypesSourceHash` | number or null (tag byte) | Signed 32-bit hash of the mimeTypes getter's Function.prototype.toString text (read with a clean iframe's toString); 0 if it is not a function, or a null marker if the property descriptor is missing. |
| `toDataURLSourceHash` | number or null (tag byte) | Signed 32-bit hash of the canvas toDataURL's Function.prototype.toString text (read with a clean iframe's toString); 0 if it is not a function, or a null marker if the property descriptor is missing. |
| `getImageDataSourceHash` | number or null (tag byte) | Signed 32-bit hash of the 2D getImageData's Function.prototype.toString text (read with a clean iframe's toString); 0 if it is not a function, or a null marker if the property descriptor is missing. |
| `integrityFlags2` | byte | 7 packed true/false checks: screen.width getter has no own prototype; canvas toBlob deleting prototype did not throw; width not shadowed on the screen object; createElement has no own prototype; createElement not shadowed on the document object; screen.width getter toString looks native; userAgent getter toString looks native. Each flag is 1 if the check passed both at init and now at collection. |
| `fillTextSourceHash` | number or null (tag byte) | Signed 32-bit hash of the 2D fillText's Function.prototype.toString text (read with a clean iframe's toString); 0 if it is not a function, or a null marker if the property descriptor is missing. |
| `createElementSourceHash` | number or null (tag byte) | Signed 32-bit hash of the createElement's Function.prototype.toString text (read with a clean iframe's toString); 0 if it is not a function, or a null marker if the property descriptor is missing. |
| `integrityFlags3` | byte | 7 packed true/false checks: clean iframe toString was used; plugins not shadowed on the navigator object; createElementNS has no own prototype; 2D fillStyle getter toString looks native; 2D fillStyle getter deleting prototype did not throw; canvas toDataURL toString looks native; canvas getContext toString looks native. Each flag is 1 if the check passed both at init and now at collection. |
| `userAgentSourceHash` | number or null (tag byte) | Signed 32-bit hash of the userAgent getter's Function.prototype.toString text (read with a clean iframe's toString); 0 if it is not a function, or a null marker if the property descriptor is missing. |
| `fontSourceHash` | number or null (tag byte) | Signed 32-bit hash of the 2D font getter's Function.prototype.toString text (read with a clean iframe's toString); 0 if it is not a function, or a null marker if the property descriptor is missing. |
| `integrityFlags4` | byte | 7 packed true/false checks: mimeTypes has a genuine object type tag; userAgent not shadowed on the navigator object; screen.height getter deleting prototype did not throw; 2D strokeStyle getter toString looks native; canvas getContext is a function; plugins getter is a function; canvas toBlob has no own prototype. Each flag is 1 if the check passed both at init and now at collection. |
| `toBlobSourceHash` | number or null (tag byte) | Signed 32-bit hash of the canvas toBlob's Function.prototype.toString text (read with a clean iframe's toString); 0 if it is not a function, or a null marker if the property descriptor is missing. |
| `integrityFlags5` | byte | 7 packed true/false checks: canvas getContext has no own prototype; mimeTypes getter deleting prototype did not throw; screen.height getter toString looks native; userAgent getter is a function; 2D fillText has no own prototype; userAgent getter has no own prototype; plugins getter deleting prototype did not throw. Each flag is 1 if the check passed both at init and now at collection. |
| `pluginsSourceHash` | number or null (tag byte) | Signed 32-bit hash of the plugins getter's Function.prototype.toString text (read with a clean iframe's toString); 0 if it is not a function, or a null marker if the property descriptor is missing. |
| `integrityFlags6` | byte | 7 packed true/false checks: canvas toDataURL has no own prototype; createElement is a function; height not shadowed on the screen object; 2D font getter has no own prototype; canvas toBlob is a function; 2D getImageData has no own prototype; 2D strokeStyle getter deleting prototype did not throw. Each flag is 1 if the check passed both at init and now at collection. |
| `strokeStyleSourceHash` | number or null (tag byte) | Signed 32-bit hash of the 2D strokeStyle getter's Function.prototype.toString text (read with a clean iframe's toString); 0 if it is not a function, or a null marker if the property descriptor is missing. |
| `screenWidthSourceHash` | number or null (tag byte) | Signed 32-bit hash of the screen.width getter's Function.prototype.toString text (read with a clean iframe's toString); 0 if it is not a function, or a null marker if the property descriptor is missing. |
| `integrityFlags7` | byte | 7 packed true/false checks: 2D font getter deleting prototype did not throw; createElement not overridden on Document.prototype; 2D font getter is a function; mimeTypes getter has no own prototype; screen.height getter is a function; 2D getImageData deleting prototype did not throw; createElementNS is a function. Each flag is 1 if the check passed both at init and now at collection. |
| `fillStyleSourceHash` | number or null (tag byte) | Signed 32-bit hash of the 2D fillStyle getter's Function.prototype.toString text (read with a clean iframe's toString); 0 if it is not a function, or a null marker if the property descriptor is missing. |
| `integrityFlags8` | byte | 7 packed true/false checks: 2D font getter toString looks native; canvas toBlob toString looks native; createElement deleting prototype did not throw; mimeTypes getter is a function; 2D fillStyle getter is a function; mimeTypes getter toString looks native; 2D getImageData is a function. Each flag is 1 if the check passed both at init and now at collection. |
| `screenHeightSourceHash` | number or null (tag byte) | Signed 32-bit hash of the screen.height getter's Function.prototype.toString text (read with a clean iframe's toString); 0 if it is not a function, or a null marker if the property descriptor is missing. |
| `createElementNSSourceHash` | number or null (tag byte) | Signed 32-bit hash of the createElementNS's Function.prototype.toString text (read with a clean iframe's toString); 0 if it is not a function, or a null marker if the property descriptor is missing. |
| `integrityFlags9` | byte | 7 packed true/false checks: canvas toDataURL is a function; canvas getContext deleting prototype did not throw; mimeTypes not shadowed on the navigator object; plugins has a genuine object type tag; canvas toDataURL deleting prototype did not throw; screen.height getter has no own prototype; createElement toString looks native. Each flag is 1 if the check passed both at init and now at collection. |
| `integrityFlags10` | byte | 7 packed true/false checks: 2D getImageData toString looks native; 2D fillText toString looks native; screen.width getter deleting prototype did not throw; 2D strokeStyle getter is a function; createElementNS deleting prototype did not throw; 2D strokeStyle getter has no own prototype; 2D fillStyle getter has no own prototype. Each flag is 1 if the check passed both at init and now at collection. |
| `integrityFlags11` | byte | 2 packed true/false checks: createElementNS toString looks native; createElementNS not overridden on Document.prototype. Each flag is 1 if the check passed both at init and now at collection. |
| `getContextSourceHash` | number or null (tag byte) | Signed 32-bit hash of the canvas getContext's Function.prototype.toString text (read with a clean iframe's toString); 0 if it is not a function, or a null marker if the property descriptor is missing. |

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "createElementNSSourceHash": -1308566534,
  "createElementSourceHash": 77957379,
  "fillStyleSourceHash": 547743182,
  "fillTextSourceHash": 321282129,
  "fontSourceHash": 1428428518,
  "getContextSourceHash": -1752233893,
  "getImageDataSourceHash": 967708324,
  "integrityChecks": [
    {
      "createElementNSLooksNative": true,
      "documentProtoCreateElementNSNotShadowed": true,
      "fillStylePrototypeDeletable": true,
      "fillTextIsFunction": true,
      "getContextLooksNative": true,
      "strokeStyleLooksNative": true,
      "toDataURLPrototypeDeletable": true
    },
    {
      "createElementIsFunction": true,
      "createElementLooksNative": true,
      "createElementNoOwnPrototype": true,
      "documentCreateElementNotShadowed": true,
      "getContextIsFunction": true,
      "navigatorUserAgentNotShadowed": true,
      "screenWidthIsFunction": true
    },
    {
      "createElementNSIsFunction": true,
      "createElementNSNoOwnPrototype": true,
      "fillStyleNoOwnPrototype": true,
      "screenHeightPrototypeDeletable": true,
      "screenWidthLooksNative": true,
      "toBlobLooksNative": true,
      "toDataURLIsFunction": true
    },
    {
      "createElementNSPrototypeDeletable": true,
      "documentCreateElementNSNotShadowed": true,
      "fillTextPrototypeDeletable": true,
      "mimeTypesNoOwnPrototype": true,
      "screenWidthPrototypeDeletable": true,
      "toBlobIsFunction": true,
      "userAgentNoOwnPrototype": true
    },
    {
      "getContextNoOwnPrototype": true,
      "getImageDataIsFunction": true,
      "getImageDataLooksNative": true,
      "mimeTypesPrototypeDeletable": true,
      "mimeTypesTagGenuine": true,
      "screenHeightNoOwnPrototype": true,
      "userAgentIsFunction": true
    },
    {
      "fillStyleIsFunction": true,
      "fontPrototypeDeletable": true,
      "screenHeightIsFunction": true,
      "screenWidthNoOwnPrototype": true,
      "strokeStylePrototypeDeletable": true,
      "toDataURLLooksNative": true,
      "userAgentPrototypeDeletable": true
    },
    {
      "documentProtoCreateElementNotShadowed": true,
      "fillTextNoOwnPrototype": true,
      "fontNoOwnPrototype": true,
      "mimeTypesLooksNative": true,
      "pluginsLooksNative": true,
      "strokeStyleNoOwnPrototype": true,
      "userAgentLooksNative": true
    },
    {
      "fillStyleLooksNative": true,
      "fontLooksNative": true,
      "navigatorPluginsNotShadowed": true,
      "pluginsPrototypeDeletable": true,
      "pluginsTagGenuine": true,
      "screenHeightNotShadowed": true,
      "toDataURLNoOwnPrototype": true
    },
    {
      "fontIsFunction": true,
      "getContextPrototypeDeletable": true,
      "iframeToStringUsed": true,
      "screenHeightLooksNative": true,
      "screenWidthNotShadowed": true,
      "strokeStyleIsFunction": true,
      "toBlobPrototypeDeletable": true
    },
    {
      "getImageDataNoOwnPrototype": true,
      "getImageDataPrototypeDeletable": true,
      "mimeTypesIsFunction": true,
      "navigatorMimeTypesNotShadowed": true,
      "pluginsIsFunction": true,
      "pluginsNoOwnPrototype": true,
      "toBlobNoOwnPrototype": true
    },
    {
      "createElementPrototypeDeletable": true,
      "fillTextLooksNative": true
    }
  ],
  "mimeTypesSourceHash": 1867944940,
  "pluginsSourceHash": -40820508,
  "screenHeightSourceHash": 792389511,
  "screenWidthSourceHash": 627726960,
  "strokeStyleSourceHash": 647317288,
  "toBlobSourceHash": -557684352,
  "toDataURLSourceHash": 1918935229,
  "userAgentSourceHash": -1733642987
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.FingerprintApiIntegrity` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- It has 2 message shapes, so the entry carries `messages` (`when`, `mask`, `order`, `fields` each) instead of top-level `order`/`fields`.
- Flag bytes are `type: "flags"` with `bits` mapping each flag name above to its bit (0 = least significant) in this build. If the bits cannot all be named the field stays `type: "byte"` and is listed under `diagnostics.flags_fallback`.
- Fresh schemas/readable payloads group `integrityFlags1` through `integrityFlags11` as `integrityChecks[0..10]`. This is a heterogeneous fixed array of named boolean objects; each index retains its original byte's bit map. Physical field names above identify source writes, not transmitted JSON keys.
