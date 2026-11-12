//! Bounded rolling-frontier proofs for a captured native-code predicate whose
//! RegExp source is a late, single straight-line assignment in the enclosing
//! body. Identity comes only from the owned body plus the scoped producer and
//! its use order; generated spellings, reference labels and source offsets are
//! never consulted.
use super::*;
use oxc_span::GetSpan;

type Symbol = oxc_semantic::SymbolId;

fn symbol(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<Symbol> {
    crate::analysis::expression_symbol(e, model.scoping)
}

fn unchanged(model: &Model<'_, '_>, s: Symbol) -> bool {
    model.binding_writes(s) == 0 && model.scoping.symbol_redeclarations(s).is_empty()
}

fn member<'a>(e: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(m) = e else {
        return None;
    };
    (!m.optional && m.property.name.as_str() == key).then_some(&m.object)
}

// Only exact unshadowed global roots participate. A local Function/RegExp
// binding is not the host namespace, regardless of its spelling.
fn global_path(model: &Model<'_, '_>, e: &Expression<'_>, path: &[&str]) -> bool {
    let Some((last, prefix)) = path.split_last() else {
        return false;
    };
    match (e, prefix) {
        (Expression::Identifier(i), []) => {
            crate::analysis::identifier_symbol(i, model.scoping).is_none()
                && i.name.as_str() == *last
        }
        (Expression::StaticMemberExpression(m), _) if !m.optional => {
            m.property.name.as_str() == *last && global_path(model, &m.object, prefix)
        }
        _ => false,
    }
}

fn plain_variable<'a>(model: &Model<'_, '_>, v: &'a VariableDeclarator<'a>) -> Option<Symbol> {
    let BindingPattern::BindingIdentifier(id) = &v.id else {
        return None;
    };
    let s = id.symbol_id.get()?;
    model
        .scoping
        .symbol_redeclarations(s)
        .is_empty()
        .then_some(s)
}

fn declarator_container(
    model: &Model<'_, '_>,
    declaration: oxc_semantic::NodeId,
) -> Option<oxc_semantic::NodeId> {
    if !matches!(
        model.nodes.kind(declaration),
        AstKind::VariableDeclarator(_)
    ) || !matches!(
        model.nodes.parent_kind(declaration),
        AstKind::VariableDeclaration(_)
    ) || matches!(
        model.nodes.parent_kind(model.nodes.parent_id(declaration)),
        AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
    ) {
        return None;
    }
    let container = model.nodes.parent_id(model.nodes.parent_id(declaration));
    matches!(
        model.nodes.kind(container),
        AstKind::FunctionBody(_) | AstKind::Program(_)
    )
    .then_some(container)
}

// The stable native-code suffix. These are host semantics, not build tokens or
// opaque strings from the capture.
const NATIVE_PATTERNS: &[&str] = &[
    r"{\s*\[\s*native\s+code\s*]\s*}\s*$",
    r"\{\s*\[native code\]\s*\}\s*$",
];

// Resolve the captured receiver to one plain declaration without initializer,
// whose unique write is a straight-line `receiver = producer` in the same
// container, and whose producer is exactly `RegExp(<native-code literal>)`.
fn native_producer(model: &Model<'_, '_>, site: FunctionSite, receiver: Symbol) -> Option<u32> {
    let declaration = model.scoping.symbol_declaration(receiver);
    let AstKind::VariableDeclarator(v) = model.nodes.kind(declaration) else {
        return None;
    };
    if plain_variable(model, v) != Some(receiver)
        || v.init.is_some()
        || model.binding_writes(receiver) != 1
        || !model.scoping.symbol_redeclarations(receiver).is_empty()
        || site
            .span
            .contains_inclusive(model.scoping.symbol_span(receiver))
    {
        return None;
    }
    let container = declarator_container(model, declaration)?;
    let mut assignment = None;
    for reference in model.scoping.get_resolved_references(receiver) {
        if !reference.is_write() {
            continue;
        }
        if assignment.is_some() {
            return None;
        }
        let parent = model.nodes.parent_id(reference.node_id());
        let AstKind::AssignmentExpression(a) = model.nodes.kind(parent) else {
            return None;
        };
        let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else {
            return None;
        };
        if a.operator.as_str() != "="
            || crate::analysis::identifier_symbol(id, model.scoping) != Some(receiver)
            || !matches!(
                model.nodes.parent_kind(parent),
                AstKind::ExpressionStatement(_)
            )
            || model.nodes.parent_id(model.nodes.parent_id(parent)) != container
        {
            return None;
        }
        assignment = Some((a.span.end, &a.right));
    }
    let (assignment_end, producer) = assignment?;
    let Expression::Identifier(_producer_id) = producer else {
        return None;
    };
    let producer = symbol(model, producer)?;
    if !unchanged(model, producer)
        || site
            .span
            .contains_inclusive(model.scoping.symbol_span(producer))
    {
        return None;
    }
    let producer_declaration = model.scoping.symbol_declaration(producer);
    let AstKind::VariableDeclarator(pv) = model.nodes.kind(producer_declaration) else {
        return None;
    };
    if plain_variable(model, pv) != Some(producer)
        || declarator_container(model, producer_declaration)? != container
        || model.nodes.get_node(producer_declaration).span().end > assignment_end
    {
        return None;
    }
    let Expression::CallExpression(call) = pv.init.as_ref()? else {
        return None;
    };
    if call.optional
        || !global_path(model, &call.callee, &["RegExp"])
        || !matches!(
            call.arguments.as_slice(),
            [Argument::StringLiteral(literal)] if NATIVE_PATTERNS.contains(&literal.value.as_str())
        )
    {
        return None;
    }
    // Every other read of the pattern must either occur after its single write
    // or be the candidate's own test. No sibling consumer can observe a
    // half-initialized capture before the producer assignment.
    for reference in model.scoping.get_resolved_references(receiver) {
        if reference.is_write() {
            continue;
        }
        let span = model.nodes.get_node(reference.node_id()).span();
        if span.start >= assignment_end || site.span.contains_inclusive(span) {
            continue;
        }
        return None;
    }
    Some(assignment_end)
}

// The predicate may be textually declared before its captured pattern is
// assigned. Prove that no value derived from the predicate can be observed
// before that assignment: every pre-assignment reference must be a plain
// alias declarator whose own references all follow the assignment.
fn uses_follow_assignment(
    model: &Model<'_, '_>,
    function: Symbol,
    assignment_end: u32,
    container: oxc_semantic::NodeId,
) -> bool {
    if model.binding_writes(function) != 0
        || !model.scoping.symbol_redeclarations(function).is_empty()
    {
        return false;
    }
    for reference in model.scoping.get_resolved_references(function) {
        let span = model.nodes.get_node(reference.node_id()).span();
        if span.start >= assignment_end {
            continue;
        }
        if reference.is_write() {
            return false;
        }
        let parent = model.nodes.parent_id(reference.node_id());
        let AstKind::VariableDeclarator(v) = model.nodes.kind(parent) else {
            return false;
        };
        let Some(Expression::Identifier(init)) = v.init.as_ref() else {
            return false;
        };
        if crate::analysis::identifier_symbol(init, model.scoping) != Some(function)
            || !matches!(
                model.nodes.parent_kind(parent),
                AstKind::VariableDeclaration(_)
            )
            || matches!(
                model.nodes.parent_kind(model.nodes.parent_id(parent)),
                AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
            )
            || model.nodes.parent_id(model.nodes.parent_id(parent)) != container
        {
            return false;
        }
        let Some(alias) = plain_variable(model, v) else {
            return false;
        };
        if !unchanged(model, alias) {
            return false;
        }
        if model
            .scoping
            .get_resolved_references(alias)
            .any(|r| model.nodes.get_node(r.node_id()).span().start < assignment_end)
        {
            return false;
        }
    }
    true
}

fn native_source_test(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [Statement::IfStatement(branch)] = body.statements.as_slice() else {
        return None;
    };
    let Expression::LogicalExpression(and) = &branch.test else {
        return None;
    };
    let Expression::BinaryExpression(kind) = &and.left else {
        return None;
    };
    let Expression::UnaryExpression(typeof_) = &kind.left else {
        return None;
    };
    let Expression::BinaryExpression(identity) = &and.right else {
        return None;
    };
    if and.operator.as_str() != "&&"
        || kind.operator.as_str() != "==="
        || typeof_.operator.as_str() != "typeof"
        || model.site_parameter_slot(&typeof_.argument, site) != Some(0)
        || !matches!(&kind.right, Expression::StringLiteral(s) if s.value == "function")
        || identity.operator.as_str() != "==="
        || model.site_parameter_slot(member(&identity.left, "toString")?, site) != Some(0)
        || !global_path(
            model,
            &identity.right,
            &["Function", "prototype", "toString"],
        )
    {
        return None;
    }
    let Statement::BlockStatement(yes) = &branch.consequent else {
        return None;
    };
    let Statement::BlockStatement(no) = branch.alternate.as_ref()? else {
        return None;
    };
    let [Statement::ReturnStatement(yes)] = yes.body.as_slice() else {
        return None;
    };
    let [Statement::ReturnStatement(no)] = no.body.as_slice() else {
        return None;
    };
    if !matches!(no.argument.as_ref()?, Expression::BooleanLiteral(v) if !v.value) {
        return None;
    }
    let Expression::UnaryExpression(outer) = yes.argument.as_ref()? else {
        return None;
    };
    let Expression::UnaryExpression(inner) = &outer.argument else {
        return None;
    };
    if outer.operator.as_str() != "!" || inner.operator.as_str() != "!" {
        return None;
    }
    let Expression::CallExpression(exec) = &inner.argument else {
        return None;
    };
    let arguments = call_arguments(exec);
    let [argument] = arguments.as_slice() else {
        return None;
    };
    let Expression::CallExpression(string) = argument else {
        return None;
    };
    if exec.optional
        || exec.arguments.len() != 1
        || string.optional
        || !string.arguments.is_empty()
        || model.site_parameter_slot(member(&string.callee, "toString")?, site) != Some(0)
    {
        return None;
    }
    let receiver = symbol(model, member(&exec.callee, "exec")?)?;
    let assignment_end = native_producer(model, site, receiver)?;
    let function = site.binding_symbol?;
    let container = declarator_container(model, model.scoping.symbol_declaration(receiver))?;
    uses_follow_assignment(model, function, assignment_end, container).then_some(())
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return Vec::new();
    };
    let Some(body) = &function.body else {
        return Vec::new();
    };
    if function.r#async || function.generator || !body.directives.is_empty() {
        return Vec::new();
    }
    let [parameter] = function.params.items.as_slice() else {
        return Vec::new();
    };
    let BindingPattern::BindingIdentifier(formal) = &parameter.pattern else {
        return Vec::new();
    };
    if function.params.rest.is_some()
        || parameter.initializer.is_some()
        || formal.symbol_id.get().is_none_or(|s| !unchanged(model, s))
        || native_source_test(model, site, body).is_none()
    {
        return Vec::new();
    }
    vec![proposal(
        "isNativeFunction",
        "shared_frontier_native_producer",
        "exact typeof/function and original Function.prototype.toString guard; double-negated exec of a captured native-code RegExp on the same formal's zero-argument toString; false alternate; the capture is a single straight-line assignment whose RegExp producer is initialized earlier in the same container, and every predicate use follows that assignment",
        f,
    )]
}
