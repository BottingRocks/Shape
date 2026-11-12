# GlobalPropertyChecks

[Signals index](README.md) · [data-shape example](global_property_checks.js) · [schema format](schema.md)

## What it collects

Runs 59 'name in object' presence tests against window, document, document.body, navigator, window.crypto and window.external, looking for automation markers (Selenium, ChromeDriver, PhantomJS, WebDriver, Sequentum), old IE / Gecko / WebKit APIs and capability APIs. Inherited properties count and no getters run; if document.body is missing, its tests are 0. The 59 yes/no answers are packed 7 per byte into nine bytes, presenceBits1 to presenceBits9, written in that order.

## Why

Likely to catch automation and headless tools directly through their leftover globals, and to check that the claimed browser engine matches which legacy and vendor APIs are present.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `presenceBits1` | flags (byte) | Byte 1 of 9 (wire byte 4 after the 4-byte seed). Carries 7 presence tests in bits 0-6; bit 7 is always 0. One bit per test: 1 if the property name is present on its object (own or inherited), 0 if not or if the object is missing. Which tests this byte packs, and in which bits, varies per build; the 59 tests are listed in the flags of presenceBits1. |
| `presenceBits2` | byte | Byte 2 of 9 (wire byte 5 after the 4-byte seed). Carries 7 presence tests in bits 0-6; bit 7 is always 0. One bit per test: 1 if the property name is present on its object (own or inherited), 0 if not or if the object is missing. Which tests this byte packs, and in which bits, varies per build; the 59 tests are listed in the flags of presenceBits1. |
| `presenceBits3` | byte | Byte 3 of 9 (wire byte 6 after the 4-byte seed). Carries 7 presence tests in bits 0-6; bit 7 is always 0. One bit per test: 1 if the property name is present on its object (own or inherited), 0 if not or if the object is missing. Which tests this byte packs, and in which bits, varies per build; the 59 tests are listed in the flags of presenceBits1. |
| `presenceBits4` | byte | Byte 4 of 9 (wire byte 7 after the 4-byte seed). Carries 7 presence tests in bits 0-6; bit 7 is always 0. One bit per test: 1 if the property name is present on its object (own or inherited), 0 if not or if the object is missing. Which tests this byte packs, and in which bits, varies per build; the 59 tests are listed in the flags of presenceBits1. |
| `presenceBits5` | byte | Byte 5 of 9 (wire byte 8 after the 4-byte seed). Carries 7 presence tests in bits 0-6; bit 7 is always 0. One bit per test: 1 if the property name is present on its object (own or inherited), 0 if not or if the object is missing. Which tests this byte packs, and in which bits, varies per build; the 59 tests are listed in the flags of presenceBits1. |
| `presenceBits6` | byte | Byte 6 of 9 (wire byte 9 after the 4-byte seed). Carries 7 presence tests in bits 0-6; bit 7 is always 0. One bit per test: 1 if the property name is present on its object (own or inherited), 0 if not or if the object is missing. Which tests this byte packs, and in which bits, varies per build; the 59 tests are listed in the flags of presenceBits1. |
| `presenceBits7` | byte | Byte 7 of 9 (wire byte 10 after the 4-byte seed). Carries 7 presence tests in bits 0-6; bit 7 is always 0. One bit per test: 1 if the property name is present on its object (own or inherited), 0 if not or if the object is missing. Which tests this byte packs, and in which bits, varies per build; the 59 tests are listed in the flags of presenceBits1. |
| `presenceBits8` | byte | Byte 8 of 9 (wire byte 11 after the 4-byte seed). Carries 7 presence tests in bits 0-6; bit 7 is always 0. One bit per test: 1 if the property name is present on its object (own or inherited), 0 if not or if the object is missing. Which tests this byte packs, and in which bits, varies per build; the 59 tests are listed in the flags of presenceBits1. |
| `presenceBits9` | byte | Byte 9 of 9, the last (wire byte 12). Carries the remaining 3 presence tests in bits 0-2; bits 3-7 are always 0. One bit per test: 1 if the property name is present on its object (own or inherited), 0 if not or if the object is missing. Which tests this byte packs, and in which bits, varies per build; the 59 tests are listed in the flags of presenceBits1. |

Flag names of `presenceBits1` (card order; the bit of each flag is per build, see the schema's `bits`):

- `window.ActiveXObject`
- `window.ApplePaySession`
- `window.File`
- `window.MutationObserver`
- `window.Notification`
- `window.SharedWorker`
- `window.TouchEvent`
- `window.XDomainRequest`
- `window._phantom`
- `window.attachEvent`
- `window.callPhantom`
- `window.createPopup`
- `window.detachEvent`
- `window.event`
- `window.external`
- `window.fireEvent`
- `window.frameElement`
- `window.globalStorage`
- `window.localStorage`
- `window.mozRTCPeerConnection`
- `window.mozRequestAnimationFrame`
- `window.phantom`
- `window.postMessage`
- `window.PushManager`
- `window.registerProtocolHandler`
- `window.requestAnimationFrame`
- `window.sessionStorage`
- `window.sidebar`
- `window.webkitRequestAnimationFrame`
- `window.webkitResolveLocalFileSystemURL`
- `window.webkitRTCPeerConnection`
- `window.BluetoothUUID`
- `window.netscape`
- `window.__fxdriver_unwrapped`
- `window._Selenium_IDE_Recorder`
- `document._Selenium_IDE_Recorder`
- `document.all`
- `document.characterSet`
- `document.charset`
- `document.compatMode`
- `document.documentMode`
- `document.images`
- `document.layers`
- `document.$cdc_asdjflasutopfhvcZLmcfl_`
- `document.__fxdriver_unwrapped`
- `document.__webdriver_script_fn`
- `body.contextMenu`
- `body.innerText`
- `body.mozRequestFullScreen`
- `body.requestFullScreen`
- `body.webkitRequestFullScreen`
- `navigator.vibrate`
- `navigator.webdriver`
- `navigator.credentials`
- `navigator.storage`
- `navigator.requestMediaKeySystemAccess`
- `navigator.bluetooth`
- `crypto.subtle`
- `external.Sequentum`

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "presenceBits": [
    {
      "body.contextMenu": false,
      "body.webkitRequestFullScreen": true,
      "navigator.bluetooth": true,
      "navigator.credentials": true,
      "navigator.vibrate": true,
      "window.ApplePaySession": false,
      "window.event": true
    },
    {
      "document.characterSet": true,
      "document.charset": true,
      "document.compatMode": true,
      "window.attachEvent": false,
      "window.fireEvent": false,
      "window.frameElement": true,
      "window.netscape": false
    },
    {
      "body.requestFullScreen": false,
      "document._Selenium_IDE_Recorder": false,
      "window.Notification": true,
      "window.__fxdriver_unwrapped": false,
      "window._phantom": false,
      "window.postMessage": true,
      "window.webkitResolveLocalFileSystemURL": true
    },
    {
      "crypto.subtle": true,
      "document.__webdriver_script_fn": false,
      "document.all": true,
      "document.images": true,
      "window.XDomainRequest": false,
      "window.callPhantom": false,
      "window.sessionStorage": true
    },
    {
      "document.layers": false,
      "external.Sequentum": false,
      "navigator.webdriver": true,
      "window.MutationObserver": true,
      "window.TouchEvent": true,
      "window.mozRTCPeerConnection": false,
      "window.registerProtocolHandler": false
    },
    {
      "body.innerText": true,
      "body.mozRequestFullScreen": false,
      "document.$cdc_asdjflasutopfhvcZLmcfl_": false,
      "navigator.storage": true,
      "window.phantom": false,
      "window.sidebar": false,
      "window.webkitRequestAnimationFrame": true
    },
    {
      "document.documentMode": false,
      "window.ActiveXObject": false,
      "window.BluetoothUUID": true,
      "window.File": true,
      "window.createPopup": false,
      "window.localStorage": true,
      "window.webkitRTCPeerConnection": true
    },
    {
      "document.__fxdriver_unwrapped": false,
      "window.PushManager": true,
      "window._Selenium_IDE_Recorder": false,
      "window.detachEvent": false,
      "window.external": true,
      "window.globalStorage": false,
      "window.mozRequestAnimationFrame": false
    },
    {
      "navigator.requestMediaKeySystemAccess": true,
      "window.SharedWorker": true,
      "window.requestAnimationFrame": true
    }
  ]
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.GlobalPropertyChecks` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Flag bytes are `type: "flags"` with `bits` mapping each flag name above to its bit (0 = least significant) in this build. If the bits cannot all be named the field stays `type: "byte"` and is listed under `diagnostics.flags_fallback`.
- Fresh schemas/readable payloads group `presenceBits1` through `presenceBits9` as `presenceBits[0..8]`. This is a heterogeneous fixed array of named boolean objects; each index retains its original byte's bit map. Physical field names above identify source writes, not transmitted JSON keys.
