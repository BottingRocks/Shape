//! Direct-body callable copies may precede captured setup. This adapter follows
//! every read of their single-origin aliases, and separately proves that each
//! alias store executed before a read. Constructor OBJECT copies still use the
//! stricter read-at-copy check in `closed_constructor`; no shared resolver changes.
use super::*;
use oxc_semantic::NodeId;

fn ready(
    m: &Model<'_, '_>,
    node: NodeId,
    owner: NodeId,
    end: u32,
    active: &mut BTreeSet<Sym>,
    depth: usize,
    follow_copies: bool,
) -> bool {
    if depth == 0 {
        return false;
    }
    let parent = m.nodes.parent_id(node);
    if follow_copies {
        let copy = match m.nodes.kind(parent) {
            AstKind::VariableDeclarator(v)
                if v.init
                    .as_ref()
                    .is_some_and(|e| e.span() == m.nodes.kind(node).span()) =>
            {
                bind(&v.id)
            }
            AstKind::AssignmentExpression(a)
                if a.operator.as_str() == "=" && a.right.span() == m.nodes.kind(node).span() =>
            {
                match &a.left {
                    AssignmentTarget::AssignmentTargetIdentifier(i) => {
                        crate::analysis::identifier_symbol(i, m.scoping)
                    }
                    _ => None,
                }
            }
            _ => None,
        };
        if let Some(s) = copy {
            let Some((rhs, copy_owner, copy_end)) = observables::observable_map_operator::origin(m, s)
            else {
                return false;
            };
            if copy_owner != owner || rhs.span() != m.nodes.kind(node).span() || !active.insert(s) {
                return false;
            }
            let okay = m
                .scoping
                .get_resolved_references(s)
                .filter(|r| r.is_read())
                .all(|r| {
                    // An alias is a runtime value: do not follow another copy to
                    // repair a read made before this alias's executed store.
                    ready(
                        m,
                        r.node_id(),
                        owner,
                        copy_end,
                        &mut BTreeSet::new(),
                        depth - 1,
                        false,
                    ) && ready(m, r.node_id(), owner, end, active, depth - 1, true)
                });
            active.remove(&s);
            return okay;
        }
    }
    let mut child = node;
    for _ in 0..256 {
        let parent = m.nodes.parent_id(child);
        match m.nodes.kind(child) {
            AstKind::Function(f) if f.r#async || f.generator => return false,
            AstKind::ArrowFunctionExpression(a) if a.r#async => return false,
            AstKind::Class(_) => return false,
            AstKind::Function(f) if parent == owner => {
                let Some(s) = fs(f).filter(|_| f.r#type == FunctionType::FunctionDeclaration)
                else {
                    return false;
                };
                if !fixed(m, s) || !active.insert(s) {
                    return false;
                }
                let okay = m
                    .scoping
                    .get_resolved_references(s)
                    .all(|r| ready(m, r.node_id(), owner, end, active, depth - 1, true));
                active.remove(&s);
                return okay;
            }
            _ => (),
        }
        if parent == owner {
            // Origins come exclusively from the existing direct-body origin
            // proof. Therefore normal completion of this later statement has
            // executed setup; no branch/loop/deferred origin is admitted.
            return m.nodes.kind(child).span().start >= end;
        }
        if parent == child {
            return false;
        }
        child = parent;
    }
    false
}

pub(super) fn publications(
    m: &Model<'_, '_>,
    factory: Sym,
    owner: NodeId,
    end: u32,
    native: Option<Span>,
) -> bool {
    fixed(m, factory)
        && m.scoping.get_resolved_references(factory).all(|r| {
            native.is_some_and(|span| span.contains_inclusive(m.nodes.kind(r.node_id()).span()))
                || ready(
                    m,
                    r.node_id(),
                    owner,
                    end,
                    &mut BTreeSet::from([factory]),
                    32,
                    true,
                )
        })
}

/// Reuse the existing unique direct-body origin and complete own class proof.
/// Only the callable publication walk differs: it also consumes assignment
/// aliases. No additional writer, conditional origin or partial factory fits.
pub(super) fn constructor(
    m: &Model<'_, '_>,
    callee: &Expression<'_>,
    factory: Sym,
    depth: usize,
) -> Option<()> {
    constructor_with_origin(m, callee, factory, depth, false)
}

pub(super) fn ordinary_constructor(
    m: &Model<'_, '_>, callee: &Expression<'_>, factory: Sym, depth: usize,
) -> Option<()> {
    constructor_with_origin(m, callee, factory, depth, true)
}

fn constructor_with_origin(
    m: &Model<'_, '_>, callee: &Expression<'_>, factory: Sym, depth: usize, ordinary: bool,
) -> Option<()> {
    if depth == 0 {
        return None;
    }
    let (value, owner, end) = observables::observable_map_operator::origin(m, id(m, callee)?)?;
    if let Some(producer) = id(m, value) {
        let (_, producer_owner, producer_end) =
            observables::observable_map_operator::origin(m, producer)?;
        if producer_owner != owner
            || !callable_origin_precedes(
                m,
                owner,
                producer_end,
                m.scoping
                    .get_reference(match value.without_parentheses() {
                        Expression::Identifier(i) => i.reference_id.get()?,
                        _ => return None,
                    })
                    .node_id(),
                None,
                0,
                false,
            )
        {
            return None;
        }
        constructor_with_origin(m, value, factory, depth - 1, ordinary)?;
        return publications(m, factory, owner, end, None).then_some(());
    }
    let c = call(value)?;
    if !c.arguments.is_empty() {
        return None;
    }
    let maker = stable_function(m, &c.callee, 0)?;
    if !matches!(m.nodes.kind(maker.node_id),AstKind::Function(f) if f.r#type==FunctionType::FunctionDeclaration)
        || !matches!(
            m.nodes.parent_kind(maker.node_id),
            AstKind::FunctionBody(_) | AstKind::Program(_)
        )
    {
        return None;
    }
    if ordinary {
        observables::observable_factory::proven_ordinary_constructor(m, maker)?;
    } else {
        observables::observable_factory::proven_native_class(m, maker)?;
    }
    let refs: Vec<_> = m
        .scoping
        .get_resolved_references(maker.binding_symbol?)
        .collect();
    if refs.len() != 1 || m.nodes.kind(refs[0].node_id()).span() != c.callee.span() {
        return None;
    }
    publications(m, factory, owner, end, Some(maker.span)).then_some(())
}
