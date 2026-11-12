//! `CanvasScenePixelSamples`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::extract::contracts::SourceClass;
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LayoutNode, SignalLayout, SlotView};
use crate::signals::analysis::{leaf_checks::*, layout_nodes::*, render_probes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `CanvasScenePixelSamples`. Card: [`CARD`].
///
/// Shape: `count, R{channelCount R{channel}}` with a terminator scalar first
/// or last. Evidence: the value closure reads `getImageData`.
pub(crate) fn recognize_canvas_scene_pixel_samples(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    claim_for(view, ctx, "CanvasScenePixelSamples")
}

/// The sample count is the scalar directly before the sample repeat (a
/// `.length`); the other scalar is the terminator.
pub(crate) fn bind_canvas_scene_pixel_samples(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    const SIGNAL: &str = "CanvasScenePixelSamples";
    let mut binding = FieldBinding::new();
    for (message, layout_message) in layout.messages.iter().enumerate() {
        let fields = &layout_message.fields;
        let leaves = field_leaves(view, layout, message).ok_or_else(|| refuse(SIGNAL, "leaf alignment failed"))?;
        let pairs = count_array_pairs(fields).ok_or_else(|| refuse(SIGNAL, "repeat without a count"))?;
        let [(count, samples)] = pairs.as_slice() else {
            return Err(refuse(SIGNAL, "expected one sample repeat"));
        };
        let scalars: Vec<usize> = (0..fields.len()).filter(|index| is_scalar(&fields[*index].node)).collect();
        if fields.len() != 3 || scalars.len() != 2 {
            return Err(refuse(SIGNAL, format!("message {message}: expected count, samples and a terminator")));
        }
        let terminator = scalars.iter().copied().find(|index| index != count).unwrap_or(0);
        if any_transform(&leaves[terminator], ".length") {
            return Err(refuse(SIGNAL, "terminator reads a length"));
        }
        // The count reads the sample list's `.length` (sometimes through an
        // opaque computed key, `x[x !== n ? "length" : …]`); then the
        // terminator must be a plain literal.
        let literal_terminator = !leaves[terminator].is_empty()
            && leaves[terminator]
                .iter()
                .all(|leaf| leaf.classes.iter().all(|class| *class == SourceClass::Literal));
        if !any_transform(&leaves[*count], ".length") && !literal_terminator {
            return Err(refuse(SIGNAL, format!("message {message}: no count reading `.length` and no literal terminator")));
        }
        let LayoutNode::Array { item } = &fields[*samples].node else {
            return Err(refuse(SIGNAL, "samples is not a repeat"));
        };
        let LayoutNode::Record { fields: item_fields } = item.as_ref() else {
            return Err(refuse(SIGNAL, "sample item is not a record"));
        };
        let [channel_count, channels] = item_fields.as_slice() else {
            return Err(refuse(SIGNAL, "sample item is not `channelCount, channels`"));
        };
        if !is_scalar(&channel_count.node) || !is_array(&channels.node) {
            return Err(refuse(SIGNAL, "sample item is not `channelCount, channels`"));
        }
        binding.insert(message, fields[*count].key.clone(), "sampleCount");
        binding.insert(message, fields[*samples].key.clone(), "samples");
        binding.insert(message, fields[terminator].key.clone(), "terminator");
        binding.insert(message, channel_count.key.clone(), "channelCount");
        binding.insert(message, channels.key.clone(), "channel");
    }
    Ok(binding)
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `CanvasScenePixelSamples`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "CanvasScenePixelSamples",
    summary: "Draws a fixed scene on an off-screen canvas of about 250x250 pixels: filled rectangles, dozens of stroked circles in solid, translucent or gradient colours, and a pentagram. It reads back all pixels and sends the colour of about 40 to 60 pixels at fixed positions (the positions are constants that differ per build). Some sampled pixels sit side by side on shape edges, to capture anti-aliasing. It sends raw pixel samples, not an image hash.",
    why: "How a browser rasterises the same scene (anti-aliasing, gradient interpolation, alpha blending, colour handling) varies with GPU, OS, graphics driver and browser engine. This makes it a classic canvas fingerprint and likely also exposes canvas noise injection or headless software rendering.",
    sample_payload: Some(r#"{
  "samples": [
    [
      255,
      198,
      84,
      66
    ],
    [
      255,
      250,
      110,
      100
    ],
    [
      255,
      250,
      110,
      100
    ],
    [
      255,
      250,
      110,
      100
    ],
    [
      255,
      90,
      154,
      150
    ],
    [
      255,
      250,
      110,
      100
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      86,
      199,
      65
    ],
    [
      255,
      228,
      107,
      107
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      250,
      110,
      100
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      190,
      100,
      120
    ],
    [
      255,
      244,
      189,
      188
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      250,
      110,
      100
    ],
    [
      255,
      82,
      127,
      175
    ],
    [
      255,
      187,
      101,
      120
    ],
    [
      255,
      250,
      110,
      100
    ],
    [
      255,
      250,
      110,
      100
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      250,
      110,
      100
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      250,
      110,
      100
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      241,
      232,
      235
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      132,
      203,
      145
    ],
    [
      255,
      177,
      143,
      74
    ],
    [
      255,
      250,
      110,
      100
    ],
    [
      255,
      216,
      101,
      115
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      241,
      235,
      239
    ],
    [
      255,
      250,
      110,
      100
    ],
    [
      255,
      250,
      110,
      100
    ],
    [
      255,
      217,
      117,
      86
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      137,
      185,
      104
    ],
    [
      255,
      82,
      127,
      175
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      240,
      245,
      250
    ],
    [
      255,
      250,
      110,
      100
    ],
    [
      255,
      242,
      108,
      104
    ],
    [
      255,
      250,
      110,
      100
    ],
    [
      255,
      250,
      110,
      100
    ],
    [
      255,
      151,
      62,
      121
    ]
  ],
  "terminator": -1
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "sampleCount",
            "Number of sampled pixels; a per-build constant, 39 in the main build (38 to 59 across builds).",
        ),
        CardField::group("samples", &[
            CardField::number("channelCount", "Number of channel values that follow; always 4."),
            CardField::number("channel", "Colour channel value of the pixel, 0 to 255.")
                .repeat("4 times: alpha, blue, green, red"),
        ])
        .repeat("once per sampled pixel"),
        CardField::number("terminator", "End marker; always -1."),
    ]),
};
