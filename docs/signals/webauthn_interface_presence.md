# WebAuthnInterfacePresence

[Signals index](README.md) · [data-shape example](webauthn_interface_presence.js) · [schema format](schema.md)

## What it collects

Checks whether four WebAuthn interface constructors exist on the page global (typeof ... === "function") and packs the four answers into one byte. It calls no WebAuthn API and touches no credentials.

## Why

All four exist in any modern browser with WebAuthn, so a value other than all-set likely points to an old engine, a stripped webview, or an automation environment that deletes or stubs these globals.

## Fields

| Field | Type | Meaning |
| --- | --- | --- |
| `flags` | flags (byte) | Packed flags, 1 = interface exists: PublicKeyCredential, AuthenticatorAttestationResponse, AuthenticatorResponse, AuthenticatorAssertionResponse. The other 4 bits are always 0, so the value is 0-15 and 15 in a normal modern browser. |

Flag names of `flags` (card order; the bit of each flag is per build, see the schema's `bits`):

- `PublicKeyCredential`
- `AuthenticatorAttestationResponse`
- `AuthenticatorResponse`
- `AuthenticatorAssertionResponse`

## Sample payload

Captured decoded values. The field tables describe the original wire layout; this example shows the decoded payload, including grouped flags and hidden encoding-only lengths. It documents one capture, not every message variant or a runtime schema.

```json
{
  "AuthenticatorAssertionResponse": true,
  "AuthenticatorAttestationResponse": true,
  "AuthenticatorResponse": true,
  "PublicKeyCredential": true
}
```

## In a build's schema

In `shape signals <program.js> --json` output this signal is `signals.WebAuthnInterfacePresence` (see [schema format](schema.md)). Its `address` is the slot in that build (`wave:<group>:<position>` or `direct:<descriptor>`).

- Positional: `type: "positional"` with its `positional` index and the signal `mask` (the salt of the stream its fields are XOR-masked with).
- `order` lists the field names above in this build's write order (it varies per build); `fields` gives each one's wire type.
- Flag bytes are `type: "flags"` with `bits` mapping each flag name above to its bit (0 = least significant) in this build. If the bits cannot all be named the field stays `type: "byte"` and is listed under `diagnostics.flags_fallback`.
- When the entire resolved message is one packed flag byte, fresh schemas/readable payloads expose its named booleans at the signal root. The schema retains the original byte under `physicalFlags`; the wrapper is a projection name, not a transmitted object.
