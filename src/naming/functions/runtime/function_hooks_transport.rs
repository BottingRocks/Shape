//! Closed installer grammar for the reset/post-loop cache-copy transport.
//!
//! Unlike the legacy bag proof, this matches the entire owned protocol in order,
//! including its directly owned wrapper. Lexical identifiers are joined only by
//! SymbolId, with local and captured identities in separate namespaces. The
//! grammar preserves resets (it does not erase arbitrary writes), every native
//! property key, operator, argument slot, branch, and publication identity.
//!
//! Captured consumers are explicitly external: no Reflect/call-through/source
//! native identity is inferred. Definite initialization and a closed passive
//! alias/escape frontier are required in the actual enclosing scope. This is a
//! direct function proof, independent of class recognition or class projection.
use super::*;
use oxc_span::GetSpan;
use std::sync::OnceLock;

const CONTRACT: &str = include_str!("function_hooks_contract.js");
pub(super) const NAME: &str = "installMethodInterceptorWithExternalHelpers";

fn canonical(source: &str) -> Option<String> {
    let allocator = Allocator::default();
    let program = crate::analysis::parse_program(&allocator, source).ok()?;
    Some(
        oxc_codegen::Codegen::new()
            .with_options(oxc_codegen::CodegenOptions {
                comments: oxc_codegen::CommentOptions::disabled(),
                ..Default::default()
            })
            .build(&program)
            .code,
    )
}

fn normalized(model: &Model<'_, '_>, site: FunctionSite) -> Option<(String, Vec<SymbolId>)> {
    let mut identifiers = BTreeMap::new();
    for node in model
        .nodes
        .iter()
        .filter(|n| site.span.contains_inclusive(n.kind().span()))
    {
        let (span, symbol) = match node.kind() {
            AstKind::BindingIdentifier(id) => (id.span, id.symbol_id.get()?),
            AstKind::IdentifierReference(id) => {
                let Some(symbol) = crate::analysis::identifier_symbol(id, model.scoping) else {
                    if !["Object", "Array", "Function", "arguments"].contains(&id.name.as_str()) {
                        return None;
                    }
                    continue;
                };
                (id.span, symbol)
            }
            AstKind::WithStatement(_) => return None,
            _ => continue,
        };
        identifiers.insert((span.start, span.end), symbol);
    }
    let mut locals = BTreeMap::new();
    let mut captures = Vec::new();
    let mut changes = Vec::new();
    for ((start, end), symbol) in identifiers {
        if !model.scoping.symbol_redeclarations(symbol).is_empty() {
            return None;
        }
        let declaration = model
            .nodes
            .kind(model.scoping.symbol_declaration(symbol))
            .span();
        let name = if site.span.contains_inclusive(declaration) {
            let next = locals.len();
            let index = *locals.entry(symbol).or_insert(next);
            // No outside writer may change any own or nested protocol binder.
            if model.scoping.get_resolved_references(symbol).any(|r| {
                r.is_write()
                    && !site
                        .span
                        .contains_inclusive(model.nodes.kind(r.node_id()).span())
            }) {
                return None;
            }
            format!("__local{index}")
        } else {
            let index = captures
                .iter()
                .position(|s| *s == symbol)
                .unwrap_or_else(|| {
                    captures.push(symbol);
                    captures.len() - 1
                });
            format!("__capture{index}")
        };
        changes.push((start, end, name));
    }
    let mut source = model
        .source
        .get(site.span.start as usize..site.span.end as usize)?
        .to_string();
    for (start, end, name) in changes.into_iter().rev() {
        source.replace_range(
            (start - site.span.start) as usize..(end - site.span.start) as usize,
            &name,
        );
    }
    Some((canonical(&source)?, captures))
}

fn expected() -> Option<String> {
    let allocator = Allocator::default();
    let program = crate::analysis::parse_program(&allocator, CONTRACT).ok()?;
    let (scoping, nodes) = crate::analysis::analyze(&program);
    let model = Model::new(
        CONTRACT,
        &program,
        &scoping,
        &nodes,
        ProgramPropertiesLimits::default(),
    )
    .ok()?;
    let site = model.all_sites().iter().find(|s| {
        s.binding_symbol
            .is_some_and(|id| scoping.symbol_name(id) == "installer")
    })?;
    Some(normalized(&model, *site)?.0)
}

/// Only direct, guaranteed initialization statements count. Both arms of a
/// conditional must establish a value; a nested function or loop cannot donate
/// an origin. The value remains an external consumer input, not a native claim.
fn initializes(model: &Model<'_, '_>, s: &Statement<'_>, target: SymbolId) -> bool {
    match s {
        Statement::VariableDeclaration(d) => d
            .declarations
            .iter()
            .any(|d| plain_local_symbol(d) == Some(target) && d.init.is_some()),
        Statement::ExpressionStatement(_) => {
            plain_assignment(s).is_some_and(|a| assignment_identifier(model, a) == Some(target))
        }
        Statement::IfStatement(branch) => {
            let arm = |s: &Statement<'_>| {
                block(s).is_some_and(|ss| {
                    let Some((last, prefix)) = ss.split_last() else {
                        return false;
                    };
                    prefix
                        .iter()
                        .all(|s| matches!(s, Statement::FunctionDeclaration(_)))
                        && initializes(model, last, target)
                })
            };
            arm(&branch.consequent) && branch.alternate.as_ref().is_some_and(|s| arm(s))
        }
        _ => false,
    }
}

/// Follow passive local copies, then reject any invocation, escape, capture or
/// unknown use before all external consumers are initialized. Hoisted enclosing
/// declarations that could expose a later textual use are checked recursively.
fn safe_uses(
    model: &Model<'_, '_>,
    binding: SymbolId,
    frontier: u32,
    owner: FunctionSite,
    visited: &mut BTreeSet<SymbolId>,
) -> Option<()> {
    if !visited.insert(binding) {
        return Some(());
    }
    for r in model.scoping.get_resolved_references(binding) {
        let span = model.nodes.kind(r.node_id()).span();
        if r.is_write() {
            return None;
        }
        if span.start < frontier {
            if model.owning_site(r.node_id()).map(|s| s.span) != Some(owner.span) {
                return None;
            }
            let alias = match model.nodes.parent_kind(r.node_id()) {
                AstKind::VariableDeclarator(d)
                    if d.init.as_ref().is_some_and(|e| e.span() == span) =>
                {
                    plain_local_symbol(d)?
                }
                _ => return None,
            };
            safe_uses(model, alias, frontier, owner, visited)?;
        } else {
            let mut current = r.node_id();
            for _ in 0..512 {
                let parent = model.nodes.parent_id(current);
                if parent == current || parent == owner.node_id {
                    break;
                }
                current = parent;
                if let AstKind::Function(f) = model.nodes.kind(current) {
                    if let Some(binding) = model.site_by_span(f.span).and_then(|s| s.binding_symbol)
                    {
                        if !visited.contains(&binding) {
                            safe_uses(model, binding, frontier, owner, visited)?;
                        }
                    }
                }
            }
        }
    }
    Some(())
}

/// Join external argument consumers to their actual reaching initializer roles.
/// The binder's behavior is still opaque: these are consumer origins, not a
/// proof of Reflect/native equivalence. Requiring the two native inputs on the
/// same binder also prevents swapping both call sites from being alpha-equivalent.
fn helper_origins(
    model: &Model<'_, '_>,
    statements: &[Statement<'_>],
    captures: &[SymbolId],
) -> Option<()> {
    let (call_through, construct, invoke) = match captures {
        [call, _, _, construct, invoke, _, _, _, _] => (call, construct, invoke),
        [call, _, _, construct, invoke, _, _, _, _, _] => (call, construct, invoke),
        [_, call, _, _, construct, invoke, _, _, _, _, _] => (call, construct, invoke),
        _ => return None,
    };
    let initializer = |target: SymbolId| {
        statements.iter().find_map(|s| {
            if let Statement::VariableDeclaration(d) = s {
                for d in &d.declarations {
                    if plain_local_symbol(d) == Some(target) {
                        if let Some(init) = &d.init {
                            return Some(init);
                        }
                    }
                }
            }
            let assignment = plain_assignment(s)?;
            (assignment_identifier(model, assignment) == Some(target)).then_some(&assignment.right)
        })
    };
    let consumer = |target, path: &[&str]| {
        let call = call_of(initializer(target)?)?;
        let args = exact_arguments(call)?;
        if args.len() != 1 || expression_path(model, args[0]) != path {
            return None;
        }
        symbol_of(model, &call.callee)
    };
    let binder = consumer(*call_through, &["Object", "call"])?;
    let binder_declaration = model.scoping.symbol_declaration(binder);
    let binder_init = model.initializer(binder)?;
    if model.binding_writes(binder) != 0
        || model.owning_site(binder_declaration).map(|s| s.span)
            != model
                .owning_site(model.scoping.symbol_declaration(*call_through))
                .map(|s| s.span)
        || binder_init.span().end >= initializer(*call_through)?.span().start
        || binder_init.span().end >= initializer(*invoke)?.span().start
    {
        return None;
    }
    for target in [*call_through, *invoke, *construct] {
        let origin = statements
            .iter()
            .find(|s| initializes(model, s, target))?
            .span();
        if model.scoping.get_resolved_references(target).any(|r| {
            r.is_write() && !origin.contains_inclusive(model.nodes.kind(r.node_id()).span())
        }) {
            return None;
        }
    }
    if consumer(*invoke, &["Object", "apply"])? != binder {
        return None;
    }
    let origin = statements
        .iter()
        .find(|s| initializes(model, s, *construct))?;
    let Statement::IfStatement(branch) = origin else {
        return None;
    };
    let arm = block(&branch.consequent)?.last()?;
    let assignment = plain_assignment(arm)?;
    if assignment_identifier(model, assignment) != Some(*construct)
        || expression_path(model, &assignment.right) != ["Reflect", "construct"]
    {
        return None;
    }
    Some(())
}

fn captures_ready(model: &Model<'_, '_>, site: FunctionSite, captures: &[SymbolId]) -> Option<()> {
    let owner = model.owning_site(site.node_id)?;
    let AstKind::Function(f) = model.nodes.kind(owner.node_id) else {
        return None;
    };
    let statements = &f.body.as_ref()?.statements;
    helper_origins(model, statements, captures)?;
    let mut frontier = 0;
    for &capture in captures {
        let declaration = model.scoping.symbol_declaration(capture);
        if model.owning_site(declaration).map(|s| s.span) != Some(owner.span)
            || !matches!(
                model.nodes.kind(declaration),
                AstKind::VariableDeclarator(_)
            )
        {
            return None;
        }
        let init = statements.iter().find(|s| initializes(model, s, capture))?;
        frontier = frontier.max(init.span().end);
    }
    if statements.iter().any(|s| {
        s.span().start < frontier
            && matches!(
                s,
                Statement::ReturnStatement(_) | Statement::ThrowStatement(_)
            )
    }) {
        return None;
    }
    safe_uses(
        model,
        site.binding_symbol?,
        frontier,
        owner,
        &mut BTreeSet::new(),
    )
}

pub(super) fn proves(model: &Model<'_, '_>, site: FunctionSite) -> Option<()> {
    // Cheap necessary prefilter; the complete grammar, not these strings, proves
    // the behavior. No generated spelling, source hash or vendor participates.
    let source = model
        .source
        .get(site.span.start as usize..site.span.end as usize)?;
    if site.span.size() > 16000
        || !source.contains("onBeforeInvoke")
        || !source.contains("bypassResult")
    {
        return None;
    }
    let (actual, captures) = normalized(model, site)?;
    static EXPECTED: OnceLock<Option<String>> = OnceLock::new();
    if Some(&actual) != EXPECTED.get_or_init(expected).as_ref() {
        return None;
    }
    captures_ready(model, site, &captures)
}

#[cfg(test)]
#[path = "function_hooks_transport_tests.rs"]
mod tests;

// A finite lowering of the same closed protocol. Declarations retain their
// actual scoped identities; only their initialization transport is projected.
// The cache lowering below consumes a complete branch rather than dropping a
// reset, a write, or a guessed alias from a bag of facts.
fn lowered_cache(
    model: &Model<'_, '_>,
    site: FunctionSite,
    branch: &IfStatement<'_>,
) -> Option<(Vec<(SymbolId, SymbolId)>, Option<Span>, [SymbolId; 5])> {
    let Expression::BinaryExpression(null_test) = branch.test.without_parentheses() else {
        return None;
    };
    if null_test.operator.as_str() != "==" || !is_null_literal(&null_test.right) {
        return None;
    }
    let Expression::ComputedMemberExpression(bucket) = &null_test.left else {
        return None;
    };
    if bucket.optional || model.site_parameter_slot(&bucket.expression, site) != Some(1) {
        return None;
    }
    let registry = symbol_of(model, &bucket.object)?;
    let key = symbol_of(model, &bucket.expression)?;
    let no = block(branch.alternate.as_ref()?)?;
    let [Statement::VariableDeclaration(cursor_decl), loop_statement, rest @ ..] = no else {
        return None;
    };
    let [cursor_decl] = cursor_decl.declarations.as_slice() else {
        return None;
    };
    let cursor = plain_local_symbol(cursor_decl)?;
    if !matches!(cursor_decl.init.as_ref(),Some(Expression::NumericLiteral(n)) if n.value==0.0) {
        return None;
    }
    let Statement::ForStatement(loop_) = loop_statement else {
        return None;
    };
    if loop_.init.is_some() || loop_.test.is_some() || loop_.update.is_some() {
        return None;
    }
    let loop_body = block(&loop_.body)?;
    let (clear, body) = match loop_body {
        [clear, rest @ ..] if plain_assignment(clear).is_some() => (Some(clear), rest),
        _ => (None, loop_body),
    };
    let (bounds, select, owner_test, hit_flag, exit) = match body {
        [Statement::IfStatement(b), s, Statement::IfStatement(o), exit] => (b, s, o, None, exit),
        [Statement::IfStatement(b), s, Statement::IfStatement(o), flag, exit] => {
            (b, s, o, Some(flag), exit)
        }
        _ => return None,
    };
    let select_assignment = plain_assignment(select)?;
    let selected = assignment_identifier(model, select_assignment)?;
    if let Some(clear) = clear {
        let clear = plain_assignment(clear)?;
        if assignment_identifier(model, clear) != Some(selected)
            || !matches!(&clear.right,Expression::UnaryExpression(u) if u.operator.as_str()=="void" && matches!(&u.argument,Expression::NumericLiteral(n) if n.value==0.0))
        {
            return None;
        }
    }
    let Expression::BinaryExpression(bound_test) = &bounds.test else {
        return None;
    };
    if bound_test.operator.as_str() != ">="
        || symbol_of(model, &bound_test.left) != Some(cursor)
        || !matches!(&bound_test.right, Expression::StaticMemberExpression(m) if !m.optional && m.property.name=="length" && same_keyed_bucket(model,&m.object,registry,key))
    {
        return None;
    }
    let (miss_flag, clear_null, stop) = match block(&bounds.consequent)? {
        [clear, stop] => (None, clear, stop),
        [first, second, stop]
            if plain_assignment(first).is_some_and(|a| is_null_literal(&a.right))
                && plain_assignment(second).is_some_and(|a| is_null_literal(&a.right)) =>
        {
            (Some(second), first, stop)
        }
        [flag, clear, stop] => (Some(flag), clear, stop),
        _ => return None,
    };
    let clear_null = plain_assignment(clear_null)?;
    if bounds.alternate.is_some()
        || assignment_identifier(model, clear_null) != Some(selected)
        || !is_null_literal(&clear_null.right)
        || !is_break(stop)
        || !is_break(exit)
    {
        return None;
    }
    let select = plain_assignment(select)?;
    let Expression::ComputedMemberExpression(element) = &select.right else {
        return None;
    };
    if assignment_identifier(model, select) != Some(selected)
        || element.optional
        || symbol_of(model, &element.expression) != Some(cursor)
        || !same_keyed_bucket(model, &element.object, registry, key)
    {
        return None;
    }
    let Expression::BinaryExpression(compare) = &owner_test.test else {
        return None;
    };
    let owner = symbol_of(model, &compare.left)?;
    if compare.operator.as_str() != "!=="
        || !member_on(model, &compare.right, selected, "object")
        || owner_test.alternate.is_some()
    {
        return None;
    }
    let [increment, next] = block(&owner_test.consequent)? else {
        return None;
    };
    if !increments(model, increment, cursor) || !is_continue(next) {
        return None;
    }
    let yes = block(&branch.consequent)?;
    let mut aliases = Vec::new();
    let mut guard = None;
    let mut loop_flag = false;
    if let (Some(hit), Some(miss)) = (hit_flag, miss_flag) {
        let [first, second] = yes else {
            return None;
        };
        if !rest.is_empty() {
            return None;
        }
        let first = plain_assignment(first)?;
        let flag = assignment_identifier(model, first)?;
        let miss = plain_assignment(miss)?;
        let hit = plain_assignment(hit)?;
        let second = plain_assignment(second)?;
        if symbol_of(model, &hit.right) == Some(selected) && is_null_literal(&miss.right) {
            let copied = assignment_identifier(model, hit)?;
            if copied == selected
                || flag != selected
                || !is_null_literal(&first.right)
                || assignment_identifier(model, second) != Some(copied)
                || !is_null_literal(&second.right)
                || assignment_identifier(model, miss) != Some(copied)
                || model.binding_writes(copied) != 3
                || model.scoping.get_resolved_references(copied).any(|r| {
                    let span = model.nodes.kind(r.node_id()).span();
                    if r.is_write() {
                        !branch.span.contains_inclusive(span)
                    } else {
                        span.start < branch.span.end || !site.span.contains_inclusive(span)
                    }
                })
            {
                return None;
            }
            aliases.push((copied, selected));
        } else {
            if flag == selected
                || !matches!(&first.right,Expression::BooleanLiteral(b)if !b.value)
                || assignment_identifier(model, miss) != Some(flag)
                || !matches!(&miss.right,Expression::BooleanLiteral(b)if !b.value)
                || assignment_identifier(model, hit) != Some(flag)
                || !matches!(&hit.right,Expression::BinaryExpression(b)if b.operator.as_str()=="!="&&symbol_of(model,&b.left)==Some(selected)&&is_null_literal(&b.right))
                || assignment_identifier(model, second) != Some(selected)
                || !is_null_literal(&second.right)
                || model.binding_writes(flag) != 3
            {
                return None;
            }
            let reads: Vec<_> = model
                .scoping
                .get_resolved_references(flag)
                .filter(|r| !r.is_write())
                .collect();
            let [read] = reads.as_slice() else {
                return None;
            };
            let span = model.nodes.kind(read.node_id()).span();
            let AstKind::IfStatement(if_) = model.nodes.parent_kind(read.node_id()) else {
                return None;
            };
            if if_.test.span() != span
                || span.start < branch.span.end
                || !site.span.contains_inclusive(if_.span)
                || model.scoping.get_resolved_references(flag).any(|r| {
                    r.is_write()
                        && !branch
                            .span
                            .contains_inclusive(model.nodes.kind(r.node_id()).span())
                })
            {
                return None;
            }
            aliases.push((flag, selected));
            guard = Some(span);
        }
        loop_flag = true;
    } else if hit_flag.is_some() || miss_flag.is_some() {
        return None;
    }
    if !loop_flag {
        match (yes, rest) {
            ([null], []) => {
                let null = plain_assignment(null)?;
                if assignment_identifier(model, null) != Some(selected)
                    || !is_null_literal(&null.right)
                {
                    return None;
                }
            }
            ([first, second], [last]) => {
                if plain_assignment(first)
                    .is_some_and(|a| matches!(&a.right, Expression::BooleanLiteral(_)))
                {
                    let flag_store = last;
                    let first = plain_assignment(first)?;
                    let second = plain_assignment(second)?;
                    let flag_store = plain_assignment(flag_store)?;
                    let flag = assignment_identifier(model, first)?;
                    if !matches!(&first.right,Expression::BooleanLiteral(b) if !b.value)
                        || assignment_identifier(model, second) != Some(selected)
                        || !is_null_literal(&second.right)
                        || assignment_identifier(model, flag_store) != Some(flag)
                        || !matches!(&flag_store.right,Expression::BinaryExpression(b) if b.operator.as_str()=="!=" && symbol_of(model,&b.left)==Some(selected)&&is_null_literal(&b.right))
                        || flag == selected
                        || model.binding_writes(flag) != 2
                    {
                        return None;
                    }
                    let reads: Vec<_> = model
                        .scoping
                        .get_resolved_references(flag)
                        .filter(|r| !r.is_write())
                        .collect();
                    let [read] = reads.as_slice() else {
                        return None;
                    };
                    let span = model.nodes.kind(read.node_id()).span();
                    let AstKind::IfStatement(if_) = model.nodes.parent_kind(read.node_id()) else {
                        return None;
                    };
                    if if_.test.span() != span
                        || span.start < branch.span.end
                        || !site.span.contains_inclusive(if_.span)
                    {
                        return None;
                    }
                    if model.scoping.get_resolved_references(flag).any(|r| {
                        r.is_write()
                            && !branch
                                .span
                                .contains_inclusive(model.nodes.kind(r.node_id()).span())
                    }) {
                        return None;
                    }
                    aliases.push((flag, selected));
                    guard = Some(span);
                } else {
                    let copy = last;
                    let first = plain_assignment(first)?;
                    let second = plain_assignment(second)?;
                    let copy = plain_assignment(copy)?;
                    if assignment_identifier(model, first) != Some(selected)
                        || !is_null_literal(&first.right)
                    {
                        return None;
                    }
                    let alias = assignment_identifier(model, second)?;
                    if alias == selected
                        || !is_null_literal(&second.right)
                        || assignment_identifier(model, copy) != Some(alias)
                        || symbol_of(model, &copy.right) != Some(selected)
                    {
                        return None;
                    }
                    // The copied record has exactly these two writes and only its
                    // guarded result/original reads thereafter; no outside writer.
                    if model.binding_writes(alias) != 2 {
                        return None;
                    }
                    for r in model.scoping.get_resolved_references(alias) {
                        let span = model.nodes.kind(r.node_id()).span();
                        if r.is_write() {
                            if !branch.span.contains_inclusive(span) {
                                return None;
                            }
                        } else if !branch.span.contains_inclusive(span) {
                            if span.start < branch.span.end || !site.span.contains_inclusive(span) {
                                return None;
                            }
                        }
                    }
                    aliases.push((alias, selected));
                }
            }
            _ => return None,
        }
    }
    for (alias, _) in &aliases {
        local_alias(model, site, *alias)?;
    }
    for s in [cursor, selected, owner] {
        if !site.span.contains_inclusive(model.scoping.symbol_span(s))
            || !model.scoping.symbol_redeclarations(s).is_empty()
        {
            return None;
        }
    }
    if [registry, key, cursor, selected, owner]
        .into_iter()
        .collect::<BTreeSet<_>>()
        .len()
        != 5
        || model.binding_writes(selected) != (if clear.is_some() { 4 } else { 3 })
        || model.binding_writes(cursor) != 1
    {
        return None;
    }
    Some((aliases, guard, [registry, key, cursor, selected, owner]))
}

fn same_keyed_bucket(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    registry: SymbolId,
    key: SymbolId,
) -> bool {
    matches!(e,Expression::ComputedMemberExpression(m) if !m.optional && symbol_of(model,&m.object)==Some(registry)&&symbol_of(model,&m.expression)==Some(key))
}

fn lowered_normalized(
    model: &Model<'_, '_>,
    site: FunctionSite,
) -> Option<(String, Vec<SymbolId>)> {
    let mut cache = None;
    for node in model
        .nodes
        .iter()
        .filter(|n| site.span.contains_inclusive(n.kind().span()))
    {
        if model.owning_site(node.id()).map(|s| s.span) != Some(site.span) {
            continue;
        }
        if let AstKind::IfStatement(branch) = node.kind() {
            if let Some(lowering) = lowered_cache(model, site, branch) {
                if cache.replace((branch.span, lowering)).is_some() {
                    return None;
                }
            }
        }
    }
    let (cache_span, mut aliases, guard, cache_ids) = match cache {
        Some((span, (aliases, guard, ids))) => (Some(span), aliases, guard, Some(ids)),
        None => (None, Vec::new(), None, None),
    };
    let descriptor = lowered_descriptor(model, site);
    if let Some((_, copy, _)) = &descriptor {
        aliases.extend(copy.iter().copied());
    }
    let frozen_guard = lowered_frozen_guard(model, site);
    let early_returns = lowered_early_returns(model, site);
    let payload = lowered_payload(model, site);
    if let Some((_, copy, _)) = &payload {
        aliases.extend(copy.iter().copied());
    }
    for (alias, _) in &aliases {
        local_alias(model, site, *alias)?;
    }
    let mut ignored = Vec::new();
    let mut declarations = Vec::new();
    for node in model
        .nodes
        .iter()
        .filter(|n| site.span.contains_inclusive(n.kind().span()))
    {
        if let AstKind::VariableDeclaration(d) = node.kind() {
            // Only standalone declarations are this transport. A loop head's
            // declaration has different lifetime semantics and is declined.
            if !matches!(
                model.nodes.parent_kind(node.id()),
                AstKind::FunctionBody(_) | AstKind::BlockStatement(_)
            ) {
                return None;
            }
            for v in &d.declarations {
                let sym = plain_local_symbol(v)?;
                if !model.scoping.symbol_redeclarations(sym).is_empty()
                    || d.kind == VariableDeclarationKind::Const && model.binding_writes(sym) != 0
                {
                    return None;
                }
                if v.init.is_none() {
                    ignored.push(v.id.span());
                }
                // A lexical direct read before its declaration is a TDZ, not
                // an interchangeable hoist. Nested reads remain guarded by
                // the complete owned wrapper and exposure frontier.
                if d.kind != VariableDeclarationKind::Var
                    && model.scoping.get_resolved_references(sym).any(|r| {
                        model.nodes.kind(r.node_id()).span().start < v.id.span().start
                            && model.owning_site(r.node_id()).map(|s| s.span)
                                == model.owning_site(node.id()).map(|s| s.span)
                    })
                {
                    return None;
                }
            }
            declarations.push(d);
        }
    }
    let alias = |s| aliases.iter().find(|(a, _)| *a == s).map_or(s, |(_, b)| *b);
    let mut ids = BTreeMap::new();
    for node in model
        .nodes
        .iter()
        .filter(|n| site.span.contains_inclusive(n.kind().span()))
    {
        let (span, symbol) = match node.kind() {
            AstKind::BindingIdentifier(id) => (id.span, id.symbol_id.get()?),
            AstKind::IdentifierReference(id) => {
                let Some(s) = crate::analysis::identifier_symbol(id, model.scoping) else {
                    if !["Object", "Array", "Function", "arguments"].contains(&id.name.as_str()) {
                        return None;
                    }
                    continue;
                };
                (id.span, s)
            }
            AstKind::WithStatement(_) => return None,
            _ => continue,
        };
        if ignored.iter().any(|s| s.contains_inclusive(span)) {
            continue;
        }
        ids.insert((span.start, span.end), alias(symbol));
    }
    let mut locals = BTreeMap::new();
    let mut captures = Vec::new();
    let mut names = BTreeMap::new();
    for symbol in ids.values() {
        if names.contains_key(symbol) {
            continue;
        }
        if !model.scoping.symbol_redeclarations(*symbol).is_empty() {
            return None;
        }
        let declaration = model
            .nodes
            .kind(model.scoping.symbol_declaration(*symbol))
            .span();
        let name = if site.span.contains_inclusive(declaration) {
            if model.scoping.get_resolved_references(*symbol).any(|r| {
                r.is_write()
                    && !site
                        .span
                        .contains_inclusive(model.nodes.kind(r.node_id()).span())
            }) {
                return None;
            }
            let index = locals.len();
            locals.insert(*symbol, index);
            format!("__local{index}")
        } else {
            let index = captures.len();
            captures.push(*symbol);
            format!("__capture{index}")
        };
        names.insert(*symbol, name);
    }
    let render = |span: Span| -> Option<String> {
        let mut value = model
            .source
            .get(span.start as usize..span.end as usize)?
            .to_string();
        for ((start, end), symbol) in ids
            .iter()
            .rev()
            .filter(|((start, end), _)| *start >= span.start && *end <= span.end)
        {
            value.replace_range(
                (start - span.start) as usize..(end - span.start) as usize,
                names.get(symbol)?,
            );
        }
        Some(value)
    };
    let mut edits: Vec<(Span, String)> = ids
        .iter()
        .filter_map(|((a, b), s)| names.get(s).map(|n| (Span::new(*a, *b), n.clone())))
        .collect();
    for d in declarations {
        if cache_span.is_some_and(|span| span.contains_inclusive(d.span)) {
            continue;
        }
        let mut value = String::new();
        for v in &d.declarations {
            if let Some(init) = &v.init {
                value.push_str(names.get(&alias(plain_local_symbol(v)?))?);
                value.push_str("=");
                value.push_str(&render(init.span())?);
                value.push(';');
            }
        }
        edits.retain(|(s, _)| !d.span.contains_inclusive(*s));
        edits.push((d.span, value));
    }
    if let (Some(cache_span), Some(cache_ids)) = (cache_span, cache_ids) {
        let [registry, key, cursor, selected, owner]: [String; 5] = cache_ids
            .map(|s| names.get(&alias(s)).cloned())
            .into_iter()
            .collect::<Option<Vec<_>>>()?
            .try_into()
            .ok()?;
        let replacement=format!("if({registry}[{key}]==null){{{selected}=null;}}else{{{cursor}=0;for(;;){{{selected}=void 0;if({cursor}>={registry}[{key}].length){{{selected}=null;break;}}{selected}={registry}[{key}][{cursor}];if({owner}!=={selected}.object){{++{cursor};continue;}}break;}}}}");
        edits.retain(|(s, _)| !cache_span.contains_inclusive(*s));
        edits.push((cache_span, replacement));
        if let Some(guard) = guard {
            edits.retain(|(s, _)| !guard.contains_inclusive(*s));
            edits.push((guard, format!("{selected}!=null")));
        }
    }
    if let Some((span, _, descriptor_ids)) = descriptor {
        let [cursor, key, record, desc]: [String; 4] = descriptor_ids
            .map(|s| names.get(&alias(s)).cloned())
            .into_iter()
            .collect::<Option<Vec<_>>>()?
            .try_into()
            .ok()?;
        let value=format!("for(;;){{{record}=void 0;if({cursor}==null){{{record}=null;break;}}{desc}=Object.getOwnPropertyDescriptor({cursor},{key});if({desc}==null){{{cursor}=Object.getPrototypeOf({cursor});continue;}}{record}={{containingObj:{cursor},desc:{desc}}};break;}}");
        edits.retain(|(s, _)| !span.contains_inclusive(*s));
        edits.push((span, value));
    }
    if let Some((span, _, [args, payload])) = payload {
        let args = names.get(&alias(args))?;
        let payload = names.get(&alias(payload))?;
        let replacement = format!(
            "{args}=arguments;{payload}={{args:{args},thisObj:this,threw:true,result:null}};"
        );
        edits.retain(|(s, _)| !span.contains_inclusive(*s));
        edits.push((span, replacement));
    }
    for (span, originals) in early_returns {
        let value = names.get(&alias(originals))?;
        edits.retain(|(s, _)| !span.contains_inclusive(*s));
        edits.push((
            span,
            format!("{value}={{originals:{value}}};return {value};"),
        ));
    }
    if let Some((branch, test, [configurable, writable])) = frozen_guard {
        let configurable = names.get(&alias(configurable))?;
        let writable = names.get(&alias(writable))?;
        edits.retain(|(s, _)| !branch.contains_inclusive(*s) && !test.contains_inclusive(*s));
        edits.push((branch, String::new()));
        edits.push((
            test,
            format!("({configurable}===false&&{writable}===false)"),
        ));
    }
    edits.sort_by_key(|(s, _)| s.start);
    if edits.windows(2).any(|w| w[0].0.end > w[1].0.start) {
        return None;
    }
    let mut source = model
        .source
        .get(site.span.start as usize..site.span.end as usize)?
        .to_string();
    for (span, value) in edits.into_iter().rev() {
        source.replace_range(
            (span.start - site.span.start) as usize..(span.end - site.span.start) as usize,
            &value,
        );
    }
    Some((canonical(&source)?, captures))
}

fn lowered_expected() -> Option<String> {
    let allocator = Allocator::default();
    let program = crate::analysis::parse_program(&allocator, CONTRACT).ok()?;
    let (scoping, nodes) = crate::analysis::analyze(&program);
    let model = Model::new(
        CONTRACT,
        &program,
        &scoping,
        &nodes,
        ProgramPropertiesLimits::default(),
    )
    .ok()?;
    let site = model.all_sites().iter().find(|s| {
        s.binding_symbol
            .is_some_and(|id| scoping.symbol_name(id) == "installer")
    })?;
    Some(lowered_normalized(&model, *site)?.0)
}

pub(super) fn proves_lowered(model: &Model<'_, '_>, site: FunctionSite) -> Option<()> {
    let source = model
        .source
        .get(site.span.start as usize..site.span.end as usize)?;
    if site.span.size() > 16000
        || !source.contains("onBeforeInvoke")
        || !source.contains("bypassResult")
    {
        return None;
    }
    let (actual, captures) = lowered_normalized(model, site)?;
    static EXPECTED: OnceLock<Option<String>> = OnceLock::new();
    if Some(&actual) != EXPECTED.get_or_init(lowered_expected).as_ref() {
        static HELPER: OnceLock<Option<String>> = OnceLock::new();
        static BOTH_HELPERS: OnceLock<Option<String>> = OnceLock::new();
        let (lookup, registry) = if Some(&actual)
            == HELPER.get_or_init(helper_installer_expected).as_ref()
        {
            let [_, lookup, _, _, _, _, _, _, _, registry] = captures.as_slice() else {
                return None;
            };
            (*lookup, *registry)
        } else if Some(&actual) == BOTH_HELPERS.get_or_init(both_helpers_expected).as_ref() {
            let [descriptor, _, lookup, _, _, _, _, _, _, _, registry] = captures.as_slice() else {
                return None;
            };
            let function = stable_helper(
                model,
                *descriptor,
                model.owning_site(site.node_id)?,
                u32::MAX,
                0,
            )?;
            let (actual_descriptor, captures) = normalized(model, function)?;
            static DESCRIPTOR: OnceLock<Option<String>> = OnceLock::new();
            if !captures.is_empty()
                || Some(&actual_descriptor)
                    != DESCRIPTOR.get_or_init(descriptor_helper_expected).as_ref()
            {
                return None;
            }
            (*lookup, *registry)
        } else {
            return None;
        };
        let helper = stable_helper(model, lookup, model.owning_site(site.node_id)?, u32::MAX, 0)?;
        let (actual_helper, helper_captures) = normalized(model, helper)?;
        static CACHE_HELPER: OnceLock<Option<String>> = OnceLock::new();
        if Some(&actual_helper) != CACHE_HELPER.get_or_init(cache_helper_expected).as_ref()
            || helper_captures.as_slice() != [registry]
        {
            return None;
        }
    }
    captures_ready(model, site, &captures)
}

// Two record locals are an optional lowering of the prototype walk, not an
// invitation to erase arbitrary aliases. The first has exactly one post-loop
// read (the immediate null guard); only the copied local is subsequently
// rebound to .desc. Both complete reset/null/hit arms are consumed here.
fn lowered_descriptor(
    model: &Model<'_, '_>,
    site: FunctionSite,
) -> Option<(Span, Vec<(SymbolId, SymbolId)>, [SymbolId; 4])> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return None;
    };
    let statements = &function.body.as_ref()?.statements;
    for (index, statement) in statements.iter().enumerate() {
        let Statement::ForStatement(loop_) = statement else {
            continue;
        };
        if loop_.init.is_some() || loop_.test.is_some() || loop_.update.is_some() {
            continue;
        }
        let (reset_a, reset_b, absent, read, miss, hit, copy, stop, cursor_copy) = match block(
            &loop_.body,
        )? {
            [a, b, Statement::IfStatement(absent), Statement::VariableDeclaration(read), Statement::IfStatement(miss), hit, copy, stop] => {
                (a, b, absent, read, miss, hit, copy, stop, None)
            }
            [a, b, Statement::IfStatement(absent), Statement::VariableDeclaration(read), Statement::IfStatement(miss), hit, record_copy, copy, stop] => {
                (a, b, absent, read, miss, hit, copy, stop, Some(record_copy))
            }
            _ => continue,
        };
        let reset_a = plain_assignment(reset_a)?;
        let reset_b = plain_assignment(reset_b)?;
        let record = assignment_identifier(model, reset_a)?;
        let copied = assignment_identifier(model, reset_b)?;
        let void_zero = |e: &Expression<'_>| matches!(e,Expression::UnaryExpression(u)if u.operator.as_str()=="void"&&matches!(&u.argument,Expression::NumericLiteral(n)if n.value==0.0));
        if !void_zero(&reset_a.right) || !void_zero(&reset_b.right) || record == copied {
            return None;
        }
        let Expression::BinaryExpression(null) = &absent.test else {
            return None;
        };
        let cursor = symbol_of(model, &null.left)?;
        if null.operator.as_str() != "=="
            || !is_null_literal(&null.right)
            || absent.alternate.is_some()
        {
            return None;
        }
        let [null_a, null_b, absent_stop] = block(&absent.consequent)? else {
            return None;
        };
        for (s, target) in [(null_a, record), (null_b, copied)] {
            let a = plain_assignment(s)?;
            if assignment_identifier(model, a) != Some(target) || !is_null_literal(&a.right) {
                return None;
            }
        }
        if !is_break(absent_stop) || !is_break(stop) {
            return None;
        }
        let [read] = read.declarations.as_slice() else {
            return None;
        };
        let descriptor = plain_local_symbol(read)?;
        let call = call_of(read.init.as_ref()?)?;
        let args = direct_call(model, call, &["Object", "getOwnPropertyDescriptor"])?;
        let [object, key] = args.as_slice() else {
            return None;
        };
        let key = symbol_of(model, key)?;
        if symbol_of(model, object) != Some(cursor)
            || model.site_parameter_slot(args[1], site) != Some(1)
        {
            return None;
        }
        if !is_null_compare(model, &miss.test, descriptor) || miss.alternate.is_some() {
            return None;
        }
        let [advance, again] = block(&miss.consequent)? else {
            return None;
        };
        let advance = plain_assignment(advance)?;
        let call = call_of(&advance.right)?;
        let args = direct_call(model, call, &["Object", "getPrototypeOf"])?;
        if assignment_identifier(model, advance) != Some(cursor)
            || args.len() != 1
            || symbol_of(model, args[0]) != Some(cursor)
            || !is_continue(again)
        {
            return None;
        }
        let hit = plain_assignment(hit)?;
        let fields = literal_object(&hit.right)?;
        if assignment_identifier(model, hit)
            != Some(if cursor_copy.is_some() {
                cursor
            } else {
                record
            })
            || fields.len() != 2
            || fields
                .get("containingObj")
                .and_then(|e| symbol_of(model, e))
                != Some(cursor)
            || fields.get("desc").and_then(|e| symbol_of(model, e)) != Some(descriptor)
        {
            return None;
        }
        if let Some(copy) = cursor_copy {
            let copy = plain_assignment(copy)?;
            if assignment_identifier(model, copy) != Some(record)
                || symbol_of(model, &copy.right) != Some(cursor)
                || model.scoping.get_resolved_references(cursor).any(|r| {
                    !loop_
                        .span
                        .contains_inclusive(model.nodes.kind(r.node_id()).span())
                })
            {
                return None;
            }
            local_alias(model, site, cursor)?;
        }
        let copy = plain_assignment(copy)?;
        if assignment_identifier(model, copy) != Some(copied)
            || symbol_of(model, &copy.right) != Some(record)
        {
            return None;
        }
        let Statement::IfStatement(guard) = statements.get(index + 1)? else {
            return None;
        };
        if !matches!(&guard.test,Expression::BinaryExpression(b)if b.operator.as_str()=="=="&&symbol_of(model,&b.left)==Some(record)&&is_null_literal(&b.right))
            || guard.alternate.is_some()
            || !is_return_null(&guard.consequent)
        {
            return None;
        }
        let outside_reads: Vec<_> = model
            .scoping
            .get_resolved_references(record)
            .filter(|r| {
                !r.is_write()
                    && !loop_
                        .span
                        .contains_inclusive(model.nodes.kind(r.node_id()).span())
            })
            .collect();
        if outside_reads.len() != 1
            || !guard
                .test
                .span()
                .contains_inclusive(model.nodes.kind(outside_reads[0].node_id()).span())
            || model.binding_writes(record) != 3
            || model.binding_writes(copied) != 4
        {
            return None;
        }
        // The scope index, not spelling, proves all five distinct roles.
        if [cursor, key, record, copied, descriptor]
            .into_iter()
            .collect::<BTreeSet<_>>()
            .len()
            != 5
        {
            return None;
        }
        return Some((
            loop_.span,
            vec![(copied, record)],
            [cursor, key, record, descriptor],
        ));
    }
    None
}

// A projected alias must be an actual own local. Removing an external write
// would change observable state even if its last local consumer is a copy.
fn local_alias(model: &Model<'_, '_>, site: FunctionSite, symbol: SymbolId) -> Option<()> {
    if !plain_local_in_body(model, site, symbol)
        || model.scoping.get_resolved_references(symbol).any(|r| {
            !site
                .span
                .contains_inclusive(model.nodes.kind(r.node_id()).span())
        })
    {
        return None;
    }
    Some(())
}

fn statement_value<'a>(
    model: &Model<'_, '_>,
    s: &'a Statement<'a>,
) -> Option<(SymbolId, &'a Expression<'a>)> {
    if let Statement::VariableDeclaration(d) = s {
        let [d] = d.declarations.as_slice() else {
            return None;
        };
        return Some((plain_local_symbol(d)?, d.init.as_ref()?));
    }
    let a = plain_assignment(s)?;
    Some((assignment_identifier(model, a)?, &a.right))
}

fn arguments_value(e: &Expression<'_>) -> bool {
    matches!(e, Expression::Identifier(id) if id.name == "arguments")
}

fn void_zero(e: &Expression<'_>) -> bool {
    matches!(e, Expression::UnaryExpression(u) if u.operator.as_str()=="void" && matches!(&u.argument,Expression::NumericLiteral(n) if n.value==0.0))
}

// Consume the entire straight-line failure-payload construction. These fresh
// objects have no accessors, computed fields or escape between construction
// and the sole copy. Every noncanonical alias has one read and one value
// origin. Only the directly owned strict wrapper can use this transport;
// its before-flow, invocation, finally override and return remain in the
// complete installer contract comparison, with all scoped identities intact.
fn lowered_payload(
    model: &Model<'_, '_>,
    site: FunctionSite,
) -> Option<(Span, Vec<(SymbolId, SymbolId)>, [SymbolId; 2])> {
    for node in model.nodes.iter() {
        let AstKind::Function(f) = node.kind() else {
            continue;
        };
        if model.owning_site(node.id()).map(|s| s.span) != Some(site.span) {
            continue;
        }
        let wrapper = model.site_by_span(f.span)?;
        let body = f.body.as_ref()?;
        if !body.directives.iter().any(|d| d.directive == "use strict") {
            continue;
        }
        let ss = &body.statements;
        let (args, first) = statement_value(model, ss.first()?)?;
        if !arguments_value(first) {
            continue;
        }
        let ti = ss
            .iter()
            .position(|s| matches!(s,Statement::TryStatement(t)if t.finalizer.is_some()))?;
        let start = ss[..ti]
            .iter()
            .rposition(|s| matches!(s, Statement::IfStatement(_)))?
            + 1;
        let region = &ss[start..ti];
        let active:Vec<_>=region.iter().filter(|s|!matches!(s,Statement::VariableDeclaration(d) if d.declarations.iter().all(|v|v.init.is_none()))).collect();
        let span = Span::new(region.first()?.span().start, region.last()?.span().end);
        let payload;
        let mut aliases = Vec::new();
        let record = |e: &Expression<'_>, raw: bool| -> Option<()> {
            let fields = literal_object(e)?;
            if !ordered_fields(e, &["args", "thisObj", "threw", "result"])
                || fields.len() != 4
                || !matches!(fields.get("thisObj")?, Expression::ThisExpression(_))
                || !matches!(fields.get("threw")?,Expression::BooleanLiteral(b)if b.value)
                || !is_null_literal(fields.get("result")?)
            {
                return None;
            }
            let value = fields.get("args")?;
            if if raw {
                !arguments_value(value)
            } else {
                symbol_of(model, value) != Some(args)
            } {
                return None;
            }
            Some(())
        };
        match active.as_slice() {
            [reset, init] => {
                let (target, value) = statement_value(model, reset)?;
                let (dest, object) = statement_value(model, init)?;
                if target != args {
                    return None;
                }
                if arguments_value(value) {
                    record(object, false)?;
                    payload = dest;
                } else {
                    record(value, true)?;
                    if symbol_of(model, object) != Some(args) {
                        return None;
                    }
                    payload = dest;
                    // args has already served the before channel; its fresh
                    // failure object is copied once and never observed again.
                    if model.scoping.get_resolved_references(args).any(|r| {
                        let s = model.nodes.kind(r.node_id()).span();
                        s.start > span.end
                    }) {
                        return None;
                    }
                }
            }
            [init, reset, a, b, c, d, copy] => {
                let (initial, object) = statement_value(model, init)?;
                if initial == args {
                    return None;
                }
                let fields = literal_object(object)?;
                if !ordered_fields(object, &["args", "thisObj"])
                    || fields.len() != 2
                    || !void_zero(fields.get("args")?)
                    || !void_zero(fields.get("thisObj")?)
                {
                    return None;
                }
                let (target, value) = statement_value(model, reset)?;
                if target != args || !arguments_value(value) {
                    return None;
                }
                for (s, key) in [(*a, "args"), (*b, "thisObj"), (*c, "threw"), (*d, "result")] {
                    let assignment = plain_assignment(s)?;
                    let (object, property) = assignment_member(assignment)?;
                    if symbol_of(model, object) != Some(initial) || property != key {
                        return None;
                    }
                    let ok = match key {
                        "args" => symbol_of(model, &assignment.right) == Some(args),
                        "thisObj" => matches!(&assignment.right, Expression::ThisExpression(_)),
                        "threw" => {
                            matches!(&assignment.right,Expression::BooleanLiteral(b)if b.value)
                        }
                        "result" => is_null_literal(&assignment.right),
                        _ => false,
                    };
                    if !ok {
                        return None;
                    }
                }
                let (dest, value) = statement_value(model, copy)?;
                if symbol_of(model, value) != Some(initial) {
                    return None;
                }
                payload = dest;
                local_alias(model, wrapper, initial)?;
                if model
                    .scoping
                    .get_resolved_references(initial)
                    .any(|r| !span.contains_inclusive(model.nodes.kind(r.node_id()).span()))
                {
                    return None;
                }
                aliases.push((initial, payload));
            }
            _ => return None,
        }
        if args == payload {
            return None;
        }
        for s in [args, payload] {
            local_alias(model, wrapper, s)?;
        }
        return Some((span, aliases, [args, payload]));
    }
    None
}

// Direct early object returns need no observable local rebind. Only the exact
// own originals record is projected; the complete surrounding frozen refusal
// and all other uses still participate in the closed grammar comparison.
fn lowered_early_returns(model: &Model<'_, '_>, site: FunctionSite) -> Vec<(Span, SymbolId)> {
    model
        .nodes
        .iter()
        .filter_map(|node| {
            if model.owning_site(node.id()).map(|s| s.span) != Some(site.span) {
                return None;
            }
            let AstKind::ReturnStatement(r) = node.kind() else {
                return None;
            };
            let fields = literal_object(r.argument.as_ref()?)?;
            if fields.len() != 1 {
                return None;
            }
            let originals = symbol_of(model, fields.get("originals")?)?;
            local_alias(model, site, originals)?;
            Some((r.span, originals))
        })
        .collect()
}

// This is an independently closed lookup role, with no caller or sibling
// evidence donated to it. The registry capture is joined back to the actual
// installer's publication registry by scoped symbol identity.
const CACHE_LOOKUP: &str = r#"function context(registry){
function lookup(object,key){
    if(registry[key]==null){return null;}
    let cursor=0;
    while(cursor<registry[key].length){
        if(object===registry[key][cursor].object){return registry[key][cursor];}
        else{++cursor;continue;}
    }
    return null;
}
}context();"#;

fn cache_helper_expected() -> Option<String> {
    let allocator = Allocator::default();
    let program = crate::analysis::parse_program(&allocator, CACHE_LOOKUP).ok()?;
    let (scoping, nodes) = crate::analysis::analyze(&program);
    let model = Model::new(
        CACHE_LOOKUP,
        &program,
        &scoping,
        &nodes,
        ProgramPropertiesLimits::default(),
    )
    .ok()?;
    let site = model.all_sites().iter().find(|s| {
        s.binding_symbol
            .is_some_and(|id| scoping.symbol_name(id) == "lookup")
    })?;
    Some(normalized(&model, *site)?.0)
}

fn helper_installer_expected() -> Option<String> {
    let begin = CONTRACT.find("let selected, cached;")?;
    let end = CONTRACT.find("before = hookFactory();")?;
    let source=format!("{}let selected=lookup(owner,key);if(selected!=null){{if(selected.result!=null){{return selected.result;}}original=selected.original;originals.value=original;}}{}",&CONTRACT[..begin],&CONTRACT[end..])
        .replacen("registry, hookFactory","registry, lookup, hookFactory",1);
    let allocator = Allocator::default();
    let program = crate::analysis::parse_program(&allocator, &source).ok()?;
    let (scoping, nodes) = crate::analysis::analyze(&program);
    let model = Model::new(
        &source,
        &program,
        &scoping,
        &nodes,
        ProgramPropertiesLimits::default(),
    )
    .ok()?;
    let site = model.all_sites().iter().find(|s| {
        s.binding_symbol
            .is_some_and(|id| scoping.symbol_name(id) == "installer")
    })?;
    Some(lowered_normalized(&model, *site)?.0)
}

// Follow only immutable, direct same-scope alias initializers, with definite
// ordering at every edge. A function declaration is hoisted; lexical alias
// values are not. Nested or conditional origins and all writers are refused.
fn stable_helper(
    model: &Model<'_, '_>,
    symbol: SymbolId,
    owner: FunctionSite,
    before: u32,
    depth: usize,
) -> Option<FunctionSite> {
    if depth > 16
        || !model.scoping.symbol_redeclarations(symbol).is_empty()
        || model.binding_writes(symbol) != 0
    {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(symbol);
    if model.owning_site(declaration).map(|s| s.span) != Some(owner.span) {
        return None;
    }
    let AstKind::Function(parent) = model.nodes.kind(owner.node_id) else {
        return None;
    };
    let statements = &parent.body.as_ref()?.statements;
    match model.nodes.kind(declaration) {
        AstKind::Function(f) if statements.iter().any(|s| s.span() == f.span) => {
            model.site_by_span(f.span)
        }
        AstKind::VariableDeclarator(d) => {
            let init = d.init.as_ref()?;
            if init.span().end>=before||!statements.iter().any(|s|matches!(s,Statement::VariableDeclaration(v) if v.declarations.iter().any(|v|v.span==d.span))){return None;}
            stable_helper(
                model,
                symbol_of(model, init)?,
                owner,
                init.span().start,
                depth + 1,
            )
        }
        _ => None,
    }
}

const DESCRIPTOR_LOOKUP: &str = r#"function lookup(object,key){
    let cursor=object;
    while(cursor!=null){
        let descriptor=Object.getOwnPropertyDescriptor(cursor,key);
        if(descriptor!=null){cursor={containingObj:cursor,desc:descriptor};return cursor;}
        else{cursor=Object.getPrototypeOf(cursor);continue;}
    }
    return null;
}lookup();"#;

fn descriptor_helper_expected() -> Option<String> {
    let allocator = Allocator::default();
    let program = crate::analysis::parse_program(&allocator, DESCRIPTOR_LOOKUP).ok()?;
    let (scoping, nodes) = crate::analysis::analyze(&program);
    let model = Model::new(
        DESCRIPTOR_LOOKUP,
        &program,
        &scoping,
        &nodes,
        ProgramPropertiesLimits::default(),
    )
    .ok()?;
    let site = model.all_sites().iter().find(|s| {
        s.binding_symbol
            .is_some_and(|id| scoping.symbol_name(id) == "lookup")
    })?;
    Some(normalized(&model, *site)?.0)
}

fn both_helpers_expected() -> Option<String> {
    let begin = CONTRACT.find("let selected, cached;")?;
    let end = CONTRACT.find("before = hookFactory();")?;
    let source=format!("{}let selected=lookup(owner,key);if(selected!=null){{if(selected.result!=null){{return selected.result;}}original=selected.original;originals.value=original;}}{}",&CONTRACT[..begin],&CONTRACT[end..])
        .replacen("registry, hookFactory","registry, lookup, descriptorLookup, hookFactory",1);
    let begin = source.find("let cursor = input;")?;
    let end = source.find("if (descriptorRecord == null)")?;
    let source = format!(
        "{}let descriptorRecord=descriptorLookup(input,key);{}",
        &source[..begin],
        &source[end..]
    );
    let allocator = Allocator::default();
    let program = crate::analysis::parse_program(&allocator, &source).ok()?;
    let (scoping, nodes) = crate::analysis::analyze(&program);
    let model = Model::new(
        &source,
        &program,
        &scoping,
        &nodes,
        ProgramPropertiesLimits::default(),
    )
    .ok()?;
    let site = model.all_sites().iter().find(|s| {
        s.binding_symbol
            .is_some_and(|id| scoping.symbol_name(id) == "installer")
    })?;
    Some(lowered_normalized(&model, *site)?.0)
}

// A split frozen-property test is a closed local boolean computation. Its
// writable carrier has exactly the two branch stores and no other consumer
// besides this next guard. The native comparisons and evaluation order are
// preserved; neither arbitrary boolean aliases nor external writes are erased.
fn lowered_frozen_guard(
    model: &Model<'_, '_>,
    site: FunctionSite,
) -> Option<(Span, Span, [SymbolId; 2])> {
    let AstKind::Function(f) = model.nodes.kind(site.node_id) else {
        return None;
    };
    let ss = &f.body.as_ref()?.statements;
    for pair in ss.windows(2) {
        let [Statement::IfStatement(compute), Statement::IfStatement(guard)] = pair else {
            continue;
        };
        let Expression::BinaryExpression(test) = &compute.test else {
            continue;
        };
        if test.operator.as_str() != "==="
            || !matches!(&test.right,Expression::BooleanLiteral(b)if !b.value)
        {
            continue;
        }
        let configurable = symbol_of(model, &test.left)?;
        let [yes] = block(&compute.consequent)? else {
            return None;
        };
        let [no] = block(compute.alternate.as_ref()?)? else {
            return None;
        };
        let yes = plain_assignment(yes)?;
        let no = plain_assignment(no)?;
        let writable = assignment_identifier(model, yes)?;
        let Expression::BinaryExpression(y) = &yes.right else {
            return None;
        };
        let Expression::BinaryExpression(n) = &no.right else {
            return None;
        };
        if y.operator.as_str() != "==="
            || n.operator.as_str() != "==="
            || symbol_of(model, &y.left) != Some(writable)
            || symbol_of(model, &n.left) != Some(configurable)
            || !matches!(&y.right,Expression::BooleanLiteral(b)if !b.value)
            || !matches!(&n.right,Expression::BooleanLiteral(b)if !b.value)
            || assignment_identifier(model, no) != Some(writable)
            || configurable == writable
            || model.binding_writes(writable) != 2
        {
            return None;
        }
        let Expression::LogicalExpression(or) = &guard.test else {
            return None;
        };
        if or.operator.as_str() != "||" || symbol_of(model, &or.left) != Some(writable) {
            return None;
        }
        local_alias(model, site, configurable)?;
        local_alias(model, site, writable)?;
        if model.scoping.get_resolved_references(writable).any(|r| {
            let span = model.nodes.kind(r.node_id()).span();
            !compute.span.contains_inclusive(span) && span != or.left.span()
        }) {
            return None;
        }
        return Some((compute.span, or.left.span(), [configurable, writable]));
    }
    None
}

fn ordered_fields(e: &Expression<'_>, names: &[&str]) -> bool {
    let Expression::ObjectExpression(o) = e.without_parentheses() else {
        return false;
    };
    o.properties.len()==names.len()&&o.properties.iter().zip(names).all(|(p,name)|matches!(p,ObjectPropertyKind::ObjectProperty(p) if !p.computed&&!p.method&&p.kind==PropertyKind::Init&&property_key_name(&p.key).as_deref()==Some(*name)))
}
