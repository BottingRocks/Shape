//! Small compositional rules. Callee identities come from scoped, single-origin
//! bindings and independent claims, never generated spellings or reference labels.
use super::*;
use oxc_span::GetSpan;

pub(super) fn native_class_claims(
    model: &Model<'_, '_>,
    finder_claims: &[crate::signals::analysis::roles::FinderClaim],
) -> Vec<crate::signals::analysis::roles::FinderClaim> {
    let mut claims: Vec<_> = model
        .all_sites()
        .iter()
        .filter_map(|site| {
            let (class_span, _) = crate::naming::functions::observables::observable_factory::proven_native_class(model, *site)?;
            Some(crate::signals::analysis::roles::FinderClaim::ClassBinding {
                role: "owned_observable_class",
                class_span,
                confidence: Confidence::High,
                evidence: vec![
                    "complete owned Observable API; direct class declaration returned through an immutable single-origin alias".into(),
                ],
            })
        })
        .collect();
    claims.extend(crate::naming::classes::subscriptions::claims(model));
    claims.extend(crate::naming::classes::ring_buffer::claims(model));
    claims.extend(crate::naming::classes::lazy_registry::claims(model));
    claims.extend(
        crate::naming::classes::batch_scheduler::claims_with_finder_claims(model, finder_claims),
    );
    claims.extend(crate::naming::classes::callback_notifier::claims(model));
    claims.extend(
        crate::naming::classes::encoder::prove(model, finder_claims)
            .into_iter()
            .map(|proof| {
                debug_assert_eq!(proof.canonical_name, "ValueEncoder");
                crate::signals::analysis::roles::FinderClaim::ClassBinding {
                    role: "owned_value_encoder_class",
                    class_span: proof.class_span,
                    confidence: Confidence::High,
                    evidence: proof.evidence,
                }
            }),
    );
    claims
}

/// A wrapper claim that proves the captured value is an exact immutable
/// `Object.create(null)` sentinel.
pub(super) fn is_sentinel_specialization(p: &Proposal) -> bool {
    p.evaluator == "shared_immutable_wrapper"
        && matches!(
            p.name.as_str(),
            "getNullPrototypeSentinel" | "isNullPrototypeSentinel"
        )
}

/// The generic state read/predicate for any plain captured variable.
pub(super) fn is_generic_captured_state(p: &Proposal) -> bool {
    p.evaluator == "shared_state_slots"
        && matches!(p.name.as_str(), "readCapturedState" | "isCapturedState")
}

/// A sentinel proof is strictly stronger than the generic captured-state
/// read/predicate over the same function, so only that exact pair is collapsed.
/// A solo generic claim, the mutable `setCapturedStateTriple` storage role, a
/// third simultaneous claim and every other family are untouched. No order,
/// count, confidence, vendor, span, hash or first-wins tie-break participates.
pub(super) fn resolve_sentinel_specificity(mut claims: Vec<Proposal>) -> Vec<Proposal> {
    if !claims.iter().any(is_sentinel_specialization) {
        return claims;
    }
    claims.retain(|p| !is_generic_captured_state(p));
    for p in claims.iter_mut() {
        if is_sentinel_specialization(p) {
            p.explanation.push(
                "the same function also carries an exact immutable null-prototype sentinel proof; that specialization supersedes the generic captured-state read/predicate, while generic-only reads and mutable storage roles are untouched"
                    .into(),
            );
        }
    }
    claims
}

// Prove timing at the reference, rather than comparing an initializer with a
// nested function's declaration span. A hoisted declaration or stable variable-
// bound function expression may precede setup, provided every reference that
// can expose/invoke it is itself after setup.
// This deliberately handles only direct body origins, not general dominance.
pub(super) fn callable_origin_precedes(
    model: &Model<'_, '_>,
    body: NodeId,
    origin_end: u32,
    use_node: NodeId,
    origin_reference: Option<NodeId>,
    depth: usize,
    follow_value_captures: bool,
) -> bool {
    if depth > 16 {
        return false;
    }
    // Taking the function value to establish this very alias is safe; it
    // does not invoke the function before the alias becomes available.
    if origin_reference == Some(use_node) {
        return true;
    }
    // A direct immutable value capture does not invoke the callable. Follow
    // every exposure of that alias instead, including further value captures.
    // Also prove the alias itself exists at each read: a later use must not
    // legitimize an earlier read of an uninitialized (or var-undefined) alias.
    if follow_value_captures {
        if let AstKind::IdentifierReference(identifier) = model.nodes.kind(use_node) {
            let declaration = model.nodes.parent_id(use_node);
            if let AstKind::VariableDeclarator(variable) = model.nodes.kind(declaration) {
                let statement = model.nodes.parent_id(declaration);
                if matches!(model.nodes.kind(statement), AstKind::VariableDeclaration(_))
                    && model.nodes.parent_id(statement) == body
                    && matches!(variable.init.as_ref(),
                        Some(Expression::Identifier(value)) if value.span == identifier.span)
                {
                    if let BindingPattern::BindingIdentifier(id) = &variable.id {
                        if let Some(symbol) = id.symbol_id.get() {
                            return model.binding_writes(symbol) == 0
                                && model.scoping.symbol_redeclarations(symbol).is_empty()
                                && model.scoping.get_resolved_references(symbol).all(
                                    |reference| {
                                        callable_origin_precedes(
                                            model,
                                            body,
                                            variable.span.end,
                                            reference.node_id(),
                                            None,
                                            depth + 1,
                                            false,
                                        ) && callable_origin_precedes(
                                            model,
                                            body,
                                            origin_end,
                                            reference.node_id(),
                                            origin_reference,
                                            depth + 1,
                                            true,
                                        )
                                    },
                                );
                        }
                    }
                }
            }
        }
    }
    let mut child = use_node;
    for _ in 0..128 {
        let parent = model.nodes.parent_id(child);
        if let AstKind::Function(function) = model.nodes.kind(child) {
            let mut binding_origin_end = None;
            let symbol = if parent == body {
                function.id.as_ref().and_then(|id| id.symbol_id.get())
            } else if let AstKind::VariableDeclarator(variable) = model.nodes.kind(parent) {
                // Creating this closure does not execute its body. Only a
                // plain direct-body binding gives us all of its exposures;
                // IIFEs, callback arguments and conditional initializers do
                // not qualify. Use the outer binding even for named expressions.
                let declaration = model.nodes.parent_id(parent);
                if matches!(
                    model.nodes.kind(declaration),
                    AstKind::VariableDeclaration(_)
                ) && model.nodes.parent_id(declaration) == body
                    && matches!(variable.init.as_ref(),
                        Some(Expression::FunctionExpression(value)) if value.span == function.span)
                {
                    binding_origin_end = Some(variable.span.end);
                    match &variable.id {
                        BindingPattern::BindingIdentifier(id) => id.symbol_id.get(),
                        _ => None,
                    }
                } else {
                    None
                }
            } else {
                None
            };
            if let Some(symbol) = symbol {
                return model.binding_writes(symbol) == 0
                    && model.scoping.symbol_redeclarations(symbol).is_empty()
                    && model
                        .scoping
                        .get_resolved_references(symbol)
                        .all(|reference| {
                            binding_origin_end.map_or(true, |end| {
                                callable_origin_precedes(
                                    model,
                                    body,
                                    end,
                                    reference.node_id(),
                                    None,
                                    depth + 1,
                                    false,
                                )
                            }) && callable_origin_precedes(
                                model,
                                body,
                                origin_end,
                                reference.node_id(),
                                origin_reference,
                                depth + 1,
                                follow_value_captures,
                            )
                        });
            }
            if parent == body {
                return false;
            }
        }
        if parent == body {
            // For an ordinary statement the use must occur after setup. This
            // also rejects immediately invoked closures created before setup.
            return model.nodes.kind(child).span().start >= origin_end;
        }
        if parent == child {
            return false;
        }
        child = parent;
    }
    false
}

pub(super) fn stable_function(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    depth: usize,
) -> Option<FunctionSite> {
    if depth > 16 {
        return None;
    }
    if let Expression::Identifier(i) = e {
        let symbol = crate::analysis::identifier_symbol(i, model.scoping)?;
        if !model.scoping.symbol_redeclarations(symbol).is_empty() {
            return None;
        }
        let writes = model.binding_writes(symbol);
        if let AstKind::Function(f) = model.nodes.kind(model.scoping.symbol_declaration(symbol)) {
            return (writes == 0).then(|| model.site_by_span(f.span)).flatten();
        }
        // Parameters/catch bindings already have a runtime origin, even when
        // written exactly once. Only a plain variable declarator can establish
        // an initializer or one later assignment as its sole callable origin.
        let declaration = model.scoping.symbol_declaration(symbol);
        let AstKind::VariableDeclarator(variable) = model.nodes.kind(declaration) else {
            return None;
        };
        if !matches!(&variable.id, BindingPattern::BindingIdentifier(id)
            if id.symbol_id.get() == Some(symbol))
            || !matches!(
                model.nodes.parent_kind(declaration),
                AstKind::VariableDeclaration(_)
            )
            || matches!(
                model.nodes.parent_kind(model.nodes.parent_id(declaration)),
                AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
            )
            || !(writes == 0 && variable.init.is_some() || writes == 1 && variable.init.is_none())
        {
            return None;
        }
        let body = model.nodes.parent_id(model.nodes.parent_id(declaration));
        if !matches!(
            model.nodes.kind(body),
            AstKind::FunctionBody(_) | AstKind::Program(_)
        ) {
            return None;
        }
        let (value, origin_end) = if let Some(value) = variable.init.as_ref() {
            (value, variable.span.end)
        } else {
            let reference = model
                .scoping
                .get_resolved_references(symbol)
                .find(|r| r.is_write())?;
            let assignment_node = model.nodes.parent_id(reference.node_id());
            let AstKind::AssignmentExpression(assignment) = model.nodes.kind(assignment_node)
            else {
                return None;
            };
            let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
                return None;
            };
            let statement = model.nodes.parent_id(assignment_node);
            if assignment.operator != AssignmentOperator::Assign
                || crate::analysis::identifier_symbol(target, model.scoping) != Some(symbol)
                || !matches!(model.nodes.kind(statement), AstKind::ExpressionStatement(_))
                || model.nodes.parent_id(statement) != body
                || assignment.span.start < variable.span.end
            {
                return None;
            }
            (&assignment.right, assignment.span.end)
        };
        let use_node = model.scoping.get_reference(i.reference_id.get()?).node_id();
        let origin_reference = match value {
            Expression::Identifier(id) => id
                .reference_id
                .get()
                .map(|reference| model.scoping.get_reference(reference).node_id()),
            _ => None,
        };
        if !callable_origin_precedes(model, body, origin_end, use_node, origin_reference, 0, true) {
            return None;
        }
        return stable_function(model, value, depth + 1);
    }
    model.function_from_expression(e)
}

pub(super) fn role<'p>(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    proposals: &'p BTreeMap<u32, Vec<Proposal>>,
) -> Option<&'p str> {
    let target = stable_function(model, e, 0)?;
    helper_role_claim(proposals.get(&target.span.start)?)
}

// A complete port-protocol definition is a naming claim, never a composable
// algorithm/readiness fact. Preserve all accepted042 role behavior.
fn helper_role_claim(claims: &[Proposal]) -> Option<&str> {
    if claims.iter().any(|p| p.evaluator == browser::persistent_id_migration::PROTOCOL_EVALUATOR) {
        return None;
    }
    let first = claims.first()?;
    (claims
        .iter()
        .all(|p| p.name == first.name && p.evaluator != "inherited_structural_label"))
    .then_some(first.name.as_str())
}

// This is a bounded owned-body proof, including the three arrow callbacks.
// Every identifier comparison uses the existing scope index; no source names,
// generated metadata, inherited labels or whole-program traversal participate.
pub(super) fn notification_broadcast(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    fn member<'a>(e: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
        let Expression::StaticMemberExpression(m) = e else {
            return None;
        };
        (!m.optional && m.property.name == key).then_some(&m.object)
    }
    fn variable<'a>(s: &'a Statement<'a>) -> Option<&'a VariableDeclarator<'a>> {
        let Statement::VariableDeclaration(d) = s else {
            return None;
        };
        let [v] = d.declarations.as_slice() else {
            return None;
        };
        Some(v)
    }
    fn binding(model: &Model<'_, '_>, p: &BindingPattern<'_>) -> Option<oxc_semantic::SymbolId> {
        let BindingPattern::BindingIdentifier(id) = p else {
            return None;
        };
        let s = id.symbol_id.get()?;
        (model.binding_writes(s) == 0 && model.scoping.symbol_redeclarations(s).is_empty())
            .then_some(s)
    }
    fn arrow<'a>(a: &'a ArrowFunctionExpression<'a>) -> Option<oxc_semantic::SymbolId> {
        let [p] = a.params.items.as_slice() else {
            return None;
        };
        if a.r#async
            || a.params.rest.is_some()
            || p.initializer.is_some()
            || !a.body.directives.is_empty()
        {
            return None;
        }
        let BindingPattern::BindingIdentifier(id) = &p.pattern else {
            return None;
        };
        id.symbol_id.get()
    }
    let symbol = |e| crate::analysis::expression_symbol(e, model.scoping);
    let capture = |e| {
        let s = symbol(e)?;
        (!site.span.contains_inclusive(model.scoping.symbol_span(s))
            && model.scoping.symbol_redeclarations(s).is_empty())
        .then_some(s)
    };
    let [parameter] = function.params.items.as_slice() else {
        return None;
    };
    if function.params.rest.is_some() || parameter.initializer.is_some() {
        return None;
    }
    let input = binding(model, &parameter.pattern)?;
    // Two complete grammars: the indexed notification, or the compact record
    // with explicit undefined count placeholders. Do not accept hybrids.
    let (Statement::ExpressionStatement(statement), aliases) = body.statements.split_last()? else {
        return None;
    };
    let indexed = match aliases.len() {
        4 => true,
        3 => false,
        _ => return None,
    };
    let keys = ["type", "status", "value", "signalIdx"];
    let mut fields = BTreeMap::new();
    for (s, key) in aliases.iter().zip(keys) {
        let v = variable(s)?;
        let id = binding(model, &v.id)?;
        if symbol(member(v.init.as_ref()?, key)?) != Some(input) {
            return None;
        }
        fields.insert(key, id);
    }
    let Expression::CallExpression(each) = &statement.expression else {
        return None;
    };
    let listeners = capture(member(&each.callee, "forEach")?)?;
    let [Argument::ArrowFunctionExpression(callback)] = each.arguments.as_slice() else {
        return None;
    };
    if each.optional || callback.expression {
        return None;
    }
    let listener = arrow(callback)?;
    if model.binding_writes(listener) != 0
        || !model.scoping.symbol_redeclarations(listener).is_empty()
    {
        return None;
    }
    let [record, collected, errored, Statement::ExpressionStatement(invoke)] =
        callback.body.statements.as_slice()
    else {
        return None;
    };
    let record = variable(record)?;
    let result = binding(model, &record.id)?;
    let Expression::ObjectExpression(object) = record.init.as_ref()? else {
        return None;
    };
    if object.properties.len() != if indexed { 5 } else { 6 } {
        return None;
    }
    let mut seen = BTreeSet::new();
    let mut total = None;
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(p) = property else {
            return None;
        };
        let key = property_key_name(&p.key)?;
        if p.computed || p.method || p.kind != PropertyKind::Init || !seen.insert(key.clone()) {
            return None;
        }
        if key == "totalSignals" {
            total = Some(capture(member(&p.value, "length")?)?);
        } else if !indexed && matches!(key.as_str(), "collectedSignals" | "erroredSignals") {
            if !matches!(&p.value, Expression::UnaryExpression(u)
                if u.operator.as_str() == "void"
                    && matches!(&u.argument, Expression::NumericLiteral(n) if n.value == 0.0))
            {
                return None;
            }
        } else if Some(*fields.get(key.as_str())?) != symbol(&p.value) {
            return None;
        }
    }
    let total = total?;
    let mut table = None;
    for (s, output_key, input_key) in [
        (collected, "collectedSignals", "hasCollected"),
        (errored, "erroredSignals", "hasErrored"),
    ] {
        let Statement::ExpressionStatement(s) = s else {
            return None;
        };
        let Expression::AssignmentExpression(a) = &s.expression else {
            return None;
        };
        let AssignmentTarget::StaticMemberExpression(left) = &a.left else {
            return None;
        };
        if a.operator.as_str() != "="
            || left.optional
            || left.property.name != output_key
            || symbol(&left.object) != Some(result)
        {
            return None;
        }
        let Expression::CallExpression(filter) = member(&a.right, "length")? else {
            return None;
        };
        let source = capture(member(&filter.callee, "filter")?)?;
        if filter.optional || table.is_some_and(|old| old != source) {
            return None;
        }
        table = Some(source);
        let [Argument::ArrowFunctionExpression(predicate)] = filter.arguments.as_slice() else {
            return None;
        };
        let item = arrow(predicate)?;
        let [Statement::ExpressionStatement(test)] = predicate.body.statements.as_slice() else {
            return None;
        };
        if !predicate.expression
            || model.binding_writes(item) != 0
            || !model.scoping.symbol_redeclarations(item).is_empty()
            || symbol(member(&test.expression, input_key)?) != Some(item)
        {
            return None;
        }
    }
    if listeners == total || table == Some(listeners) || table == Some(total) {
        return None;
    }
    let Expression::CallExpression(invoke) = &invoke.expression else {
        return None;
    };
    if invoke.optional
        || symbol(&invoke.callee) != Some(listener)
        || invoke.arguments.len() != 1
        || call_arguments(invoke).first().and_then(|a| symbol(a)) != Some(result)
    {
        return None;
    }
    Some(())
}

// Exact bounded dispatch shape, with callee roles established before reference
// matching. No names, IDs, property bags or unordered fact sets identify it.
pub(super) fn hash_dispatch(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
    proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> bool {
    fn check(
        model: &Model<'_, '_>,
        site: FunctionSite,
        body: &FunctionBody<'_>,
        proposals: &BTreeMap<u32, Vec<Proposal>>,
    ) -> Option<()> {
        let [Statement::IfStatement(branch)] = body.statements.as_slice() else {
            return None;
        };
        let Expression::BinaryExpression(test) = &branch.test else {
            return None;
        };
        if test.operator.as_str() != "===" {
            return None;
        }
        let Expression::UnaryExpression(typeof_) = &test.left else {
            return None;
        };
        if typeof_.operator.as_str() != "typeof"
            || model.site_parameter_slot(&typeof_.argument, site) != Some(0)
            || !matches!(&test.right, Expression::StringLiteral(s) if s.value == "string")
        {
            return None;
        }
        let Statement::BlockStatement(strings) = &branch.consequent else {
            return None;
        };
        let [Statement::ExpressionStatement(string_call)] = strings.body.as_slice() else {
            return None;
        };
        let Expression::CallExpression(string_call) = &string_call.expression else {
            return None;
        };
        let forwards = |c: &CallExpression<'_>, expected| {
            !c.optional
                && role(model, &c.callee, proposals) == Some(expected)
                && c.arguments.len() == 4
                && call_arguments(c)
                    .iter()
                    .enumerate()
                    .all(|(i, a)| model.site_parameter_slot(a, site) == Some(i as u32))
        };
        if !forwards(string_call, "hashStringAsync") {
            return None;
        }
        let Statement::BlockStatement(other) = branch.alternate.as_ref()? else {
            return None;
        };
        let [Statement::VariableDeclaration(declaration), Statement::IfStatement(view_test), Statement::IfStatement(dispatch)] =
            other.body.as_slice()
        else {
            return None;
        };
        let [variable] = declaration.declarations.as_slice() else {
            return None;
        };
        let BindingPattern::BindingIdentifier(flag) = &variable.id else {
            return None;
        };
        let flag = flag.symbol_id.get()?;
        let initial = crate::analysis::expression_symbol(variable.init.as_ref()?, model.scoping)?;
        if site
            .span
            .contains_inclusive(model.scoping.symbol_span(initial))
            || !model.scoping.symbol_redeclarations(flag).is_empty()
            || model.binding_writes(flag) != 1
            || crate::analysis::expression_symbol(&view_test.test, model.scoping) != Some(flag)
            || view_test.alternate.is_some()
            || crate::analysis::expression_symbol(&dispatch.test, model.scoping) != Some(flag)
        {
            return None;
        }
        let Statement::BlockStatement(view_body) = &view_test.consequent else {
            return None;
        };
        let [Statement::ExpressionStatement(assignment)] = view_body.body.as_slice() else {
            return None;
        };
        let Expression::AssignmentExpression(assignment) = &assignment.expression else {
            return None;
        };
        if assignment.operator.as_str() != "="
            || !matches!(&assignment.left, AssignmentTarget::AssignmentTargetIdentifier(i)
                if crate::analysis::identifier_symbol(i, model.scoping) == Some(flag))
        {
            return None;
        }
        let Expression::CallExpression(is_view) = &assignment.right else {
            return None;
        };
        let Expression::StaticMemberExpression(api) = &is_view.callee else {
            return None;
        };
        if is_view.optional
            || api.optional
            || api.property.name != "isView"
            || expression_path(model, &api.object) != ["ArrayBuffer"]
            || is_view.arguments.len() != 1
            || call_arguments(is_view)
                .first()
                .and_then(|a| model.site_parameter_slot(a, site))
                != Some(0)
        {
            return None;
        }
        let Statement::BlockStatement(views) = &dispatch.consequent else {
            return None;
        };
        let [Statement::ExpressionStatement(view_call)] = views.body.as_slice() else {
            return None;
        };
        let Expression::CallExpression(view_call) = &view_call.expression else {
            return None;
        };
        if !forwards(view_call, "hashArrayBufferViewAsync") {
            return None;
        }
        let Statement::BlockStatement(errors) = dispatch.alternate.as_ref()? else {
            return None;
        };
        let [Statement::ExpressionStatement(error_call)] = errors.body.as_slice() else {
            return None;
        };
        let Expression::CallExpression(error_call) = &error_call.expression else {
            return None;
        };
        if error_call.optional
            || model.site_parameter_slot(&error_call.callee, site) != Some(3)
            || error_call.arguments.len() != 1
        {
            return None;
        }
        let Argument::NewExpression(error) = &error_call.arguments[0] else {
            return None;
        };
        if expression_path(model, &error.callee) != ["TypeError"] || !error.arguments.is_empty() {
            return None;
        }
        Some(())
    }
    check(model, site, body, proposals).is_some()
}

// Bounded owned statements keep callback, catch and parameter identities distinct.
pub(super) fn completion_adapter(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [Statement::TryStatement(t)] = body.statements.as_slice() else {
        return None;
    };
    if t.finalizer.is_some() {
        return None;
    }
    let [Statement::ExpressionStatement(task), Statement::ExpressionStatement(done)] =
        t.block.body.as_slice()
    else {
        return None;
    };
    let Expression::CallExpression(task) = &task.expression else {
        return None;
    };
    let Expression::CallExpression(done) = &done.expression else {
        return None;
    };
    if task.optional
        || done.optional
        || !task.arguments.is_empty()
        || !done.arguments.is_empty()
        || model.site_parameter_slot(&task.callee, site) != Some(0)
        || model.site_parameter_slot(&done.callee, site) != Some(1)
    {
        return None;
    }
    let handler = t.handler.as_ref()?;
    let BindingPattern::BindingIdentifier(error) = &handler.param.as_ref()?.pattern else {
        return None;
    };
    let error = error.symbol_id.get()?;
    let [Statement::ExpressionStatement(failed)] = handler.body.body.as_slice() else {
        return None;
    };
    let Expression::CallExpression(failed) = &failed.expression else {
        return None;
    };
    if failed.optional
        || model.site_parameter_slot(&failed.callee, site) != Some(1)
        || failed.arguments.len() != 1
        || call_arguments(failed)
            .first()
            .and_then(|e| crate::analysis::expression_symbol(e, model.scoping))
            != Some(error)
        || model.binding_writes(error) != 0
        || !model.scoping.symbol_redeclarations(error).is_empty()
    {
        return None;
    }
    Some(())
}

pub(super) fn plain_local(
    model: &Model<'_, '_>,
    v: &VariableDeclarator<'_>,
) -> Option<oxc_semantic::SymbolId> {
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
pub(super) fn one_variable<'a>(s: &'a Statement<'a>) -> Option<&'a VariableDeclarator<'a>> {
    let Statement::VariableDeclaration(d) = s else {
        return None;
    };
    let [v] = d.declarations.as_slice() else {
        return None;
    };
    Some(v)
}
pub(super) fn null_test(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    symbol: oxc_semantic::SymbolId,
    op: &str,
) -> bool {
    matches!(e, Expression::BinaryExpression(b) if b.operator.as_str() == op
        && crate::analysis::expression_symbol(&b.left, model.scoping) == Some(symbol)
        && matches!(&b.right, Expression::NullLiteral(_)))
}
pub(super) fn global_unary_argument(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    path: &[&str],
    symbol: oxc_semantic::SymbolId,
) -> bool {
    matches!(e, Expression::CallExpression(c) if !c.optional && expression_path(model, &c.callee) == path
        && c.arguments.len() == 1 && call_arguments(c).first().and_then(|e| crate::analysis::expression_symbol(e, model.scoping)) == Some(symbol))
}

pub(super) fn descriptor_search(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [initial, Statement::WhileStatement(w), Statement::ReturnStatement(end)] =
        body.statements.as_slice()
    else {
        return None;
    };
    if !matches!(end.argument.as_ref()?, Expression::NullLiteral(_)) {
        return None;
    }
    let v = one_variable(initial)?;
    let cursor = plain_local(model, v)?;
    if model.site_parameter_slot(v.init.as_ref()?, site) != Some(0)
        || model.binding_writes(cursor) != 2
        || !null_test(model, &w.test, cursor, "!=")
    {
        return None;
    }
    let Statement::BlockStatement(block) = &w.body else {
        return None;
    };
    let [descriptor, Statement::IfStatement(branch)] = block.body.as_slice() else {
        return None;
    };
    let descriptor = one_variable(descriptor)?;
    let desc = plain_local(model, descriptor)?;
    let Expression::CallExpression(c) = descriptor.init.as_ref()? else {
        return None;
    };
    if c.optional
        || expression_path(model, &c.callee) != ["Object", "getOwnPropertyDescriptor"]
        || c.arguments.len() != 2
        || call_arguments(c)
            .first()
            .and_then(|e| crate::analysis::expression_symbol(e, model.scoping))
            != Some(cursor)
        || call_arguments(c)
            .get(1)
            .and_then(|e| model.site_parameter_slot(e, site))
            != Some(1)
        || model.binding_writes(desc) != 0
        || !null_test(model, &branch.test, desc, "!=")
    {
        return None;
    }
    let Statement::BlockStatement(found) = &branch.consequent else {
        return None;
    };
    let [Statement::ExpressionStatement(pack), Statement::ReturnStatement(ret)] =
        found.body.as_slice()
    else {
        return None;
    };
    let Expression::AssignmentExpression(pack) = &pack.expression else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(target) = &pack.left else {
        return None;
    };
    let Expression::ObjectExpression(record) = &pack.right else {
        return None;
    };
    if pack.operator.as_str() != "="
        || crate::analysis::identifier_symbol(target, model.scoping) != Some(cursor)
        || crate::analysis::expression_symbol(ret.argument.as_ref()?, model.scoping) != Some(cursor)
        || record.properties.len() != 2
    {
        return None;
    }
    let mut keys = BTreeSet::new();
    for p in &record.properties {
        let ObjectPropertyKind::ObjectProperty(p) = p else {
            return None;
        };
        let key = property_key_name(&p.key)?;
        let expected = match key.as_str() {
            "containingObj" => cursor,
            "desc" => desc,
            _ => return None,
        };
        if p.computed
            || p.method
            || p.kind != PropertyKind::Init
            || !keys.insert(key)
            || crate::analysis::expression_symbol(&p.value, model.scoping) != Some(expected)
        {
            return None;
        }
    }
    let Statement::BlockStatement(next) = branch.alternate.as_ref()? else {
        return None;
    };
    let [Statement::ExpressionStatement(step), Statement::ContinueStatement(continue_)] =
        next.body.as_slice()
    else {
        return None;
    };
    let Expression::AssignmentExpression(step) = &step.expression else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(target) = &step.left else {
        return None;
    };
    if continue_.label.is_some()
        || step.operator.as_str() != "="
        || crate::analysis::identifier_symbol(target, model.scoping) != Some(cursor)
        || !global_unary_argument(model, &step.right, &["Object", "getPrototypeOf"], cursor)
    {
        return None;
    }
    Some(())
}

pub(super) fn document_capture(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [Statement::ExpressionStatement(s)] = body.statements.as_slice() else {
        return None;
    };
    let Expression::AssignmentExpression(a) = &s.expression else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(target) = &a.left else {
        return None;
    };
    let symbol = crate::analysis::identifier_symbol(target, model.scoping)?;
    if a.operator.as_str() != "="
        || site
            .span
            .contains_inclusive(model.scoping.symbol_span(symbol))
        || model.formal_symbol_slot(symbol).is_some()
        || !model.scoping.symbol_redeclarations(symbol).is_empty()
    {
        return None;
    }
    let AstKind::VariableDeclarator(v) = model.nodes.kind(model.scoping.symbol_declaration(symbol))
    else {
        return None;
    };
    if plain_local(model, v) != Some(symbol) {
        return None;
    }
    let Expression::StaticMemberExpression(doc) = &a.right else {
        return None;
    };
    let Expression::StaticMemberExpression(global) = &doc.object else {
        return None;
    };
    if doc.optional
        || global.optional
        || doc.property.name != "document"
        || global.property.name != "global"
        || model.site_parameter_slot(&global.object, site) != Some(0)
    {
        return None;
    }
    Some(())
}

pub(super) fn prototype_property_count(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [Statement::IfStatement(branch)] = body.statements.as_slice() else {
        return None;
    };
    let Expression::BinaryExpression(test) = &branch.test else {
        return None;
    };
    if test.operator.as_str() != "=="
        || model.site_parameter_slot(&test.left, site) != Some(0)
        || !matches!(&test.right, Expression::NullLiteral(_))
    {
        return None;
    }
    let input = crate::analysis::expression_symbol(&test.left, model.scoping)?;
    let Statement::BlockStatement(base) = &branch.consequent else {
        return None;
    };
    let [Statement::ReturnStatement(zero)] = base.body.as_slice() else {
        return None;
    };
    if !matches!(zero.argument.as_ref()?, Expression::NumericLiteral(n) if n.value == 0.0) {
        return None;
    }
    let Statement::BlockStatement(next) = branch.alternate.as_ref()? else {
        return None;
    };
    let [Statement::ReturnStatement(sum)] = next.body.as_slice() else {
        return None;
    };
    let Expression::BinaryExpression(sum) = sum.argument.as_ref()? else {
        return None;
    };
    let Expression::StaticMemberExpression(length) = &sum.left else {
        return None;
    };
    let Expression::CallExpression(recurse) = &sum.right else {
        return None;
    };
    if sum.operator.as_str() != "+"
        || length.optional
        || length.property.name != "length"
        || !global_unary_argument(
            model,
            &length.object,
            &["Object", "getOwnPropertyNames"],
            input,
        )
        || recurse.optional
        || recurse.arguments.len() != 1
        || stable_function(model, &recurse.callee, 0)?.span != site.span
        || !global_unary_argument(
            model,
            call_arguments(recurse).first()?,
            &["Object", "getPrototypeOf"],
            input,
        )
    {
        return None;
    }
    Some(())
}

pub(super) fn evaluate_shared(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Option<Proposal> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return None;
    };
    let body = function.body.as_ref()?;
    if !body.directives.is_empty() || function.r#async || function.generator {
        return None;
    }
    let make = |name, detail| Some(proposal(name, "shared_scoped_role", detail, f));
    let plain_formals = function.params.rest.is_none() && function.params.items.iter().all(|p| {
        p.initializer.is_none() && matches!(&p.pattern, BindingPattern::BindingIdentifier(id)
            if id.symbol_id.get().is_some_and(|s| model.binding_writes(s) == 0 && model.scoping.symbol_redeclarations(s).is_empty()))
    });
    if plain_formals && function.params.items.len() == 2 {
        if completion_adapter(model, site, body).is_some() {
            return make("runTaskWithCompletionCallback", "exact try task-slot0(); completion-slot1(); catch(error) completion-slot1(error), no returns or finally; distinct immutable formal and catch identities");
        }
        if descriptor_search(model, site, body).is_some() {
            return make("findPropertyDescriptorInPrototypeChain", "walks slot0 via Object.getPrototypeOf until null; Object.getOwnPropertyDescriptor(cursor, slot1) returns exact containingObj/desc record; local cursor and descriptor identities proven");
        }
    }
    if plain_formals
        && function.params.items.len() == 1
        && document_capture(model, site, body).is_some()
    {
        return make("captureGlobalDocument", "sole assignment stores formal slot0.global.document in a distinct captured plain variable binding");
    }
    if plain_formals
        && function.params.items.len() == 1
        && prototype_property_count(model, site, body).is_some()
    {
        return make("countOwnPropertiesAlongPrototypeChain", "null formal returns zero; otherwise returns Object.getOwnPropertyNames(formal).length plus this exact function recursively applied to Object.getPrototypeOf(formal), through a stable scoped function origin");
    }
    if notification_broadcast(model, site, function, body).is_some() {
        return make("broadcastSignalNotification", "forEach listener receives the exact local record forwarding formal type/status/value, with either signalIdx or explicit undefined count placeholders, totalSignals from a distinct captured table length, and collectedSignals/erroredSignals from hasCollected/hasErrored filters of the same distinct captured state table; all local identities are immutable and unredeclared");
    }
    if f.returns.len() == 1 {
        if let Some(child) = stable_function(model, f.returns[0], 0) {
            if model
                .enclosing_function(child.node_id)
                .is_some_and(|p| p.span == site.span)
            {
                match role(model, f.returns[0], proposals) {
                    Some("nextKeystreamByte")
                        if f.calls.len() == 4
                            && f.calls.iter().all(|c| {
                                c.arguments.is_empty()
                                    && stable_function(model, &c.callee, 0)
                                        .is_some_and(|target| target.span == child.span)
                            })
                            && f.loops == 0
                            && f.throws == 0
                            && f.news == 0 =>
                    {
                        return make("createKeystreamByteSource", "returns the independently proven nested nextKeystreamByte after exactly four zero-argument warm-up calls through single-origin scoped bindings");
                    }
                    Some("writeInternedString")
                        if f.calls.len() == 1
                            && global_call(model, f, &["Object", "create"])
                            && f.loops == 0
                            && f.throws == 0
                            && f.news == 0 =>
                    {
                        let c = f.calls[0];
                        if c.arguments.len() == 1
                            && matches!(&c.arguments[0], Argument::NullLiteral(_))
                        {
                            return make("createInternedStringWriter", "allocates a null-prototype cache and returns its independently proven nested writeInternedString");
                        }
                    }
                    _ => (),
                }
            }
        }
    }
    if hash_dispatch(model, site, body, proposals) {
        return make("hashStringOrArrayBufferViewAsync", "dispatches formal slots 0..3 unchanged to independently proven hashStringAsync/hashArrayBufferViewAsync under typeof-string and guarded ArrayBuffer.isView tests; otherwise calls error slot 3 with new TypeError");
    }
    // A real return-call relationship, not merely a call somewhere in a body.
    if f.statements == 1 && f.calls.len() == 1 && f.returns.len() == 1 {
        if let Expression::CallExpression(c) = f.returns[0] {
            let args = call_arguments(c);
            if role(model, &c.callee, proposals) == Some("arraysEqualByIndex")
                && args.len() == 2
                && model.site_parameter_slot(args[0], site) == Some(0)
                && crate::analysis::expression_symbol(args[1], model.scoping).is_some_and(|s| {
                    model.formal_symbol_slot(s).is_none()
                        && !site.span.contains_inclusive(model.scoping.symbol_span(s))
                })
            {
                return make("matchesArrayContents", "returns independently proven arraysEqualByIndex(formal slot 0, captured comparison array) through single-origin scoped bindings");
            }
        }
    }
    // Straight-line factory construction supports methods in the literal or
    // assigned once afterwards. Branch-dependent or duplicate writes refuse.
    if f.calls.is_empty()
        && f.returns.len() == 1
        && f.assignments.len() <= 3
        && f.loops == 0
        && f.throws == 0
        && f.news == 0
        && body.statements.iter().all(|s| {
            matches!(
                s,
                Statement::FunctionDeclaration(_)
                    | Statement::VariableDeclaration(_)
                    | Statement::ExpressionStatement(_)
                    | Statement::ReturnStatement(_)
            )
        })
    {
        if let Some(result) = crate::analysis::expression_symbol(f.returns[0], model.scoping) {
            if let Some(Expression::ObjectExpression(object)) = model.initializer(result) {
                let declaration = model.scoping.symbol_declaration(result);
                if !site
                    .span
                    .contains_inclusive(model.scoping.symbol_span(result))
                    || !model.scoping.symbol_redeclarations(result).is_empty()
                    || !matches!(model.nodes.kind(declaration), AstKind::VariableDeclarator(v)
                        if matches!(&v.id, BindingPattern::BindingIdentifier(_)))
                    || !matches!(
                        model.nodes.parent_kind(declaration),
                        AstKind::VariableDeclaration(_)
                    )
                    || f.deletes != 0
                    || !matches!(body.statements.last(), Some(Statement::ReturnStatement(_)))
                    || !model.scoping.get_resolved_references(result).all(|r| {
                        match model.nodes.parent_kind(r.node_id()) {
                            AstKind::ReturnStatement(_) => true,
                            AstKind::StaticMemberExpression(m) => f.assignments.iter().any(|a| {
                                matches!(&a.left, AssignmentTarget::StaticMemberExpression(left)
                                    if left.span == m.span)
                            }),
                            _ => false,
                        }
                    })
                {
                    return None;
                }
                let expected = [
                    ("next", "recordSignalValue"),
                    ("error", "recordSignalError"),
                    ("complete", "recordSignalBytesOnComplete"),
                ];
                let literal_methods: Option<BTreeMap<_, _>> = object
                    .properties
                    .iter()
                    .map(|property| {
                        let ObjectPropertyKind::ObjectProperty(property) = property else {
                            return None;
                        };
                        if property.computed
                            || property.method
                            || property.kind != PropertyKind::Init
                        {
                            return None;
                        }
                        Some((property_key_name(&property.key)?, &property.value))
                    })
                    .collect();
                if let Some(mut methods) = literal_methods {
                    // Duplicate literal keys and overwriting real methods are
                    // ambiguous. Generated void-0 placeholders are not methods.
                    if methods.len() != object.properties.len() {
                        return None;
                    }
                    let mut written = BTreeSet::new();
                    let writes_valid = f.assignments.iter().all(|a| {
                        let AssignmentTarget::StaticMemberExpression(m) = &a.left else {
                            return false;
                        };
                        if a.span.start <= object.span.end
                            || a.operator.as_str() != "="
                            || crate::analysis::expression_symbol(&m.object, model.scoping)
                                != Some(result)
                            || !written.insert(m.property.name.to_string())
                        {
                            return false;
                        }
                        if methods.get(m.property.name.as_str()).is_some_and(|v| {
                            !matches!(v, Expression::UnaryExpression(u)
                                if u.operator.as_str() == "void" && matches!(&u.argument,
                                    Expression::NumericLiteral(n) if n.value == 0.0))
                        }) {
                            return false;
                        }
                        methods.insert(m.property.name.to_string(), &a.right);
                        true
                    });
                    if writes_valid
                        && object.properties.len() <= 3
                        && methods.len() == 3
                        && model.binding_writes(result) == 0
                        && expected.iter().all(|(method, name)| {
                            methods.get(*method).is_some_and(|value| {
                                role(model, value, proposals) == Some(*name)
                                    && stable_function(model, value, 0).is_some_and(|child| {
                                        model
                                            .enclosing_function(child.node_id)
                                            .is_some_and(|p| p.span == site.span)
                                    })
                            })
                        })
                    {
                        return make("createSignalObserver", "returns the straight-line object wired, in its literal or by single property assignments, to independently proven nested recordSignalValue, recordSignalError and recordSignalBytesOnComplete callbacks");
                    }
                }
            }
        }
    }
    if function.params.items.is_empty()
        && f.statements == 1
        && f.calls.len() == 1
        && f.returns.is_empty()
        && f.assignments.is_empty()
    {
        if let Statement::IfStatement(branch) = &body.statements[0] {
            if branch.alternate.is_none() {
                if let (Expression::Identifier(test), Statement::BlockStatement(block)) =
                    (&branch.test, &branch.consequent)
                {
                    if let [Statement::ExpressionStatement(statement)] = block.body.as_slice() {
                        if let Expression::CallExpression(c) = &statement.expression {
                            if let Expression::StaticMemberExpression(m) = &c.callee {
                                let receiver =
                                    crate::analysis::expression_symbol(&m.object, model.scoping);
                                if m.property.name == "empty"
                                    && c.arguments.is_empty()
                                    && !c.optional
                                    && receiver.is_some()
                                    && receiver
                                        == crate::analysis::identifier_symbol(test, model.scoping)
                                    && receiver.is_some_and(|s| {
                                        model.binding_writes(s) == 0
                                            || (model.binding_writes(s) == 1
                                                && model.initializer(s).is_none())
                                    })
                                {
                                    return make("callEmptyIfPresent", "sole action is if (capturedReceiver) capturedReceiver.empty(); no arguments or return value; receiver binding with no reassignment after its source initialization");
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    // Exact concatenation tree retains delimiters, channel order and parameter
    // slots. A different color syntax or channel ordering has different meaning.
    if f.statements == 1
        && f.calls.is_empty()
        && f.returns.len() == 1
        && function.params.items.len() == 1
    {
        let mut parts = Vec::new();
        rgba_parts(f.returns[0], &mut parts);
        if parts.len() == 9 && parts.iter().enumerate().all(|(i, e)| {
            if i % 2 == 0 {
                matches!(e, Expression::StringLiteral(s) if s.value == match i {0 => "rgba(", 8 => ")", _ => ","})
            } else {
                matches!(e, Expression::ComputedMemberExpression(m)
                    if model.site_parameter_slot(&m.object, site) == Some(0)
                        && matches!(&m.expression, Expression::NumericLiteral(n) if n.value == ((i-1)/2) as f64))
            }
        }) {
            return make("formatRgbaChannels", "returns exactly rgba(slot0[0],slot0[1],slot0[2],slot0[3]) with comma delimiters");
        }
    }
    None
}
pub(super) fn rgba_parts<'a>(e: &'a Expression<'a>, parts: &mut Vec<&'a Expression<'a>>) {
    if let Expression::BinaryExpression(b) = e {
        if b.operator.as_str() == "+" {
            rgba_parts(&b.left, parts);
            parts.push(&b.right);
            return;
        }
    }
    parts.push(e);
}

/// The frame-half label proved by `signals_seeded_signal_frame`.
pub(super) const SEEDED_EMITTER_FRAME: &str = "emitSeededSignalFrame";
/// The canonical label proved by `encoding_seeded_hash_encoding`.
pub(super) const SEEDED_EMITTER_ENCODING: &str = "emitSeededHashEncoding";

/// Collapse the two independently proved labels of the shared seeded-emitter
/// shape onto one governed name, and only when the complete bounded agreement
/// is present on the same proposal list.
pub(super) fn reconcile_agreed_aliases(list: &mut Vec<Proposal>) {
    let proofs_agree = list
        .iter()
        .any(|p| p.name == SEEDED_EMITTER_FRAME && p.evaluator == "signals_seeded_signal_frame")
        && list.iter().any(|p| {
            p.name == SEEDED_EMITTER_ENCODING && p.evaluator == "encoding_seeded_hash_encoding"
        });
    if !proofs_agree {
        return;
    }
    if list.iter().any(|p| {
        p.evaluator != "inherited_structural_label"
            && p.name != SEEDED_EMITTER_FRAME
            && p.name != SEEDED_EMITTER_ENCODING
    }) {
        return;
    }
    list.retain(|p| {
        !(p.name == SEEDED_EMITTER_FRAME && p.evaluator == "signals_seeded_signal_frame")
    });
    for p in list.iter_mut().filter(|p| {
        p.name == SEEDED_EMITTER_ENCODING && p.evaluator != "inherited_structural_label"
    }) {
        p.explanation.push(format!(
            "the {} and {} independent proofs agree on one owned seeded-emitter body; canonical governed label {} retained",
            SEEDED_EMITTER_FRAME, SEEDED_EMITTER_ENCODING, SEEDED_EMITTER_ENCODING
        ));
    }
}

/// A unique current-source role proof may disambiguate weaker inherited body
/// templates within the enumerated groups. Queue context roles need not be
/// universal synonyms. Independent disagreements and arbitrary inherited labels
/// remain ambiguous; there is no generic preference by name.
pub(super) fn reconcile_synonyms(list: &mut Vec<Proposal>) {
    let independent: BTreeSet<_> = list
        .iter()
        .filter(|p| p.evaluator != "inherited_structural_label")
        .map(|p| p.name.clone())
        .collect();
    if independent.len() != 1 {
        return;
    }
    let name = independent.first().unwrap();
    let pairs: &[&[&str]] = &[
        &["runScheduledQueueTask", "runSequentialQueueTask"],
        &[
            "createOnceContinuation",
            "createSequentialQueueOnceCallback",
        ],
        &[
            "defineClassMembers",
            "installPropertyDescriptors",
            "defineClassProperties",
        ],
        &[
            "createClassMembersInstaller",
            "createInstallPropertyDescriptors",
            "createUnusedClassPropertyDefiner",
        ],
    ];
    let Some(pair) = pairs.iter().find(|pair| pair.contains(&name.as_str())) else {
        return;
    };
    if !list.iter().all(|p| pair.contains(&p.name.as_str())) {
        return;
    }
    let dropped: BTreeSet<_> = list
        .iter()
        .filter(|p| p.name != *name)
        .map(|p| p.name.clone())
        .collect();
    if dropped.is_empty() {
        return;
    }
    list.retain(|p| p.name == *name);
    list[0].explanation.push(format!(
        "unique current-source role proof disambiguates weaker inherited body-template claims: {}",
        dropped.into_iter().collect::<Vec<_>>().join(", ")
    ));
}

pub(super) fn proves_method_interceptor(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &oxc_ast::ast::Function<'_>,
    body: &oxc_ast::ast::FunctionBody<'_>,
) -> bool {
    crate::signals::analysis::roles::function_interceptor::proves_method_interceptor(model, site, function, body)
}

#[cfg(test)]
mod migration_protocol_export_tests {
    use super::*;

    fn claim(name: &str, evaluator: &str) -> Proposal {
        Proposal { name: name.into(), evaluator: evaluator.into(), confidence: Confidence::Medium,
            evidence: Vec::new(), explanation: Vec::new() }
    }

    #[test]
    fn protocol_definition_never_supplies_composable_helper_fact() {
        let evaluator = browser::persistent_id_migration::PROTOCOL_EVALUATOR;
        assert_eq!(helper_role_claim(&[claim("runPersistentIdMigrationProtocol", evaluator)]), None);
        // Even a misnamed protocol proposal cannot authorize numeric consumers.
        assert_eq!(helper_role_claim(&[claim("writeNumber", evaluator)]), None);
        assert_eq!(helper_role_claim(&[claim("writeNumber", "thread_posthost_numeric"), claim("writeNumber", evaluator)]), None);
        assert_eq!(helper_role_claim(&[claim("writeNumber", "thread_posthost_numeric")]), Some("writeNumber"));
        assert_eq!(helper_role_claim(&[claim("writeNumber", "inherited_structural_label")]), None);
    }
}
