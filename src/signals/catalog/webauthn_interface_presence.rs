//! `WebAuthnInterfacePresence`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{bind_in_order, BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use crate::signals::analysis::{leaf_checks::*, property_census::*, census_probes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `WebAuthnInterfacePresence`. Card: [`CARD`].
pub(crate) fn recognize_webauthn_interface_presence(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let census = census(view, ctx);
    (one_byte(view)
        && probes_all(view, &census, &["PublicKeyCredential", "AuthenticatorAttestationResponse"])
        && !outranked(&census, "WebAuthnInterfacePresence"))
        .then(|| Claim::new(["typeof PublicKeyCredential / Authenticator*Response", "one flag byte"]))
}

pub(crate) fn bind_webauthn_interface_presence(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    bind_in_order(layout, &["flags"])
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `WebAuthnInterfacePresence`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "WebAuthnInterfacePresence",
    summary: "Checks whether four WebAuthn interface constructors exist on the page global (typeof ... === \"function\") and packs the four answers into one byte. It calls no WebAuthn API and touches no credentials.",
    why: "All four exist in any modern browser with WebAuthn, so a value other than all-set likely points to an old engine, a stripped webview, or an automation environment that deletes or stubs these globals.",
    sample_payload: Some(r#"{
  "AuthenticatorAssertionResponse": true,
  "AuthenticatorAttestationResponse": true,
  "AuthenticatorResponse": true,
  "PublicKeyCredential": true
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::byte(
            "flags",
            "Packed flags, 1 = interface exists: PublicKeyCredential, AuthenticatorAttestationResponse, AuthenticatorResponse, AuthenticatorAssertionResponse. The other 4 bits are always 0, so the value is 0-15 and 15 in a normal modern browser.",
        )
        .flags(&[
            "PublicKeyCredential",
            "AuthenticatorAttestationResponse",
            "AuthenticatorResponse",
            "AuthenticatorAssertionResponse",
        ]),
    ]),
};
