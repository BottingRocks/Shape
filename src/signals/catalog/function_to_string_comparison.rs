//! `FunctionToStringComparison`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LayoutLeaf, LayoutNode, LeafKind, LeafOp, SignalLayout, SlotView};
use crate::signals::extract::contracts::SourceClass;
use buffer_writes::slot_info;
use crate::signals::analysis::{buffer_writes, leaf_checks::*, layout_nodes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// The leaf of a field, also through a choice whose other arms are empty
/// (an optional write of that one leaf).
fn sole_leaf(node: &LayoutNode) -> Option<&LayoutLeaf> {
    match node {
        LayoutNode::Choice { arms } => {
            let mut written = arms.iter().filter(|arm| !matches!(arm.node, LayoutNode::Empty));
            match (written.next(), written.next()) {
                (Some(arm), None) => leaf_of(&arm.node),
                _ => None,
            }
        }
        other => leaf_of(other),
    }
}

/// Name every top-level leaf of every message by its kind.
fn bind_by_kind(layout: &SignalLayout, names: &[(LeafKind, &str)]) -> Result<FieldBinding, BindRefusal> {
    let mut binding = FieldBinding::new();
    for (index, message) in layout.messages.iter().enumerate() {
        if message.fields.len() != names.len() {
            return Err(BindRefusal::new(format!(
                "message {index}: expected {} fields, found {}",
                names.len(),
                message.fields.len()
            )));
        }
        let mut used = Vec::new();
        for field in &message.fields {
            let kind = sole_leaf(&field.node).map(|leaf| leaf.kind);
            let matches: Vec<&str> = names
                .iter()
                .filter(|(k, _)| Some(*k) == kind)
                .map(|(_, name)| *name)
                .collect();
            match matches.as_slice() {
                [name] if !used.contains(name) => {
                    used.push(*name);
                    binding.insert(index, field.key.clone(), *name);
                }
                _ => {
                    return Err(BindRefusal::new(format!(
                        "message {index}: field {} has no unique kind match",
                        field.key
                    )))
                }
            }
        }
    }
    Ok(binding)
}

/// Every emission is one flag byte and one counter number whose write sits
/// in a one-armed choice (`CHOICE:cond{NUMBER|}`: the encoder's buffer
/// argument is `count !== void 0 ? record : other`, the empty arm unreachable).
fn optional_counter(view: &SlotView) -> bool {
    !view.emissions().is_empty()
        && view.emissions().iter().all(|emission| {
            let [a, b] = emission.leaves.as_slice() else {
                return false;
            };
            let (number, byte) = match (a.op, b.op) {
                (LeafOp::Number, LeafOp::Byte) => (a, b),
                (LeafOp::Byte, LeafOp::Number) => (b, a),
                _ => return false,
            };
            number.repeat_depth == 0
                && byte.repeat_depth == 0
                && byte.choice_depth == 0
                && number.choice_depth == 1
                && (emission.shape.contains("{NUMBER|}") || emission.shape.contains("{|NUMBER}"))
        })
}

/// The counter leaf counts calls of a `toString` getter: its value reads a
/// binding whose `++` update sits in a function that returns
/// `Function.prototype.toString` (`function g() { ++n; return
/// Function.prototype.toString; }`). Read from the counter alone, so a
/// comparison byte the tracer wrote from another register still leaves the
/// slot identified.
fn counts_to_string_getter(view: &SlotView, ctx: &FinderCtx) -> bool {
    use oxc_ast::ast::Expression;
    use oxc_ast::AstKind;
    use oxc_span::GetSpan;
    let nodes = ctx.parse.nodes;
    let enclosing = |id: oxc_semantic::NodeId| {
        nodes
            .ancestor_ids(id)
            .find(|a| matches!(nodes.kind(*a), AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)))
    };
    let returns_to_string = |function: oxc_semantic::NodeId| {
        ctx.nodes_within(nodes.kind(function).span()).into_iter().any(|id| {
            let AstKind::ReturnStatement(ret) = nodes.kind(id) else { return false };
            let Some(Expression::StaticMemberExpression(outer)) = &ret.argument else { return false };
            let Expression::StaticMemberExpression(inner) = &outer.object else { return false };
            let Expression::Identifier(global) = &inner.object else { return false };
            outer.property.name == "toString"
                && inner.property.name == "prototype"
                && global.name == "Function"
                && ctx.identifier_symbol(global).is_none()
                && enclosing(id) == Some(function)
        })
    };
    view.leaves()
        .filter(|leaf| leaf.op == LeafOp::Number && has_transform(leaf, "count"))
        .filter_map(|leaf| ctx.value_slice(leaf, 1))
        .any(|slice| {
            slice
                .updates
                .iter()
                .any(|(_, update)| enclosing(*update).is_some_and(returns_to_string))
        })
}

/// `FunctionToStringComparison`. Card: [`CARD`].
pub(crate) fn recognize_function_to_string_comparison(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let shaped = plain_ops(view, &[LeafOp::Number, LeafOp::Byte])
        || plain_ops(view, &[LeafOp::Byte, LeafOp::Number])
        || optional_counter(view);
    if !shaped {
        return None;
    }
    let counter = view
        .leaves()
        .any(|leaf| leaf.op == LeafOp::Number && has_transform(leaf, "count") && leaf.classes.contains(&SourceClass::LoopIteration));
    if !counter {
        return None;
    }
    let info = slot_info(view, ctx);
    let compared = info.facts.member("toString") && info.facts.global("Function");
    let getter = info.facts.call("defineProperty") || view.leaves().any(|leaf| has_transform(leaf, ".defineProperty"));
    (compared || getter && info.facts.string("toString") || counts_to_string_getter(view, ctx))
        .then(|| claim(&["toString getter read counter", "implicit vs Function.prototype.toString comparison"]))
        .flatten()
}

pub(crate) fn bind_function_to_string_comparison(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    bind_by_kind(
        layout,
        &[(LeafKind::Number, "toStringGetterCalls"), (LeafKind::Byte, "probeFlags")],
    )
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `FunctionToStringComparison`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "FunctionToStringComparison",
    summary: "Creates an empty function and gives it its own 'toString' getter that counts how often it is read and returns the real Function.prototype.toString. It then compares the function turned into a string implicitly (\"\" + f) with Function.prototype.toString.call(f), and reports how many times the getter was read, whether the two strings matched, and whether the comparison threw.",
    why: "Likely detects patched or hooked Function.prototype.toString / valueOf (a common trick of automation tools and anti-detection extensions that try to make their patches look native); a normal browser gives count 1, equal, no error.",
    sample_payload: Some(r#"{
  "probeFlags": {
    "equal": true,
    "threw": false
  },
  "toStringGetterCalls": 1
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "toStringGetterCalls",
            "How many times the function's own toString getter was read during the comparison. 1 on an unmodified browser; 0 means valueOf or Symbol.toPrimitive was patched, more than 1 means a hooked toString reads it again.",
        ),
        CardField::byte(
            "probeFlags",
            "Two flags: threw (the comparison raised an exception) and equal (both string forms were identical). Other bits are 0. Normal browser: equal set, threw clear.",
        )
        .flags(&["threw", "equal"]),
    ]),
};
