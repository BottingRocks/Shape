//! `EmojiChannelSamples`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use crate::signals::analysis::{layout_nodes::*, render_probes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `EmojiChannelSamples`. Card: [`CARD`].
///
/// Shape: `count, R{N}` (shared with VideoCanPlayType and FontMetrics; the
/// candidate with the closer evidence wins). Evidence: glyph drawing and
/// pixel readback (`fillText`, `getImageData`).
pub(crate) fn recognize_emoji_channel_samples(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    claim_for(view, ctx, "EmojiChannelSamples")
}

pub(crate) fn bind_emoji_channel_samples(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    bind_count_and_items("EmojiChannelSamples", layout, "sampleCount", "samples")
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `EmojiChannelSamples`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "EmojiChannelSamples",
    summary: "Draws 8 emoji (sunglasses face, keycap 6, baby bottle, England flag twice, bar chart, circled 'congratulation' ideograph, Ophiuchus) in black 94px sans-serif on a 96x96 canvas and reads one channel of one pixel for each: alpha for seven of them and red for the bar chart. It sends the sample count and the 8 values once.",
    why: "Whether ink appears at those points, and whether the bar chart is coloured, depends on the emoji font, its version and the rasteriser, giving a cheap OS/browser/font fingerprint that also catches canvas spoofing (likely).",
    sample_payload: Some(r#"{
  "samples": [
    255,
    255,
    0,
    253,
    0,
    0,
    255,
    255
  ]
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number("sampleCount", "Number of samples that follow (8)."),
        CardField::number(
            "samples",
            "The sampled channel byte, 0..255 (alpha: 0 = no ink at that point; red on the bar chart: 0 = monochrome glyph); written last probe first in the main build.",
        )
        .repeat("each emoji probe"),
    ]),
};
