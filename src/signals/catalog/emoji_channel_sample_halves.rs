//! `EmojiChannelSampleHalves`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LeafView, SignalLayout, SlotView};
use crate::signals::analysis::{layout_nodes::*, render_probes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `EmojiChannelSampleHalves`. Card: [`CARD`].
///
/// Shape: `count, R{N}, count, R{N}`. Evidence: `getImageData`.
pub(crate) fn recognize_emoji_channel_sample_halves(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    claim_for(view, ctx, "EmojiChannelSampleHalves")
}

/// Each `count, repeat` pair is named by the half its values are sliced
/// from: `samples.slice(0, n / 2)` is the first half, `samples.slice(n / 2)`
/// the second. Builds write either half first; the wire position decides
/// nothing.
pub(crate) fn bind_emoji_channel_sample_halves(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    const SIGNAL: &str = "EmojiChannelSampleHalves";
    let mut binding = FieldBinding::new();
    for (message, layout_message) in layout.messages.iter().enumerate() {
        let fields = &layout_message.fields;
        let pairs = count_array_pairs(fields).ok_or_else(|| refuse(SIGNAL, "repeat without a count"))?;
        if fields.len() != 4 || pairs != [(0, 1), (2, 3)] {
            return Err(refuse(SIGNAL, format!("message {message}: expected two `count, repeat` halves")));
        }
        let leaves = field_leaves(view, layout, message).ok_or_else(|| refuse(SIGNAL, "leaf alignment failed"))?;
        let mut halves = Vec::new();
        for (count, values) in pairs {
            let mut pair_leaves = leaves[count].clone();
            pair_leaves.extend(leaves[values].iter().copied());
            let half = sliced_half(&pair_leaves, ctx)
                .ok_or_else(|| refuse(SIGNAL, format!("message {message}: fields {count}, {values} not read from one half slice")))?;
            halves.push((count, values, half));
        }
        if halves[0].2 == halves[1].2 {
            return Err(refuse(SIGNAL, format!("message {message}: both pairs read the same half")));
        }
        for (count, values, half) in halves {
            let (count_name, values_name) = match half {
                Half::First => ("firstHalfCount", "firstHalfSamples"),
                Half::Second => ("secondHalfCount", "secondHalfSamples"),
            };
            binding.insert(message, fields[count].key.clone(), count_name);
            binding.insert(message, fields[values].key.clone(), values_name);
        }
    }
    Ok(binding)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Half {
    First,
    Second,
}

/// The half every leaf's value is sliced from: the nearest `slice` call of
/// each value's backward slice must be `slice(0, n / 2)` (first) or
/// `slice(n / 2)` (second), and all leaves must agree.
fn sliced_half(leaves: &[&LeafView], ctx: &FinderCtx) -> Option<Half> {
    use oxc_ast::ast::{Argument, BinaryOperator, Expression};
    let halving = |argument: &Argument| match argument {
        Argument::BinaryExpression(binary) => {
            binary.operator == BinaryOperator::Division
                && matches!(&binary.right, Expression::NumericLiteral(two) if two.value == 2.0)
        }
        _ => false,
    };
    let mut found = None;
    for leaf in leaves {
        let slice = ctx.value_slice(leaf, 4)?;
        let calls: Vec<_> = slice.call_sites.iter().filter(|(method, _, _)| method == "slice").collect();
        let nearest = calls.iter().map(|(_, _, depth)| *depth).min()?;
        for (_, node, _) in calls.iter().filter(|(_, _, depth)| *depth == nearest) {
            let oxc_ast::AstKind::CallExpression(call) = ctx.parse.nodes.kind(*node) else {
                return None;
            };
            let half = match call.arguments.as_slice() {
                [Argument::NumericLiteral(zero), end] if zero.value == 0.0 && halving(end) => Half::First,
                [start] if halving(start) => Half::Second,
                _ => return None,
            };
            if found.is_some_and(|seen| seen != half) {
                return None;
            }
            found = Some(half);
        }
    }
    found
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `EmojiChannelSampleHalves`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "EmojiChannelSampleHalves",
    summary: "Draws 8 emoji, one at a time, in black 94px sans-serif on a 96x96 canvas and reads a single colour channel (R, G, B or A) of one pixel per emoji, at a build-specific position. The 8 samples are split into two halves and each half is sent as a count followed by its values. If the page is cleaned up before all emoji are drawn, the samples collected so far are sent.",
    why: "The pixel values depend on the installed emoji font, its version and the graphics rasteriser, so they fingerprint the operating system and browser and expose spoofed or randomised canvas output (likely).",
    sample_payload: Some(r#"{
  "firstHalfSamples": [
    0,
    0,
    232,
    162
  ],
  "secondHalfSamples": [
    255,
    31,
    142,
    9
  ]
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "firstHalfCount",
            "Number of samples in the first half (4 normally, fewer if cut short).",
        ),
        CardField::number(
            "firstHalfSamples",
            "One channel byte, 0..255, of the sampled pixel, or -1 if drawing or reading failed; written last sample first.",
        )
        .repeat("each sample of the first half"),
        CardField::number("secondHalfCount", "Number of samples in the second half (4 normally)."),
        CardField::number(
            "secondHalfSamples",
            "One channel byte, 0..255, or -1 on error; written last sample first.",
        )
        .repeat("each sample of the second half"),
    ]),
};
