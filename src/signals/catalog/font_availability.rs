//! `FontAvailability`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LayoutNode, LeafKind, SignalLayout, SlotView};
use crate::signals::analysis::{leaf_checks::*, layout_nodes::*, render_probes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField, CardMessage};


/// A number leaf, or a number-or-nothing choice (a loop that writes from
/// its own condition: the nothing arm is the loop exit).
fn is_number_or_loop_exit(node: &LayoutNode) -> bool {
    match node {
        LayoutNode::Leaf(leaf) => leaf.kind == LeafKind::Number,
        LayoutNode::Choice { arms } => {
            arms.len() == 2
                && arms.iter().any(|arm| matches!(&arm.node, LayoutNode::Leaf(leaf) if leaf.kind == LeafKind::Number))
                && arms.iter().any(|arm| matches!(arm.node, LayoutNode::Empty))
        }
        _ => false,
    }
}

/// A byte leaf, or a choice whose arms are byte leaves or nothing.
fn is_byte_or_nothing(node: &LayoutNode) -> bool {
    match node {
        LayoutNode::Leaf(leaf) => leaf.kind == LeafKind::Byte,
        LayoutNode::Choice { arms } => {
            arms.iter().any(|arm| matches!(&arm.node, LayoutNode::Leaf(leaf) if leaf.kind == LeafKind::Byte))
                && arms.iter().all(|arm| match &arm.node {
                    LayoutNode::Leaf(leaf) => leaf.kind == LeafKind::Byte,
                    LayoutNode::Empty => true,
                    _ => false,
                })
        }
        _ => false,
    }
}

/// `FontAvailability`. Card: [`CARD`].
///
/// Shape: family A `count, R{N}, version, seed` in any order; family B
/// `count, R{byte}, partial byte, version`. Evidence: `measureText` on the
/// slot's path or the font-width runner / queue roles.
pub(crate) fn recognize_font_availability(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    claim_for(view, ctx, "FontAvailability")
}

/// Family A: results are the repeat, the count the scalar before it, the
/// shuffle seed the scalar drawn from the random pool (`.shift()`), the
/// version the remaining scalar. Family B: the packed presence bytes are the
/// byte repeat, the count the scalar before it, the trailing partial byte
/// (a byte, or a byte-or-nothing choice) `fontPresenceTail`, the version the
/// remaining scalar.
pub(crate) fn bind_font_availability(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    const SIGNAL: &str = "FontAvailability";
    let mut binding = FieldBinding::new();
    for (message, layout_message) in layout.messages.iter().enumerate() {
        let fields = &layout_message.fields;
        let leaves = field_leaves(view, layout, message).ok_or_else(|| refuse(SIGNAL, "leaf alignment failed"))?;
        let pairs = count_array_pairs(fields).ok_or_else(|| refuse(SIGNAL, "repeat without a count"))?;
        let [(count, results)] = pairs.as_slice() else {
            return Err(refuse(SIGNAL, "expected one result repeat"));
        };
        let LayoutNode::Array { item } = &fields[*results].node else {
            return Err(refuse(SIGNAL, "results is not a repeat"));
        };
        let rest: Vec<usize> = (0..fields.len()).filter(|i| i != count && i != results).collect();
        if !is_number_or_loop_exit(item) {
            if !is_byte_or_nothing(item) {
                return Err(refuse(SIGNAL, "results repeat is neither numbers nor packed bytes"));
            }
            let (tails, versions): (Vec<usize>, Vec<usize>) =
                rest.iter().copied().partition(|index| is_byte_or_nothing(&fields[*index].node));
            let ([tail], [version]) = (tails.as_slice(), versions.as_slice()) else {
                return Err(refuse(SIGNAL, format!("message {message}: expected one partial byte and one version")));
            };
            if !is_scalar(&fields[*version].node) {
                return Err(refuse(SIGNAL, "family B version is not a scalar"));
            }
            binding.insert(message, fields[*count].key.clone(), "fontCount");
            binding.insert(message, fields[*results].key.clone(), "fontPresenceBits");
            binding.insert(message, fields[*tail].key.clone(), "fontPresenceTail");
            binding.insert(message, fields[*version].key.clone(), "fontListVersion");
            continue;
        }
        let [a, b] = rest.as_slice() else {
            return Err(refuse(SIGNAL, format!("message {message}: expected four fields")));
        };
        if !is_scalar(&fields[*a].node) || !is_scalar(&fields[*b].node) {
            return Err(refuse(SIGNAL, "version/seed are not scalars"));
        }
        let pooled = |index: usize| any_transform(&leaves[index], ".shift()");
        let (seed, version) = match (pooled(*a), pooled(*b)) {
            (true, false) => (*a, *b),
            (false, true) => (*b, *a),
            _ => return Err(refuse(SIGNAL, "cannot tell the shuffle seed from the list version")),
        };
        binding.insert(message, fields[*count].key.clone(), "fontCount");
        binding.insert(message, fields[*results].key.clone(), "fontResults");
        binding.insert(message, fields[version].key.clone(), "fontListVersion");
        binding.insert(message, fields[seed].key.clone(), "shuffleSeed");
    }
    Ok(binding)
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `FontAvailability`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "FontAvailability",
    summary: "Tests whether each of 47 named fonts (Andale Mono to Wingdings 3, plus the fake name 'Helkevtrica') is installed: it measures a test string on a canvas with the font and a sans-serif, then a serif fallback, and calls the font present if the width differs from the plain fallback. Fonts are probed a few at a time in the background.",
    why: "The set of installed fonts strongly identifies the operating system and the device's installed software; the fake font should always be absent, so a 'present' result points to a spoofed measureText (likely).",
    sample_payload: Some(r#"{
  "fontListVersion": 5,
  "fontResults": [
    1,
    2,
    1,
    1,
    1,
    1,
    1,
    1,
    1,
    1,
    1,
    1,
    2,
    1,
    2,
    1,
    1,
    1,
    2,
    2,
    1,
    1,
    2,
    2,
    1,
    1,
    1,
    2,
    2,
    2,
    2,
    2,
    2,
    2,
    1,
    1,
    1,
    1,
    1,
    2,
    2,
    1,
    2,
    1,
    1,
    1,
    1
  ],
  "shuffleSeed": 0.5613393995242942
}"#),
    by_name: false,
    body: CardBody::Messages(&[
        CardMessage {
            when: "family A (most builds, including the main build): fonts are probed in a shuffled order; one message at init with all results 0, then one after every probed font with all results so far.",
            fields: &[
                CardField::number("fontCount", "Number of result slots (47)."),
                CardField::number(
                    "fontResults",
                    "0 = not probed yet, 1 = not installed, 2 = installed. Slots are in shuffled probe order and written last slot first in the main build.",
                )
                .repeat("each font"),
                CardField::number("fontListVersion", "Version of the font list (5)."),
                CardField::number(
                    "shuffleSeed",
                    "Random fraction between 0 and 1 that seeded the shuffle, so the receiver can rebuild the probe order.",
                ),
            ],
        },
        CardMessage {
            when: "family B (starbucks, tdbank and some puma builds): no shuffle; one message after all fonts are probed, nothing if there is no 2D canvas.",
            fields: &[
                CardField::number("fontCount", "Number of fonts tested (47)."),
                CardField::byte(
                    "fontPresenceBits",
                    "8 fonts per byte, one bit each (1 = installed), in font-list order (some builds walk the list from the end); one byte per full group of 8 fonts (5 bytes for 47 fonts).",
                )
                .repeat("each full group of 8 fonts")
                .flags(&[
                    "Andale Mono",
                    "Arial Narrow",
                    "Arial Unicode MS",
                    "Batang",
                    "Bell MT",
                    "Brush Script",
                    "Brush Script MT",
                    "Calibri",
                    "Charter",
                    "Courier",
                    "Courier New",
                    "Curlz MT",
                    "DejaVu Sans",
                    "DejaVu Sans Mono",
                    "DejaVu Serif Condensed",
                    "Droid Sans",
                    "Droid Sans Fallback",
                    "Droid Serif",
                    "Forte",
                    "Futura",
                    "Geneva",
                    "Hei",
                    "Leelawadee",
                    "Levenim MT",
                    "Liberation Sans",
                    "Liberation Sans Narrow",
                    "Marlett",
                    "Meiryo UI",
                    "Microsoft Uighur",
                    "Microsoft YaHei UI",
                    "MS Mincho",
                    "MS UI Gothic",
                    "NanumGothic",
                    "Nirmala UI",
                    "Palatino",
                    "Papyrus",
                    "PMingLiU",
                    "PT Serif",
                    "SimHei",
                    "STIXVariants",
                    "STSong",
                    "Traditional Arabic",
                    "Urdu Typesetting",
                    "Verdana",
                    "Wingdings",
                    "Wingdings 3",
                    "Helkevtrica",
                ]),
                CardField::byte(
                    "fontPresenceTail",
                    "The leftover fonts that do not fill a whole byte (7 for 47 fonts), one bit each, packed the same way; not sent when the font count is a multiple of 8.",
                )
                .flags(&[
                    "Andale Mono",
                    "Arial Narrow",
                    "Arial Unicode MS",
                    "Batang",
                    "Bell MT",
                    "Brush Script",
                    "Brush Script MT",
                    "Calibri",
                    "Charter",
                    "Courier",
                    "Courier New",
                    "Curlz MT",
                    "DejaVu Sans",
                    "DejaVu Sans Mono",
                    "DejaVu Serif Condensed",
                    "Droid Sans",
                    "Droid Sans Fallback",
                    "Droid Serif",
                    "Forte",
                    "Futura",
                    "Geneva",
                    "Hei",
                    "Leelawadee",
                    "Levenim MT",
                    "Liberation Sans",
                    "Liberation Sans Narrow",
                    "Marlett",
                    "Meiryo UI",
                    "Microsoft Uighur",
                    "Microsoft YaHei UI",
                    "MS Mincho",
                    "MS UI Gothic",
                    "NanumGothic",
                    "Nirmala UI",
                    "Palatino",
                    "Papyrus",
                    "PMingLiU",
                    "PT Serif",
                    "SimHei",
                    "STIXVariants",
                    "STSong",
                    "Traditional Arabic",
                    "Urdu Typesetting",
                    "Verdana",
                    "Wingdings",
                    "Wingdings 3",
                    "Helkevtrica",
                ]),
                CardField::number("fontListVersion", "Version of the font list (5)."),
            ],
        },
    ]),
};
