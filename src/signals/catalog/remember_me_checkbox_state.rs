//! `RememberMeCheckboxState`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{bind_in_order, BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use crate::signals::analysis::shape_text::*;
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `RememberMeCheckboxState`. Card: [`CARD`].
pub(crate) fn recognize_remember_me_checkbox_state(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    // One bare value: `getElementById(..).checked`, else
    // `getElementsByName(..)[0].className.indexOf("checked") > -1`, else "".
    // A keyless generic value (no named-object keys), one top-level field.
    let bare_single = view.is_generic_object()
        && view.leaves().next().is_some()
        && view.leaves().all(|leaf| leaf.generic_key.is_none())
        && top_count_is(&view.shapes(), 1);
    let reads = view.uses_members(&["getElementById", "getElementsByName", "checked", "className"])
        && view.calls_with("indexOf", "checked");
    (bare_single && reads).then(|| {
        Claim::new([
            "one keyless generic value",
            "getElementById(..).checked / getElementsByName(..)[0].className.indexOf(\"checked\")",
        ])
    })
}

pub(crate) fn bind_remember_me_checkbox_state(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    bind_in_order(layout, &["rememberMe"])
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `RememberMeCheckboxState`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "RememberMeCheckboxState",
    summary: "At the last moment before the data is sent, reads the state of the login form's \"remember me\" checkbox and sends it as one bare value (not an object). It looks for an element with id rememberMe, then id TravelerLoginForm_rememberMe, and sends its checked property. If neither exists it falls back to the first element named shouldRememberUser and sends whether its class name contains \"checked\" (a styled, non-native checkbox). If none of these exist it sends the empty string. It reads the page only; nothing is changed.",
    why: "Likely ties the request to a login attempt and records whether the user asked to stay signed in. Credential-stuffing tools usually submit the login form without touching that box, so its state helps separate scripted logins from real ones (the use is inferred; the source only collects the value).",
    sample_payload: None,
    by_name: true,
    body: CardBody::Fields(&[
        CardField::boolean(
            "rememberMe",
            "Boolean: the checked state of #rememberMe (or #TravelerLoginForm_rememberMe); otherwise whether the first [name=shouldRememberUser] element's className contains \"checked\". The empty string \"\" (a STRING) when none of these elements is on the page.",
        ),
    ]),
};
