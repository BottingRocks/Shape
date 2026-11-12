//! Local, monotone constructor epochs. Every writer is enumerated by SymbolId;
//! native origin validity and executed publication readiness are separate proofs.
use super::*;

pub(super) struct Epochs {
    pub(super) owner: NodeId,
    pub(super) stores: BTreeSet<(u32, u32)>,
    pub(super) makers: Vec<FunctionSite>,
}

fn looping(kind: AstKind<'_>) -> bool {
    matches!(
        kind,
        AstKind::ForStatement(_)
            | AstKind::ForInStatement(_)
            | AstKind::ForOfStatement(_)
            | AstKind::WhileStatement(_)
            | AstKind::DoWhileStatement(_)
    )
}

/// Only ordinary same-activation assignment statements qualify. A deferred
/// setter, loop, conditional expression or compound assignment cannot establish
/// an epoch. The RHS is not presumed executed merely because it exists.
pub(super) fn collect(m: &Model<'_, '_>, constructor: Sym) -> Option<Epochs> {
    if !m.scoping.symbol_redeclarations(constructor).is_empty() || m.binding_writes(constructor) < 2
    {
        return None;
    }
    let d = m.scoping.symbol_declaration(constructor);
    let AstKind::VariableDeclarator(v) = m.nodes.kind(d) else {
        return None;
    };
    let declaration = m.nodes.parent_id(d);
    let owner = m.nodes.parent_id(declaration);
    if v.init.is_some()
        || !matches!(m.nodes.kind(declaration), AstKind::VariableDeclaration(_))
        || !matches!(m.nodes.kind(owner), AstKind::FunctionBody(_))
    {
        return None;
    }
    if m.nodes.iter().any(|n| matches!(n.kind(), AstKind::Program(p) if crate::naming::program::has_dynamic_scope(p))) {
        return None;
    }
    // Keep constructor objects local and opaque. Arbitrary publication,
    // aliasing, API replacement or reflective use could mutate a class after
    // its body was proved. Only construction, direct native static calls and
    // non-exposing type/identity tests are supported by this new path.
    for r in m
        .scoping
        .get_resolved_references(constructor)
        .filter(|r| r.is_read())
    {
        let node = r.node_id();
        let span = m.nodes.kind(node).span();
        let parent = m.nodes.parent_id(node);
        let closed = match m.nodes.kind(parent) {
            AstKind::NewExpression(n) => n.callee.span() == span,
            AstKind::StaticMemberExpression(p)
                if !p.optional
                    && p.object.span() == span
                    && matches!(p.property.name.as_str(), "from" | "of" | "empty") =>
            {
                matches!(m.nodes.kind(m.nodes.parent_id(parent)), AstKind::CallExpression(c)
                    if !c.optional && c.callee.span() == p.span)
            }
            AstKind::BinaryExpression(b) => {
                b.operator.as_str() == "instanceof" && b.right.span() == span
            }
            AstKind::UnaryExpression(u) => u.operator.as_str() == "typeof",
            _ => false,
        };
        if !closed {
            return None;
        }
    }
    let mut stores = BTreeSet::new();
    let mut makers = Vec::new();
    for r in m
        .scoping
        .get_resolved_references(constructor)
        .filter(|r| r.is_write())
    {
        let assignment = m.nodes.parent_id(r.node_id());
        let AstKind::AssignmentExpression(a) = m.nodes.kind(assignment) else {
            return None;
        };
        let AssignmentTarget::AssignmentTargetIdentifier(i) = &a.left else {
            return None;
        };
        let statement = m.nodes.parent_id(assignment);
        if a.operator != AssignmentOperator::Assign
            || crate::analysis::identifier_symbol(i, m.scoping) != Some(constructor)
            || !matches!(m.nodes.kind(statement), AstKind::ExpressionStatement(_))
        {
            return None;
        }
        let mut node = statement;
        loop {
            if node == owner {
                break;
            }
            let kind = m.nodes.kind(node);
            if looping(kind)
                || matches!(
                    kind,
                    AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) | AstKind::Class(_)
                )
            {
                return None;
            }
            let parent = m.nodes.parent_id(node);
            if parent == node {
                return None;
            }
            node = parent;
        }
        let c = call(&a.right)?;
        if !c.arguments.is_empty() {
            return None;
        }
        let maker = stable_function(m, &c.callee, 0)?;
        let ms = maker.binding_symbol?;
        // An additional exposure could invoke a maker against an uninitialized
        // constructor through its captured installed methods. Never ignore it.
        let refs: Vec<_> = m.scoping.get_resolved_references(ms).collect();
        if refs.len() != 1 || m.nodes.kind(refs[0].node_id()).span() != c.callee.span() {
            return None;
        }
        stores.insert((a.span.start, a.span.end));
        makers.push(maker);
    }
    (stores.len() == m.binding_writes(constructor) && !stores.is_empty()).then_some(Epochs {
        owner,
        stores,
        makers,
    })
}

fn initializes(s: &Statement<'_>, barriers: &BTreeSet<(u32, u32)>, depth: usize) -> bool {
    if depth == 0 {
        return false;
    }
    match s {
        Statement::ExpressionStatement(e) => {
            barriers.contains(&(e.expression.span().start, e.expression.span().end))
        }
        Statement::VariableDeclaration(v) => v
            .declarations
            .iter()
            .any(|d| barriers.contains(&(d.span.start, d.span.end))),
        Statement::BlockStatement(b) => b.body.iter().any(|s| initializes(s, barriers, depth - 1)),
        Statement::IfStatement(i) => {
            initializes(&i.consequent, barriers, depth - 1)
                && i.alternate
                    .as_ref()
                    .is_some_and(|s| initializes(s, barriers, depth - 1))
        }
        // Try/catch completion, switch fallthrough and loops are deliberately
        // not summarized. A store *inside* their currently executed block can
        // still dominate a later point in that same block.
        _ => false,
    }
}

fn siblings_ready(ss: &[Statement<'_>], child: Span, barriers: &BTreeSet<(u32, u32)>) -> bool {
    ss.iter()
        .take_while(|s| s.span().end <= child.start)
        .any(|s| initializes(s, barriers, 32))
}

/// Walk lexical callable/alias edges before testing dominance. Anonymous
/// callbacks are treated as published at creation; declared closures are traced
/// through ALL references, including escapes. Cycles and loop routes refuse.
fn ready_use(
    m: &Model<'_, '_>,
    node: NodeId,
    owner: NodeId,
    barriers: &BTreeSet<(u32, u32)>,
    deferred_bodies: &[Span],
    active: &mut BTreeSet<Sym>,
    depth: usize,
) -> bool {
    if depth == 0 {
        return false;
    }
    // Only fully proved API function bodies qualify; eager class-definition
    // syntax and formal parameter metadata are outside every deferred span.
    // All maker calls and constructor object reads stay closed by collect/prove.
    if deferred_bodies.iter().any(|span| span.contains_inclusive(m.nodes.kind(node).span())) {
        return true;
    }
    // Reading a hoisted function to initialize a plain immutable alias need
    // not invoke it. Follow every alias use, and independently require that
    // the alias initializer dominates those reads/publications.
    let parent = m.nodes.parent_id(node);
    if let AstKind::VariableDeclarator(v) = m.nodes.kind(parent) {
        if v.init
            .as_ref()
            .is_some_and(|e| e.span() == m.nodes.kind(node).span())
        {
            if let Some(s) = binding(&v.id) {
                if fixed(m, s) && active.insert(s) {
                    let barrier = BTreeSet::from([(v.span.start, v.span.end)]);
                    let okay = m.scoping.get_resolved_references(s).all(|r| {
                        ready_use(
                            m,
                            r.node_id(),
                            owner,
                            &barrier,
                            deferred_bodies,
                            &mut BTreeSet::new(),
                            depth - 1,
                        ) && ready_use(m, r.node_id(), owner, barriers, deferred_bodies, active, depth - 1)
                    });
                    active.remove(&s);
                    return okay;
                }
                return false;
            }
        }
    }
    let mut child = node;
    // Record the innermost executed-block proof, but do not accept it until
    // callable boundaries have been examined. A closure's declaration offset
    // cannot make its captured reads ready.
    let mut dominated = false;
    for _ in 0..256 {
        if child == owner {
            return dominated;
        }
        let parent = m.nodes.parent_id(child);
        let kind = m.nodes.kind(child);
        if looping(kind) || matches!(kind, AstKind::Class(_)) {
            return false;
        }
        if let AstKind::Function(f) = kind {
            if f.r#async || f.generator {
                return false;
            }
            if let Some(s) = symbol(f).filter(|_| f.r#type == FunctionType::FunctionDeclaration) {
                if !fixed(m, s) || !active.insert(s) {
                    return false;
                }
                let okay = m
                    .scoping
                    .get_resolved_references(s)
                    .all(|r| ready_use(m, r.node_id(), owner, barriers, deferred_bodies, active, depth - 1));
                active.remove(&s);
                return okay;
            }
            // A function expression may run as soon as its value is created.
            // This conservative route includes IIFEs and escaped callbacks.
        }
        // No async suspension semantics are inferred for arrows either.
        if matches!(kind, AstKind::ArrowFunctionExpression(a) if a.r#async) {
            return false;
        }
        let child_span = m.nodes.kind(child).span();
        dominated |= match m.nodes.kind(parent) {
            AstKind::BlockStatement(b) => siblings_ready(&b.body, child_span, barriers),
            AstKind::FunctionBody(b) => siblings_ready(&b.statements, child_span, barriers),
            AstKind::Program(p) => siblings_ready(&p.body, child_span, barriers),
            _ => false,
        };
        if parent == child {
            return false;
        }
        child = parent;
    }
    false
}

pub(super) fn publications_ready(m: &Model<'_, '_>, factory: Sym, epochs: &Epochs) -> bool {
    fixed(m, factory)
        && m.scoping.get_resolved_references(factory).all(|r| {
            ready_use(
                m,
                r.node_id(),
                epochs.owner,
                &epochs.stores,
                &[],
                &mut BTreeSet::from([factory]),
                32,
            )
        })
}

pub(super) fn prove(m: &Model<'_, '_>, callee: &Expression<'_>, factory: Sym) -> Option<()> {
    let epochs = collect(m, id(m, callee)?)?;
    // Never confuse a well-ordered store with a valid native class. Every
    // reachable definition must independently satisfy the full class contract.
    for maker in &epochs.makers {
        observable_factory::proven_native_class(m, *maker)?;
    }
    publications_ready(m, factory, &epochs).then_some(())
}

/// Operator-specific extension: a complete own native class may capture this
/// operator in its deferred methods. Reuse the same epoch enumeration and
/// readiness walk; only fully proved, single-call makers supply deferred spans.
/// The legacy join entry keeps an empty deferred set and its exact semantics.
pub(super) fn prove_operator(m: &Model<'_, '_>, callee: &Expression<'_>, factory: Sym) -> Option<()> {
    let epochs = collect(m, id(m, callee)?)?;
    let mut deferred_bodies = Vec::new();
    for maker in &epochs.makers {
        deferred_bodies.extend(observable_factory::proven_deferred_api_bodies(m, *maker)?);
    }
    // Deferred methods may execute immediately after a maker's result store.
    // A callable value copied into a capture must therefore already exist at
    // every maker call; a hoisted declaration alone cannot repair a late copy.
    // Keep this additive path bounded to direct immutable aliases of factory.
    // Alias chains remain conservatively refused rather than treating their
    // RHS reads as hoisted declarations.
    for r in m.scoping.get_resolved_references(factory) {
        let node = r.node_id();
        if let AstKind::VariableDeclarator(v) = m.nodes.parent_kind(node) {
            if v.init.as_ref().is_some_and(|e| e.span() == m.nodes.kind(node).span()) {
                let alias = binding(&v.id)?;
                let declaration = m.nodes.parent_id(m.nodes.parent_id(node));
                if !fixed(m, alias) || m.nodes.parent_id(declaration) != epochs.owner {
                    return None;
                }
                if m.scoping.get_resolved_references(alias).any(|r| {
                    matches!(m.nodes.parent_kind(r.node_id()), AstKind::VariableDeclarator(_) | AstKind::AssignmentExpression(_))
                }) { return None; }
                let barrier = BTreeSet::from([(v.span.start, v.span.end)]);
                for maker in &epochs.makers {
                    for call in m.scoping.get_resolved_references(maker.binding_symbol?) {
                        if !ready_use(m, call.node_id(), epochs.owner, &barrier, &[], &mut BTreeSet::new(), 32) {
                            return None;
                        }
                    }
                }
            }
        }
    }
    (fixed(m, factory) && m.scoping.get_resolved_references(factory).all(|r| {
        ready_use(m, r.node_id(), epochs.owner, &epochs.stores, &deferred_bodies,
            &mut BTreeSet::from([factory]), 32)
    })).then_some(())
}

#[cfg(test)]
mod tests {
    use super::super::joined_stream_state_and_cleanup::{inspect, ACTUAL, CONTROL};
    use super::*;
    const MAKER: &str = include_str!(
        "../../../../../tests/fixtures/joined_stream_state_and_cleanup/native-control-maker.js"
    );
    const INIT: &str = "$KEY_38 = $THREAD_1_28126();";
    const JOIN: &str = "$THREAD_5_131738";

    fn multi(tail: &str) -> String {
        assert_eq!(CONTROL.matches(INIT).count(), 1);
        let declarations = format!(
            "{}\n{}\n",
            MAKER.replacen("$THREAD_1_28126", "$THREAD_8_900001", 1),
            MAKER.replacen("$THREAD_1_28126", "$THREAD_8_900002", 1)
        );
        CONTROL.replacen(INIT, &format!("{declarations}{tail}"), 1)
    }
    const EPOCHS: &str = "if (epochBranch) { $KEY_38 = $THREAD_1_28126(); $THREAD_5_131738([]); } else { $KEY_38 = $THREAD_8_900001(); $THREAD_5_131738([]); } $KEY_38 = $THREAD_8_900002(); $THREAD_5_131738([]);";

    fn check(source: &str, expected: bool) {
        inspect(source, JOIN, |m, f| {
            let [_, r] = body(f).unwrap() else {
                panic!("join body")
            };
            let Expression::NewExpression(n) = ret(r).unwrap() else {
                panic!("new")
            };
            assert!(
                single_native_origin(m, &n.callee, symbol(f).unwrap(), 8).is_none(),
                "positive must exercise new proof"
            );
            assert_eq!(
                epochs::prove(m, &n.callee, symbol(f).unwrap()).is_some(),
                expected,
                "origin contract"
            );
            assert_eq!(prove_join(m, f).is_some(), expected, "full parent");
        });
    }

    #[test]
    fn joined_stream_state_and_cleanup_actual_epoch_component() {
        inspect(ACTUAL, "$THREAD_3_64859", |m, f| {
            let [_, r] = body(f).unwrap() else {
                panic!("join body")
            };
            let Expression::NewExpression(n) = ret(r).unwrap() else {
                panic!("new")
            };
            let epochs =
                collect(m, id(m, &n.callee).unwrap()).expect("all actual store epochs collected");
            assert_eq!(epochs.stores.len(), 3);
            let ready = publications_ready(m, symbol(f).unwrap(), &epochs);
            assert!(ready, "all actual join publications must be ready");
            for maker in &epochs.makers {
                assert!(
                    observable_factory::proven_native_class(m, *maker).is_some(),
                    "approved actual source class must pass"
                );
            }
            let makers: Vec<_> = epochs.makers.iter().map(|site| serde_json::json!({"start":site.span.start,"end":site.span.end,"native_class_proved":observable_factory::proven_native_class(m,*site).is_some()})).collect();
            println!(
                "{}",
                serde_json::json!({"epoch_collection":true,"publication_readiness":ready,"factories":makers})
            );
        });
    }

    #[test]
    fn joined_stream_state_and_cleanup_multi_epoch_and_alpha_and_branch_permutation() {
        let s = multi(EPOCHS);
        check(&s, true);
        check(
            &s.replace("$KEY_38", "constructorEpoch")
                .replace("epochBranch", "alternateBranch"),
            true,
        );
        check(&multi("if (!epochBranch) { $KEY_38 = $THREAD_8_900001(); } else { $KEY_38 = $THREAD_1_28126(); } $THREAD_5_131738([]); $KEY_38 = $THREAD_8_900002(); $THREAD_5_131738([]);"),true);
        check(
            &multi(&EPOCHS.replace(
                "$KEY_38 = $THREAD_8_900002();",
                "publish($THREAD_5_131738); $KEY_38 = $THREAD_8_900002();",
            )),
            true,
        );
        let result = crate::naming::name_threads(&s, true).unwrap();
        let parent = result
            .report
            .outcomes
            .iter()
            .find(|o| o.binding.name == JOIN)
            .unwrap();
        assert!(parent.applied, "{parent:?}");
        assert_eq!(
            parent.role_name.as_deref(),
            Some("joinObservableLastValues")
        );
        assert_eq!(
            parent.evaluator.as_deref(),
            Some("shared_observable_fork_join")
        );
        assert!(parent.explanation.iter().any(
            |s| s.contains("Whole join protocol: native constructor origin and exposure readiness")
        ));
        // Established single-origin controls keep their exact role/evaluator.
        let old = crate::naming::name_threads(CONTROL, true).unwrap();
        let old_parent = old
            .report
            .outcomes
            .iter()
            .find(|o| o.binding.name == JOIN)
            .unwrap();
        assert_eq!(parent.role_name, old_parent.role_name);
        assert_eq!(parent.evaluator, old_parent.evaluator);
        assert_eq!(parent.explanation, old_parent.explanation);
    }

    #[test]
    fn joined_stream_state_and_cleanup_epoch_refusals() {
        for tail in [
            // Unsupported additional definition and uninitialized route.
            EPOCHS.replace("$KEY_38 = $THREAD_8_900002();", "$KEY_38 = other;"),
            EPOCHS.replace("$KEY_38 = $THREAD_8_900001();", ""),
            EPOCHS.replace("if (epochBranch)", "$THREAD_5_131738([]); if (epochBranch)"),
            EPOCHS.replace(
                "$KEY_38 = $THREAD_8_900002();",
                "$KEY_38 = $THREAD_8_900002(); publish($THREAD_5_131738); $KEY_38 = undefined;",
            ),
            EPOCHS.replace(
                "if (epochBranch)",
                "publish($THREAD_5_131738); if (epochBranch)",
            ),
            EPOCHS.replace(
                "if (epochBranch)",
                "let exposed = $THREAD_5_131738; publish(exposed); if (epochBranch)",
            ),
            EPOCHS.replace(
                "if (epochBranch)",
                "let alias; publish(alias); alias = $THREAD_5_131738; if (epochBranch)",
            ),
            EPOCHS.replace(
                "$KEY_38 = $THREAD_8_900002();",
                "for (;;) { $KEY_38 = $THREAD_8_900002(); break; }",
            ),
            EPOCHS.replace(
                "$KEY_38 = $THREAD_8_900002();",
                "function deferred() { $KEY_38 = $THREAD_8_900002(); } deferred();",
            ),
            EPOCHS.replace(
                "$KEY_38 = $THREAD_8_900002();",
                "$THREAD_8_900002(); $KEY_38 = $THREAD_8_900002();",
            ),
        ] {
            check(&multi(&tail), false);
        }
        check(
            &multi(EPOCHS).replace(
                "$KEY_38 = $THREAD_8_900002();",
                "function badMaker() { return {}; } $KEY_38 = badMaker();",
            ),
            false,
        );
        for replacement in [
            "publish($KEY_38); $KEY_38 = $THREAD_8_900002();",
            "const escapedConstructor = $KEY_38; publish(escapedConstructor); $KEY_38 = $THREAD_8_900002();",
            "$KEY_38.prototype.subscribe = other; $KEY_38 = $THREAD_8_900002();",
            "$KEY_38.from = other; $KEY_38 = $THREAD_8_900002();",
        ] {
            check(&multi(&EPOCHS.replace("$KEY_38 = $THREAD_8_900002();", replacement)), false);
        }
        // Native body validity remains mandatory even for a well-ordered store.
        let reentrant = multi(EPOCHS).replacen(
            "function $THREAD_8_900001() {",
            "function $THREAD_8_900001() { $THREAD_5_131738([]);",
            1,
        );
        check(&reentrant, false);
        let aliased = multi(&format!("const earlyConstructor = $KEY_38; {EPOCHS}"));
        let start = aliased.find("function $THREAD_5_131738(").unwrap();
        let offset = start + aliased[start..].find("return new $KEY_38(").unwrap();
        let mut early_alias = aliased.clone();
        early_alias.replace_range(
            offset..offset + "return new $KEY_38(".len(),
            "return new earlyConstructor(",
        );
        // This alias is deliberately initialized before its origin. Its whole
        // parent must refuse (the callee has a distinct immutable SymbolId).
        inspect(&early_alias, JOIN, |m, f| {
            assert!(prove_join(m, f).is_none())
        });
    }

    #[test]
    fn joined_stream_state_and_cleanup_multi_epoch_operational_refusals() {
        for (from, to) in [
            ("$KEY_3[$KEY_7] = $AN;", "$KEY_3[$AN] = $KEY_7;"),
            ("--$KEY_4;", "++$KEY_4;"),
            (
                "$KEY_1.next($KEY_3);",
                "$KEY_1.complete(); $KEY_1.next($KEY_3);",
            ),
            ("$AO => $AO.unsubscribe()", "$AO => void 0"),
            ("$KEY_1.error($AN);", "$KEY_1.error($AN); extra();"),
        ] {
            let s = multi(EPOCHS);
            let start = s.find("function $THREAD_5_131738(").unwrap();
            let end = start
                + s[start..].find("return $THREAD_6_109885;").unwrap()
                + "return $THREAD_6_109885;".len();
            assert!(s[start..end].contains(from));
            let changed = format!(
                "{}{}{}",
                &s[..start],
                s[start..end].replace(from, to),
                &s[end..]
            );
            inspect(&changed, JOIN, |m, f| assert!(prove_join(m, f).is_none()));
        }
    }
}
