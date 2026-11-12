//! Projected layout trees ([`LayoutNode`]): leaf counts and kinds, wire-shape
//! equality, and aligning a message's top-level fields with the view leaves
//! that fill them.
//!
//! Helps binders that walk the projected layout before naming fields.
//! Used by: `CanvasReadbackProbe`, `CanvasScenePixelSamples`,
//! `CanvasToDataUrl`, `EmojiChannelSampleHalves`, `EmojiChannelSamples`,
//! `EmojiGlyphRgbaSamples`, `FontAvailability`, `FontMetrics`,
//! `FunctionToStringComparison`, `OfflineAudioRenderSamples`, `UserAgentData`,
//! `VideoCanPlayType`, `WebGlContext`, `WebGlRenderHash`; helper modules
//! [`described_fields`](super::described_fields),
//! [`field_solver`](super::field_solver), [`flag_bytes`](super::flag_bytes).

use super::{BindRefusal, FieldBinding, FinderCtx, LayoutField, LayoutLeaf, LayoutNode, LeafKind, LeafOp, LeafView, MessageLayout, SignalLayout, SlotView};

/// Leaves under a layout node, in projection order (one per leaf or byte
/// select; the view leaves of the node).
pub(crate) fn node_leaf_count(node: &LayoutNode) -> usize {
    match node {
        LayoutNode::Leaf(_) | LayoutNode::ByteSelect { .. } => 1,
        LayoutNode::Array { item } => node_leaf_count(item),
        LayoutNode::Record { fields } => fields.iter().map(|field| node_leaf_count(&field.node)).sum(),
        LayoutNode::Choice { arms } => arms.iter().map(|arm| node_leaf_count(&arm.node)).sum(),
        LayoutNode::Empty | LayoutNode::Unknown { .. } => 0,
    }
}

/// The projected leaf kinds under a node, in order.
pub(crate) fn node_kinds(node: &LayoutNode, out: &mut Vec<LeafKind>) {
    match node {
        LayoutNode::Leaf(leaf) => out.push(leaf.kind),
        LayoutNode::ByteSelect { .. } => out.push(LeafKind::Byte),
        LayoutNode::Array { item } => node_kinds(item, out),
        LayoutNode::Record { fields } => fields.iter().for_each(|field| node_kinds(&field.node, out)),
        LayoutNode::Choice { arms } => arms.iter().for_each(|arm| node_kinds(&arm.node, out)),
        LayoutNode::Empty | LayoutNode::Unknown { .. } => {}
    }
}

/// A projected leaf kind accepts a view leaf op (`Value` accepts any).
pub(crate) fn kind_matches(kind: LeafKind, op: LeafOp) -> bool {
    matches!(
        (kind, op),
        (LeafKind::Value, _)
            | (LeafKind::Number, LeafOp::Number)
            | (LeafKind::String, LeafOp::String)
            | (LeafKind::Byte, LeafOp::Byte)
    )
}

/// Whether a node writes a non-marker value of `kind` (a literal byte is a
/// null/absent marker, not a value).
pub(crate) fn writes_value(node: &LayoutNode, kind: LeafKind) -> bool {
    match node {
        LayoutNode::Leaf(leaf) => leaf.kind == kind && leaf.byte.is_none(),
        LayoutNode::ByteSelect { .. } => kind == LeafKind::Byte,
        LayoutNode::Array { item } => writes_value(item, kind),
        LayoutNode::Record { fields } => fields.iter().any(|field| writes_value(&field.node, kind)),
        LayoutNode::Choice { arms } => arms.iter().any(|arm| writes_value(&arm.node, kind)),
        LayoutNode::Empty | LayoutNode::Unknown { .. } => false,
    }
}

/// A stable ordering code of a leaf kind (for kind sets).
pub(crate) fn kind_code(kind: LeafKind) -> u8 {
    match kind {
        LeafKind::Number => 0,
        LeafKind::String => 1,
        LeafKind::Byte => 2,
        LeafKind::Value => 3,
    }
}

/// The projected leaf kinds under a node, each with whether it is a literal byte.
pub(crate) fn leaf_kinds(node: &LayoutNode, out: &mut Vec<(LeafKind, bool)>) {
    match node {
        LayoutNode::Leaf(leaf) => out.push((leaf.kind, leaf.byte.is_some())),
        LayoutNode::ByteSelect { .. } => out.push((LeafKind::Byte, false)),
        LayoutNode::Array { item } => leaf_kinds(item, out),
        LayoutNode::Record { fields } => fields.iter().for_each(|field| leaf_kinds(&field.node, out)),
        LayoutNode::Choice { arms } => arms.iter().for_each(|arm| leaf_kinds(&arm.node, out)),
        LayoutNode::Empty | LayoutNode::Unknown { .. } => {}
    }
}

/// The leaf of a plain leaf or byte-select node.
pub(crate) fn leaf_of(node: &LayoutNode) -> Option<&LayoutLeaf> {
    match node {
        LayoutNode::Leaf(leaf) | LayoutNode::ByteSelect { leaf, .. } => Some(leaf),
        _ => None,
    }
}

/// Equal wire structure of two nodes, record field keys included.
pub(crate) fn node_shape_eq(left: &LayoutNode, right: &LayoutNode) -> bool {
    match (left, right) {
        (LayoutNode::Leaf(a), LayoutNode::Leaf(b)) => leaf_shape_eq(a, b),
        (
            LayoutNode::ByteSelect { leaf: a, then_byte: at, else_byte: ae },
            LayoutNode::ByteSelect { leaf: b, then_byte: bt, else_byte: be },
        ) => leaf_shape_eq(a, b) && at == bt && ae == be,
        (LayoutNode::Array { item: a }, LayoutNode::Array { item: b }) => node_shape_eq(a, b),
        (LayoutNode::Record { fields: a }, LayoutNode::Record { fields: b }) => fields_shape_eq(a, b),
        (LayoutNode::Choice { arms: a }, LayoutNode::Choice { arms: b }) => {
            a.len() == b.len()
                && a.iter().zip(b).all(|(x, y)| x.label == y.label && node_shape_eq(&x.node, &y.node))
        }
        (LayoutNode::Empty, LayoutNode::Empty) => true,
        (LayoutNode::Unknown { reason: a }, LayoutNode::Unknown { reason: b }) => a == b,
        _ => false,
    }
}

/// Equal wire structure of two leaves (kind, mask, literal byte, XOR operand, key).
pub(crate) fn leaf_shape_eq(a: &LayoutLeaf, b: &LayoutLeaf) -> bool {
    a.kind == b.kind && a.mask == b.mask && a.byte == b.byte && a.xor == b.xor && a.key == b.key
}

/// Equal keys and wire structure of two field lists.
pub(crate) fn fields_shape_eq(a: &[crate::signals::analysis::LayoutField], b: &[crate::signals::analysis::LayoutField]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.key == y.key && node_shape_eq(&x.node, &y.node))
}

/// An emission part renders `message`: same mask and field structure.
pub(crate) fn same_message(part: &MessageLayout, message: &MessageLayout) -> bool {
    part.mask == message.mask && fields_shape_eq(&part.fields, &message.fields)
}

/// Equal wire structure (kinds, masks, literal bytes, XOR operands, keys),
/// ignoring provenance.
pub(crate) fn same_fields(left: &[crate::signals::analysis::LayoutField], right: &[crate::signals::analysis::LayoutField]) -> bool {
    left.len() == right.len() && left.iter().zip(right).all(|(a, b)| same_node(&a.node, &b.node))
}

/// Equal wire structure of two nodes, record field keys ignored (see [`same_fields`]).
pub(crate) fn same_node(left: &LayoutNode, right: &LayoutNode) -> bool {
    let leaf = |a: &crate::signals::analysis::LayoutLeaf, b: &crate::signals::analysis::LayoutLeaf| {
        a.kind == b.kind && a.mask == b.mask && a.byte == b.byte && a.xor == b.xor && a.key == b.key
    };
    match (left, right) {
        (LayoutNode::Leaf(a), LayoutNode::Leaf(b)) => leaf(a, b),
        (
            LayoutNode::ByteSelect {
                leaf: a,
                then_byte: at,
                else_byte: ae,
            },
            LayoutNode::ByteSelect {
                leaf: b,
                then_byte: bt,
                else_byte: be,
            },
        ) => leaf(a, b) && at == bt && ae == be,
        (LayoutNode::Array { item: a }, LayoutNode::Array { item: b }) => same_node(a, b),
        (LayoutNode::Record { fields: a }, LayoutNode::Record { fields: b }) => same_fields(a, b),
        (LayoutNode::Choice { arms: a }, LayoutNode::Choice { arms: b }) => {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.label == y.label && same_node(&x.node, &y.node))
        }
        (LayoutNode::Empty, LayoutNode::Empty) => true,
        (LayoutNode::Unknown { reason: a }, LayoutNode::Unknown { reason: b }) => a == b,
        _ => false,
    }
}

/// The node is a repeat (`Array`).
pub(crate) fn is_array(node: &LayoutNode) -> bool {
    matches!(node, LayoutNode::Array { .. })
}

/// The node is a plain number leaf.
pub(crate) fn is_scalar(node: &LayoutNode) -> bool {
    matches!(node, LayoutNode::Leaf(leaf) if leaf.kind == LeafKind::Number)
}

/// The view leaves of each top-level field of message `message`, aligned by
/// evaluation order with the emission that first produced the layout.
pub(crate) fn field_leaves<'v>(view: &'v SlotView, layout: &SignalLayout, message: usize) -> Option<Vec<Vec<&'v LeafView>>> {
    let target = layout.messages.get(message)?;
    for emission in &layout.emissions {
        let leaves = &view.emissions().get(emission.emission)?.leaves;
        if !emission.covers(leaves.len()) {
            continue;
        }
        for (index, (_, part)) in emission.parts.iter().enumerate() {
            if part.fields == target.fields && part.mask == target.mask {
                let indices = emission.part_leaves(index);
                let mut cursor = 0;
                let mut out = Vec::new();
                for field in &part.fields {
                    let count = node_leaf_count(&field.node);
                    out.push(indices.get(cursor..cursor + count)?.iter().map(|leaf| &leaves[*leaf]).collect());
                    cursor += count;
                }
                return Some(out);
            }
        }
    }
    None
}

/// `count, array` pairs of a message: every array field must be directly
/// preceded by a scalar count. Returns `(count index, array index)`.
pub(crate) fn count_array_pairs(fields: &[LayoutField]) -> Option<Vec<(usize, usize)>> {
    let mut pairs = Vec::new();
    for (index, field) in fields.iter().enumerate() {
        if is_array(&field.node) {
            if index == 0 || !is_scalar(&fields[index - 1].node) {
                return None;
            }
            pairs.push((index - 1, index));
        }
    }
    Some(pairs)
}

/// A binder refusal prefixed with the signal name.
pub(crate) fn refuse(signal: &str, reason: impl std::fmt::Display) -> BindRefusal {
    BindRefusal::new(format!("{signal}: {reason}"))
}

/// Bind a `count, array<scalar>` message (`[count, items]` in either the
/// only layout of a single-message signal).
pub(crate) fn bind_count_and_items(
    signal: &str,
    layout: &SignalLayout,
    count: &str,
    items: &str,
) -> Result<FieldBinding, BindRefusal> {
    let mut binding = FieldBinding::new();
    for (message, layout_message) in layout.messages.iter().enumerate() {
        let fields = &layout_message.fields;
        match fields.as_slice() {
            [first, second] if is_scalar(&first.node) && is_array(&second.node) => {
                binding.insert(message, first.key.clone(), count);
                binding.insert(message, second.key.clone(), items);
            }
            _ => return Err(refuse(signal, format!("message {message}: expected `count, repeat`"))),
        }
    }
    Ok(binding)
}

/// Member path of a leaf's caller-side value.
pub(crate) fn leaf_path(ctx: &FinderCtx, leaf: &LeafView) -> Vec<crate::signals::analysis::write_sites::PathStep> {
    ctx.caller_value_node(leaf).map(|node| ctx.member_path(node, 6)).unwrap_or_default()
}
