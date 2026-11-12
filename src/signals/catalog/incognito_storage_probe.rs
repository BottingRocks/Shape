//! `IncognitoStorageProbe`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LayoutNode, LeafKind, LeafOp, SignalLayout, SlotView};
use crate::signals::analysis::{shape_text::*, leaf_checks::*, field_solver::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `IncognitoStorageProbe`. Card: [`CARD`].
pub(crate) fn recognize_incognito_storage_probe(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let shape = view.emissions().iter().all(|emission| {
        emission.leaves.iter().filter(|leaf| leaf.op == LeafOp::String).count() == 1
            && emission.leaves.iter().any(|leaf| leaf.op == LeafOp::Byte)
            && emission.leaves.iter().all(|leaf| leaf.op != LeafOp::Number)
    });
    if top_count_is(&view.shapes(), 2) && shape && ctx.has_role(view, "platform_result_emitter") {
        return claim(&["STRING + BYTE", "written by the proven platform_result_emitter"]);
    }
    // The emitter role is unique per program; a lowered try/catch copies the
    // fulfilment handler, so match the handler itself: a `.then` callback
    // of a call to the proven platform_detection_prober.
    if top_count_is(&view.shapes(), 2) && shape && platform::in_result_callback(view, ctx) {
        return claim(&["STRING + BYTE", "written in a .then callback of the proven platform_detection_prober"]);
    }
    None
}

pub(crate) fn bind_incognito_storage_probe(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    let code = |f: &Field| f.writes(LeafKind::String);
    // The flag is a byte select, or an `if` whose two arms write one tag
    // byte each (a literal pair, not a null marker).
    let tag_pair = |f: &Field| {
        matches!(f.node, LayoutNode::Choice { arms } if arms.len() == 2
            && arms.iter().all(|arm| matches!(&arm.node, LayoutNode::Leaf(leaf) if leaf.kind == LeafKind::Byte)))
    };
    let private = |f: &Field| !f.writes(LeafKind::String) && (f.writes(LeafKind::Byte) || tag_pair(f));
    solve_all_messages(view, layout, &[("browserCode", &code), ("isPrivateMode", &private)])
}

mod platform {
    //! The fulfilment handlers of the platform-detection promise.

    use oxc_ast::ast::{Argument, Expression};
    use oxc_ast::AstKind;
    use oxc_span::{GetSpan, Span};

    use crate::signals::analysis::{FinderCtx, SlotView};

    #[derive(Hash, PartialEq, Eq)]
    struct CallbacksKey;

    /// The function a callee denotes: a function declaration, or a fixed
    /// binding initialised (through other fixed bindings) with one.
    fn callee_function(ctx: &FinderCtx, callee: &Expression) -> Option<Span> {
        let scoping = ctx.parse.scoping;
        let nodes = ctx.parse.nodes;
        let mut current = callee.without_parentheses();
        for _ in 0..6 {
            match current {
                Expression::FunctionExpression(function) => return Some(function.span),
                Expression::ArrowFunctionExpression(arrow) => return Some(arrow.span),
                Expression::Identifier(identifier) => {
                    let symbol = ctx.identifier_symbol(identifier)?;
                    if scoping.symbol_declarations(symbol).count() != 1 {
                        return None;
                    }
                    let writes: Vec<_> =
                        scoping.get_resolved_references(symbol).filter(|reference| reference.is_write()).collect();
                    let declaration = scoping.symbol_declaration(symbol);
                    match (nodes.kind(declaration), writes.as_slice()) {
                        (AstKind::Function(function), []) => return Some(function.span),
                        (AstKind::VariableDeclarator(declarator), []) => {
                            current = declarator.init.as_ref()?.without_parentheses();
                        }
                        // `let f; ... f = g;`: one plain assignment, no initialiser.
                        (AstKind::VariableDeclarator(declarator), [write]) if declarator.init.is_none() => {
                            let assignment = nodes
                                .ancestor_ids(write.node_id())
                                .take(4)
                                .find_map(|id| match nodes.kind(id) {
                                    AstKind::AssignmentExpression(assignment) => Some(assignment),
                                    _ => None,
                                })?;
                            if assignment.operator != oxc_syntax::operator::AssignmentOperator::Assign
                                || !matches!(&assignment.left, oxc_ast::ast::AssignmentTarget::AssignmentTargetIdentifier(target) if target.span == nodes.kind(write.node_id()).span())
                            {
                                return None;
                            }
                            current = assignment.right.without_parentheses();
                        }
                        _ => return None,
                    }
                }
                _ => return None,
            }
        }
        None
    }

    /// Spans of every `prober().then(callback)` callback.
    fn callbacks(ctx: &FinderCtx) -> std::rc::Rc<Vec<Span>> {
        ctx.memo(CallbacksKey, || {
            let probers: Vec<Span> = ctx
                .claims
                .iter()
                .filter(|claim| claim.function && claim.role == "platform_detection_prober")
                .map(|claim| claim.span)
                .collect();
            let mut out = Vec::new();
            if probers.len() != 1 {
                return out;
            }
            for node in ctx.parse.nodes.iter() {
                let AstKind::CallExpression(call) = node.kind() else { continue };
                let Expression::StaticMemberExpression(member) = call.callee.without_parentheses() else {
                    continue;
                };
                if member.property.name != "then" {
                    continue;
                }
                let Expression::CallExpression(receiver) = member.object.without_parentheses() else {
                    continue;
                };
                if callee_function(ctx, &receiver.callee) != Some(probers[0]) {
                    continue;
                }
                // Inline, or a named handler (`.then(handler)`).
                let Some(callback) = call.arguments.first().and_then(Argument::as_expression).and_then(|argument| callee_function(ctx, argument))
                else {
                    continue;
                };
                out.push(callback);
            }
            out
        })
    }

    /// Every write of the slot lies inside one fulfilment handler of the
    /// platform-detection promise.
    pub(crate) fn in_result_callback(view: &SlotView, ctx: &FinderCtx) -> bool {
        let callbacks = callbacks(ctx);
        let mut writes = view.leaves().map(|leaf| leaf.write.site()).peekable();
        writes.peek().is_some()
            && writes.all(|site| {
                callbacks
                    .iter()
                    .any(|callback| callback.start <= site.start && site.end <= callback.end)
            })
    }
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `IncognitoStorageProbe`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "IncognitoStorageProbe",
    summary: "Detects whether the browser is in private/incognito mode. It first guesses the browser family from navigator.vendor, the userAgent, a few engine-specific properties and the length of eval.toString(). It then runs that browser's private-mode test: on Safari, openDatabase/localStorage write failures or an IndexedDB Blob store error; on Chromium, a temporary storage quota smaller than the JS heap limit (or webkitRequestFileSystem failing); on Firefox, a missing navigator.serviceWorker; on IE/legacy Edge, a missing indexedDB. It cleans up the temporary database and storage key afterwards. On some paths where the needed API is missing, it never reports.",
    why: "Private browsing and incognito windows are common with fraud, account farming and bot tooling. The browser-family code also lets the vendor check the claimed user agent against how the engine actually behaves (likely).",
    sample_payload: Some(r#"{
  "browserCode": "D",
  "isPrivateMode": false
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::string(
            "browserCode",
            "Browser family the probe detected: \"F\" Safari, \"A\" Brave, \"B\" Edge (Chromium), \"C\" Opera, \"D\" other Chrome, \"E\" Google-vendor browser without \"Chrome\" in the userAgent, \"G\" Firefox, \"H\" IE/legacy Edge, \"UD\" unknown.",
        ),
        CardField::boolean(
            "isPrivateMode",
            "Whether private/incognito mode was detected. The byte holds one of two tag values meaning true or false, and the two values differ between builds. Always false when browserCode is \"UD\".",
        ),
    ]),
};

