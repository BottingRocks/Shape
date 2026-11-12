//! `CorsCookieValue`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{bind_by_object_key, BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use crate::signals::analysis::leaf_checks::*;
use crate::signals::catalog::card::{Card, CardBody, CardField, CardMessage};


/// `CorsCookieValue`. Card: [`CARD`].
pub(crate) fn recognize_cors_cookie_value(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    // `{value: <cookie value>}` (or `{}`) from `document.cookie.split("; ")`.
    let keys_value = view
        .leaves()
        .all(|leaf| leaf.generic_key.as_deref() == Some("value"));
    let cookie = view.uses_member("cookie") && view.calls_with("split", "; ");
    if view.is_generic_object() && view.leaves().next().is_some() && keys_value && cookie {
        return claim(&["named object {value}", "document.cookie.split(\"; \")"]);
    }
    None
}

pub(crate) fn bind_cors_cookie_value(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    bind_by_object_key(layout, &[("value", "value")])
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `CorsCookieValue`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "CorsCookieValue",
    summary: "Reads document.cookie once, looks for the cookie named NyJB5Jgn and sends its value as an object { value }. If the cookie is not there it sends an empty object. The value is not trimmed or URL-decoded and is cut at its first '='. The collector is registered as custom_corsCookie. It never writes a cookie.",
    why: "The NyJB5Jgn cookie is likely set by another vendor script or server response; reading it back likely links this session to an earlier visit or confirms that a cross-origin/server step really ran (the purpose is not visible in the source).",
    sample_payload: None,
    by_name: true,
    body: CardBody::Messages(&[
        CardMessage {
            when: "a NyJB5Jgn cookie exists",
            fields: &[
                CardField::string(
                    "value",
                    "The cookie's value as raw text (up to the first '='); undefined if the cookie has no '='.",
                ),
            ],
        },
        CardMessage {
            when: "no NyJB5Jgn cookie",
            fields: &[
            ],
        },
    ]),
};
