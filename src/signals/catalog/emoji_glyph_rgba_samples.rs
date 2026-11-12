//! `EmojiGlyphRgbaSamples`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LayoutNode, SignalLayout, SlotView};
use crate::signals::analysis::{layout_nodes::*, render_probes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `EmojiGlyphRgbaSamples`. Card: [`CARD`].
///
/// Shape: `count, R{count R{N}}` (no terminator, unlike the scene samples).
/// Evidence: `fillText` and `getImageData` in the slot's sampling helper.
pub(crate) fn recognize_emoji_glyph_rgba_samples(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    claim_for(view, ctx, "EmojiGlyphRgbaSamples")
}

pub(crate) fn bind_emoji_glyph_rgba_samples(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    const SIGNAL: &str = "EmojiGlyphRgbaSamples";
    let mut binding = FieldBinding::new();
    for (message, layout_message) in layout.messages.iter().enumerate() {
        let [count, rows] = layout_message.fields.as_slice() else {
            return Err(refuse(SIGNAL, format!("message {message}: expected `rowCount, rows`")));
        };
        let LayoutNode::Array { item } = &rows.node else {
            return Err(refuse(SIGNAL, "rows is not a repeat"));
        };
        let LayoutNode::Record { fields } = item.as_ref() else {
            return Err(refuse(SIGNAL, "row is not a record"));
        };
        let [sample_count, pixels] = fields.as_slice() else {
            return Err(refuse(SIGNAL, "row is not `sampleCount, pixels`"));
        };
        if !is_scalar(&count.node) || !is_scalar(&sample_count.node) || !is_array(&pixels.node) {
            return Err(refuse(SIGNAL, "unexpected row layout"));
        }
        binding.insert(message, count.key.clone(), "rowCount");
        binding.insert(message, rows.key.clone(), "rows");
        binding.insert(message, sample_count.key.clone(), "sampleCount");
        binding.insert(message, pixels.key.clone(), "pixels");
    }
    Ok(binding)
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `EmojiGlyphRgbaSamples`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "EmojiGlyphRgbaSamples",
    summary: "Draws about 20 emoji (ZWJ sequences, flags, very recent code points) one at a time in black 94px sans-serif on a 96x96 canvas and reads the full RGBA value of two fixed pixels for each, packed into one 32-bit number. After each emoji it sends the rows collected so far, and it sends a final message with all rows when done.",
    why: "The pixels change with the emoji font, the emoji/Unicode version supported, colour vs. fallback rendering and anti-aliasing, so they fingerprint the OS and browser and detect canvas spoofing (likely).",
    sample_payload: Some(r#"{
  "rows": [
    [
      -1114946561,
      -677437185
    ],
    [
      -1417522945,
      -253251073
    ],
    [
      -1738996225,
      1701144063
    ],
    [
      1701144063,
      1296911871
    ],
    [
      1972876799,
      -2036485633
    ],
    [
      -241423617,
      -274452737
    ],
    [
      -1466342145,
      1216982783
    ],
    [
      -687734273,
      -1219471617
    ],
    [
      -480881921,
      -1045342721
    ],
    [
      -1432582401,
      -2123276289
    ],
    [
      -211612417,
      -110620929
    ],
    [
      -2127616425,
      -2027219713
    ],
    [
      759454205,
      1016160767
    ],
    [
      -44288001,
      -801953793
    ],
    [
      608003327,
      810520575
    ],
    [
      857872127,
      1161046015
    ],
    [
      -527497217,
      -3189761
    ],
    [
      -185272833,
      -421009153
    ],
    [
      -3395073,
      -391373569
    ],
    [
      508038815,
      727391231
    ],
    [
      4,
      -1467976193
    ]
  ]
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "rowCount",
            "Number of emoji rows that follow (grows by one per partial message; 20 or 21 when complete).",
        ),
        CardField::group("rows", &[
            CardField::number(
                "sampleCount",
                "Number of pixel samples for this emoji (2 normally; fewer if drawing failed).",
            ),
            CardField::number(
                "pixels",
                "Pixel colour packed as R<<24 | G<<16 | B<<8 | A, a signed 32-bit number (0 = transparent; negative when red is 128 or more).",
            )
            .repeat("each sampled pixel of this emoji"),
        ])
        .repeat("each emoji drawn so far"),
    ]),
};
