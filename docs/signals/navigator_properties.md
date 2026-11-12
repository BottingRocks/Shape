# NavigatorProperties

[Signals index](README.md) · [data-shape example](navigator_properties.js) · [schema format](schema.md)

## What it collects

Copies a fixed list of basic browser properties and sends them all in one message at startup: window size and position values, screen size and colour depth, the navigator identity strings (userAgent, platform, vendor, appVersion and so on), CPU core count, touch points, doNotTrack, navigator.webdriver, isSecureContext, whether the location bar and toolbar are visible, and Opera's version() if it exists. Missing values are sent as a null marker. The four yes/no values are packed into one flags byte.

## Why

These are the core fingerprint values. A vendor likely cross-checks them against each other and against the HTTP headers (e.g. userAgent vs platform vs screen size), and navigator.webdriver = true or hidden toolbars directly point to automation or headless browsers.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `innerWidth` | number or null (tag byte) | window.innerWidth, viewport width in CSS pixels, or a null marker if missing. |
| `innerWidth.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `innerWidth.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `screenHeight` | number or null (tag byte) | screen.height in CSS pixels, or a null marker if missing. |
| `screenHeight.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `screenHeight.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `locationbarVisible` | null (tag byte) | window.locationbar.visible. When it is a boolean nothing is written here and its value goes into the flags byte; otherwise a null marker is written. |
| `locationbarVisible.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `locationbarVisible.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `outerWidth` | number or null (tag byte) | window.outerWidth, browser window width in pixels, or a null marker if missing. |
| `outerWidth.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `outerWidth.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `oscpu` | string or null (tag byte) | navigator.oscpu (Firefox only, e.g. "Windows NT 10.0; Win64; x64"), or a null marker if missing. |
| `oscpu.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `oscpu.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `availWidth` | number or null (tag byte) | screen.availWidth, screen width minus OS bars, or a null marker if missing. |
| `availWidth.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `availWidth.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `userAgent` | string or null (tag byte) | navigator.userAgent, or a null marker if missing. |
| `userAgent.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `userAgent.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `webdriver` | null (tag byte) | Whether navigator.webdriver === true (automation). When present nothing is written here and the value goes into the flags byte; one null marker if navigator has no webdriver property, another if it is present but null. |
| `webdriver.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `webdriver.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `innerHeight` | number or null (tag byte) | window.innerHeight, viewport height in CSS pixels, or a null marker if missing. |
| `innerHeight.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `innerHeight.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `colorDepth` | number or null (tag byte) | screen.colorDepth, usually 24 or 30, or a null marker if missing. |
| `colorDepth.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `colorDepth.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `appCodeName` | string or null (tag byte) | navigator.appCodeName, normally "Mozilla", or a null marker if missing. |
| `appCodeName.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `appCodeName.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `appVersion` | string or null (tag byte) | navigator.appVersion, the user agent without the "Mozilla/" prefix, or a null marker if missing. |
| `appVersion.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `appVersion.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `platform` | string or null (tag byte) | navigator.platform, e.g. "Win32", "MacIntel", "Linux x86_64", or a null marker if missing. |
| `platform.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `platform.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `productSub` | string or null (tag byte) | navigator.productSub, "20030107" in Chrome/Safari, "20100101" in Firefox, or a null marker if missing. |
| `productSub.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `productSub.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `pixelDepth` | number or null (tag byte) | screen.pixelDepth, usually equal to colorDepth, or a null marker if missing. |
| `pixelDepth.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `pixelDepth.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `cpuClass` | string or null (tag byte) | navigator.cpuClass (old Internet Explorer only), or a null marker if missing (the normal case). |
| `cpuClass.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `cpuClass.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `vendorSub` | string or null (tag byte) | navigator.vendorSub, normally empty, or a null marker if missing. |
| `vendorSub.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `vendorSub.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `isSecureContext` | null (tag byte) | window.isSecureContext (page loaded over HTTPS or localhost). When it is a boolean nothing is written here and its value goes into the flags byte; otherwise a null marker is written. |
| `isSecureContext.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `isSecureContext.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `vendor` | string or null (tag byte) | navigator.vendor, e.g. "Google Inc.", "Apple Computer, Inc." or empty in Firefox, or a null marker if missing. |
| `vendor.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `vendor.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `product` | string or null (tag byte) | navigator.product, normally "Gecko", or a null marker if missing. |
| `product.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `product.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `appName` | string or null (tag byte) | navigator.appName, normally "Netscape", or a null marker if missing. |
| `appName.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `appName.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `screenX` | number or null (tag byte) | window.screenX, window's left position on the screen, or a null marker if missing. |
| `screenX.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `screenX.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `toolbarVisible` | null (tag byte) | window.toolbar.visible. When it is a boolean nothing is written here and its value goes into the flags byte; otherwise a null marker is written. |
| `toolbarVisible.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `toolbarVisible.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `flags` | flags (byte) | Packed booleans, 1 = true: locationbar visible, navigator.webdriver true, isSecureContext, toolbar visible. A bit is 0 when its value was missing. |
| `maxTouchPoints` | number or null (tag byte) | navigator.maxTouchPoints, 0 on non-touch devices, often 5 or 10 on touch screens, or a null marker if missing. |
| `maxTouchPoints.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `maxTouchPoints.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `hardwareConcurrency` | number or null (tag byte) | navigator.hardwareConcurrency, number of logical CPU cores, or a null marker if missing. |
| `hardwareConcurrency.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `hardwareConcurrency.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `buildID` | string or null (tag byte) | navigator.buildID (Firefox only, usually "20181001000000"), or a null marker if missing. |
| `buildID.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `buildID.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `screenWidth` | number or null (tag byte) | screen.width in CSS pixels, or a null marker if missing. |
| `screenWidth.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `screenWidth.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `devicePixelRatio` | number or null (tag byte) | window.devicePixelRatio, e.g. 1, 1.25, 2 or 3. One null marker if the property is absent, another if it is present but null. |
| `devicePixelRatio.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `devicePixelRatio.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `doNotTrack` | string or null (tag byte) | navigator.doNotTrack, e.g. "1", "0" or "unspecified". One null marker if the property is absent, another if it is present but null. |
| `doNotTrack.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `doNotTrack.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `operaVersion` | string or null (tag byte) | Result of window.opera.version() (old Opera only), or a null marker if it does not exist (the normal case). |
| `operaVersion.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `operaVersion.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `outerHeight` | number or null (tag byte) | window.outerHeight, browser window height in pixels, or a null marker if missing. |
| `outerHeight.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `outerHeight.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `availHeight` | number or null (tag byte) | screen.availHeight, screen height minus OS bars, or a null marker if missing. |
| `availHeight.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `availHeight.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `screenY` | number or null (tag byte) | window.screenY, window's top position on the screen, or a null marker if missing. |
| `screenY.nullMarker` | null (tag byte) | First component of this field's null marker. |
| `screenY.nullFlag` | null (tag byte) | Second component of this field's null marker. |
| `nullMarker` | null (tag byte) | The alternative written in place of a property's value when it is missing, for the properties whose null-marker tag takes two bytes (tag 160 and above, about the last 5-6 null-capable properties of the build): this first byte is the tag. Properties with a smaller tag write a single null-marker byte, described with each property. |
| `nullFlag` | null (tag byte) | Second byte of a two-byte null marker, always 1, written right after nullMarker. |

Flag names of `flags` (card order; the bit of each flag is per build, see the schema's `bits`):

- `locationbarVisible`
- `webdriver`
- `isSecureContext`
- `toolbarVisible`

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "appCodeName": "Mozilla",
  "appName": "Netscape",
  "appVersion": "5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Safari/537.36",
  "availHeight": 1050,
  "availWidth": 1920,
  "buildID": null,
  "colorDepth": 24,
  "cpuClass": null,
  "devicePixelRatio": 2,
  "doNotTrack": null,
  "flags": {
    "isSecureContext": true,
    "locationbarVisible": true,
    "toolbarVisible": true,
    "webdriver": false
  },
  "hardwareConcurrency": 18,
  "innerHeight": 287,
  "innerWidth": 1920,
  "maxTouchPoints": 0,
  "operaVersion": null,
  "oscpu": null,
  "outerHeight": 1050,
  "outerWidth": 1920,
  "pixelDepth": 24,
  "platform": "MacIntel",
  "product": "Gecko",
  "productSub": "20030107",
  "screenHeight": 1080,
  "screenWidth": 1920,
  "screenX": 673,
  "screenY": -1050,
  "userAgent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Safari/537.36",
  "vendor": "Google Inc.",
  "vendorSub": ""
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.NavigatorProperties` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Flag bytes are `type: "flags"` with `bits` mapping each flag name above to its bit (0 = least significant) in this build. If the bits cannot all be named the field stays `type: "byte"` and is listed under `diagnostics.flags_fallback`.
