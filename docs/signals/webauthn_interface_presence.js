// WebAuthnInterfacePresence: captured example.
export default {
  "AuthenticatorAssertionResponse": true,
  "AuthenticatorAttestationResponse": true,
  "AuthenticatorResponse": true,
  "PublicKeyCredential": true
};

// Illustrative: an engine without WebAuthn; all four interfaces are missing.
export const webAuthnAbsent = {
  "AuthenticatorAssertionResponse": false,
  "AuthenticatorAttestationResponse": false,
  "AuthenticatorResponse": false,
  "PublicKeyCredential": false
};

// Illustrative: only PublicKeyCredential survives; the response interfaces were deleted or stubbed.
export const partialInterfaces = {
  "AuthenticatorAssertionResponse": false,
  "AuthenticatorAttestationResponse": false,
  "AuthenticatorResponse": false,
  "PublicKeyCredential": true
};

// Captured from a second build of the collector; decoded positional payload.
export const capturedSecondBuild = {
  "AuthenticatorAssertionResponse": true,
  "AuthenticatorAttestationResponse": true,
  "AuthenticatorResponse": true,
  "PublicKeyCredential": true
};
