//! `LocationUrlComponents`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LeafKind, LeafOp, LeafView, SignalLayout, SlotView, ValueSlice};
use crate::signals::analysis::{leaf_checks::*, field_solver::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `LocationUrlComponents` evidence: location protocol/hostname strings.
pub(crate) fn is_location_url(leaves: &[&LeafView]) -> bool {
    // Components are plain (unrepeated) string fields; a repeated string of
    // a property-name census can carry every location path at once.
    let string_reads = |api: &str| {
        leaves
            .iter()
            .any(|leaf| leaf.op == LeafOp::String && leaf.repeat_depth == 0 && has_api(leaf, api))
    };
    // One component's provenance can be lost through a shared register; the
    // protocol plus two of the other components suffice.
    let others = ["location.hostname", "location.port", "location.href.substring()"]
        .iter()
        .filter(|api| string_reads(api))
        .count();
    string_reads("location.protocol") && others >= 2 && !any_api(leaves, "document.referrer")
}

/// `LocationUrlComponents`. Card: [`CARD`].
pub(crate) fn recognize_location_url_components(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    if is_location_url(&leaves_of(view)) {
        return claim(&["string values read location.protocol and location.hostname"]);
    }
    None
}

pub(crate) fn bind_location_url_components(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    // `href` is absent in some builds; the field set is the build's own.
    // Leaf provenance can carry another field's source through a shared
    // register, so a field whose write-site slice reads `location.<part>`
    // nearest is named by that read; otherwise by its provenance path.
    let part = |api: &'static str, member: &'static str| {
        move |f: &Field, slice: &ValueSlice| {
            if !f.writes(LeafKind::String) {
                return false;
            }
            let nearest = slice.nearest_members();
            if nearest.contains(&"location") {
                nearest.contains(&member)
            } else {
                f.api(api)
            }
        }
    };
    let hostname = part("location.hostname", "hostname");
    let href = part("location.href.substring()", "href");
    let protocol = part("location.protocol", "protocol");
    let port = part("location.port", "port");
    solve_by_slices(
        view,
        layout,
        ctx,
        2,
        &[("hostname", &hostname), ("href", &href), ("protocol", &protocol), ("port", &port)],
    )
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `LocationUrlComponents`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "LocationUrlComponents",
    summary: "Reads the current page's address from window.location and sends its parts as text: the hostname, the full URL (cut to its first 100 characters), the protocol and the port. Nothing is hashed; the strings are sent as they are. It runs once when the collector starts.",
    why: "Tells the vendor which page and site the collector is actually running on, so a token or payload replayed from another site, a local file or about:blank can be spotted. A hostname or port that is missing (not even an empty string) is likely a sign of an emulated or spoofed browser, since real browsers always return strings.",
    sample_payload: Some(r#"{
  "hostname": "www.target.com",
  "href": "https://www.target.com/login?client_id=ecom-web-1.0.0&ui_namespace=ui-default&back_button_action=bro",
  "port": "",
  "protocol": "https:"
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::string(
            "hostname",
            "location.hostname, e.g. \"www.example.com\"; empty for file: or about:blank pages. Or a null marker if hostname is null/undefined (only in fake or emulated environments).",
        ).nullable(),
        CardField::string(
            "href",
            "location.href, the full page URL, cut to its first 100 characters. Some builds leave this field out.",
        ),
        CardField::string("protocol", "location.protocol, e.g. \"https:\", \"http:\" or \"file:\"."),
        CardField::string(
            "port",
            "location.port as text, usually empty (\"\") for default ports, e.g. \"8080\" otherwise. Or a null marker if port is null/undefined (only in fake or emulated environments).",
        ).nullable(),
    ]),
};
