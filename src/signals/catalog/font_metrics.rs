//! `FontMetrics`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use crate::signals::analysis::{layout_nodes::*, render_probes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `FontMetrics`. Card: [`CARD`].
///
/// Shape: `count, R{N}` (or a repeat of a number-or-nothing choice).
/// Evidence: the seeded font-metrics roles, or `measureText` with bounding
/// boxes on the value path.
pub(crate) fn recognize_font_metrics(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    claim_for(view, ctx, "FontMetrics")
}

pub(crate) fn bind_font_metrics(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    bind_count_and_items("FontMetrics", layout, "familyCount", "familyMetricsHash")
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `FontMetrics`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "FontMetrics",
    summary: "Measures 43 fixed characters (symbols, currency signs, CJK, Hangul, special and private-use characters) at 1000px in each of the five generic font families (sans-serif, serif, monospace, cursive, fantasy) with canvas measureText, joins each character's width and four bounding-box values as hex text, and hashes the text per family. It sends the family count and one hash per family; nothing is sent if there is no 2D canvas.",
    why: "Which real fonts the generic families map to, and their exact glyph sizes, depend on the OS, installed fonts and text renderer, so these hashes fingerprint the platform and reveal spoofed text metrics (likely).",
    sample_payload: Some(r#"{
  "familyMetricsHash": [
    -2112551573,
    -1100428147,
    1514521280,
    157432748,
    412628950
  ]
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number("familyCount", "Number of font-family hashes that follow (5)."),
        CardField::number(
            "familyMetricsHash",
            "Signed 32-bit hash of the measurement text for one family; in the main build in order sans-serif, serif, monospace, cursive, fantasy.",
        )
        .repeat("each generic font family"),
    ]),
};
