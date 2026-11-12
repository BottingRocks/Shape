# GrecaptchaAccessorHookFlags

[Signals index](README.md) · [data-shape example](grecaptcha_accessor_hook_flags.js) · [schema format](schema.md)

## What it collects

Walks window and its prototype chain to find where the 'grecaptcha' property (Google reCAPTCHA) is defined, without triggering any getter. At that level it checks whether grecaptcha is a getter/setter accessor rather than a plain value, and whether the accessor's real source text contains 'BrowserAutomationStudio'. It also records whether window has its own FeaturePolicy, LayoutShiftAttribution and CompressionStream properties. Runs once at startup; all 11 answers are yes/no flags packed into two bytes.

## Why

Browser Automation Studio and similar bot tools hook grecaptcha with accessors to solve or intercept CAPTCHAs, so these flags likely expose that tool directly; the three feature bits are likely browser family/version hints.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `hookFlagsA` | flags (byte) | Seven flags: featurePolicy (window has own FeaturePolicy), getterHasBasMarker (getter source contains 'BrowserAutomationStudio'), layoutShiftAttribution (own LayoutShiftAttribution), compressionStream (own CompressionStream), getterHasToString (getter's toString is a function), descriptorPresent (a property descriptor was found), setterHasToString (setter's toString is a function). Bit 7 is 0. |
| `hookFlagsB` | flags (byte) | Four flags: setterHasBasMarker (setter source contains 'BrowserAutomationStudio'), setterIsFunction (grecaptcha has a setter), ownFound (grecaptcha found on window or one of its prototypes), getterIsFunction (grecaptcha has a getter). Bits 4-7 are 0. No reCAPTCHA: all 0; normal reCAPTCHA: only ownFound (and descriptorPresent) set. |

Flag names of `hookFlagsA` (card order; the bit of each flag is per build, see the schema's `bits`):

- `featurePolicy`
- `getterHasBasMarker`
- `layoutShiftAttribution`
- `compressionStream`
- `getterHasToString`
- `descriptorPresent`
- `setterHasToString`

Flag names of `hookFlagsB` (card order; the bit of each flag is per build, see the schema's `bits`):

- `setterHasBasMarker`
- `setterIsFunction`
- `ownFound`
- `getterIsFunction`

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "hookChecks": [
    {
      "compressionStream": true,
      "descriptorPresent": false,
      "getterHasBasMarker": false,
      "getterIsFunction": false,
      "layoutShiftAttribution": true,
      "ownFound": false,
      "setterHasToString": false
    },
    {
      "featurePolicy": true,
      "getterHasToString": false,
      "setterHasBasMarker": false,
      "setterIsFunction": false
    }
  ]
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.GrecaptchaAccessorHookFlags` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Flag bytes are `type: "flags"` with `bits` mapping each flag name above to its bit (0 = least significant) in this build. If the bits cannot all be named the field stays `type: "byte"` and is listed under `diagnostics.flags_fallback`.
- Fresh schemas/readable payloads group `hookFlagsA` through `hookFlagsB` as `hookChecks[0..1]`. This is a heterogeneous fixed array of named boolean objects; each index retains its original byte's bit map. Physical field names above identify source writes, not transmitted JSON keys.
