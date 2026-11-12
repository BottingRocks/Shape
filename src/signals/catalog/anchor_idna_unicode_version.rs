//! `AnchorIdnaUnicodeVersion`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{bind_in_order, BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use crate::signals::analysis::{leaf_checks::*, property_census::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `AnchorIdnaUnicodeVersion`. Card: [`CARD`].
pub(crate) fn recognize_anchor_idna_unicode_version(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let census = census(view, ctx);
    (count_then_bit_bytes(view) && census.string_containing("xn--") && census.calls_with("createElement", "a"))
        .then(|| Claim::new(["anchor hostname IDNA serialisation (xn--)", "count + two bit bytes"]))
}

pub(crate) fn bind_anchor_idna_unicode_version(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    bind_in_order(layout, &["testCount", "resultBits1", "resultBits2"])
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `AnchorIdnaUnicodeVersion`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "AnchorIdnaUnicodeVersion",
    summary: "Creates 12 detached <a> elements, each with href set to \"http://\" plus one Unicode character. The characters were chosen so that each one first appeared in a different Unicode version (1.1 up to 13.0). For each anchor it reads back the href the browser's URL parser produced and records whether that href is free of a '%' sign, i.e. whether the engine did not percent-encode the host. It sends the count of tests and the 12 pass/fail results.",
    why: "How the URL/IDNA parser handles characters from newer Unicode versions depends on the engine and on its IDNA/Unicode table version. This likely lets the vendor check the claimed browser engine and version, and spot spoofed user agents or unusual URL-parser stacks.",
    sample_payload: None,
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number("testCount", "Number of characters tested; always 12."),
        CardField::byte(
            "resultBits1",
            "Pass/fail bits for 8 of the 12 characters: 1 if the href has no '%' (host IDNA-converted or left raw), 0 if it was percent-encoded or the test threw. Tested characters in order: U+0061 'a' (control), U+AAE1 (Unicode 6.0), U+20BA (6.2), U+061C (6.3), U+0978 (7.0), U+ABA2 (8.0), U+23FE (9.0), U+0864 (10.0), U+0560 (11.0), U+0C77 (12.0), U+32FF (12.1), U+31BC (13.0).",
        )
        .flags(&[
            "U+0061",
            "U+AAE1",
            "U+20BA",
            "U+061C",
            "U+0978",
            "U+ABA2",
            "U+23FE",
            "U+0864",
            "U+0560",
            "U+0C77",
            "U+32FF",
            "U+31BC",
        ]),
        CardField::byte(
            "resultBits2",
            "The other 4 pass/fail bits in the low 4 bits (same meaning as resultBits1); the upper 4 bits are 0.",
        )
        .flags(&[
            "U+0061",
            "U+AAE1",
            "U+20BA",
            "U+061C",
            "U+0978",
            "U+ABA2",
            "U+23FE",
            "U+0864",
            "U+0560",
            "U+0C77",
            "U+32FF",
            "U+31BC",
        ]),
    ]),
};
