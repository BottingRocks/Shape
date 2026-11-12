//! Class-only dependency proof for a lowered two-output lookup. Coalescing is
//! allowed only after proving the complete closed transport; the existing typed
//! interceptor proof must then accept the whole program with those copies
//! erased. No production rename or function-role claim uses the projection.
use super::*;

fn assignment<'a>(s: &'a Statement<'a>) -> Option<&'a AssignmentExpression<'a>> {
    match expression(s)? {
        Expression::AssignmentExpression(a) if a.operator == AssignmentOperator::Assign => Some(a),
        _ => None,
    }
}
fn writes(model: &Model<'_, '_>, s: &Statement<'_>, to: SymbolId) -> bool {
    assignment(s).is_some_and(|a| assigned(model, a) == Some(to))
}
fn null_write(model: &Model<'_, '_>, s: &Statement<'_>, to: SymbolId) -> bool {
    assignment(s).is_some_and(|a| {
        assigned(model, a) == Some(to)
            && matches!(a.right.without_parentheses(), Expression::NullLiteral(_))
    })
}
fn copy(model: &Model<'_, '_>, s: &Statement<'_>) -> Option<(SymbolId, SymbolId)> {
    let a = assignment(s)?;
    Some((assigned(model, a)?, symbol(model, &a.right)?))
}
fn stop(s: &Statement<'_>) -> bool {
    matches!(s, Statement::BreakStatement(b) if b.label.is_none())
}
fn null_pair<'a>(
    model: &Model<'_, '_>,
    s: &'a Statement<'a>,
    alias: SymbolId,
    record: SymbolId,
) -> Option<&'a Statement<'a>> {
    let [first, second, last] = block_statements(s)? else {
        return None;
    };
    (null_write(model, first, record) && null_write(model, second, alias) && stop(last))
        .then_some(second)
}
fn loop_body<'a>(s: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    let Statement::ForStatement(l) = s else {
        return None;
    };
    if l.init.is_some() || l.test.is_some() || l.update.is_some() {
        return None;
    }
    block_statements(&l.body)
}

struct Transport {
    alias: SymbolId,
    record: SymbolId,
    removed: Vec<Span>,
    record_writes: Vec<Span>,
    internal_reads: Vec<Span>,
    guard: Span,
    descriptor: bool,
}
/// The descriptor's two outputs are cleared together and assigned together on
/// every exit. No other statements fit this transport. Descriptor lookup and
/// prototype advance themselves remain obligations of the typed proof.
fn descriptor(model: &Model<'_, '_>, f: &Function<'_>) -> Option<Transport> {
    let ss = &body(f)?.statements;
    for pair in ss.windows(2) {
        let Some(
            [clear, clear_alias, Statement::IfStatement(absent), _lookup, _advance, hit, copied, exit],
        ) = loop_body(&pair[0])
        else {
            continue;
        };
        let (alias, record) = copy(model, copied)?;
        let Statement::IfStatement(guard) = &pair[1] else {
            return None;
        };
        let Expression::BinaryExpression(test) = guard.test.without_parentheses() else {
            return None;
        };
        if test.operator != oxc_syntax::operator::BinaryOperator::Equality
            || symbol(model, &test.left) != Some(record)
            || !matches!(test.right.without_parentheses(), Expression::NullLiteral(_))
            || guard.alternate.is_some()
            || !matches!(block_statements(&guard.consequent)?, [Statement::ReturnStatement(r)]
                if matches!(r.argument.as_ref()?.without_parentheses(), Expression::NullLiteral(_)))
            || absent.alternate.is_some()
            || !stop(exit)
            || !writes(model, clear, record)
            || !undefined(&assignment(clear)?.right)
            || !writes(model, clear_alias, alias)
            || !undefined(&assignment(clear_alias)?.right)
            || !writes(model, hit, record)
        {
            return None;
        }
        let absent_alias = null_pair(model, &absent.consequent, alias, record)?;
        let [absent_record, ..] = block_statements(&absent.consequent)? else {
            return None;
        };
        return Some(Transport {
            alias,
            record,
            removed: vec![clear_alias.span(), absent_alias.span(), copied.span()],
            record_writes: vec![
                assignment(clear)?.left.span(),
                assignment(absent_record)?.left.span(),
                assignment(hit)?.left.span(),
            ],
            internal_reads: vec![],
            guard: test.left.span(),
            descriptor: true,
        });
    }
    None
}
/// The cache's null bucket, exhausted scan and matching record each initialize
/// both outputs. Only a zero cursor and the exact loop may precede selection.
fn cache(model: &Model<'_, '_>, site: FunctionSite) -> Option<Transport> {
    for kind in owned(model, site) {
        let AstKind::IfStatement(branch) = kind else {
            continue;
        };
        let Some(alternate) = branch.alternate.as_ref().and_then(block_statements) else {
            continue;
        };
        let [Statement::VariableDeclaration(zero), scan] = alternate else {
            continue;
        };
        let [d] = zero.declarations.as_slice() else {
            continue;
        };
        if !matches!(d.init.as_ref().map(Expression::without_parentheses), Some(Expression::NumericLiteral(n)) if n.value == 0.0)
        {
            continue;
        }
        let Some([Statement::IfStatement(bounds), select, _mismatch, copied, exit]) =
            loop_body(scan)
        else {
            continue;
        };
        let Some((alias, record)) = copy(model, copied) else {
            continue;
        };
        if branch.alternate.is_none()
            || bounds.alternate.is_some()
            || !stop(exit)
            || !writes(model, select, record)
        {
            return None;
        }
        let [bucket_record, bucket_alias] = block_statements(&branch.consequent)? else {
            return None;
        };
        if !null_write(model, bucket_record, record) || !null_write(model, bucket_alias, alias) {
            return None;
        }
        let bounds_alias = null_pair(model, &bounds.consequent, alias, record)?;
        let [bounds_record, ..] = block_statements(&bounds.consequent)? else {
            return None;
        };
        // One read of record outside transport, the immediately following
        // nonnull guard. All field consumers refer to alias, never record.
        let Statement::IfStatement(mismatch) = _mismatch else {
            return None;
        };
        let Expression::BinaryExpression(comparison) = mismatch.test.without_parentheses() else {
            return None;
        };
        let internal = [&comparison.left, &comparison.right]
            .into_iter()
            .find_map(|e| {
                member(e)
                    .filter(|(root, key)| *key == "object" && symbol(model, root) == Some(record))
                    .map(|(root, _)| root.span())
            })?;
        let refs: Vec<_> = model
            .scoping
            .get_resolved_references(record)
            .filter(|r| {
                r.is_read()
                    && ![assignment(copied).unwrap().right.span(), internal]
                        .contains(&model.nodes.kind(r.node_id()).span())
            })
            .collect();
        let [guard] = refs.as_slice() else {
            return None;
        };
        let guard_span = model.nodes.kind(guard.node_id()).span();
        if guard_span.start <= branch.span.end {
            return None;
        }
        return Some(Transport {
            alias,
            record,
            removed: vec![bucket_alias.span(), bounds_alias.span(), copied.span()],
            record_writes: vec![
                assignment(bucket_record)?.left.span(),
                assignment(bounds_record)?.left.span(),
                assignment(select)?.left.span(),
            ],
            internal_reads: vec![internal],
            guard: guard_span,
            descriptor: false,
        });
    }
    None
}
/// The observed post-loop copy shares both null-bucket and exhausted-scan
/// results. The complete reference check below rejects captures and escapes;
/// the projected typed proof still owns bounds, cursor, key and receiver.
fn cache_after_loop(model: &Model<'_, '_>, site: FunctionSite) -> Option<Transport> {
    for kind in owned(model, site) {
        let AstKind::IfStatement(branch) = kind else {
            continue;
        };
        let Some([Statement::VariableDeclaration(zero), scan, copied]) =
            branch.alternate.as_ref().and_then(block_statements)
        else {
            continue;
        };
        let [d] = zero.declarations.as_slice() else {
            continue;
        };
        if !matches!(d.init.as_ref().map(Expression::without_parentheses), Some(Expression::NumericLiteral(n)) if n.value == 0.0)
        {
            continue;
        }
        let Some(
            [clear, Statement::IfStatement(bounds), select, Statement::IfStatement(mismatch), exit],
        ) = loop_body(scan)
        else {
            continue;
        };
        let (alias, record) = copy(model, copied)?;
        let [bucket_record, bucket_alias] = block_statements(&branch.consequent)? else {
            return None;
        };
        let [bounds_record, bounds_exit] = block_statements(&bounds.consequent)? else {
            return None;
        };
        if bounds.alternate.is_some()
            || !stop(exit)
            || !stop(bounds_exit)
            || !null_write(model, bucket_record, record)
            || !null_write(model, bucket_alias, alias)
            || !null_write(model, bounds_record, record)
            || !writes(model, clear, record)
            || !undefined(&assignment(clear)?.right)
            || !writes(model, select, record)
        {
            return None;
        }
        let Expression::BinaryExpression(comparison) = mismatch.test.without_parentheses() else {
            return None;
        };
        let internal = [&comparison.left, &comparison.right]
            .into_iter()
            .find_map(|e| {
                member(e)
                    .filter(|(root, key)| *key == "object" && symbol(model, root) == Some(record))
                    .map(|(root, _)| root.span())
            })?;
        let refs: Vec<_> = model
            .scoping
            .get_resolved_references(record)
            .filter(|r| {
                r.is_read()
                    && ![assignment(copied).unwrap().right.span(), internal]
                        .contains(&model.nodes.kind(r.node_id()).span())
            })
            .collect();
        let [guard] = refs.as_slice() else {
            return None;
        };
        let guard = model.nodes.kind(guard.node_id()).span();
        if guard.start <= branch.span.end {
            return None;
        }
        return Some(Transport {
            alias,
            record,
            // Every normal exit overwrites record before a read: bounds
            // assigns null, otherwise select assigns the bucket record. The
            // final typed proof validates those exact tests and assignments.
            removed: vec![bucket_alias.span(), clear.span(), copied.span()],
            record_writes: vec![
                assignment(bucket_record)?.left.span(),
                assignment(clear)?.left.span(),
                assignment(bounds_record)?.left.span(),
                assignment(select)?.left.span(),
            ],
            internal_reads: vec![internal],
            guard,
            descriptor: false,
        });
    }
    None
}

fn edits(model: &Model<'_, '_>, site: FunctionSite, t: &Transport) -> Option<Vec<(Span, String)>> {
    if t.alias == t.record {
        return None;
    }
    for s in [t.alias, t.record] {
        let AstKind::VariableDeclarator(d) = model.nodes.kind(model.scoping.symbol_declaration(s))
        else {
            return None;
        };
        if d.span.end >= t.record_writes.iter().map(|s| s.start).min()?
            || d.init.is_some()
            || !model.scoping.symbol_redeclarations(s).is_empty()
            || model
                .owning_site(model.scoping.symbol_declaration(s))
                .map(|o| o.span)
                != Some(site.span)
        {
            return None;
        }
    }
    let name = model.scoping.symbol_name(t.record).to_string();
    // Prevent introducing capture in a nested lexical block. Both symbols are
    // own locals, and no other binding with the destination spelling occurs in
    // this callable (nested callables are excluded separately below).
    for n in model.nodes.iter() {
        if let AstKind::BindingIdentifier(id) = n.kind() {
            if site.span.contains_inclusive(id.span)
                && id.name.as_str() == name
                && id.symbol_id.get() != Some(t.record)
            {
                return None;
            }
        }
    }
    let mut result: Vec<_> = t.removed.iter().map(|s| (*s, String::new())).collect();
    let removed = |s: Span| t.removed.iter().any(|r| r.contains_inclusive(s));
    let mut guard_reads = 0;
    for r in model.scoping.get_resolved_references(t.record) {
        let span = model.nodes.kind(r.node_id()).span();
        if removed(span) {
            continue;
        }
        if r.is_write() {
            if r.is_read() || !t.record_writes.contains(&span) {
                return None;
            }
        } else if r.is_read() && t.internal_reads.contains(&span) {
            continue;
        } else if r.is_read() && span == t.guard {
            guard_reads += 1;
        } else {
            return None;
        }
    }
    if guard_reads != 1 {
        return None;
    }
    let mut rebound = 0;
    let mut value_writes = 0;
    for r in model.scoping.get_resolved_references(t.alias) {
        let span = model.nodes.kind(r.node_id()).span();
        if removed(span) {
            continue;
        }
        if model.owning_site(r.node_id()).map(|o| o.span) != Some(site.span)
            || span.start <= t.guard.end
        {
            return None;
        }
        if r.is_write() {
            let AstKind::AssignmentExpression(a) = model.nodes.parent_kind(r.node_id()) else {
                return None;
            };
            if !t.descriptor
                || r.is_read()
                || assigned(model, a) != Some(t.alias)
                || !member(&a.right).is_some_and(|(root, key)| {
                    key == "desc" && symbol(model, root) == Some(t.alias)
                })
            {
                return None;
            }
            rebound += 1;
        }
        if r.is_read() {
            match model.nodes.parent_kind(r.node_id()) {
                AstKind::StaticMemberExpression(m) if !m.optional => {
                    let allowed = if t.descriptor {
                        ["containingObj", "desc", "configurable", "writable", "value"]
                            .contains(&m.property.name.as_str())
                    } else {
                        ["result", "original"].contains(&m.property.name.as_str())
                    };
                    if !allowed {
                        return None;
                    }
                    let parent = model.nodes.parent_id(r.node_id());
                    match model.nodes.parent_kind(parent) {
                        AstKind::AssignmentExpression(a) if a.left.span() == m.span => {
                            let wrapper = model.function_from_expression(&a.right)?;
                            if !t.descriptor
                                || m.property.name != "value"
                                || a.operator != AssignmentOperator::Assign
                                || model.owning_site(wrapper.node_id).map(|s| s.span)
                                    != Some(site.span)
                            {
                                return None;
                            }
                            value_writes += 1;
                        }
                        AstKind::UpdateExpression(_) | AstKind::UnaryExpression(_) => return None,
                        _ => {}
                    }
                }
                AstKind::CallExpression(c) if t.descriptor && !c.optional => {
                    let native = |e: &Expression<'_>, key: &str| {
                        member(e).is_some_and(|(root, k)| k == key
                            && matches!(root.without_parentheses(), Expression::Identifier(id)
                                if id.name == "Object" && crate::analysis::identifier_symbol(id, model.scoping).is_none()))
                    };
                    let args = call_arguments(c);
                    if args.len() != c.arguments.len()
                        || args.len() != 3
                        || !(native(&c.callee, "defineProperty") && args[2].span() == span
                            || native(args[0], "hasOwnProperty")
                                && args[1].span() == span
                                && matches!(args[2].without_parentheses(), Expression::StringLiteral(k) if k.value == "value"))
                    {
                        return None;
                    }
                }
                _ => return None,
            }
        }
        result.push((span, name.clone()));
    }
    if rebound != usize::from(t.descriptor) || value_writes != usize::from(t.descriptor) {
        return None;
    }
    Some(result)
}

pub(super) fn proves(model: &Model<'_, '_>, site: FunctionSite, f: &Function<'_>) -> Option<()> {
    // Either lookup may already have the canonical single-output form. Every
    // projected transport is proved independently; the final typed proof must
    // accept both complete protocols together.
    let descriptor = descriptor(model, f);
    let cache = cache(model, site).or_else(|| cache_after_loop(model, site));
    if descriptor.is_none() && cache.is_none() {
        return None;
    }
    let mut changes = Vec::new();
    for transport in [descriptor, cache].into_iter().flatten() {
        changes.extend(edits(model, site, &transport)?);
    }
    changes.sort_by_key(|(s, _)| s.start);
    if changes.windows(2).any(|w| w[0].0.end > w[1].0.start) {
        return None;
    }
    let mut projected = model.source.to_string();
    for (span, replacement) in changes.into_iter().rev() {
        projected.replace_range(span.start as usize..span.end as usize, &replacement);
    }
    let allocator = oxc_allocator::Allocator::default();
    let program = crate::analysis::parse_program(&allocator, &projected).ok()?;
    let (scoping, nodes) = crate::analysis::analyze(&program);
    let projected_model = Model::new(
        &projected,
        &program,
        &scoping,
        &nodes,
        crate::signals::schema::ProgramPropertiesLimits::default(),
    )
    .ok()?;
    let projected_site = *projected_model
        .all_sites()
        .iter()
        .find(|s| s.span.start == site.span.start)?;
    let AstKind::Function(function) = projected_model.nodes.kind(projected_site.node_id) else {
        return None;
    };
    crate::naming::proves_method_interceptor(
        &projected_model,
        projected_site,
        function,
        body(function)?,
    )
    .then_some(())
}
