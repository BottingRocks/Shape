//! Small expression algebra for ordered numeric state updates. No rewrites or
//! commutation: literal values, operators, and scoped identities remain exact.
use super::media_entropy::{number, symbol};
use super::*;
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::naming) enum E {
    S(SymbolId),
    N(u64),
    Field(Box<E>, String),
    Op(String, Box<E>, Box<E>),
    Void,
}
use E::*;
pub(in crate::naming) fn s(x: SymbolId) -> E {
    S(x)
}
pub(in crate::naming) fn n(x: f64) -> E {
    N(x.to_bits())
}
pub(in crate::naming) fn field(x: E, k: &str) -> E {
    Field(Box::new(x), k.into())
}
pub(in crate::naming) fn op(k: &str, a: E, b: E) -> E {
    Op(k.into(), Box::new(a), Box::new(b))
}
pub(in crate::naming) fn expr(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
) -> Option<E> {
    fn inner(model: &Model<'_, '_>, e: &Expression<'_>, depth: usize) -> Option<E> {
        if depth > 32 {
            return None;
        }
        let child = |e: &Expression<'_>| inner(model, e, depth + 1);
        Some(match e.without_parentheses() {
            Expression::Identifier(_) => s(symbol(model, e)?),
            Expression::NumericLiteral(v) => n(v.value),
            Expression::StaticMemberExpression(m) if !m.optional => {
                field(child(&m.object)?, m.property.name.as_str())
            }
            Expression::BinaryExpression(b) => {
                op(b.operator.as_str(), child(&b.left)?, child(&b.right)?)
            }
            Expression::LogicalExpression(b) => {
                op(b.operator.as_str(), child(&b.left)?, child(&b.right)?)
            }
            Expression::UnaryExpression(u)
                if u.operator.as_str() == "void" && number(&u.argument, 0.0) =>
            {
                Void
            }
            _ => return None,
        })
    }
    inner(model, e, 0)
}
pub(in crate::naming) fn eq(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    want: E,
) -> bool {
    expr(model, e) == Some(want)
}
/// A clock may be a direct callable or the result of the existing independently
/// source-proved elapsed-timer factory. Its value is established before use.
pub(in crate::naming) fn clock_origin(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
) -> bool {
    use super::media_entropy::{call, capture};
    use oxc_span::GetSpan;
    if super::super::resolve::ordered_callable(model, e, 0).is_some() {
        return true;
    }
    let Some(id) = capture(model, site, e) else {
        return false;
    };
    if model.binding_writes(id) == 2 {
        return branch_clock_origin(model, site, id);
    }
    if model.binding_writes(id) > 1 {
        return false;
    }
    let Some(value) = model.symbol_value(id) else {
        return false;
    };
    if value.span().end > site.span.start {
        return false;
    }
    let Some(factory) = call(value) else {
        return false;
    };
    if factory.arguments.len() > 1 || call_arguments(factory).len() != factory.arguments.len() {
        return false;
    }
    let Some(target) = super::super::resolve::ordered_callable(model, &factory.callee, 0) else {
        return false;
    };
    crate::naming::functions::scheduling::elapsed_timer::evaluate(
        model,
        target,
        &Facts::default(),
        &BTreeMap::new(),
    )
    .iter()
    .any(|p| p.name == "createElapsedTimer" && p.evaluator == "shared_elapsed_timer")
}

/// Two source-proved clock stores may occupy opposite if/else arms, or an
/// early-return setup and its outer fallthrough. Each callback must have its
/// own preceding store; all writers belong to the same enclosing function.
/// An earlier opaque initializer is preserved and overwritten by that proved
/// store; it is never treated as evidence about the clock value.
fn lexical_owner(model: &Model<'_, '_>, node: NodeId) -> Option<NodeId> {
    let mut current = node;
    for _ in 0..512 {
        let parent = model.nodes.parent_id(current);
        if parent == current {
            return None;
        }
        if matches!(
            model.nodes.kind(parent),
            AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
        ) {
            return Some(parent);
        }
        current = parent;
    }
    None
}

fn branch_clock_origin(model: &Model<'_, '_>, site: FunctionSite, id: SymbolId) -> bool {
    use super::media_entropy::{assigned_symbol, call, plain_binding};
    use oxc_span::GetSpan;
    let prove = || -> Option<()> {
        let AstKind::VariableDeclarator(v) = model.nodes.kind(model.scoping.symbol_declaration(id))
        else {
            return None;
        };
        if plain_binding(model, v, None) != Some(id) {
            return None;
        }
        let writes: Vec<_> = model
            .scoping
            .get_resolved_references(id)
            .filter(|r| r.is_write())
            .collect();
        if writes.len() != 2 {
            return None;
        }
        let owner = lexical_owner(model, site.node_id)?;
        let mut factory_site = None;
        let mut stores = Vec::new();
        let callback_block = model.nodes.parent_id(site.node_id);
        let mut dominates = false;
        for r in writes {
            let assignment_node = model.nodes.parent_id(r.node_id());
            let AstKind::AssignmentExpression(a) = model.nodes.kind(assignment_node) else {
                return None;
            };
            if v.span.end >= a.span.start
                || a.operator.as_str() != "="
                || assigned_symbol(model, &a.left) != Some(id)
                || lexical_owner(model, assignment_node) != Some(owner)
            {
                return None;
            }
            let statement = model.nodes.parent_id(assignment_node);
            if !matches!(model.nodes.kind(statement), AstKind::ExpressionStatement(_)) {
                return None;
            }
            let block = model.nodes.parent_id(statement);
            if !matches!(
                model.nodes.kind(block),
                AstKind::BlockStatement(_) | AstKind::FunctionBody(_)
            ) {
                return None;
            }
            if stores.iter().any(|(seen, _)| *seen == block) {
                return None;
            }
            stores.push((block, a.span));
            let factory = call(&a.right)?;
            if !factory.arguments.is_empty() {
                return None;
            }
            let target = super::super::resolve::ordered_callable(model, &factory.callee, 0)?;
            if factory_site.is_some_and(|p| p != target.span) {
                return None;
            }
            factory_site = Some(target.span);
            if !crate::naming::functions::scheduling::elapsed_timer::evaluate(
                model,
                target,
                &Facts::default(),
                &BTreeMap::new(),
            )
            .iter()
            .any(|p| p.name == "createElapsedTimer" && p.evaluator == "shared_elapsed_timer")
            {
                return None;
            }
            if block == callback_block && a.span.end < site.span.start {
                dominates = true;
            }
        }
        if !dominates {
            return None;
        }
        let [(left, left_store), (right, right_store)] = stores.as_slice() else {
            return None;
        };
        let lp = model.nodes.parent_id(*left);
        let rp = model.nodes.parent_id(*right);
        if lp == rp {
            let AstKind::IfStatement(branch) = model.nodes.kind(lp) else {
                return None;
            };
            return matches!(
                branch.alternate.as_ref(),
                Some(Statement::BlockStatement(_))
            )
            .then_some(());
        }
        let (inner, store, outer, outer_store) = if lp == owner {
            (*right, *right_store, *left, *left_store)
        } else if rp == owner {
            (*left, *left_store, *right, *right_store)
        } else {
            return None;
        };
        if model.nodes.parent_id(outer) != owner {
            return None;
        }
        let AstKind::BlockStatement(block) = model.nodes.kind(inner) else {
            return None;
        };
        if !matches!(block.body.last(), Some(Statement::ReturnStatement(_)))
            || outer_store.start < block.span.end
        {
            return None;
        }
        let mut ancestor = inner;
        for _ in 0..512 {
            if ancestor == owner {
                break;
            }
            if matches!(model.nodes.kind(ancestor), AstKind::TryStatement(t) if t.finalizer.is_some())
            {
                return None;
            }
            let parent = model.nodes.parent_id(ancestor);
            if parent == ancestor {
                return None;
            }
            ancestor = parent;
        }
        // A break/continue after the inner clock store could escape the return
        // and execute the outer reset. Refuse even conservatively nested ones.
        for node in model.nodes.iter() {
            if node.kind().span().start >= store.end
                && block.span.contains_inclusive(node.kind().span())
                && matches!(
                    node.kind(),
                    AstKind::BreakStatement(_) | AstKind::ContinueStatement(_)
                )
                && lexical_owner(model, node.id()) == Some(owner)
            {
                return None;
            }
        }
        Some(())
    };
    prove().is_some()
}
