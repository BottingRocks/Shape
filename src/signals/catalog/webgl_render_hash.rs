//! `WebGlRenderHash`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::extract::contracts::SourceClass;
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LayoutNode, LeafKind, LeafOp, SignalLayout, SlotView};
use crate::signals::analysis::{layout_nodes::*, render_probes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField, CardMessage};


/// `WebGlRenderHash`. Card: [`CARD`].
///
/// Shape: every message is three single fields (kind byte, status, digest)
/// with at least one byte. Evidence: shader compilation on the slot's path
/// or the `webgl_fingerprint_probe` role.
pub(crate) fn recognize_webgl_render_hash(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    claim_for(view, ctx, "WebGlRenderHash")
}

/// Per message: the literal byte below the null-marker range is the record
/// kind; the value field (a number, or a hash-or-null choice) is the status
/// when it is a literal number and the digest otherwise; the null byte is the
/// other one. The kind/status/digest positions must agree across messages.
pub(crate) fn bind_webgl_render_hash(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    const SIGNAL: &str = "WebGlRenderHash";
    let mut binding = FieldBinding::new();
    let mut positions: Option<(usize, usize, usize)> = None;
    // The kind position of every message with exactly one byte below the
    // null-marker range.
    let kind_positions: std::collections::BTreeSet<usize> = layout
        .messages
        .iter()
        .filter_map(|layout_message| {
            let fields = &layout_message.fields;
            let kinds: Vec<usize> = (0..fields.len())
                .filter(|i| matches!(&fields[*i].node, LayoutNode::Leaf(leaf) if leaf.kind == LeafKind::Byte && leaf.byte.map_or(true, |b| b < 128)))
                .collect();
            match kinds.as_slice() {
                [kind] => Some(*kind),
                _ => None,
            }
        })
        .collect();
    for (message, layout_message) in layout.messages.iter().enumerate() {
        let fields = &layout_message.fields;
        let leaves = field_leaves(view, layout, message).ok_or_else(|| refuse(SIGNAL, "leaf alignment failed"))?;
        if fields.len() != 3 {
            return Err(refuse(SIGNAL, format!("message {message}: expected three fields")));
        }
        // A byte at or above 128 is a null marker; any other byte is the
        // record kind (a literal tag, or a computed one).
        let byte_of = |index: usize| match &fields[index].node {
            LayoutNode::Leaf(leaf) if leaf.kind == LeafKind::Byte => Some(leaf.byte),
            _ => None,
        };
        let mut kinds: Vec<usize> = (0..3).filter(|i| byte_of(*i).is_some_and(|b| b.map_or(true, |b| b < 128))).collect();
        let mut nulls: Vec<usize> = (0..3).filter(|i| byte_of(*i).is_some_and(|b| b.is_some_and(|b| b >= 128))).collect();
        let values: Vec<usize> = (0..3).filter(|i| byte_of(*i).is_none()).collect();
        // A kind tag at or above 128 looks like a null marker: take the kind
        // position another message proves.
        if kinds.is_empty() && nulls.len() == 2 {
            if let Some(kind) = kind_positions.iter().copied().find(|position| nulls.contains(position)) {
                kinds = vec![kind];
                nulls.retain(|index| *index != kind);
            }
        }
        let ([kind], [null], [value]) = (kinds.as_slice(), nulls.as_slice(), values.as_slice()) else {
            return Err(refuse(SIGNAL, format!("message {message}: expected a kind byte, a null byte and a value")));
        };
        let literal = leaves[*value]
            .iter()
            .filter(|leaf| leaf.op != LeafOp::Byte)
            .all(|leaf| leaf.classes.iter().all(|class| *class == SourceClass::Literal));
        let status_record = literal && is_scalar(&fields[*value].node);
        let (status, digest) = if status_record { (*value, *null) } else { (*null, *value) };
        match positions {
            None => positions = Some((*kind, status, digest)),
            Some(known) if known == (*kind, status, digest) => {}
            Some(_) => return Err(refuse(SIGNAL, "kind/status/digest positions differ between messages")),
        }
        binding.insert(message, fields[*kind].key.clone(), "kind");
        binding.insert(message, fields[status].key.clone(), "status");
        binding.insert(message, fields[digest].key.clone(), "digest");
    }
    Ok(binding)
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `WebGlRenderHash`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "WebGlRenderHash",
    summary: "Draws one fixed coloured triangle with a minimal WebGL program on a 150x150 canvas, reads back a rectangle of pixels and sends a 32-bit hash of them. If any step of the WebGL pipeline fails, it sends a status code naming the first failed step instead.",
    why: "The exact rendered pixels depend on the GPU, driver and rasteriser, so the hash fingerprints the graphics stack, and the failure step likely flags headless or software-only environments.",
    sample_payload: Some(r#"{
  "digest": 1847286926,
  "kind": 70,
  "status": null
}"#),
    by_name: false,
    body: CardBody::Messages(&[
        CardMessage {
            when: "a WebGL step failed",
            fields: &[
                CardField::byte(
                    "kind",
                    "Record-kind tag: a fixed value meaning \"status record\" (27 in the main build).",
                ),
                CardField::number(
                    "status",
                    "First failed step: 0 = no typed arrays, 1 = no WebGL context, 2 = vertex createShader returned null, 3 = vertex shader failed to compile, 4 = fragment createShader returned null, 5 = fragment shader failed to compile, 6 = createProgram returned null, 7 = link failed.",
                ),
                CardField::null("digest", "Always a null marker (no hash in a status record)."),
            ],
        },
        CardMessage {
            when: "rendering succeeded",
            fields: &[
                CardField::byte(
                    "kind",
                    "Record-kind tag: a fixed value meaning \"digest record\" (57 in the main build).",
                ),
                CardField::null("status", "Always a null marker (no status in a digest record)."),
                CardField::null(
                    "digest",
                    "Signed 32-bit hash (h*31 + word) of the read-back pixel buffer, or a null marker if the hash came back null.",
                ),
            ],
        },
    ]),
};
