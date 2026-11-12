//! A finite operational predicate, without an ASCII, hostname or policy claim.
//!
//! This first lifetime variant accepts only four distinct, write-free lexical
//! `let` declarations without initializers. They contain undefined after their
//! declaration executes, NOT numeric character bounds. Every enclosing-owner
//! reference must be a direct sibling call after all four declarations. An
//! unreferenced owner is also safe, but is never treated as executed evidence.
//! Relational coercions and short-circuit order remain part of the contract;
//! even undefined bounds do not justify reducing an object input to `x === 45`.
use super::*;
use oxc_span::GetSpan;

fn binary<'a>(e: &'a Expression<'a>, op: &str) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::BinaryExpression(b) = e.without_parentheses() else {
        return None;
    };
    (b.operator.as_str() == op).then_some((&b.left, &b.right))
}
fn logical<'a>(
    e: &'a Expression<'a>,
    op: &str,
) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::LogicalExpression(b) = e.without_parentheses() else {
        return None;
    };
    (b.operator.as_str() == op).then_some((&b.left, &b.right))
}
fn sym(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<SymbolId> {
    crate::analysis::expression_symbol(e.without_parentheses(), model.scoping)
}
fn immutable(model: &Model<'_, '_>, s: SymbolId) -> bool {
    model.binding_writes(s) == 0 && model.scoping.symbol_redeclarations(s).is_empty()
}
fn range(model: &Model<'_, '_>, e: &Expression<'_>, input: SymbolId) -> Option<[SymbolId; 2]> {
    let (low, high) = logical(e, "&&")?;
    let (first, low) = binary(low, ">=")?;
    let (second, high) = binary(high, "<=")?;
    if sym(model, first) != Some(input) || sym(model, second) != Some(input) {
        return None;
    }
    Some([sym(model, low)?, sym(model, high)?])
}

fn prove(model: &Model<'_, '_>, site: FunctionSite) -> Option<()> {
    uninitialized_bounds(model, site, predicate_parts(model, site)?)
}

fn uninitialized_bounds(
    model: &Model<'_, '_>,
    site: FunctionSite,
    bounds: [SymbolId; 4],
) -> Option<()> {
    // The nearest enclosing function is the only entry to this declaration's
    // lexical body. Do not infer readiness from declaration text ordering.
    let mut owner_id = model.nodes.parent_id(site.node_id);
    loop {
        match model.nodes.kind(owner_id) {
            AstKind::Function(_) => break,
            AstKind::ArrowFunctionExpression(_) | AstKind::Program(_) => return None,
            _ => owner_id = model.nodes.parent_id(owner_id),
        }
    }
    let AstKind::Function(owner) = model.nodes.kind(owner_id) else {
        return None;
    };
    if owner.r#async || owner.generator {
        return None;
    }
    let owner_symbol = owner.id.as_ref()?.symbol_id.get()?;
    let container = model.nodes.parent_id(owner_id);
    // Exclude exports, global properties, function expressions and Annex B
    // block declarations. Strict scope forbids caller/arguments reflection.
    if !matches!(model.nodes.kind(container), AstKind::FunctionBody(_))
        || model.scoping.symbol_declaration(owner_symbol) != owner_id
        || !immutable(model, owner_symbol)
        || !model
            .scoping
            .scope_flags(owner.scope_id.get()?)
            .is_strict_mode()
    {
        return None;
    }

    // Fail closed on dynamic scope, including shadowed eval spellings. This
    // scans the whole source, not just the predicate's otherwise closed body.
    if model.nodes.iter().any(|n| {
        matches!(n.kind(), AstKind::WithStatement(_))
            || matches!(n.kind(), AstKind::IdentifierReference(i) if i.name == "eval")
    }) {
        return None;
    }
    let mut ready = 0;
    for bound in bounds {
        if !immutable(model, bound) {
            return None;
        }
        let decl = model.scoping.symbol_declaration(bound);
        let AstKind::VariableDeclarator(v) = model.nodes.kind(decl) else {
            return None;
        };
        let declaration_id = model.nodes.parent_id(decl);
        let AstKind::VariableDeclaration(declaration) = model.nodes.kind(declaration_id) else {
            return None;
        };
        if declaration.kind != VariableDeclarationKind::Let
            || v.init.is_some()
            || !matches!(&v.id, BindingPattern::BindingIdentifier(i) if i.symbol_id.get() == Some(bound))
            || model.nodes.parent_id(declaration_id) != container
            || declaration.span.end > owner.span.start
        {
            return None;
        }
        // Every read and every possible write belongs to the owner; no aliases
        // from other bundle epochs and no global lookalike may supply bounds.
        if model.scoping.get_resolved_references(bound).any(|r| {
            r.is_write()
                || !owner
                    .span
                    .contains_inclusive(model.nodes.kind(r.node_id()).span())
        }) {
            return None;
        }
        ready = ready.max(declaration.span.end);
    }
    for reference in model.scoping.get_resolved_references(owner_symbol) {
        let call_id = model.nodes.parent_id(reference.node_id());
        let AstKind::CallExpression(call) = model.nodes.kind(call_id) else {
            return None;
        };
        let statement_id = model.nodes.parent_id(call_id);
        if reference.is_write()
            || call.optional
            || sym(model, &call.callee) != Some(owner_symbol)
            || !matches!(
                model.nodes.kind(statement_id),
                AstKind::ExpressionStatement(_)
            )
            || model.nodes.parent_id(statement_id) != container
            || call.span.start < ready
        {
            return None;
        }
    }
    let target = site.binding_symbol?;
    if !immutable(model, target)
        || model.scoping.symbol_declaration(target) != site.node_id
        || model.binding_references(target) != 1
    {
        return None;
    }
    // Account for the complete exposure: exactly one plain method-call argument
    // inside the guarded owner. No claim about that method's native identity.
    let reference = model.scoping.get_resolved_references(target).next()?;
    let call_id = model.nodes.parent_id(reference.node_id());
    let AstKind::CallExpression(call) = model.nodes.kind(call_id) else {
        return None;
    };
    let [argument] = call.arguments.as_slice() else {
        return None;
    };
    let Expression::StaticMemberExpression(method) = call.callee.without_parentheses() else {
        return None;
    };
    if call.optional
        || method.optional
        || method.property.name != "every"
        || sym(model, argument.as_expression()?) != Some(target)
        || !owner.span.contains_inclusive(call.span)
    {
        return None;
    }
    Some(())
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
) -> Vec<Proposal> {
    if prove(model, site).is_none() {
        if initialized(model, site).is_none() {
            if conditional_initialized(model, site).is_none() {
                return flow_proposal(model, site, f);
            }
            return vec![proposal("capturedTwoRangeOr45Predicate", "signals_captured_range_predicate",
                "whole one-formal conditional: the ordered two captured inclusive range ANDs are ORed; its sole true branch returns boolean true and its sole else branch returns the same immutable formal strictly equal to literal45; comparisons are boolean, so this preserves exact short-circuit/coercion behavior of the direct OR spelling without algebraic reordering; four distinct scoped captures each have one completed direct contiguous scalar-producer store before every owner entry/publication, with all producer copies, reads, writers and hoisted entry-root references checked; known uninitialized bounds, early publication, conditional/deferred/later writes and owner escapes are refused; native method identity, producer results and every consumer stay opaque, with no numeric/ASCII/URL purpose claim", f)];
        }
        return vec![proposal("capturedTwoRangeOr45Predicate", "signals_captured_range_predicate",
            "complete exact ordered two-range AND/OR return plus strict literal45 on one immutable formal and four distinct scoped captures; both exclusive same-frame setup arms complete the same contiguous four-store scalar-source protocol before every actual owner entry; exact local charCodeAt-returning producer and all copies are initialized with every writer checked; inert owner aliases are followed through every callee use and all hoisted entry-root references are gated by completed stores, never function declaration offsets; no later/deferred/compound/loop writes, partial arms, early entry/exposure or external owner escapes; native method identity and captured results remain opaque, with JavaScript coercions and short-circuit order preserved and no numeric/ASCII/URL inference", f)];
    }
    vec![proposal("capturedTwoRangeOr45Predicate", "signals_captured_range_predicate",
        "entire one-formal return is (input >= firstLower && input <= firstUpper) || (input >= secondLower && input <= secondUpper) || input === 45, preserving relational coercions and short-circuit order; four distinct scoped write-free let captures without initializers contain undefined after declaration execution, with no numeric/ASCII/URL inference; all owner entries are direct sibling calls after every capture declaration, or owner has no references; sole predicate exposure is one in-owner every argument whose native identity remains opaque; strict lexical scope, no redeclarations, foreign-epoch stores, eval, with, extra statements or effects", f)]
}

// The initialized variant is deliberately separate: the reviewed write-free
// proof and its proposal bytes above retain priority and remain unchanged.
#[derive(Debug)]
struct InitializedProof {
    epochs: [NodeId; 2],
    ready: [u32; 2],
    entry_roots: BTreeSet<NodeId>,
    aliases: BTreeSet<SymbolId>,
}

fn predicate_parts(model: &Model<'_, '_>, site: FunctionSite) -> Option<[SymbolId; 4]> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return None;
    };
    let body = function.body.as_ref()?;
    let [parameter] = function.params.items.as_slice() else {
        return None;
    };
    let BindingPattern::BindingIdentifier(input) = &parameter.pattern else {
        return None;
    };
    let input = input.symbol_id.get()?;
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || parameter.initializer.is_some()
        || !body.directives.is_empty()
        || !immutable(model, input)
        || model.binding_references(input) != 5
    {
        return None;
    }
    let [Statement::ReturnStatement(result)] = body.statements.as_slice() else {
        return None;
    };
    let (ranges, literal) = logical(result.argument.as_ref()?, "||")?;
    let (first, second) = logical(ranges, "||")?;
    let (value, literal) = binary(literal, "===")?;
    if sym(model, value) != Some(input)
        || !matches!(literal.without_parentheses(), Expression::NumericLiteral(n) if n.value == 45.0)
    {
        return None;
    }
    let [a, b] = range(model, first, input)?;
    let [c, d] = range(model, second, input)?;
    let bounds = [a, b, c, d];
    (bounds.iter().collect::<BTreeSet<_>>().len() == 4 && !bounds.contains(&input))
        .then_some(bounds)
}

fn enclosing_function(model: &Model<'_, '_>, mut node: NodeId) -> Option<NodeId> {
    loop {
        node = model.nodes.parent_id(node);
        match model.nodes.kind(node) {
            AstKind::Function(_) => return Some(node),
            AstKind::ArrowFunctionExpression(_) | AstKind::Program(_) => return None,
            _ => (),
        }
    }
}

// This is a local scalar/copy proof, not a call to the global last-writer index.
// The only supported producer is an exact local function returning its sole
// formal's zero-argument .charCodeAt() call. Native method identity and return
// value (including object/undefined/throw) remain opaque. Only a completed store
// establishes readiness; no numeric constant or initialized-port inference.
fn scalar_producer(
    model: &Model<'_, '_>,
    symbol: SymbolId,
    container: NodeId,
    at: u32,
    depth: usize,
) -> Option<()> {
    if depth > 4 || !model.scoping.symbol_redeclarations(symbol).is_empty() {
        return None;
    }
    let decl = model.scoping.symbol_declaration(symbol);
    let AstKind::VariableDeclarator(v) = model.nodes.kind(decl) else {
        return None;
    };
    let declaration_id = model.nodes.parent_id(decl);
    let AstKind::VariableDeclaration(declaration) = model.nodes.kind(declaration_id) else {
        return None;
    };
    if !matches!(
        declaration.kind,
        VariableDeclarationKind::Let | VariableDeclarationKind::Const
    ) || !matches!(&v.id, BindingPattern::BindingIdentifier(i) if i.symbol_id.get() == Some(symbol))
        || model.nodes.parent_id(declaration_id) != container
        || declaration.span.end > at
    {
        return None;
    }
    let (value, value_at) = if let Some(init) = &v.init {
        if model.binding_writes(symbol) != 0 {
            return None;
        }
        (init, declaration.span.start)
    } else {
        let writes: Vec<_> = model
            .scoping
            .get_resolved_references(symbol)
            .filter(|r| r.is_write())
            .collect();
        let [reference] = writes.as_slice() else {
            return None;
        };
        let assign_id = model.nodes.parent_id(reference.node_id());
        let AstKind::AssignmentExpression(assign) = model.nodes.kind(assign_id) else {
            return None;
        };
        let statement_id = model.nodes.parent_id(assign_id);
        if assign.operator.as_str() != "="
            || !matches!(&assign.left, AssignmentTarget::AssignmentTargetIdentifier(i)
            if crate::analysis::identifier_symbol(i, model.scoping) == Some(symbol))
            || !matches!(
                model.nodes.kind(statement_id),
                AstKind::ExpressionStatement(_)
            )
            || model.nodes.parent_id(statement_id) != container
            || declaration.span.end > assign.span.start
            || assign.span.end > at
        {
            return None;
        }
        (&assign.right, assign.span.start)
    };
    match value.without_parentheses() {
        Expression::Identifier(_) => {
            scalar_producer(model, sym(model, value)?, container, value_at, depth + 1)
        }
        Expression::FunctionExpression(f) => {
            if f.id.is_some() || f.r#async || f.generator || f.params.rest.is_some() {
                return None;
            }
            let [parameter] = f.params.items.as_slice() else {
                return None;
            };
            let BindingPattern::BindingIdentifier(input) = &parameter.pattern else {
                return None;
            };
            let input = input.symbol_id.get()?;
            let body = f.body.as_ref()?;
            let [Statement::ReturnStatement(ret)] = body.statements.as_slice() else {
                return None;
            };
            let Expression::CallExpression(call) = ret.argument.as_ref()?.without_parentheses()
            else {
                return None;
            };
            let Expression::StaticMemberExpression(method) = call.callee.without_parentheses()
            else {
                return None;
            };
            (parameter.initializer.is_none()
                && body.directives.is_empty()
                && immutable(model, input)
                && model.binding_references(input) == 1
                && !call.optional
                && call.arguments.is_empty()
                && !method.optional
                && method.property.name == "charCodeAt"
                && sym(model, &method.object) == Some(input))
            .then_some(())
        }
        _ => None,
    }
}

// Does evaluating this reference necessarily follow a completed epoch? A
// function declaration's textual position never counts. A hoisted top-level
// callable is usable only if EVERY reference that could call/expose it is
// gated in turn. Cycles are refused, not assumed initialized. After four stores
// there are no future writes in this frame; downstream external consumers can
// retain/reenter a callable without introducing another producer epoch.
fn epoch_entry(
    model: &Model<'_, '_>,
    node: NodeId,
    epochs: &[NodeId; 2],
    ready: &[u32; 2],
    roots: &mut BTreeSet<NodeId>,
    visiting: &mut BTreeSet<NodeId>,
    depth: usize,
) -> Option<()> {
    if depth > 12 {
        return None;
    }
    let mut cursor = node;
    let mut callable = None;
    loop {
        match model.nodes.kind(cursor) {
            AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => callable = Some(cursor),
            AstKind::Class(_) | AstKind::StaticBlock(_) => return None,
            AstKind::Program(_) => return None,
            _ => (),
        }
        let parent = model.nodes.parent_id(cursor);
        if let Some(index) = epochs.iter().position(|id| *id == parent) {
            if let Some(function_id) = callable {
                if model.nodes.parent_id(function_id) != parent || !visiting.insert(function_id) {
                    return None;
                }
                let AstKind::Function(function) = model.nodes.kind(function_id) else {
                    return None;
                };
                let s = function.id.as_ref()?.symbol_id.get()?;
                if !immutable(model, s) || model.scoping.symbol_declaration(s) != function_id {
                    return None;
                }
                let references: Vec<_> = model.scoping.get_resolved_references(s).collect();
                if references.is_empty() {
                    return None;
                } // never execution evidence
                for r in references {
                    epoch_entry(
                        model,
                        r.node_id(),
                        epochs,
                        ready,
                        roots,
                        visiting,
                        depth + 1,
                    )?;
                }
                visiting.remove(&function_id);
                roots.insert(function_id);
                return Some(());
            }
            // A direct sibling statement must execute after the completed run
            // of stores in this particular arm. Nested branches/loops are fine
            // for later consumers; they cannot skip the earlier sibling run.
            return (model.nodes.kind(cursor).span().start >= ready[index]).then_some(());
        }
        if cursor == parent {
            return None;
        }
        cursor = parent;
    }
}

fn initialized(model: &Model<'_, '_>, site: FunctionSite) -> Option<InitializedProof> {
    initialized_bounds(model, site, predicate_parts(model, site)?)
}

fn initialized_bounds(
    model: &Model<'_, '_>,
    site: FunctionSite,
    bounds: [SymbolId; 4],
) -> Option<InitializedProof> {
    let owner_id = enclosing_function(model, site.node_id)?;
    let AstKind::Function(owner) = model.nodes.kind(owner_id) else {
        return None;
    };
    let container = model.nodes.parent_id(owner_id);
    let AstKind::FunctionBody(_) = model.nodes.kind(container) else {
        return None;
    };
    let setup_id = model.nodes.parent_id(container);
    let AstKind::Function(setup) = model.nodes.kind(setup_id) else {
        return None;
    };
    let owner_symbol = owner.id.as_ref()?.symbol_id.get()?;
    if owner.r#async
        || owner.generator
        || setup.r#async
        || setup.generator
        || !immutable(model, owner_symbol)
        || model.scoping.symbol_declaration(owner_symbol) != owner_id
        || !model
            .scoping
            .scope_flags(setup.scope_id.get()?)
            .is_strict_mode()
        || model.nodes.iter().any(|n| {
            matches!(n.kind(), AstKind::WithStatement(_))
                || matches!(n.kind(),AstKind::IdentifierReference(i) if i.name=="eval")
        })
    {
        return None;
    }
    let mut stores = Vec::new();
    let mut branch_id = None;
    for (slot, bound) in bounds.iter().enumerate() {
        if !model.scoping.symbol_redeclarations(*bound).is_empty() {
            return None;
        }
        let decl = model.scoping.symbol_declaration(*bound);
        let AstKind::VariableDeclarator(v) = model.nodes.kind(decl) else {
            return None;
        };
        let declaration_id = model.nodes.parent_id(decl);
        let AstKind::VariableDeclaration(declaration) = model.nodes.kind(declaration_id) else {
            return None;
        };
        if declaration.kind != VariableDeclarationKind::Let
            || v.init.is_some()
            || !matches!(&v.id,BindingPattern::BindingIdentifier(i) if i.symbol_id.get()==Some(*bound))
            || model.nodes.parent_id(declaration_id) != container
        {
            return None;
        }
        let mut writes = 0;
        for reference in model.scoping.get_resolved_references(*bound) {
            if !reference.is_write() {
                if !owner
                    .span
                    .contains_inclusive(model.nodes.kind(reference.node_id()).span())
                {
                    return None;
                }
                continue;
            }
            writes += 1;
            let assign_id = model.nodes.parent_id(reference.node_id());
            let AstKind::AssignmentExpression(assign) = model.nodes.kind(assign_id) else {
                return None;
            };
            let statement_id = model.nodes.parent_id(assign_id);
            let epoch = model.nodes.parent_id(statement_id);
            let branch = model.nodes.parent_id(epoch);
            let AstKind::IfStatement(branch_node) = model.nodes.kind(branch) else {
                return None;
            };
            if !matches!(model.nodes.kind(epoch), AstKind::BlockStatement(_))
                || !matches!(
                    model.nodes.kind(statement_id),
                    AstKind::ExpressionStatement(_)
                )
                || model.nodes.parent_id(branch) != container
                || declaration.span.end > branch_node.span.start
                || assign.operator.as_str() != "="
                || !matches!(&assign.left,AssignmentTarget::AssignmentTargetIdentifier(i)
                    if crate::analysis::identifier_symbol(i,model.scoping)==Some(*bound))
            {
                return None;
            }
            if branch_id.is_some_and(|id| id != branch) {
                return None;
            }
            branch_id = Some(branch);
            let Expression::CallExpression(call) = assign.right.without_parentheses() else {
                return None;
            };
            let [argument] = call.arguments.as_slice() else {
                return None;
            };
            let Expression::StringLiteral(literal) =
                argument.as_expression()?.without_parentheses()
            else {
                return None;
            };
            if call.optional || literal.value.encode_utf16().count() != 1 {
                return None;
            }
            let producer = sym(model, &call.callee)?;
            scalar_producer(model, producer, container, branch_node.span.start, 0)?;
            stores.push((epoch, statement_id, slot, producer, literal.value.as_str()));
        }
        if writes != 2 {
            return None;
        }
    }
    let AstKind::IfStatement(branch) = model.nodes.kind(branch_id?) else {
        return None;
    };
    let Statement::BlockStatement(first) = &branch.consequent else {
        return None;
    };
    let Statement::BlockStatement(second) = branch.alternate.as_ref()? else {
        return None;
    };
    let first_id = stores
        .iter()
        .find(|s| model.nodes.kind(s.0).span() == first.span)?
        .0;
    let second_id = stores
        .iter()
        .find(|s| model.nodes.kind(s.0).span() == second.span)?
        .0;
    let epochs = [first_id, second_id];
    let mut ready = [0, 0];
    let mut signatures = Vec::new();
    for (index, epoch) in epochs.iter().enumerate() {
        let AstKind::BlockStatement(block) = model.nodes.kind(*epoch) else {
            return None;
        };
        let mut own: Vec<_> = stores.iter().filter(|s| s.0 == *epoch).collect();
        own.sort_by_key(|s| model.nodes.kind(s.1).span().start);
        if own.len() != 4 || own.iter().map(|s| s.2).collect::<BTreeSet<_>>().len() != 4 {
            return None;
        }
        // No interleaved effects/exposures or partial initialization. Each arm
        // has one contiguous ordered four-store run with equal source protocol.
        let start = block
            .body
            .iter()
            .position(|s| s.span() == model.nodes.kind(own[0].1).span())?;
        for (offset, store) in own.iter().enumerate() {
            if block.body.get(start + offset)?.span() != model.nodes.kind(store.1).span() {
                return None;
            }
        }
        ready[index] = model.nodes.kind(own[3].1).span().end;
        signatures.push(own.iter().map(|s| (s.2, s.3, s.4)).collect::<Vec<_>>());
    }
    if signatures[0] != signatures[1] {
        return None;
    }
    let target = site.binding_symbol?;
    if !immutable(model, target)
        || model.binding_references(target) != 1
        || model.scoping.symbol_declaration(target) != site.node_id
    {
        return None;
    }
    let reference = model.scoping.get_resolved_references(target).next()?;
    let AstKind::CallExpression(call) = model.nodes.parent_kind(reference.node_id()) else {
        return None;
    };
    let [arg] = call.arguments.as_slice() else {
        return None;
    };
    let Expression::StaticMemberExpression(method) = call.callee.without_parentheses() else {
        return None;
    };
    if call.optional
        || method.optional
        || method.property.name != "every"
        || sym(model, arg.as_expression()?) != Some(target)
        || !owner.span.contains_inclusive(call.span)
    {
        return None;
    }
    let mut proof = InitializedProof {
        epochs,
        ready,
        entry_roots: BTreeSet::new(),
        aliases: BTreeSet::new(),
    };
    // Only inert, single-origin lexical callable copies may precede stores.
    // Every use of each copy is still checked for actual epoch readiness.
    let mut pending = vec![owner_symbol];
    let mut uses = 0;
    while let Some(current) = pending.pop() {
        if !proof.aliases.insert(current) || proof.aliases.len() > 8 || !immutable(model, current) {
            return None;
        }
        for reference in model.scoping.get_resolved_references(current) {
            let node = reference.node_id();
            let parent = model.nodes.parent_id(node);
            if let AstKind::VariableDeclarator(v) = model.nodes.kind(parent) {
                let declaration_id = model.nodes.parent_id(parent);
                if model.nodes.parent_id(declaration_id) == container
                    && v.init.as_ref().and_then(|v| sym(model, v)) == Some(current)
                {
                    let BindingPattern::BindingIdentifier(alias) = &v.id else {
                        return None;
                    };
                    let AstKind::VariableDeclaration(d) = model.nodes.kind(declaration_id) else {
                        return None;
                    };
                    if !matches!(
                        d.kind,
                        VariableDeclarationKind::Let | VariableDeclarationKind::Const
                    ) || d.span.end > branch.span.start
                    {
                        return None;
                    }
                    let source_decl = model.scoping.symbol_declaration(current);
                    if matches!(
                        model.nodes.kind(source_decl),
                        AstKind::VariableDeclarator(_)
                    ) {
                        let AstKind::VariableDeclaration(source_statement) =
                            model.nodes.parent_kind(source_decl)
                        else {
                            return None;
                        };
                        if source_statement.span.end > d.span.start {
                            return None;
                        }
                    }
                    pending.push(alias.symbol_id.get()?);
                    continue;
                }
            }
            // A value escape through an unknown consumer is refused. Actual
            // owner entries use the callee directly, never an argument/port.
            let AstKind::CallExpression(call) = model.nodes.kind(parent) else {
                return None;
            };
            if call.optional || sym(model, &call.callee) != Some(current) {
                return None;
            }
            epoch_entry(
                model,
                node,
                &epochs,
                &ready,
                &mut proof.entry_roots,
                &mut BTreeSet::new(),
                0,
            )?;
            uses += 1;
        }
    }
    if uses == 0 || proof.entry_roots.is_empty() {
        return None;
    }
    Some(proof)
}

#[cfg(test)]
mod tests {
    use super::*;
    // One focused lib executable includes the unchanged package001 integration
    // controls plus the initialized production and private-Model cases below.
    mod retained {
        use crate as shape;
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/naming_captured_range_predicate.rs"
        ));
    }
    const SPLIT_EPOCH: &str =
        include_str!("../../../../tests/fixtures/captured_range_predicate/direct-call-parent.js");
    const SHARED_EPOCH: &str =
        include_str!("../../../../tests/fixtures/captured_range_predicate/initialized-bounds-parent.js");
    const FOUR_EPOCH: &str =
        include_str!("../../../../tests/fixtures/captured_range_predicate/multi-epoch-constructor.js");
    const ROLE: &str = "signals_captured_range_predicate";

    fn model_proof(source: &str, name: &str, expected: bool) {
        let allocator = Allocator::default();
        let program =
            crate::analysis::parse_program(&allocator, source).expect("valid whole context");
        let (scoping, nodes) = crate::analysis::analyze(&program);
        let model = Model::new(
            source,
            &program,
            &scoping,
            &nodes,
            ProgramPropertiesLimits::default(),
        )
        .unwrap();
        let matches: Vec<_>=model.all_sites().iter().filter(|s|
            matches!(model.nodes.kind(s.node_id),AstKind::Function(f) if f.id.as_ref().is_some_and(|i|i.name==name))).collect();
        assert_eq!(
            matches.len(),
            1,
            "full target identity/cardinality, never partial truth"
        );
        let proof = initialized(&model, *matches[0]);
        assert_eq!(proof.is_some(), expected, "{proof:?}");
        if let Some(proof) = proof {
            assert_ne!(proof.epochs[0], proof.epochs[1]);
            assert!(proof.ready.iter().all(|r| *r > 0));
            assert_eq!(
                proof.aliases.len(),
                2,
                "owner plus the one inert source-backed callable copy"
            );
            assert_eq!(
                proof.entry_roots.len(),
                6,
                "three actual callable roots per mutually exclusive epoch"
            );
        }
    }
    fn production(source: &str, name: &str, expected: bool) {
        let result = name_threads(source, true).unwrap();
        let own: Vec<_> = result
            .report
            .outcomes
            .iter()
            .filter(|o| o.binding.name == name)
            .collect();
        assert_eq!(
            own.len(),
            1,
            "actual production enumeration, not a partial guard"
        );
        assert_eq!(
            own[0].evaluator.as_deref() == Some(ROLE) && own[0].applied,
            expected,
            "{:?}",
            own[0]
        );
        let claims: Vec<_> = result
            .report
            .outcomes
            .iter()
            .filter(|o| o.applied && o.evaluator.as_deref() == Some(ROLE))
            .collect();
        assert_eq!(
            claims.len(),
            usize::from(expected),
            "production family cardinality"
        );
        let mut replay = source.to_owned();
        for e in result.report.edits.iter().rev() {
            replay.replace_range(e.start as usize..e.end as usize, &e.replacement);
        }
        assert_eq!(replay, result.source);
    }
    fn changed(from: &str, to: &str) -> String {
        assert!(SPLIT_EPOCH.contains(from), "mutation exists");
        SPLIT_EPOCH.replacen(from, to, 1)
    }
    #[test]
    fn initialized_whole_actual_model_and_production_identity() {
        model_proof(SPLIT_EPOCH, "$THREAD_0_19047", true);
        production(SPLIT_EPOCH, "$THREAD_0_19047", true);
        let alpha = SPLIT_EPOCH
            .replace("$KEY_225", "firstLowerBound")
            .replace("$KEY_433", "firstUpperBound")
            .replace("$KEY_395", "secondLowerBound")
            .replace("$KEY_434", "secondUpperBound")
            .replace("$DO", "datum")
            .replace("$KEY_219", "policyAlias");
        model_proof(&alpha, "$THREAD_0_19047", true);
        production(&alpha, "$THREAD_0_19047", true);
        let parens=changed("return $DO >= $KEY_225 && $DO <= $KEY_433 || $DO >= $KEY_395 && $DO <= $KEY_434 || $DO === 45;",
            "return (($DO >= $KEY_225 && $DO <= $KEY_433) || ($DO >= $KEY_395 && $DO <= $KEY_434)) || ($DO === 45);");
        model_proof(&parens, "$THREAD_0_19047", true);
        production(&parens, "$THREAD_0_19047", true);
    }
    #[test]
    fn captured_range_whole_production_outcome_role_and_edits_exact() {
        let result = name_threads(SHARED_EPOCH, true).unwrap();
        let saved: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/captured_range_predicate/parent-outcome-control.json"
        ))
        .unwrap();
        let outcomes: Vec<_> = result
            .report
            .outcomes
            .iter()
            .filter(|o| o.function_id == "fn@61632..61780")
            .collect();
        assert_eq!(outcomes.len(), 1);
        assert_eq!(serde_json::to_value(outcomes[0]).unwrap(), saved["outcome"]);
        let report = serde_json::to_value(&result.report).unwrap();
        let roles: Vec<_> = report["function_roles"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["function_span"]["start"] == 61632)
            .collect();
        assert_eq!(roles.len(), 1);
        assert_eq!(*roles[0], saved["role"]);
        for old in saved["own_edits"].as_array().unwrap() {
            assert!(
                report["edits"].as_array().unwrap().contains(old),
                "exact old edit retained"
            );
        }
        fn strip_timings(value: &mut serde_json::Value) {
            match value {
                serde_json::Value::Object(map) => {
                    map.remove("timings_ms");
                    for child in map.values_mut() {
                        strip_timings(child);
                    }
                }
                serde_json::Value::Array(items) => {
                    for child in items {
                        strip_timings(child);
                    }
                }
                _ => (),
            }
        }
        let mut current = report;
        // Keep every function outcome and role assertion above. The full-report control also
        // includes the independently recognized class.
        let mut prior: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/captured_range_predicate/parent-report-after-extension.json"
        ))
        .unwrap();
        strip_timings(&mut current);
        strip_timings(&mut prior);
        assert_eq!(
            current, prior,
            "whole accepted range predicate production metadata unchanged; timings_ms only ignored"
        );
        production(SHARED_EPOCH, "$THREAD_5_41252", true);
    }
    #[test]
    fn initialized_partial_order_false_branch_and_future_writers_refused() {
        let negatives = [
            changed(
                "$KEY_433 = $KEY_1(\"9\");",
                "if (flag) $KEY_433 = $KEY_1(\"9\");",
            ),
            changed(
                "$KEY_434 = $KEY_1(\"z\");",
                "defer(() => $KEY_434 = $KEY_1(\"z\"));",
            ),
            changed("$KEY_225 = $KEY_1(\"0\");", "$KEY_225 += $KEY_1(\"0\");"),
            changed(
                "$KEY_395 = $KEY_1(\"a\");",
                "while (flag) { $KEY_395 = $KEY_1(\"a\"); }",
            ),
            changed(
                "let $KEY_219 = $THREAD_0_154524;",
                "let $KEY_219 = $THREAD_0_154524; later(() => $KEY_225 = unknown);",
            ),
            changed("$KEY_1 = $B;", "$KEY_1 = $B; $B = foreign;"),
            changed(
                "let $B = (function(sourceString)",
                "$KEY_1 = $B; let $B = (function(sourceString)",
            ),
            changed("$KEY_395 = $KEY_1(\"a\");", "$KEY_395 = foreign(\"a\");"),
        ];
        for source in negatives {
            model_proof(&source, "$THREAD_0_19047", false);
            production(&source, "$THREAD_0_19047", false);
        }
        // False arm specifically: a complete true arm cannot mask before-init.
        let needle = "$KEY_395 = $KEY_1(\"a\");";
        let at = SPLIT_EPOCH.rfind(needle).unwrap();
        let mut false_arm = SPLIT_EPOCH.to_owned();
        false_arm.insert_str(at, "$KEY_219({}); ");
        model_proof(&false_arm, "$THREAD_0_19047", false);
        production(&false_arm, "$THREAD_0_19047", false);
    }
    #[test]
    fn initialized_early_hoisting_aliases_reentry_and_external_escape_refused() {
        for (from, to) in [
            (
                "let $KEY_219 = $THREAD_0_154524;",
                "let $KEY_219 = $THREAD_0_154524; $KEY_219({});",
            ),
            (
                "let $KEY_219 = $THREAD_0_154524;",
                "let $KEY_219 = $THREAD_0_154524; expose($KEY_219);",
            ),
            (
                "let $KEY_219 = $THREAD_0_154524;",
                "let $KEY_219 = nextAlias; let nextAlias = $THREAD_0_154524;",
            ),
            (
                "$KEY_395 = $KEY_1(\"a\");",
                "early($THREAD_5_141136_2); $KEY_395 = $KEY_1(\"a\");",
            ),
            (
                "$KEY_395 = $KEY_1(\"a\");",
                "$THREAD_5_90762_2(); $KEY_395 = $KEY_1(\"a\");",
            ),
            (
                "return sourceString.charCodeAt();",
                "$KEY_219({}); return sourceString.charCodeAt();",
            ),
            (
                "let $KEY_219 = $THREAD_0_154524;",
                "let $KEY_219 = $THREAD_0_154524; const leakedBound = $KEY_225;",
            ),
        ] {
            let source = changed(from, to);
            model_proof(&source, "$THREAD_0_19047", false);
            production(&source, "$THREAD_0_19047", false);
        }
    }
    #[test]
    fn initialized_full_own_producer_and_predicate_not_partial_keywords() {
        for (from, to) in [
            (
                "return sourceString.charCodeAt();",
                "sideEffect(); return sourceString.charCodeAt();",
            ),
            (
                "return sourceString.charCodeAt();",
                "sourceString.charCodeAt();",
            ),
            (
                "return sourceString.charCodeAt();",
                "return sourceString.charCodeAt(1);",
            ),
            ("$DO >= $KEY_225", "$KEY_225 <= $DO"),
            ("$DO <= $KEY_433", "$DO <= $KEY_255"),
            ("$DO === 45", "$DO == 45"),
            ("$DO === 45", "$DO === 46"),
            (
                "$DO >= $KEY_225 && $DO <= $KEY_433",
                "$DO <= $KEY_433 && $DO >= $KEY_225",
            ),
            ("return $DO >=", "effect(); return $DO >="),
            (
                "$CU.every($THREAD_0_19047)",
                "$CU.every($THREAD_0_19047, extra)",
            ),
        ] {
            let source = changed(from, to);
            model_proof(&source, "$THREAD_0_19047", false);
            production(&source, "$THREAD_0_19047", false);
        }
    }
    #[test]
    fn four_epoch_four_epoch_whole_context_remains_unproved() {
        model_proof(FOUR_EPOCH, "$THREAD_1_81012", false);
        let alpha = FOUR_EPOCH
            .replace("$KEY_219", "opaqueFirstLower")
            .replace("$KEY_406", "opaqueFirstUpper")
            .replace("$KEY_276", "opaqueSecondLower")
            .replace("$KEY_60", "opaqueSecondUpper");
        model_proof(&alpha, "$THREAD_1_81012", false);
        // Changing only the final epoch cannot license last-textual-writer inference.
        let needle = "$KEY_219 =";
        let at = FOUR_EPOCH.rfind(needle).expect("actual final epoch store");
        let mut final_epoch = FOUR_EPOCH.to_owned();
        final_epoch.replace_range(at..at + needle.len(), "$KEY_219 +=");
        model_proof(&final_epoch, "$THREAD_1_81012", false);
    }
}

// Conditional spelling is admitted only with its own initialized, single-run
// lifetime proof below. The undefined/write-free variant is not extended.
fn sole_statement<'a>(statements: &'a [Statement<'a>]) -> Option<&'a Statement<'a>> {
    let [statement] = statements else {
        return None;
    };
    let mut statement = statement;
    for _ in 0..4 {
        match statement {
            Statement::BlockStatement(block) => {
                let [inner] = block.body.as_slice() else {
                    return None;
                };
                statement = inner;
            }
            _ => return Some(statement),
        }
    }
    None
}

fn branch_return<'a>(statement: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    let mut statement = statement;
    for _ in 0..4 {
        match statement {
            Statement::BlockStatement(block) => {
                let [inner] = block.body.as_slice() else {
                    return None;
                };
                statement = inner;
            }
            Statement::ReturnStatement(ret) => return ret.argument.as_ref(),
            _ => return None,
        }
    }
    None
}

fn conditional_parts(model: &Model<'_, '_>, site: FunctionSite) -> Option<[SymbolId; 4]> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return None;
    };
    let body = function.body.as_ref()?;
    let [parameter] = function.params.items.as_slice() else {
        return None;
    };
    let BindingPattern::BindingIdentifier(input) = &parameter.pattern else {
        return None;
    };
    let input = input.symbol_id.get()?;
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || parameter.initializer.is_some()
        || !body.directives.is_empty()
        || !immutable(model, input)
        || model.binding_references(input) != 5
    {
        return None;
    }
    let Statement::IfStatement(branch) = sole_statement(body.statements.as_slice())? else {
        return None;
    };
    if !matches!(branch_return(&branch.consequent)?.without_parentheses(),Expression::BooleanLiteral(value) if value.value)
    {
        return None;
    }
    let (value, literal) = binary(branch_return(branch.alternate.as_ref()?)?, "===")?;
    if sym(model, value) != Some(input)
        || !matches!(literal.without_parentheses(),Expression::NumericLiteral(value) if value.value==45.0)
    {
        return None;
    }
    let (first, second) = logical(&branch.test, "||")?;
    let [a, b] = range(model, first, input)?;
    let [c, d] = range(model, second, input)?;
    let bounds = [a, b, c, d];
    (bounds.iter().collect::<BTreeSet<_>>().len() == 4 && !bounds.contains(&input))
        .then_some(bounds)
}

#[derive(Debug)]
struct ConditionalProof {
    ready: u32,
    entry_roots: BTreeSet<NodeId>,
    aliases: BTreeSet<SymbolId>,
    owner_entries: usize,
}

fn conditional_initialized(model: &Model<'_, '_>, site: FunctionSite) -> Option<ConditionalProof> {
    conditional_initialized_bounds(model, site, conditional_parts(model, site)?)
}

fn conditional_initialized_bounds(
    model: &Model<'_, '_>,
    site: FunctionSite,
    bounds: [SymbolId; 4],
) -> Option<ConditionalProof> {
    let owner_id = enclosing_function(model, site.node_id)?;
    let AstKind::Function(owner) = model.nodes.kind(owner_id) else {
        return None;
    };
    let container = model.nodes.parent_id(owner_id);
    let AstKind::FunctionBody(setup_body) = model.nodes.kind(container) else {
        return None;
    };
    let AstKind::Function(setup) = model.nodes.parent_kind(container) else {
        return None;
    };
    let owner_symbol = owner.id.as_ref()?.symbol_id.get()?;
    if owner.r#async
        || owner.generator
        || setup.r#async
        || setup.generator
        || !immutable(model, owner_symbol)
        || model.scoping.symbol_declaration(owner_symbol) != owner_id
        || !model
            .scoping
            .scope_flags(setup.scope_id.get()?)
            .is_strict_mode()
        || model.nodes.iter().any(|node| {
            matches!(node.kind(), AstKind::WithStatement(_))
                || matches!(node.kind(),AstKind::IdentifierReference(id) if id.name=="eval")
        })
    {
        return None;
    }
    let mut stores = Vec::new();
    let mut producer = None;
    for bound in bounds {
        if !model.scoping.symbol_redeclarations(bound).is_empty() {
            return None;
        }
        let declaration_id = model.scoping.symbol_declaration(bound);
        let AstKind::VariableDeclarator(declarator) = model.nodes.kind(declaration_id) else {
            return None;
        };
        let statement_id = model.nodes.parent_id(declaration_id);
        let AstKind::VariableDeclaration(declaration) = model.nodes.kind(statement_id) else {
            return None;
        };
        if declaration.kind != VariableDeclarationKind::Let
            || declarator.init.is_some()
            || !matches!(&declarator.id,BindingPattern::BindingIdentifier(id) if id.symbol_id.get()==Some(bound))
            || model.nodes.parent_id(statement_id) != container
        {
            return None;
        }
        let mut writes = 0;
        for reference in model.scoping.get_resolved_references(bound) {
            if !reference.is_write() {
                if !owner
                    .span
                    .contains_inclusive(model.nodes.kind(reference.node_id()).span())
                {
                    return None;
                }
                continue;
            }
            writes += 1;
            let assign_id = model.nodes.parent_id(reference.node_id());
            let AstKind::AssignmentExpression(assign) = model.nodes.kind(assign_id) else {
                return None;
            };
            let store_id = model.nodes.parent_id(assign_id);
            if !matches!(model.nodes.kind(store_id), AstKind::ExpressionStatement(_))
                || model.nodes.parent_id(store_id) != container
                || declaration.span.end > assign.span.start
                || assign.operator.as_str() != "="
                || !matches!(&assign.left,AssignmentTarget::AssignmentTargetIdentifier(id)
                    if crate::analysis::identifier_symbol(id,model.scoping)==Some(bound))
            {
                return None;
            }
            let Expression::CallExpression(call) = assign.right.without_parentheses() else {
                return None;
            };
            let [argument] = call.arguments.as_slice() else {
                return None;
            };
            let Expression::StringLiteral(literal) =
                argument.as_expression()?.without_parentheses()
            else {
                return None;
            };
            if call.optional || literal.value.encode_utf16().count() != 1 {
                return None;
            }
            let source = sym(model, &call.callee)?;
            if producer.is_some_and(|p| p != source) {
                return None;
            }
            producer = Some(source);
            // Exact local producer/copy origin and readiness at this store;
            // no native identity or numeric return value is inferred.
            scalar_producer(model, source, container, assign.span.start, 0)?;
            stores.push(store_id);
        }
        // A known uninitialized capture is explicitly refused, never used as
        // an opaque initialized value. Later/conditional/loop writes also fail.
        if writes != 1 {
            return None;
        }
    }
    stores.sort_by_key(|id| model.nodes.kind(*id).span().start);
    let first = model.nodes.kind(stores[0]).span();
    let start = setup_body
        .statements
        .iter()
        .position(|s| s.span() == first)?;
    for (offset, id) in stores.iter().enumerate() {
        if setup_body.statements.get(start + offset)?.span() != model.nodes.kind(*id).span() {
            return None;
        }
    }
    let ready = model.nodes.kind(stores[3]).span().end;
    let target = site.binding_symbol?;
    if !immutable(model, target)
        || model.binding_references(target) != 1
        || model.scoping.symbol_declaration(target) != site.node_id
    {
        return None;
    }
    let reference = model.scoping.get_resolved_references(target).next()?;
    let AstKind::CallExpression(call) = model.nodes.parent_kind(reference.node_id()) else {
        return None;
    };
    let [argument] = call.arguments.as_slice() else {
        return None;
    };
    let Expression::StaticMemberExpression(method) = call.callee.without_parentheses() else {
        return None;
    };
    if call.optional
        || method.optional
        || method.property.name != "every"
        || sym(model, argument.as_expression()?) != Some(target)
        || !owner.span.contains_inclusive(call.span)
    {
        return None;
    }
    let mut proof = ConditionalProof {
        ready,
        entry_roots: BTreeSet::new(),
        aliases: BTreeSet::new(),
        owner_entries: 0,
    };
    let mut pending = vec![owner_symbol];
    while let Some(current) = pending.pop() {
        if !proof.aliases.insert(current) || proof.aliases.len() > 8 || !immutable(model, current) {
            return None;
        }
        for reference in model.scoping.get_resolved_references(current) {
            let node = reference.node_id();
            let parent = model.nodes.parent_id(node);
            if let AstKind::VariableDeclarator(copy) = model.nodes.kind(parent) {
                let statement_id = model.nodes.parent_id(parent);
                if model.nodes.parent_id(statement_id) == container
                    && copy.init.as_ref().and_then(|e| sym(model, e)) == Some(current)
                {
                    let BindingPattern::BindingIdentifier(alias) = &copy.id else {
                        return None;
                    };
                    let AstKind::VariableDeclaration(declaration) = model.nodes.kind(statement_id)
                    else {
                        return None;
                    };
                    if !matches!(
                        declaration.kind,
                        VariableDeclarationKind::Let | VariableDeclarationKind::Const
                    ) || declaration.span.end > first.start
                    {
                        return None;
                    }
                    let source_declaration = model.scoping.symbol_declaration(current);
                    if matches!(
                        model.nodes.kind(source_declaration),
                        AstKind::VariableDeclarator(_)
                    ) {
                        let AstKind::VariableDeclaration(source) =
                            model.nodes.parent_kind(source_declaration)
                        else {
                            return None;
                        };
                        if source.span.end > declaration.span.start {
                            return None;
                        }
                    }
                    pending.push(alias.symbol_id.get()?);
                    continue;
                }
            }
            let AstKind::CallExpression(call) = model.nodes.kind(parent) else {
                return None;
            };
            if call.optional || sym(model, &call.callee) != Some(current) {
                return None;
            }
            // Reuse only the reference/exposure gate, with ONE setup statement
            // list and ONE readiness frontier. The repeated array entry does
            // not invent a second epoch or relax the reviewed two-arm proof.
            epoch_entry(
                model,
                node,
                &[container, container],
                &[ready, ready],
                &mut proof.entry_roots,
                &mut BTreeSet::new(),
                0,
            )?;
            proof.owner_entries += 1;
        }
    }
    if proof.owner_entries == 0 || proof.entry_roots.is_empty() {
        return None;
    }
    Some(proof)
}

#[cfg(test)]
#[path = "captured_range_predicate_conditional_tests.rs"]
mod conditional_tests;

// Finite self-authored Boolean grammar. Each comparison is an observable event,
// including repeated comparisons of the same operands. Never substitute truth
// tables for coercion traces: object ToPrimitive may change on every call.
enum FlowExpr {
    Constant(bool),
    Read(SymbolId),
    Compare(usize),
    And(Box<Self>, Box<Self>),
    Or(Box<Self>, Box<Self>),
}
enum FlowStmt {
    Declare(SymbolId, Option<FlowExpr>),
    Store(SymbolId, FlowExpr),
    Return(FlowExpr),
    Branch(FlowExpr, Vec<Self>, Vec<Self>),
}
struct FlowGrammar {
    input: SymbolId,
    bounds: Vec<SymbolId>,
    locals: BTreeSet<SymbolId>,
    budget: usize,
}
impl FlowGrammar {
    fn expr(
        &mut self,
        model: &Model<'_, '_>,
        e: &Expression<'_>,
        depth: usize,
    ) -> Option<FlowExpr> {
        if depth > 16 || self.budget == 0 {
            return None;
        }
        self.budget -= 1;
        match e.without_parentheses() {
            Expression::BooleanLiteral(b) => Some(FlowExpr::Constant(b.value)),
            Expression::Identifier(_) => {
                let symbol = sym(model, e)?;
                self.locals
                    .contains(&symbol)
                    .then_some(FlowExpr::Read(symbol))
            }
            Expression::LogicalExpression(b) => {
                let left = Box::new(self.expr(model, &b.left, depth + 1)?);
                let right = Box::new(self.expr(model, &b.right, depth + 1)?);
                match b.operator.as_str() {
                    "&&" => Some(FlowExpr::And(left, right)),
                    "||" => Some(FlowExpr::Or(left, right)),
                    _ => None,
                }
            }
            Expression::BinaryExpression(b) => {
                if sym(model, &b.left) != Some(self.input) {
                    return None;
                }
                if b.operator.as_str() == "===" {
                    return matches!(b.right.without_parentheses(), Expression::NumericLiteral(n) if n.value == 45.0)
                        .then_some(FlowExpr::Compare(4));
                }
                let bound = sym(model, &b.right)?;
                if bound == self.input || self.locals.contains(&bound) {
                    return None;
                }
                let index = if let Some(index) = self.bounds.iter().position(|s| *s == bound) {
                    index
                } else {
                    if self.bounds.len() == 4 {
                        return None;
                    }
                    self.bounds.push(bound);
                    self.bounds.len() - 1
                };
                (b.operator.as_str() == if index % 2 == 0 { ">=" } else { "<=" })
                    .then_some(FlowExpr::Compare(index))
            }
            _ => None,
        }
    }
    fn statements(
        &mut self,
        model: &Model<'_, '_>,
        statements: &[Statement<'_>],
        depth: usize,
    ) -> Option<Vec<FlowStmt>> {
        let mut result = Vec::new();
        for statement in statements {
            result.extend(self.statement(model, statement, depth)?);
        }
        Some(result)
    }
    fn statement(
        &mut self,
        model: &Model<'_, '_>,
        statement: &Statement<'_>,
        depth: usize,
    ) -> Option<Vec<FlowStmt>> {
        if depth > 16 || self.budget == 0 {
            return None;
        }
        self.budget -= 1;
        Some(match statement {
            Statement::BlockStatement(b) => self.statements(model, &b.body, depth + 1)?,
            Statement::VariableDeclaration(v) => {
                if v.kind != VariableDeclarationKind::Let {
                    return None;
                }
                let mut result = Vec::new();
                for decl in &v.declarations {
                    let BindingPattern::BindingIdentifier(id) = &decl.id else {
                        return None;
                    };
                    let symbol = id.symbol_id.get()?;
                    if self.locals.len() == 4
                        || !self.locals.insert(symbol)
                        || !model.scoping.symbol_redeclarations(symbol).is_empty()
                    {
                        return None;
                    }
                    let init = match &decl.init {
                        Some(e) => Some(self.expr(model, e, depth + 1)?),
                        None => None,
                    };
                    result.push(FlowStmt::Declare(symbol, init));
                }
                result
            }
            Statement::ExpressionStatement(e) => {
                let Expression::AssignmentExpression(a) = e.expression.without_parentheses() else {
                    return None;
                };
                let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else {
                    return None;
                };
                let symbol = crate::analysis::identifier_symbol(id, model.scoping)?;
                if a.operator.as_str() != "=" || !self.locals.contains(&symbol) {
                    return None;
                }
                vec![FlowStmt::Store(
                    symbol,
                    self.expr(model, &a.right, depth + 1)?,
                )]
            }
            Statement::ReturnStatement(r) => vec![FlowStmt::Return(self.expr(
                model,
                r.argument.as_ref()?,
                depth + 1,
            )?)],
            Statement::IfStatement(b) => {
                let test = self.expr(model, &b.test, depth + 1)?;
                let yes = self.statement(model, &b.consequent, depth + 1)?;
                let no = match &b.alternate {
                    Some(no) => self.statement(model, no, depth + 1)?,
                    None => Vec::new(),
                };
                vec![FlowStmt::Branch(test, yes, no)]
            }
            _ => return None,
        })
    }
}

struct FlowRun {
    bits: u8,
    trace: Vec<usize>,
    locals: std::collections::BTreeMap<SymbolId, Option<bool>>,
}
impl FlowRun {
    fn expr(&mut self, e: &FlowExpr) -> Option<bool> {
        Some(match e {
            FlowExpr::Constant(b) => *b,
            FlowExpr::Read(s) => self.locals.get(s).copied().flatten()?,
            FlowExpr::Compare(index) => {
                if self.trace.len() == 5 {
                    return None;
                }
                let value = self.bits & (1 << self.trace.len()) != 0;
                self.trace.push(*index);
                value
            }
            FlowExpr::And(a, b) => self.expr(a)? && self.expr(b)?,
            FlowExpr::Or(a, b) => self.expr(a)? || self.expr(b)?,
        })
    }
    fn statements(&mut self, statements: &[FlowStmt]) -> Option<Option<bool>> {
        for statement in statements {
            match statement {
                FlowStmt::Declare(s, e) => {
                    let value = match e {
                        Some(e) => Some(self.expr(e)?),
                        None => None,
                    };
                    self.locals.insert(*s, value);
                }
                FlowStmt::Store(s, e) => {
                    if !self.locals.contains_key(s) {
                        return None;
                    }
                    let value = self.expr(e)?;
                    self.locals.insert(*s, Some(value));
                }
                FlowStmt::Return(e) => return Some(Some(self.expr(e)?)),
                FlowStmt::Branch(e, yes, no) => {
                    let branch = if self.expr(e)? { yes } else { no };
                    if let Some(value) = self.statements(branch)? {
                        return Some(Some(value));
                    }
                }
            }
        }
        Some(None)
    }
}
fn flow_parts(model: &Model<'_, '_>, site: FunctionSite) -> Option<[SymbolId; 4]> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return None;
    };
    let body = function.body.as_ref()?;
    let [parameter] = function.params.items.as_slice() else {
        return None;
    };
    let BindingPattern::BindingIdentifier(input) = &parameter.pattern else {
        return None;
    };
    let input = input.symbol_id.get()?;
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || parameter.initializer.is_some()
        || !body.directives.is_empty()
        || !immutable(model, input)
    {
        return None;
    }
    let mut grammar = FlowGrammar {
        input,
        bounds: Vec::new(),
        locals: BTreeSet::new(),
        budget: 128,
    };
    let statements = grammar.statements(model, &body.statements, 0)?;
    let bounds: [SymbolId; 4] = grammar.bounds.try_into().ok()?;
    // Every use/writer of every temporary is closed in this complete function.
    // The grammar excludes nested functions, calls, property reads and escapes.
    for local in &grammar.locals {
        if bounds.contains(local)
            || model.scoping.get_resolved_references(*local).any(|r| {
                !site
                    .span
                    .contains_inclusive(model.nodes.kind(r.node_id()).span())
            })
        {
            return None;
        }
    }
    let canonical = FlowExpr::Or(
        Box::new(FlowExpr::Or(
            Box::new(FlowExpr::And(
                Box::new(FlowExpr::Compare(0)),
                Box::new(FlowExpr::Compare(1)),
            )),
            Box::new(FlowExpr::And(
                Box::new(FlowExpr::Compare(2)),
                Box::new(FlowExpr::Compare(3)),
            )),
        )),
        Box::new(FlowExpr::Compare(4)),
    );
    // Bits are consumed per EVENT, not per operand: repeated comparisons cannot
    // borrow a previous result. Thirty-two streams exhaust five Boolean events.
    for bits in 0..32 {
        let mut expected = FlowRun {
            bits,
            trace: Vec::new(),
            locals: Default::default(),
        };
        let value = expected.expr(&canonical)?;
        let mut actual = FlowRun {
            bits,
            trace: Vec::new(),
            locals: Default::default(),
        };
        if actual.statements(&statements)? != Some(value) || actual.trace != expected.trace {
            return None;
        }
    }
    Some(bounds)
}
fn flow_proposal(model: &Model<'_, '_>, site: FunctionSite, f: &Facts<'_>) -> Vec<Proposal> {
    let Some(bounds) = flow_parts(model, site) else {
        return Vec::new();
    };
    let lifetime = if uninitialized_bounds(model, site, bounds).is_some() {
        "write-free uninitialized let captures; every owner entry follows declaration execution"
    } else if initialized_bounds(model, site, bounds).is_some() {
        "two exclusive initialized setup epochs with all stores and owner entry roots closed"
    } else if conditional_initialized_bounds(model, site, bounds).is_some() {
        "single initialized setup epoch with all producer stores, aliases and owner entries closed"
    } else {
        return Vec::new();
    };
    vec![proposal("capturedTwoRangeOr45Predicate", "signals_captured_range_predicate",
        &format!("complete bounded Boolean grammar on one immutable formal, four distinct captured inclusive-bound operands and strict literal45; every path has the exact ordered comparison/coercion event trace and Boolean return of the canonical short-circuit predicate; local Boolean declarations, initialization, writers and uses are closed with no calls, escapes or extra effects; {lifetime}; receiver/native identity, bound values and API purpose remain opaque, with no numeric/ASCII/hostname/policy inference"), f)]
}

#[cfg(test)]
#[path = "captured_range_predicate_flow_tests.rs"]
mod flow_tests;
