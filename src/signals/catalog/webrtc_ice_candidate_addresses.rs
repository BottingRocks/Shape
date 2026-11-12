//! `WebRtcIceCandidateAddresses`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LeafKind, LeafOp, SignalLayout, SlotView};
use crate::signals::analysis::{shape_text::*, leaf_checks::*, field_solver::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `WebRtcIceCandidateAddresses`. Card: [`CARD`].
pub(crate) fn recognize_webrtc_ice_candidate_addresses(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let leaves = leaves_of(view);
    let record_string = |transform: &str| {
        leaves
            .iter()
            .any(|leaf| leaf.repeat_depth == 1 && leaf.op == LeafOp::String && has_transform(leaf, transform))
    };
    let records = record_string(".candidate") && record_string(".ip");
    let role = ctx.has_role(view, "webrtc_candidate_emitter");
    let shape = top_count_is(&view.shapes(), 2)
        && leaves
            .iter()
            .all(|leaf| (leaf.repeat_depth == 0) == (leaf.op == LeafOp::Number));
    if shape && (records || role) {
        return claim(&["NUMBER + repeat of {STRING, STRING}", "ICE candidate records (type, ip)"]);
    }
    None
}

pub(crate) fn bind_webrtc_ice_candidate_addresses(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    let mut binding = FieldBinding::new();
    for message in 0..layout.messages.len() {
        let fields = message_fields(view, layout, message)?;
        let count = |f: &Field| f.is_plain(LeafKind::Number);
        let records = |f: &Field| f.is_array();
        solve(message, &fields, &[("recordCount", &count), ("candidates", &records)], &mut binding)?;
        for field in fields.iter().filter(|f| f.is_array()) {
            let item = field
                .item_fields()
                .ok_or_else(|| BindRefusal::new("WebRtcIceCandidateAddresses: candidate item is not a record"))?;
            let kind = |f: &Field| f.tr(".candidate");
            let ip = |f: &Field| f.tr(".ip");
            solve(message, &item, &[("candidateType", &kind), ("ip", &ip)], &mut binding)?;
        }
    }
    Ok(binding)
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `WebRtcIceCandidateAddresses`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "WebRtcIceCandidateAddresses",
    summary: "Creates a throw-away RTCPeerConnection with no STUN/TURN servers, opens a data channel and creates an offer so the browser gathers its local ICE candidates. From each candidate line it pulls the IPv4 or full IPv6 address and the candidate type, drops duplicate addresses, and sends the list. No remote host is contacted; the result is cached for the page.",
    why: "Local IP addresses can reveal the real network behind a proxy or VPN and identify a machine across sessions (likely); mDNS-only or empty results are also informative about the browser.",
    sample_payload: Some(r#"{
  "candidates": [

  ]
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "recordCount",
            "Number of unique candidate addresses that follow; often 0 on modern browsers, which hide host IPs behind mDNS names.",
        ),
        CardField::group("candidates", &[
            CardField::string(
                "candidateType",
                "Candidate type: \"host\", \"srflx\", \"prflx\" or \"relay\", or empty if none matched; usually \"host\".",
            ),
            CardField::string(
                "ip",
                "IPv4 dotted-quad or full 8-group IPv6 address from the candidate line, e.g. \"192.168.1.23\".",
            ),
        ])
        .repeat("once per unique address (last found first in the main build)"),
    ]),
};
