//! Accessor-only proof of captured descriptor/cache helpers. These functions
//! inspect source and scoped writers; proposals and sibling-family roles are
//! never inputs. The apply/factory captures remain opaque call protocols.
use super::*;
use oxc_span::GetSpan;

fn stable(model: &Model<'_, '_>, s: Symbol) -> bool {
    model.binding_writes(s) == 0 && model.scoping.symbol_redeclarations(s).is_empty()
}

// A hoisted interceptor can precede capture initialization. Refuse exposure
// before initialization, including through an early alias (bounded recursion).
fn exposed_after(
    model: &Model<'_, '_>,
    s: Symbol,
    end: u32,
    container: oxc_semantic::NodeId,
    depth: usize,
) -> bool {
    if depth > 8 || !stable(model, s) {
        return false;
    }
    model.scoping.get_resolved_references(s).all(|r| {
        // A textually later reference inside a hoisted nested function is not
        // evidence that initialization ran. Only direct uses in the capture
        // container (or the recursively checked early alias) establish order.
        let mut ancestor = r.node_id();
        while ancestor != container {
            if matches!(
                model.nodes.kind(ancestor),
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) | AstKind::Program(_)
            ) {
                return false;
            }
            ancestor = model.nodes.parent_id(ancestor);
        }
        if model.nodes.get_node(r.node_id()).span().start >= end {
            return true;
        }
        let node = model.nodes.parent_id(r.node_id());
        let AstKind::VariableDeclarator(v) = model.nodes.kind(node) else {
            return false;
        };
        let Some(init) = v.init.as_ref() else {
            return false;
        };
        symbol(model, init) == Some(s)
            && matches!(
                model.nodes.parent_kind(node),
                AstKind::VariableDeclaration(_)
            )
            && matches!(
                model
                    .nodes
                    .kind(model.nodes.parent_id(model.nodes.parent_id(node))),
                AstKind::FunctionBody(_) | AstKind::Program(_)
            )
            && declared(model, v)
                .is_some_and(|alias| exposed_after(model, alias, end, container, depth + 1))
    })
}

fn origin<'a>(
    model: &Model<'_, 'a>,
    s: Symbol,
    outer: Symbol,
    depth: usize,
) -> Option<&'a Function<'a>> {
    if depth > 8 || !model.scoping.symbol_redeclarations(s).is_empty() {
        return None;
    }
    let node = model.scoping.symbol_declaration(s);
    if let AstKind::Function(f) = model.nodes.kind(node) {
        return (stable(model, s)
            && matches!(
                model.nodes.parent_kind(node),
                AstKind::FunctionBody(_) | AstKind::Program(_)
            ))
        .then_some(f);
    }
    let AstKind::VariableDeclarator(v) = model.nodes.kind(node) else {
        return None;
    };
    if declared(model, v) != Some(s)
        || !matches!(
            model.nodes.parent_kind(node),
            AstKind::VariableDeclaration(_)
        )
    {
        return None;
    }
    let container = model.nodes.parent_id(model.nodes.parent_id(node));
    if !matches!(
        model.nodes.kind(container),
        AstKind::FunctionBody(_) | AstKind::Program(_)
    ) {
        return None;
    }
    let (value, end) = if let Some(init) = v.init.as_ref() {
        if !stable(model, s) {
            return None;
        }
        (init, v.span.end)
    } else {
        if model.binding_writes(s) != 1 {
            return None;
        }
        let r = model
            .scoping
            .get_resolved_references(s)
            .find(|r| r.is_write())?;
        let parent = model.nodes.parent_id(r.node_id());
        let AstKind::AssignmentExpression(a) = model.nodes.kind(parent) else {
            return None;
        };
        if a.operator.as_str() != "="
            || target_symbol(model, &a.left) != Some(s)
            || !matches!(
                model.nodes.parent_kind(parent),
                AstKind::ExpressionStatement(_)
            )
            || model.nodes.parent_id(model.nodes.parent_id(parent)) != container
        {
            return None;
        }
        (&a.right, a.span.end)
    };
    if model
        .nodes
        .parent_id(model.scoping.symbol_declaration(outer))
        != container
        || !exposed_after(model, outer, end, container, 0)
    {
        return None;
    }
    let source = symbol(model, value)?;
    // An initialized alias may not read another alias before its initializer.
    let declaration = model.scoping.symbol_declaration(source);
    if let AstKind::VariableDeclarator(source_v) = model.nodes.kind(declaration) {
        if source_v.init.is_none() || source_v.span.end >= value.span().start {
            return None;
        }
    }
    origin(model, source, outer, depth + 1)
}

fn helper<'a>(
    model: &Model<'_, 'a>,
    init: &Expression<'_>,
    outer: Symbol,
) -> Option<(FunctionSite, &'a Function<'a>)> {
    let callee = symbol(model, &call(init)?.callee)?;
    let f = origin(model, callee, outer, 0)?;
    if f.r#async
        || f.generator
        || f.params.rest.is_some()
        || f.params.items.len() != 2
        || f.params
            .items
            .iter()
            .any(|p| p.initializer.is_some() || plain_parameter(model, &p.pattern).is_none())
        || !f.body.as_ref()?.directives.is_empty()
    {
        return None;
    }
    Some((model.function_from_symbol(callee)?, f))
}

pub(super) fn descriptor(model: &Model<'_, '_>, item: &Item<'_>, outer: Symbol) -> Option<()> {
    let (_, init) = declared_init(model, item)?;
    let (site, f) = helper(model, init, outer)?;
    // This existing structural utility independently checks the complete
    // prototype walk, exact global calls, receiver/key, returns, and writers.
    crate::naming::shared::descriptor_search(model, site, f.body.as_ref()?)
}

pub(super) fn registry(model: &Model<'_, '_>, item: &Item<'_>, outer: Symbol) -> Option<Reference> {
    let (_, init) = declared_init(model, item)?;
    let (_, f) = helper(model, init, outer)?;
    let owner = plain_parameter(model, &f.params.items[0].pattern)?;
    let property = plain_parameter(model, &f.params.items[1].pattern)?;
    if !stable(model, owner) || !stable(model, property) {
        return None;
    }
    let items = flatten(&f.body.as_ref()?.statements);
    let [guard, counter_item, scan, end] = items.as_slice() else {
        return None;
    };
    if !returns_null(guard)
        || !matches!(item_statement(guard), Some(Statement::IfStatement(g)) if g.alternate.is_none())
        || !matches!(item_statement(end), Some(Statement::ReturnStatement(r)) if r.argument.as_ref().is_some_and(is_null))
    {
        return None;
    }
    let registry = bucket_null_test(model, branch_test(guard)?, property)?;
    let (counter, init) = declared_init(model, counter_item)?;
    if !number(init, 0.0) {
        return None;
    }
    let Statement::WhileStatement(scan) = item_statement(scan)? else {
        return None;
    };
    let (left, right) = binary_test(&scan.test, "<")?;
    if symbol(model, left) != Some(counter) || !bucket_length(model, right, property, &registry) {
        return None;
    }
    let [Statement::IfStatement(hit)] = block(&scan.body)?.body.as_slice() else {
        return None;
    };
    let (left, right) = binary_test(&hit.test, "===")?;
    if symbol(model, left) != Some(owner)
        || !member(right, "object")
            .is_some_and(|e| bucket_element(model, e, property, &registry, counter))
    {
        return None;
    }
    let [Statement::ReturnStatement(ret)] = block(&hit.consequent)?.body.as_slice() else {
        return None;
    };
    if !ret
        .argument
        .as_ref()
        .is_some_and(|e| bucket_element(model, e, property, &registry, counter))
    {
        return None;
    }
    let [step, Statement::ContinueStatement(next)] =
        block(hit.alternate.as_ref()?)?.body.as_slice()
    else {
        return None;
    };
    if !increment_index(model, step, counter)
        || next.label.is_some()
        || model.binding_writes(counter) != 1
    {
        return None;
    }
    Some(registry)
}
