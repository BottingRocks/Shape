//! Resolve grouped producer registrations, without interpreting producer bodies.
//! The consumer must transport each array element to a coordinator's second
//! argument, and that coordinator must invoke that argument with six arguments.
//! Four-argument platform hooks and descriptor callbacks do not satisfy this ABI.
use std::collections::{BTreeMap, BTreeSet, HashMap};

use oxc_ast::{ast::*, AstKind};
use oxc_semantic::{NodeId, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::AssignmentOperator;
use serde::{Deserialize, Serialize};

use super::super::bindings::{array_elements, call_arguments, numeric_index, FunctionSite, Model};
use crate::signals::schema::{BindingRef, FunctionRef, SourceSpan};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GroupedRegistration {
    pub registry: BindingRef,
    pub slot: usize,
    pub function: Option<FunctionRef>,
    pub evidence: Vec<SourceSpan>,
    pub unresolved: Vec<String>,
}

type Environment<'a> = HashMap<SymbolId, &'a Expression<'a>>;

struct Discovery<'m, 's, 'a> {
    model: &'m Model<'s, 'a>,
    calls: HashMap<NodeId, Vec<&'a CallExpression<'a>>>,
    ordered_calls: BTreeSet<u32>,
}

impl<'m, 's, 'a> Discovery<'m, 's, 'a> {
    fn symbol(&self, e: &Expression<'_>) -> Option<SymbolId> {
        match e.without_parentheses() {
            Expression::Identifier(id) => {
                crate::analysis::identifier_symbol(id, self.model.scoping)
            }
            _ => None,
        }
    }

    /// A variable has exactly one plain origin. A guarded origin is usable only
    /// when every active call/escape is also proved inside that branch below.
    /// Do not let symbol_value's initializer preference hide a reassignment.
    fn value(&self, symbol: SymbolId) -> Option<&'a Expression<'a>> {
        if !self.model.scoping.symbol_redeclarations(symbol).is_empty() {
            return None;
        }
        let declaration = self.model.scoping.symbol_declaration(symbol);
        let AstKind::VariableDeclarator(var) = self.model.nodes.kind(declaration) else {
            return None;
        };
        let list = self
            .model
            .nodes
            .parent_id(self.model.nodes.parent_id(declaration));
        if !matches!(
            self.model.nodes.kind(list),
            AstKind::FunctionBody(_) | AstKind::Program(_)
        ) {
            return None;
        }
        match (var.init.as_ref(), self.model.binding_writes(symbol)) {
            (Some(value), 0) => Some(value),
            (None, 1) => {
                let reference = self
                    .model
                    .scoping
                    .get_resolved_references(symbol)
                    .find(|r| r.is_write())?;
                let assignment_node = self.model.nodes.parent_id(reference.node_id());
                let AstKind::AssignmentExpression(a) = self.model.nodes.kind(assignment_node)
                else {
                    return None;
                };
                let statement = self.model.nodes.parent_id(assignment_node);
                (a.operator == AssignmentOperator::Assign
                    && matches!(&a.left, AssignmentTarget::AssignmentTargetIdentifier(id)
                        if crate::analysis::identifier_symbol(id, self.model.scoping) == Some(symbol))
                    && matches!(self.model.nodes.kind(statement), AstKind::ExpressionStatement(_))
                    && (self.model.nodes.parent_id(statement) == list
                        || self.guarded_origin(symbol).is_some())
                    && a.span.start >= var.span.end).then_some(&a.right)
            }
            _ => None,
        }
    }

    // The sole extension to straight-line initialization: one assignment in a
    // direct, single-arm if block owned by the declaration's callable. No
    // sibling alternatives, loops, nested writers, or general reaching-defs.
    fn guarded_origin(&self, symbol: SymbolId) -> Option<NodeId> {
        let declaration = self.model.scoping.symbol_declaration(symbol);
        let AstKind::VariableDeclarator(var) = self.model.nodes.kind(declaration) else {
            return None;
        };
        if var.init.is_some() || self.model.binding_writes(symbol) != 1 {
            return None;
        }
        let reference = self.model.scoping.get_resolved_references(symbol).find(|r| r.is_write())?;
        let assignment = self.model.nodes.parent_id(reference.node_id());
        let AstKind::AssignmentExpression(a) = self.model.nodes.kind(assignment) else {
            return None;
        };
        let statement = self.model.nodes.parent_id(assignment);
        if a.operator != AssignmentOperator::Assign
            || !matches!(self.model.nodes.kind(statement), AstKind::ExpressionStatement(_))
        {
            return None;
        }
        let block = self.model.nodes.parent_id(statement);
        if !matches!(self.model.nodes.kind(block), AstKind::BlockStatement(_)) {
            return None;
        }
        let branch = self.model.nodes.parent_id(block);
        let AstKind::IfStatement(guard) = self.model.nodes.kind(branch) else {
            return None;
        };
        let declaration_list = self.model.nodes.parent_id(self.model.nodes.parent_id(declaration));
        (guard.alternate.is_none()
            && guard.consequent.span() == self.model.nodes.get_node(block).span()
            && self.model.nodes.parent_id(branch) == declaration_list)
            .then_some(block)
    }

    fn within(&self, node: NodeId, region: NodeId) -> bool {
        node == region || self.model.nodes.ancestor_ids(node).any(|ancestor| ancestor == region)
    }

    fn substituted(&self, e: &'a Expression<'a>, env: &Environment<'a>) -> &'a Expression<'a> {
        self.symbol(e)
            .and_then(|s| env.get(&s).copied())
            .unwrap_or(e)
    }

    // Bounded ordering proof: straight-line origins in one callable, or a
    // deferred callable whose every call/escape occurs after that origin.
    // Alias transport itself does not execute the deferred callable.
    fn after(
        &self,
        node: NodeId,
        boundary: u32,
        owner: Option<NodeId>,
        region: Option<NodeId>,
        seen: &mut BTreeSet<u32>,
    ) -> bool {
        let site = self.model.owning_site(node);
        if site.map(|s| s.node_id) == owner {
            return self.model.nodes.get_node(node).span().start >= boundary
                && region.is_none_or(|region| self.within(node, region));
        }
        let Some(site) = site else {
            return false;
        };
        if seen.len() >= 32 { return false; }
        if !seen.insert(site.span.start) {
            return region.is_none();
        }
        let result = match self.model.nodes.kind(site.node_id) {
            AstKind::ArrowFunctionExpression(_) => self.after(site.node_id, boundary, owner, region, seen),
            AstKind::Function(f) if f.r#type != FunctionType::FunctionDeclaration => {
                self.after(site.node_id, boundary, owner, region, seen)
            }
            _ => site
                .binding_symbol
                .is_some_and(|s| self.uses_after(s, boundary, owner, region, seen, &mut BTreeSet::new())),
        };
        seen.remove(&site.span.start);
        result
    }

    fn uses_after(
        &self,
        symbol: SymbolId,
        boundary: u32,
        owner: Option<NodeId>,
        region: Option<NodeId>,
        seen: &mut BTreeSet<u32>,
        aliases: &mut BTreeSet<SymbolId>,
    ) -> bool {
        if aliases.len() >= 32 { return false; }
        if !aliases.insert(symbol) {
            return region.is_none();
        }
        if region.is_some() && !self.model.scoping.get_resolved_references(symbol).any(|r| r.is_read()) {
            return false;
        }
        self.model
            .scoping
            .get_resolved_references(symbol)
            .filter(|r| r.is_read())
            .all(|reference| {
                let alias = match self.model.nodes.parent_kind(reference.node_id()) {
                    AstKind::VariableDeclarator(v) => match &v.id {
                        BindingPattern::BindingIdentifier(id) => id.symbol_id.get(),
                        _ => None,
                    },
                    AstKind::AssignmentExpression(a) if self.symbol(&a.right) == Some(symbol) => {
                        match &a.left {
                            AssignmentTarget::AssignmentTargetIdentifier(id) => {
                                crate::analysis::identifier_symbol(id, self.model.scoping)
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                };
                if let Some(alias) = alias {
                    self.value(alias).is_some()
                        && self.uses_after(alias, boundary, owner, region, seen, aliases)
                } else {
                    self.after(reference.node_id(), boundary, owner, region, seen)
                }
            })
    }

    fn origin_precedes(&self, e: &Expression<'_>, value: &Expression<'_>) -> bool {
        let Expression::Identifier(id) = e.without_parentheses() else {
            return false;
        };
        let Some(reference) = id.reference_id.get() else {
            return false;
        };
        let Some(symbol) = self.symbol(e) else {
            return false;
        };
        let declaration = self.model.scoping.symbol_declaration(symbol);
        self.after(
            self.model.scoping.get_reference(reference).node_id(),
            value.span().end,
            self.model.owning_site(declaration).map(|s| s.node_id),
            self.guarded_origin(symbol),
            &mut BTreeSet::new(),
        )
    }

    fn slot(&self, e: &'a Expression<'a>, site: FunctionSite, depth: usize) -> Option<u32> {
        if depth > 16 {
            return None;
        }
        if let Some(slot) = self.model.incoming_slot(e) {
            return (slot.owner == site.node_id).then_some(slot.index);
        }
        let value = self.value(self.symbol(e)?)?;
        if !self.origin_precedes(e, value) {
            return None;
        }
        self.slot(value, site, depth + 1)
    }

    fn array(
        &self,
        e: &'a Expression<'a>,
        env: &Environment<'a>,
        depth: usize,
    ) -> Option<(SymbolId, &'a Expression<'a>)> {
        if depth > 16 {
            return None;
        }
        let e = self.substituted(e, env);
        let symbol = self.symbol(e)?;
        let value = self.value(symbol)?;
        if !self.origin_precedes(e, value) {
            return None;
        }
        if matches!(value.without_parentheses(), Expression::ArrayExpression(_)) {
            Some((symbol, value.without_parentheses()))
        } else {
            self.array(value, env, depth + 1)
        }
    }

    fn function(
        &self,
        e: &'a Expression<'a>,
        env: &Environment<'a>,
        depth: usize,
    ) -> Option<FunctionSite> {
        if depth > 16 {
            return None;
        }
        let e = self.substituted(e, env).without_parentheses();
        if let Some(symbol) = self.symbol(e) {
            let declaration = self.model.scoping.symbol_declaration(symbol);
            if let AstKind::Function(f) = self.model.nodes.kind(declaration) {
                return (self.model.binding_writes(symbol) == 0
                    && self.model.scoping.symbol_redeclarations(symbol).is_empty())
                .then(|| self.model.site_by_span(f.span))
                .flatten();
            }
            let value = self.value(symbol)?;
            if !self.origin_precedes(e, value) {
                return None;
            }
            return self.function(value, env, depth + 1);
        }
        if let Expression::CallExpression(call) = e {
            if let Some(result) = self.identity_holder(call) {
                return self.function(result, env, depth + 1);
            }
            let (local, result, _) = self.transport(call, env)?;
            return self.function(result, &local, depth + 1);
        }
        self.model.function_from_expression(e)
    }

    // A previously inserted anonymous identity holder. Recognizing this exact
    // inert shape makes naming idempotent without evaluating general factories.
    fn identity_holder(&self, call: &'a CallExpression<'a>) -> Option<&'a Expression<'a>> {
        let Expression::ArrowFunctionExpression(f) = call.callee.without_parentheses() else {
            return None;
        };
        if call.optional
            || f.r#async
            || f.expression
            || !call.arguments.is_empty()
            || !f.params.items.is_empty()
            || f.params.rest.is_some()
            || !f.body.directives.is_empty()
        {
            return None;
        }
        let [Statement::VariableDeclaration(v), Statement::ReturnStatement(r)] =
            f.body.statements.as_slice()
        else {
            return None;
        };
        if v.kind != VariableDeclarationKind::Const || v.declarations.len() != 1 {
            return None;
        }
        let declaration = &v.declarations[0];
        let BindingPattern::BindingIdentifier(id) = &declaration.id else {
            return None;
        };
        let symbol = id.symbol_id.get()?;
        if !id.name.starts_with("waveCollector")
            || self.model.binding_writes(symbol) != 0
            || self.symbol(r.argument.as_ref()?) != Some(symbol)
        {
            return None;
        }
        let init = declaration.init.as_ref()?;
        matches!(
            init.without_parentheses(),
            Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_)
        )
        .then_some(init)
    }

    // Resolve a transport callee without entering function-return transport.
    // In particular, a recursive factory or a call-valued alias cannot donate
    // a helper body (or recurse back into transport discovery).
    fn transport_callee(
        &self,
        e: &'a Expression<'a>,
        depth: usize,
    ) -> Option<FunctionSite> {
        if depth > 16 {
            return None;
        }
        if let Some(symbol) = self.symbol(e) {
            let declaration = self.model.scoping.symbol_declaration(symbol);
            if let AstKind::Function(f) = self.model.nodes.kind(declaration) {
                return (self.model.binding_writes(symbol) == 0
                    && self.model.scoping.symbol_redeclarations(symbol).is_empty()
                    && matches!(
                        self.model.nodes.parent_kind(declaration),
                        AstKind::FunctionBody(_) | AstKind::Program(_)
                    ))
                .then(|| self.model.site_by_span(f.span))
                .flatten();
            }
            let value = self.value(symbol)?;
            if !self.origin_precedes(e, value) {
                return None;
            }
            return self.transport_callee(value, depth + 1);
        }
        match e.without_parentheses() {
            Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_) => {
                self.model.function_from_expression(e.without_parentheses())
            }
            _ => None,
        }
    }

    /// The recovered collector-wave transport is a stable callable containing only
    /// parameter-indexed assignments and a final return. No arbitrary factory
    /// evaluation, branches, calls, globals, or guessed return ownership.
    fn transport(
        &self,
        call: &'a CallExpression<'a>,
        env: &Environment<'a>,
    ) -> Option<(
        Environment<'a>,
        &'a Expression<'a>,
        Vec<&'a AssignmentExpression<'a>>,
    )> {
        let site = self.transport_callee(&call.callee, 0)?;
        let named = self.symbol(&call.callee).is_some();
        let (params, body) = match self.model.nodes.kind(site.node_id) {
            AstKind::ArrowFunctionExpression(f) if !f.expression && !f.r#async => {
                (&f.params, &f.body)
            }
            AstKind::Function(f) if !f.r#async && !f.generator => {
                (&f.params, f.body.as_ref()?)
            }
            _ => return None,
        };
        if call.optional
            || !body.directives.is_empty()
            || params.rest.is_some()
            || params.items.len() != call.arguments.len()
            || call_arguments(call).len() != call.arguments.len()
            || body.statements.is_empty()
        {
            return None;
        }
        let mut local = env.clone();
        let mut parameters = BTreeSet::new();
        for (param, arg) in params.items.iter().zip(call_arguments(call)) {
            if param.initializer.is_some() {
                return None;
            }
            let BindingPattern::BindingIdentifier(id) = &param.pattern else {
                return None;
            };
            let symbol = id.symbol_id.get()?;
            if !parameters.insert(symbol) || self.model.binding_writes(symbol) != 0 {
                return None;
            }
            let actual = self.substituted(arg, env);
            let inert = match actual.without_parentheses() {
                Expression::Identifier(_) => self.array(actual, env, 0).is_some() || self.function(actual, env, 0).is_some(),
                Expression::NumericLiteral(_) | Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_) => true,
                Expression::CallExpression(c) => self.identity_holder(c).is_some(),
                _ => false,
            };
            if !inert { return None; }
            local.insert(symbol, actual);
        }
        let mut writes = Vec::new();
        let (last, preceding) = body.statements.split_last()?;
        for statement in preceding {
            let Statement::ExpressionStatement(s) = statement else {
                return None;
            };
            let Expression::AssignmentExpression(a) = &s.expression else {
                return None;
            };
            let AssignmentTarget::ComputedMemberExpression(member) = &a.left else {
                return None;
            };
            if a.operator != AssignmentOperator::Assign
                || (named
                    && [&member.object, &member.expression, &a.right]
                        .iter()
                        .any(|e| !self.symbol(e).is_some_and(|s| parameters.contains(&s))))
                || self.array(&member.object, &local, 0).is_none()
                || numeric_index(self.substituted(&member.expression, &local)).is_none()
                || self.function(&a.right, &local, 0).is_none()
            {
                return None;
            }
            writes.push(a.as_ref());
        }
        let Statement::ReturnStatement(result) = last else {
            return None;
        };
        let result = result.argument.as_ref()?;
        if named && !self.symbol(result).is_some_and(|s| parameters.contains(&s)) {
            return None;
        }
        // Return transport only; never execute an inner factory for its effects.
        self.function(self.substituted(result, &local), &local, 0)?;
        Some((local, result, writes))
    }

    fn consumer(
        &self,
        call: &'a CallExpression<'a>,
    ) -> Option<(SymbolId, &'a Expression<'a>, Span)> {
        let Expression::StaticMemberExpression(member) = &call.callee else {
            return None;
        };
        if call.optional
            || member.optional
            || member.property.name != "forEach"
            || call.arguments.len() != 1
        {
            return None;
        }
        let (symbol, array) = self.array(&member.object, &HashMap::new(), 0)?;
        Some((symbol, array, self.consumer_abi(call)?))
    }

    // Keep ABI ownership independent of registry origin resolution. A mutable
    // registry is resolved at its activation contexts, never via value().
    fn consumer_abi(&self, call: &'a CallExpression<'a>) -> Option<Span> {
        let Expression::StaticMemberExpression(member) = &call.callee else {
            return None;
        };
        if call.optional || member.optional || member.property.name != "forEach"
            || call.arguments.len() != 1 {
            return None;
        }
        let callback = self.function(*call_arguments(call).first()?, &HashMap::new(), 0)?;
        for invoke in self.calls.get(&callback.node_id).into_iter().flatten() {
            let args = call_arguments(invoke);
            if invoke.optional
                || !self.ordered_calls.contains(&invoke.span.start)
                || !self.site_reaches(callback, invoke.span)
                || args.len() < 6
                || args.len() != invoke.arguments.len()
                || self.slot(args[1], callback, 0) != Some(0)
            {
                continue;
            }
            let Some(coordinator) = self.function(&invoke.callee, &HashMap::new(), 0).or_else(|| {
                self.coordinator_before_registries(&invoke.callee, self.symbol(&member.object)?)
            }) else {
                continue;
            };
            let abi = self
                .calls
                .get(&coordinator.node_id)
                .into_iter()
                .flatten()
                .any(|body_call| {
                    !body_call.optional
                        && self.ordered_calls.contains(&body_call.span.start)
                        && self.site_reaches(coordinator, body_call.span)
                        && body_call.arguments.len() == 6
                        && call_arguments(body_call).len() == 6
                        && self.slot(&body_call.callee, coordinator, 0) == Some(1)
                });
            if abi {
                return Some(invoke.span);
            }
        }
        None
    }

    // A mutable constructor alias need not invalidate an unrelated immutable
    // coordinator. For the finite replacement lane only, its straight-line
    // owner initializer dominates every literal registry origin. An active
    // consumption before those origins is still rejected by ReplacementFlow.
    fn coordinator_before_registries(&self, e: &'a Expression<'a>, root: SymbolId) -> Option<FunctionSite> {
        let declaration = self.model.scoping.symbol_declaration(root);
        let AstKind::VariableDeclarator(var) = self.model.nodes.kind(declaration) else { return None; };
        if var.init.is_some() || !self.model.scoping.symbol_redeclarations(root).is_empty() { return None; }
        let owner = self.model.owning_site(declaration)?.node_id;
        let mut earliest = u32::MAX;
        let mut writes = 0;
        for reference in self.model.scoping.get_resolved_references(root).filter(|r| r.is_write()) {
            let node = self.model.nodes.parent_id(reference.node_id());
            let AstKind::AssignmentExpression(a) = self.model.nodes.kind(node) else { return None; };
            if reference.is_read() || a.operator != AssignmentOperator::Assign
                || self.model.owning_site(node)?.node_id != owner
                || !matches!(a.right.without_parentheses(), Expression::ArrayExpression(_)) {
                return None;
            }
            earliest = earliest.min(a.span.start);
            writes += 1;
        }
        if !(2..=16).contains(&writes) { return None; }
        self.coordinator_owner_value(e, owner, earliest, 0)
    }

    fn coordinator_owner_value(&self, e: &'a Expression<'a>, owner: NodeId, before: u32, depth: usize) -> Option<FunctionSite> {
        if depth >= 16 { return None; }
        if let Some(symbol) = self.symbol(e) {
            if !self.model.scoping.symbol_redeclarations(symbol).is_empty() { return None; }
            let declaration = self.model.scoping.symbol_declaration(symbol);
            let list = match self.model.nodes.kind(declaration) {
                AstKind::Function(f) if self.model.binding_writes(symbol) == 0 => {
                    let body = self.model.nodes.parent_id(declaration);
                    return (matches!(self.model.nodes.kind(body), AstKind::FunctionBody(_))
                        && self.model.nodes.parent_id(body) == owner)
                        .then(|| self.model.site_by_span(f.span)).flatten();
                }
                AstKind::VariableDeclarator(_) => self.model.nodes.parent_id(self.model.nodes.parent_id(declaration)),
                _ => return None,
            };
            if !matches!(self.model.nodes.kind(list), AstKind::FunctionBody(_))
                || self.model.nodes.parent_id(list) != owner || self.guarded_origin(symbol).is_some() {
                return None;
            }
            let value = self.value(symbol)?;
            if value.span().end >= before { return None; }
            return self.coordinator_owner_value(value, owner, value.span().start, depth + 1);
        }
        match e.without_parentheses() {
            Expression::FunctionExpression(f) if !f.r#async && !f.generator => self.model.function_from_expression(e),
            Expression::ArrowFunctionExpression(f) if !f.r#async => self.model.function_from_expression(e),
            _ => None,
        }
    }
}

pub(crate) fn discover(model: &Model<'_, '_>) -> Vec<GroupedRegistration> {
    let mut calls: HashMap<NodeId, Vec<_>> = HashMap::new();
    for node in model.nodes.iter() {
        if let AstKind::CallExpression(call) = node.kind() {
            if let Some(owner) = model.owning_site(node.id()) {
                calls.entry(owner.node_id).or_default().push(call);
            }
        }
    }
    let mut d = Discovery { model, calls, ordered_calls: BTreeSet::new() };
    for node in model.nodes.iter() {
        if let AstKind::CallExpression(call) = node.kind() {
            if d.activation_ordered(node.id()) { d.ordered_calls.insert(call.span.start); }
        }
    }
    let empty = HashMap::new();
    let mut consumers = BTreeMap::new();
    let mut replacements = BTreeMap::new();
    for node in model.nodes.iter() {
        if let AstKind::CallExpression(call) = node.kind() {
            if let Some((symbol, array, invocation)) = d.consumer(call) {
                consumers
                    .entry(symbol)
                    .or_insert((array, Vec::new()))
                    .1
                    .push((call.span, invocation, node.id()));
            } else if let Some(invocation) = d.consumer_abi(call) {
                if let Expression::StaticMemberExpression(member) = &call.callee {
                    if let Some(symbol) = d.symbol(&member.object) {
                        replacements.entry(symbol).or_insert_with(Vec::new)
                            .push((call.span, invocation, node.id()));
                    }
                }
            }
        }
    }
    let mut registrations = Vec::new();
    for (root, (array, consumers)) in consumers {
        let mut slots = BTreeMap::new();
        if let Some(elements) = array_elements(array) {
            for (index, element) in elements.iter().enumerate() {
                slots.insert(
                    index,
                    (
                        d.function(element, &empty, 0),
                        vec![element.span().into()],
                        Vec::new(),
                    ),
                );
            }
        }
        let mut errors = Vec::new();
        let mut boundary = array.span().end;
        let region = d.guarded_origin(root);
        let root_list = region.unwrap_or_else(|| {
            model.nodes.parent_id(
                model.nodes.parent_id(model.scoping.symbol_declaration(root)),
            )
        });
        let mut allowed_transports = BTreeMap::new();
        let mut writes = Vec::new();
        // One AST pass for writes, including the tiny parameter transport helper.
        for node in model.nodes.iter() {
            if let AstKind::CallExpression(call) = node.kind() {
                if let Some((env, _, assignments)) = d.transport(call, &empty) {
                    let mut used = false;
                    for a in assignments {
                        if let AssignmentTarget::ComputedMemberExpression(m) = &a.left {
                            if d.array(&m.object, &env, 0).is_some_and(|(s, _)| s == root) {
                                writes.push((a, env.clone(), call.span));
                                used = true;
                            }
                        }
                    }
                    if used {
                        // A transport call contributes writes only when its
                        // evaluation is a straight-line statement in the array
                        // owner's list, not an uncalled nested donor or branch.
                        let mut statement = model.nodes.parent_id(node.id());
                        match model.nodes.kind(statement) {
                            AstKind::AssignmentExpression(a) if a.right.span() == call.span => {
                                statement = model.nodes.parent_id(statement);
                            }
                            AstKind::VariableDeclarator(v) if v.init.as_ref().is_some_and(|e| e.span() == call.span) => {
                                statement = model.nodes.parent_id(statement);
                            }
                            _ => {}
                        }
                        if !matches!(model.nodes.kind(statement), AstKind::ExpressionStatement(_) | AstKind::VariableDeclaration(_))
                            || model.nodes.parent_id(statement) != root_list {
                            errors.push("registry transport is not evaluated in a straight-line owner statement".into());
                        }
                        allowed_transports.insert(call.span.start, call.span);
                    }
                }
            }
            let AstKind::AssignmentExpression(a) = node.kind() else {
                continue;
            };
            let AssignmentTarget::ComputedMemberExpression(m) = &a.left else {
                continue;
            };
            if !d
                .array(&m.object, &empty, 0)
                .is_some_and(|(s, _)| s == root)
            {
                continue;
            }
            let statement = model.nodes.parent_id(node.id());
            let list = model.nodes.parent_id(statement);
            if a.operator != AssignmentOperator::Assign
                || !matches!(model.nodes.kind(statement), AstKind::ExpressionStatement(_))
                || list != root_list
            {
                errors.push(
                    "registry population is not a straight-line indexed assignment".to_string(),
                );
            }
            writes.push((a, empty.clone(), a.span));
        }
        writes.sort_by_key(|(_, _, span)| span.start);
        for (a, env, span) in writes {
            boundary = boundary.max(span.end);
            let AssignmentTarget::ComputedMemberExpression(m) = &a.left else {
                continue;
            };
            let Some(index) = numeric_index(d.substituted(&m.expression, &env)) else {
                errors.push("registry has a nonconstant index write".into());
                continue;
            };
            let index = index as usize;
            if slots.contains_key(&index) {
                errors.push(format!("registry slot {index} is written more than once"));
            }
            slots.insert(
                index,
                (d.function(&a.right, &env, 0), vec![span.into()], Vec::new()),
            );
        }
        let root_owner = model
            .owning_site(model.scoping.symbol_declaration(root))
            .map(|s| s.node_id);
        for (_, _, node) in &consumers {
            if !d.after(*node, boundary, root_owner, region, &mut BTreeSet::new()) {
                errors.push(
                    "registry population does not precede every active consumption/escape".into(),
                );
            }
        }
        // Reject mutation/escape through every resolved alias of this array.
        for symbol in model.scoping.symbol_ids() {
            let Some(value) = d.value(symbol) else {
                continue;
            };
            let is_root = symbol == root;
            if !is_root && !d.array(value, &empty, 0).is_some_and(|(s, _)| s == root) {
                continue;
            }
            for reference in model
                .scoping
                .get_resolved_references(symbol)
                .filter(|r| r.is_read())
            {
                if region.is_some()
                    && !d.after(reference.node_id(), array.span().end,
                        root_owner, region, &mut BTreeSet::new())
                {
                    errors.push("guarded registry read is not dominated by its own branch origin".into());
                }
                let parent = model.nodes.parent_kind(reference.node_id());
                let allowed = match parent {
                    AstKind::ComputedMemberExpression(m) => {
                        let member_node = model.nodes.parent_id(reference.node_id());
                        !matches!(
                            model.nodes.parent_kind(member_node),
                            AstKind::UpdateExpression(_) | AstKind::UnaryExpression(_)
                        ) && numeric_index(&m.expression).is_some()
                    }
                    AstKind::StaticMemberExpression(m) => {
                        m.property.name == "forEach"
                            && consumers.iter().any(|(c, _, _)| c.start == m.span.start)
                    }
                    AstKind::VariableDeclarator(v) => {
                        matches!(&v.id, BindingPattern::BindingIdentifier(id)
                            if id.symbol_id.get().is_some_and(|alias| d.value(alias).is_some()))
                            && v.init.as_ref().is_some_and(|e| d.symbol(e) == Some(symbol))
                    }
                    AstKind::AssignmentExpression(a) => match &a.left {
                        AssignmentTarget::AssignmentTargetIdentifier(id) => {
                            crate::analysis::identifier_symbol(id, model.scoping)
                                .is_some_and(|alias| d.value(alias).is_some())
                                && d.symbol(&a.right) == Some(symbol)
                        }
                        _ => false,
                    },
                    AstKind::CallExpression(_) => {
                        let span = model.nodes.get_node(reference.node_id()).span();
                        allowed_transports.values().any(|call| call.start <= span.start && span.end <= call.end)
                    }
                    _ => false,
                };
                if !allowed {
                    errors.push(format!(
                        "registry alias {} escapes or has an unsupported use at {}",
                        model.binding_ref(symbol).name,
                        model.nodes.get_node(reference.node_id()).span().start
                    ));
                }
            }
        }
        for (slot, (function, mut evidence, mut unresolved)) in slots {
            evidence.push(array.span().into());
            evidence.extend(
                consumers
                    .iter()
                    .flat_map(|(call, invoke, _)| [SourceSpan::from(*call), SourceSpan::from(*invoke)]),
            );
            unresolved.extend(errors.iter().cloned());
            if function.is_none() {
                unresolved.push("registry value has no unique stable function origin".into());
            }
            registrations.push(GroupedRegistration {
                registry: model.binding_ref(root),
                slot,
                function: function.map(|site| model.function_ref(site)),
                evidence,
                unresolved,
            });
        }
    }
    for (root, consumers) in replacements {
        if let Some(rows) = d.replacement_registrations(root, &consumers) {
            registrations.extend(rows);
        }
    }
    registrations
}

// A finite projection of the owner's statements onto registry replacements and
// callable aliases. Expressions, conditions, properties and producer bodies are
// never evaluated. A state is one reaching array origin, the reaching callable
// identities of the explicitly followed aliases, and escaped consumer contexts.
// Keeping these facts in the SAME state prevents sibling branches donating an
// origin or a callable to each other.
#[derive(Clone)]
struct Consumption {
    at: Span,
    deferred: bool,
    predicates: Vec<(SymbolId, u32)>,
    captured: Vec<(SymbolId, u32)>,
    capture_pending: bool,
    consumer: usize,
}

type ReplacementState = (Option<usize>, Vec<Option<u32>>, BTreeSet<(usize, bool)>);

struct ReplacementFlow<'d, 'm, 's, 'a> {
    discovery: &'d Discovery<'m, 's, 'a>,
    origins: Vec<(Span, &'a Expression<'a>)>,
    aliases: Vec<SymbolId>,
    writes: BTreeMap<u32, Vec<(usize, &'a Expression<'a>)>>,
    events: Vec<Consumption>,
    reached: BTreeMap<usize, BTreeSet<usize>>,
    exceptional: Vec<Vec<ReplacementState>>,
    failed: bool,
}

impl<'m, 's, 'a> Discovery<'m, 's, 'a> {
    fn site_reaches(&self, site: FunctionSite, at: Span) -> bool {
        let body = match self.model.nodes.kind(site.node_id) {
            AstKind::Function(f) => f.body.as_ref().map(|b| b.statements.as_slice()),
            AstKind::ArrowFunctionExpression(f) => Some(f.body.statements.as_slice()),
            _ => None,
        };
        body.is_some_and(|body| replacement_reaches(self, body, at).1)
    }

    fn site_activation_supported(&self, site: FunctionSite, at: Span) -> bool {
        let body = match self.model.nodes.kind(site.node_id) {
            AstKind::Function(f) => f.body.as_ref().map(|b| b.statements.as_slice()),
            AstKind::ArrowFunctionExpression(f) => Some(f.body.statements.as_slice()),
            _ => None,
        };
        body.is_some_and(|body| body.iter().all(|s| replacement_control_supported(s, at))
            && body.iter().any(|s| replacement_activation_supported(s, at)))
    }

    fn opaque_loop_completes(&self, statement: &Statement<'_>) -> bool {
        let (body, no_test_exit, must_run) = match statement {
            Statement::ForStatement(s) => (&s.body, s.test.as_ref().is_none_or(|t|
                matches!(t.without_parentheses(), Expression::BooleanLiteral(b) if b.value)), false),
            Statement::WhileStatement(s) => (&s.body,
                matches!(s.test.without_parentheses(), Expression::BooleanLiteral(b) if b.value), false),
            Statement::DoWhileStatement(s) => (&s.body,
                matches!(s.test.without_parentheses(), Expression::BooleanLiteral(b) if b.value), true),
            _ => return true,
        };
        if !no_test_exit && !must_run { return true; }
        let loop_span = statement.span();
        let breaks = self.model.nodes.iter().any(|node| {
            let AstKind::BreakStatement(b) = node.kind() else { return false; };
            if b.label.is_some() || b.span.start < body.span().start || b.span.end > body.span().end {
                return false;
            }
            let nearest = self.model.nodes.ancestor_ids(node.id()).find(|id| matches!(self.model.nodes.kind(*id),
                AstKind::ForStatement(_) | AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
                | AstKind::WhileStatement(_) | AstKind::DoWhileStatement(_) | AstKind::SwitchStatement(_)
                | AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)));
            nearest.is_some_and(|id| self.model.nodes.get_node(id).span() == loop_span)
                && replacement_statement_reaches(self, body, b.span).1
        });
        if no_test_exit { breaks }
        else { breaks || replacement_statement_reaches(self, body, Span::new(0, 0)).0 }
    }

    fn inert_expression(&self, expression: &Expression<'_>) -> bool {
        match expression.without_parentheses() {
            Expression::NumericLiteral(_) | Expression::StringLiteral(_)
            | Expression::BooleanLiteral(_) | Expression::NullLiteral(_)
            | Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => true,
            Expression::Identifier(_) => self.symbol(expression).is_some_and(|s|
                matches!(self.model.nodes.kind(self.model.scoping.symbol_declaration(s)), AstKind::Function(_))),
            Expression::ObjectExpression(object) => object.properties.iter().all(|p| {
                let ObjectPropertyKind::ObjectProperty(p) = p else { return false; };
                !p.computed && !p.method && p.kind == PropertyKind::Init && self.inert_expression(&p.value)
            }),
            _ => false,
        }
    }

    fn statement_may_throw(&self, statement: &Statement<'_>) -> bool {
        match statement {
            Statement::FunctionDeclaration(_) | Statement::EmptyStatement(_) => false,
            Statement::ExpressionStatement(s) => !self.inert_expression(&s.expression),
            Statement::VariableDeclaration(v) => v.declarations.iter()
                .any(|v| v.init.as_ref().is_some_and(|e| !self.inert_expression(e))),
            Statement::ReturnStatement(r) => r.argument.as_ref().is_some_and(|e| !self.inert_expression(e)),
            Statement::BlockStatement(b) => b.body.iter().any(|s| self.statement_may_throw(s)),
            Statement::IfStatement(b) => !self.inert_expression(&b.test)
                || self.statement_may_throw(&b.consequent)
                || b.alternate.as_ref().is_some_and(|s| self.statement_may_throw(s)),
            _ => true,
        }
    }

    // A span somewhere inside a statement is not an execution-order proof.
    // Follow only always-evaluated expression wrappers up to the statement.
    fn activation_ordered(&self, node: NodeId) -> bool {
        let mut current = node;
        for _ in 0..128 {
            let parent = self.model.nodes.parent_id(current);
            match self.model.nodes.kind(parent) {
                AstKind::ExpressionStatement(_) | AstKind::VariableDeclaration(_)
                | AstKind::ReturnStatement(_) | AstKind::ThrowStatement(_) => return true,
                AstKind::IfStatement(branch) => {
                    let at = self.model.nodes.get_node(current).span();
                    return branch.test.span().start <= at.start && at.end <= branch.test.span().end;
                }
                AstKind::CallExpression(call) if !call.optional => {}
                AstKind::AssignmentExpression(a) if a.operator == AssignmentOperator::Assign
                    && a.right.span() == self.model.nodes.get_node(current).span()
                    && matches!(a.left, AssignmentTarget::AssignmentTargetIdentifier(_)) => {}
                AstKind::NewExpression(_) | AstKind::ParenthesizedExpression(_)
                | AstKind::ObjectProperty(_) | AstKind::ObjectExpression(_)
                | AstKind::VariableDeclarator(_) => {}
                _ => return false,
            }
            current = parent;
        }
        false
    }

    // Only a statement directly in the owner, or inside its blocks/if arms.
    // Loops, finally, switch, nested/dormant writers and expression writes
    // require a different proof and cannot contribute here.
    fn replacement_statement(&self, node: NodeId, owner: NodeId) -> Option<Span> {
        let statement = self.model.nodes.parent_id(node);
        if !matches!(self.model.nodes.kind(statement),
            AstKind::ExpressionStatement(_) | AstKind::VariableDeclaration(_)) {
            return None;
        }
        let mut current = statement;
        for _ in 0..128 {
            let parent = self.model.nodes.parent_id(current);
            if parent == owner {
                return Some(self.model.nodes.get_node(statement).span());
            }
            if !matches!(self.model.nodes.kind(parent),
                AstKind::BlockStatement(_) | AstKind::IfStatement(_) | AstKind::FunctionBody(_)
                | AstKind::CatchClause(_))
                && !matches!(self.model.nodes.kind(parent), AstKind::TryStatement(t) if t.finalizer.is_none()) {
                return None;
            }
            current = parent;
        }
        None
    }

    fn replacement_alias(
        &self,
        reference: NodeId,
        symbol: SymbolId,
    ) -> Option<SymbolId> {
        match self.model.nodes.parent_kind(reference) {
            AstKind::VariableDeclarator(v)
                if v.init.as_ref().is_some_and(|e| self.symbol(e) == Some(symbol)) => {
                match &v.id {
                    BindingPattern::BindingIdentifier(id) => id.symbol_id.get(),
                    _ => None,
                }
            }
            AstKind::AssignmentExpression(a)
                if a.operator == AssignmentOperator::Assign && self.symbol(&a.right) == Some(symbol) => {
                match &a.left {
                    AssignmentTarget::AssignmentTargetIdentifier(id) =>
                        crate::analysis::identifier_symbol(id, self.model.scoping),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn consumption_uses(
        &self,
        symbol: SymbolId,
        identity: u32,
        owner: NodeId,
        context: &Consumption,
        aliases: &mut BTreeSet<SymbolId>,
        trail: &mut BTreeSet<(u32, u32)>,
        events: &mut Vec<Consumption>,
    ) -> Option<()> {
        if trail.len() >= 32 || events.len() >= 128 || aliases.len() >= 32 {
            return None;
        }
        // A cycle is an unknown activation context, not a successful proof.
        if !trail.insert((symbol.index() as u32, identity)) {
            return None;
        }
        for reference in self.model.scoping.get_resolved_references(symbol).filter(|r| r.is_read()) {
            if let Some(alias) = self.replacement_alias(reference.node_id(), symbol) {
                aliases.insert(alias);
                let mut next = context.clone();
                // Alias transport snapshots the callable. Its earlier alias
                // need no longer hold the same value when the snapshot is used.
                next.predicates.retain(|(s, _)| *s != symbol);
                next.captured.retain(|(s, _)| *s != symbol);
                if next.deferred { next.captured.push((alias, identity)); }
                else { next.predicates.push((alias, identity)); }
                self.consumption_uses(alias, identity, owner, &next, aliases, trail, events)?;
                continue;
            }
            let direct = match self.model.nodes.parent_kind(reference.node_id()) {
                AstKind::CallExpression(call) => !call.optional && self.symbol(&call.callee) == Some(symbol)
                    && call.callee.span() == self.model.nodes.get_node(reference.node_id()).span(),
                AstKind::NewExpression(call) => self.symbol(&call.callee) == Some(symbol)
                    && call.callee.span() == self.model.nodes.get_node(reference.node_id()).span(),
                _ => false,
            };
            if !direct && !matches!(self.model.nodes.parent_kind(reference.node_id()),
                AstKind::CallExpression(_) | AstKind::NewExpression(_)
                | AstKind::ReturnStatement(_) | AstKind::ObjectProperty(_)) {
                return None;
            }
            if matches!(self.model.nodes.parent_kind(reference.node_id()), AstKind::ObjectProperty(_)) {
                let property = self.model.nodes.parent_id(reference.node_id());
                let object = self.model.nodes.parent_id(property);
                // Merely putting a function in a dormant property bag is not
                // an activation. Only a directly returned/passed literal is
                // an escape here; no property lookup is interpreted.
                if !matches!(self.model.nodes.parent_kind(object),
                    AstKind::ReturnStatement(_) | AstKind::CallExpression(_) | AstKind::NewExpression(_)) {
                    return None;
                }
            }
            let mut next = context.clone();
            if !direct {
                next.predicates.retain(|(s, _)| *s != symbol);
                if aliases.contains(&symbol) && !next.captured.contains(&(symbol, identity)) {
                    next.captured.push((symbol, identity));
                }
            }
            next.deferred |= !direct;
            self.consumption_context(reference.node_id(), owner, &next, aliases, trail, events)?;
        }
        trail.remove(&(symbol.index() as u32, identity));
        Some(())
    }

    fn consumption_context(
        &self,
        node: NodeId,
        owner: NodeId,
        context: &Consumption,
        aliases: &mut BTreeSet<SymbolId>,
        trail: &mut BTreeSet<(u32, u32)>,
        events: &mut Vec<Consumption>,
    ) -> Option<()> {
        let site = self.model.owning_site(node)?;
        if !self.activation_ordered(node) { return None; }
        if !self.site_activation_supported(site, self.model.nodes.get_node(node).span()) {
            return None;
        }
        if !self.site_reaches(site, self.model.nodes.get_node(node).span()) {
            // An uncalled/unreachable nested donor supplies no activation.
            return Some(());
        }
        if site.node_id == owner {
            // One capture boundary is the recovered module/consumer family.
            // Multiple independently timed captures need a separate proof.
            if context.captured.len() > 1 { return None; }
            let mut event = context.clone();
            event.at = self.model.nodes.get_node(node).span();
            events.push(event);
            return (events.len() <= 128).then_some(());
        }
        let mut pending = context.clone();
        if !pending.captured.is_empty() { pending.capture_pending = true; }
        let context = &pending;
        // Only lexically contained callables may carry a context back to this
        // registry owner. Do not walk out through a parent/module caller.
        let owner_span = self.model.nodes.get_node(owner).span();
        if site.span.start < owner_span.start || site.span.end > owner_span.end {
            return None;
        }
        if matches!(self.model.nodes.kind(site.node_id), AstKind::Function(f) if f.r#async || f.generator)
            || matches!(self.model.nodes.kind(site.node_id), AstKind::ArrowFunctionExpression(f) if f.r#async) {
            return None;
        }
        match self.model.nodes.kind(site.node_id) {
            AstKind::Function(f) if f.r#type == FunctionType::FunctionDeclaration => {
                let symbol = site.binding_symbol?;
                if f.r#async || f.generator || self.model.binding_writes(symbol) != 0
                    || !self.model.scoping.symbol_redeclarations(symbol).is_empty() {
                    return None;
                }
                self.consumption_uses(symbol, site.span.start, owner, context, aliases, trail, events)
            }
            AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => {
                // Function expressions may be passed or returned and invoked
                // later. Creation alone is a lower bound, never a guessed call.
                // Follow a lexical callable binding if one exists.
                if let Some(symbol) = site.binding_symbol {
                    aliases.insert(symbol);
                    let mut next = context.clone();
                    next.predicates.push((symbol, site.span.start));
                    self.consumption_uses(symbol, site.span.start, owner, &next, aliases, trail, events)
                } else {
                    let mut next = context.clone();
                    let direct = matches!(self.model.nodes.parent_kind(site.node_id),
                        AstKind::CallExpression(call) if !call.optional && call.callee.span() == site.span);
                    if !direct && !matches!(self.model.nodes.parent_kind(site.node_id),
                        AstKind::CallExpression(_) | AstKind::NewExpression(_) | AstKind::ReturnStatement(_)) {
                        return None;
                    }
                    next.deferred |= !direct;
                    self.consumption_context(site.node_id, owner, &next, aliases, trail, events)
                }
            }
            _ => None,
        }
    }

    fn replacement_registrations(
        &self,
        root: SymbolId,
        consumers: &[(Span, Span, NodeId)],
    ) -> Option<Vec<GroupedRegistration>> {
        let declaration = self.model.scoping.symbol_declaration(root);
        let AstKind::VariableDeclarator(var) = self.model.nodes.kind(declaration) else { return None; };
        // The single guarded-origin lane has its own owner/proof. This lane
        // handles only bare locals replaced by at least two literal arrays.
        if var.init.is_some() || !self.model.scoping.symbol_redeclarations(root).is_empty() {
            return None;
        }
        let owner = self.model.owning_site(declaration)?.node_id;
        let list = self.model.nodes.parent_id(self.model.nodes.parent_id(declaration));
        if self.model.nodes.parent_id(list) != owner
            || !matches!(self.model.nodes.kind(list), AstKind::FunctionBody(_)) {
            return None;
        }
        let mut origins = Vec::new();
        for reference in self.model.scoping.get_resolved_references(root) {
            if reference.is_write() {
                let node = self.model.nodes.parent_id(reference.node_id());
                let AstKind::AssignmentExpression(a) = self.model.nodes.kind(node) else { return None; };
                if reference.is_read() || a.operator != AssignmentOperator::Assign
                    || self.model.owning_site(node)?.node_id != owner
                    || !matches!(a.right.without_parentheses(), Expression::ArrayExpression(_)) {
                    return None;
                }
                let statement = self.replacement_statement(node, owner)?;
                if statement.start < var.span.end { return None; }
                let array = a.right.without_parentheses();
                for element in array_elements(array)? {
                    if !matches!(element.without_parentheses(), Expression::Identifier(_)
                        | Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_)) {
                        return None;
                    }
                    self.function(element, &HashMap::new(), 0)?;
                }
                origins.push((statement, array));
            } else if reference.is_read() {
                // No property-bag/alias mutation proof is borrowed from the
                // old resolver. Every read must be this binding's ABI consumer.
                let member = self.model.nodes.parent_id(reference.node_id());
                let call = self.model.nodes.parent_id(member);
                if !consumers.iter().any(|(_, _, n)| *n == call) { return None; }
            }
        }
        if !(2..=16).contains(&origins.len()) { return None; }
        origins.sort_by_key(|(s, _)| s.start);
        let mut events = Vec::new();
        let mut aliases = BTreeSet::new();
        for (consumer, (_, _, node)) in consumers.iter().enumerate() {
            let context_ok = self.consumption_context(*node, owner, &Consumption {
                at: Span::new(0, 0), deferred: false, predicates: Vec::new(),
                captured: Vec::new(), capture_pending: false, consumer,
            }, &mut aliases, &mut BTreeSet::new(), &mut events);
            if context_ok.is_none() { return None; }
        }
        if events.is_empty() { return None; }
        let aliases: Vec<_> = aliases.into_iter().collect();
        let callable_value = |value: &Expression<'_>| match value.without_parentheses() {
            Expression::FunctionExpression(f) => !f.r#async && !f.generator,
            Expression::ArrowFunctionExpression(f) => !f.r#async,
            Expression::Identifier(_) => self.symbol(value).is_some_and(|symbol| {
                aliases.contains(&symbol) || matches!(
                    self.model.nodes.kind(self.model.scoping.symbol_declaration(symbol)),
                    AstKind::Function(f) if !f.r#async && !f.generator
                        && self.model.binding_writes(symbol) == 0
                        && self.model.scoping.symbol_redeclarations(symbol).is_empty())
            }),
            _ => false,
        };
        let mut writes: BTreeMap<u32, Vec<_>> = BTreeMap::new();
        for (index, symbol) in aliases.iter().enumerate() {
            if !self.model.scoping.symbol_redeclarations(*symbol).is_empty() { return None; }
            let declaration = self.model.scoping.symbol_declaration(*symbol);
            let AstKind::VariableDeclarator(v) = self.model.nodes.kind(declaration) else { return None; };
            if self.model.owning_site(declaration)?.node_id != owner { return None; }
            let list = self.model.nodes.parent_id(self.model.nodes.parent_id(declaration));
            if self.model.nodes.parent_id(list) != owner
                || !matches!(self.model.nodes.kind(list), AstKind::FunctionBody(_)) {
                return None;
            }
            if let Some(value) = &v.init {
                if !callable_value(value) { return None; }
                let AstKind::VariableDeclaration(list) = self.model.nodes.parent_kind(declaration) else { return None; };
                if list.declarations.len() != 1 { return None; }
                let statement = self.replacement_statement(declaration, owner)?;
                writes.entry(statement.start).or_default().push((index, value));
            }
            let AstKind::VariableDeclaration(list) = self.model.nodes.parent_kind(declaration) else { return None; };
            if list.kind == VariableDeclarationKind::Const && self.model.binding_writes(*symbol) != 0 {
                return None;
            }
            for reference in self.model.scoping.get_resolved_references(*symbol).filter(|r| r.is_write()) {
                let node = self.model.nodes.parent_id(reference.node_id());
                let AstKind::AssignmentExpression(a) = self.model.nodes.kind(node) else { return None; };
                if a.operator != AssignmentOperator::Assign || reference.is_read()
                    || self.model.owning_site(node)?.node_id != owner
                    || !callable_value(&a.right) { return None; }
                if a.span.start < v.span.end { return None; }
                writes.entry(self.replacement_statement(node, owner)?.start).or_default().push((index, &a.right));
            }
        }
        for (start, assignments) in &writes {
            for (_, value) in assignments {
                if let Some(symbol) = self.symbol(value).filter(|s| aliases.contains(s)) {
                    let declaration = self.model.scoping.symbol_declaration(symbol);
                    if self.model.nodes.get_node(declaration).span().end >= *start { return None; }
                }
            }
        }
        let mut flow = ReplacementFlow {
            discovery: self, origins, aliases, writes, events,
            reached: BTreeMap::new(), exceptional: Vec::new(), failed: false,
        };
        let body = match self.model.nodes.kind(owner) {
            AstKind::Function(f) => &f.body.as_ref()?.statements,
            AstKind::ArrowFunctionExpression(f) if !f.expression => &f.body.statements,
            _ => return None,
        };
        // Validate syntax independently of liveness. An event behind an
        // unconditional return is dormant, whereas one hidden in a loop/try
        // needs an unsupported ordering proof and must refuse this lane.
        if !flow.events.iter().all(|e| body.iter().any(|s| replacement_activation_supported(s, e.at))) {
            return None;
        }
        flow.statements(body, vec![(None, vec![None; flow.aliases.len()], BTreeSet::new())]);
        if flow.failed || flow.reached.is_empty() { return None; }
        let mut rows = Vec::new();
        for (origin, contexts) in &flow.reached {
            let (statement, array) = flow.origins[*origin];
            for (slot, element) in array_elements(array)?.iter().enumerate() {
                let function = self.function(element, &HashMap::new(), 0)?;
                let mut evidence = vec![statement.into(), array.span().into(), element.span().into()];
                for event in contexts {
                    let context = &flow.events[*event];
                    let (call, invocation, _) = consumers[context.consumer];
                    evidence.extend([SourceSpan::from(context.at), SourceSpan::from(call), SourceSpan::from(invocation)]);
                }
                rows.push(GroupedRegistration {
                    registry: self.model.binding_ref(root), slot,
                    function: Some(self.model.function_ref(function)), evidence, unresolved: Vec::new(),
                });
            }
        }
        Some(rows)
    }
}

impl<'d, 'm, 's, 'a> ReplacementFlow<'d, 'm, 's, 'a> {
    fn inert(&self, expression: &Expression<'_>) -> bool {
        self.discovery.inert_expression(expression)
    }

    fn may_throw(&self, statement: &Statement<'_>) -> bool {
        match statement {
            Statement::FunctionDeclaration(_) | Statement::EmptyStatement(_)
            | Statement::BlockStatement(_) | Statement::IfStatement(_) | Statement::TryStatement(_) => false,
            Statement::ExpressionStatement(_) | Statement::VariableDeclaration(_)
                if self.origins.iter().any(|(s, _)| *s == statement.span())
                    || self.writes.contains_key(&statement.span().start) => false,
            Statement::ReturnStatement(returned) => returned.argument.as_ref().is_some_and(|e| !self.inert(e)),
            _ => self.discovery.statement_may_throw(statement),
        }
    }

    fn exceptional(&mut self, states: &[ReplacementState]) {
        if let Some(exceptional) = self.exceptional.last_mut() {
            exceptional.extend(states.iter().cloned());
            *exceptional = exceptional.drain(..).collect::<BTreeSet<_>>().into_iter().collect();
            if exceptional.len() > 128 { self.failed = true; }
        }
    }

    fn callable(&self, expression: &Expression<'_>, state: &ReplacementState) -> Option<u32> {
        let symbol = self.discovery.symbol(expression)?;
        if let Some(index) = self.aliases.iter().position(|s| *s == symbol) {
            return state.1[index];
        }
        let declaration = self.discovery.model.scoping.symbol_declaration(symbol);
        match self.discovery.model.nodes.kind(declaration) {
            AstKind::Function(f) if self.discovery.model.binding_writes(symbol) == 0 => Some(f.span.start),
            _ => None,
        }
    }

    fn captured(&self, event: usize, state: &ReplacementState) -> bool {
        self.events[event].captured.iter().all(|(symbol, identity)| {
            self.aliases.iter().position(|s| s == symbol).is_some_and(|i| state.1[i] == Some(*identity))
        })
    }

    fn observe(&mut self, event: usize, state: &ReplacementState) {
        if !self.events[event].predicates.iter().all(|(symbol, identity)| {
            self.aliases.iter().position(|s| s == symbol)
                .is_some_and(|i| state.1[i] == Some(*identity))
        }) { return; }
        if let Some(origin) = state.0 {
            self.reached.entry(origin).or_default().insert(event);
        } else {
            self.failed = true;
        }
    }

    fn active(&mut self, state: &mut ReplacementState) {
        for (event, captured) in state.2.clone() {
            if captured || self.captured(event, state) {
                state.2.insert((event, true));
                self.observe(event, state);
            }
        }
    }

    fn markers(&mut self, span: Span, states: &mut [ReplacementState]) {
        for event in 0..self.events.len() {
            let at = self.events[event].at;
            if span.start <= at.start && at.end <= span.end {
                for state in states.iter_mut() {
                    let captured = self.captured(event, state);
                    if self.events[event].deferred {
                        if captured { state.2.insert((event, true)); }
                        if self.events[event].capture_pending { state.2.insert((event, false)); }
                    }
                    if captured { self.observe(event, state); }
                }
            }
        }
    }

    fn statements(&mut self, statements: &[Statement<'a>], mut states: Vec<ReplacementState>) -> Vec<ReplacementState> {
        for statement in statements {
            if states.is_empty() || self.failed { break; }
            // An opaque expression can throw. Keep whole reaching states for
            // the nearest catch instead of donating only its registry origin.
            let may_throw = self.may_throw(statement);
            if may_throw { self.exceptional(&states); }
            states = self.statement(statement, states);
            if may_throw { self.exceptional(&states); }
            // Bound state growth without joining mutually exclusive origins.
            states = states.into_iter().collect::<BTreeSet<_>>().into_iter().collect();
            if states.len() > 128 { self.failed = true; break; }
        }
        states
    }

    fn statement(&mut self, statement: &Statement<'a>, mut states: Vec<ReplacementState>) -> Vec<ReplacementState> {
        match statement {
            Statement::BlockStatement(block) => self.statements(&block.body, states),
            Statement::TryStatement(attempt) if attempt.finalizer.is_none() => {
                self.exceptional.push(Vec::new());
                let mut normal = self.statements(&attempt.block.body, states);
                let exceptional = self.exceptional.pop().unwrap_or_default();
                if let Some(handler) = &attempt.handler {
                    normal.extend(self.statements(&handler.body.body, exceptional));
                } else if let Some(outer) = self.exceptional.last_mut() {
                    outer.extend(exceptional);
                }
                normal
            }
            Statement::IfStatement(branch) => {
                if !self.inert(&branch.test) { self.exceptional(&states); }
                self.markers(branch.test.span(), &mut states);
                if !self.inert(&branch.test) { self.exceptional(&states); }
                if let Expression::BooleanLiteral(test) = branch.test.without_parentheses() {
                    return if test.value { self.statement(&branch.consequent, states) }
                        else if let Some(alternate) = &branch.alternate { self.statement(alternate, states) }
                        else { states };
                }
                let mut yes = self.statement(&branch.consequent, states.clone());
                let no = match &branch.alternate {
                    Some(alternate) => self.statement(alternate, states),
                    None => states,
                };
                yes.extend(no);
                yes
            }
            Statement::ExpressionStatement(_) | Statement::VariableDeclaration(_)
                | Statement::ReturnStatement(_) | Statement::ThrowStatement(_) => {
                let span = statement.span();
                // Activations within a compound expression with tracked writes
                // are deliberately refused: a source offset is not evaluation
                // order. Accepted writes are whole plain statements only.
                self.markers(span, &mut states);
                if let Some(origin) = self.origins.iter().position(|(s, _)| *s == span) {
                    for state in &mut states { state.0 = Some(origin); self.active(state); }
                }
                if let Some(writes) = self.writes.get(&span.start).cloned() {
                    for (alias, value) in writes {
                        for state in &mut states {
                            state.1[alias] = match value.without_parentheses() {
                                Expression::FunctionExpression(f) => Some(f.span.start),
                                Expression::ArrowFunctionExpression(f) => Some(f.span.start),
                                _ => self.callable(value, state),
                            };
                            self.active(state);
                        }
                    }
                }
                if matches!(statement, Statement::ReturnStatement(_) | Statement::ThrowStatement(_)) {
                    if self.may_throw(statement) { self.exceptional(&states); }
                    Vec::new()
                } else { states }
            }
            Statement::ForStatement(_) | Statement::ForInStatement(_) | Statement::ForOfStatement(_)
                | Statement::WhileStatement(_) | Statement::DoWhileStatement(_) => {
                    if self.discovery.opaque_loop_completes(statement) { states } else { Vec::new() }
                }
            Statement::FunctionDeclaration(_) | Statement::ClassDeclaration(_)
                | Statement::EmptyStatement(_) | Statement::DebuggerStatement(_) => states,
            // No implicit success for switch/with/labels/finally or new AST
            // statement kinds. Loops above are opaque only after the exhaustive
            // symbol-write/read and activation checks proved no tracked effect.
            _ => { self.failed = true; Vec::new() },
        }
    }
}

fn replacement_activation_supported(statement: &Statement<'_>, at: Span) -> bool {
    let span = statement.span();
    if at.start < span.start || at.end > span.end { return false; }
    match statement {
        Statement::BlockStatement(block) => block.body.iter().any(|s| replacement_activation_supported(s, at)),
        Statement::IfStatement(branch) => {
            let test = branch.test.span();
            (test.start <= at.start && at.end <= test.end)
                || replacement_activation_supported(&branch.consequent, at)
                || branch.alternate.as_ref().is_some_and(|s| replacement_activation_supported(s, at))
        }
        Statement::TryStatement(attempt) if attempt.finalizer.is_none() => {
            attempt.block.body.iter().any(|s| replacement_activation_supported(s, at))
                || attempt.handler.as_ref().is_some_and(|h|
                    h.body.body.iter().any(|s| replacement_activation_supported(s, at)))
        }
        Statement::ExpressionStatement(_) | Statement::VariableDeclaration(_)
            | Statement::ReturnStatement(_) | Statement::ThrowStatement(_) => true,
        _ => false,
    }
}

// Lexical reachability only: blocks, branch alternatives and exceptional exits.
// This never evaluates a test or a call. Literal booleans are syntax facts.
fn replacement_reaches(discovery: &Discovery<'_, '_, '_>, statements: &[Statement<'_>], at: Span) -> (bool, bool) {
    let mut normal = true;
    for statement in statements {
        if !normal { break; }
        let (next, found) = replacement_statement_reaches(discovery, statement, at);
        if found { return (next, true); }
        normal = next;
    }
    (normal, false)
}

fn replacement_statement_reaches(discovery: &Discovery<'_, '_, '_>, statement: &Statement<'_>, at: Span) -> (bool, bool) {
    match statement {
        Statement::BlockStatement(block) => replacement_reaches(discovery, &block.body, at),
        Statement::IfStatement(branch) => {
            let test = branch.test.span();
            if test.start <= at.start && at.end <= test.end { return (true, true); }
            if let Expression::BooleanLiteral(test) = branch.test.without_parentheses() {
                return if test.value { replacement_statement_reaches(discovery, &branch.consequent, at) }
                    else { branch.alternate.as_ref().map_or((true, false), |s| replacement_statement_reaches(discovery, s, at)) };
            }
            let yes = replacement_statement_reaches(discovery, &branch.consequent, at);
            let no = branch.alternate.as_ref().map_or((true, false), |s| replacement_statement_reaches(discovery, s, at));
            (yes.0 || no.0, yes.1 || no.1)
        }
        Statement::TryStatement(attempt) if attempt.finalizer.is_none() => {
            let normal = replacement_reaches(discovery, &attempt.block.body, at);
            let caught = if attempt.block.body.iter().any(|s| discovery.statement_may_throw(s)) {
                attempt.handler.as_ref().map_or((false, false), |h| replacement_reaches(discovery, &h.body.body, at))
            } else { (false, false) };
            (normal.0 || caught.0, normal.1 || caught.1)
        }
        Statement::ReturnStatement(_) | Statement::ThrowStatement(_)
            | Statement::BreakStatement(_) | Statement::ContinueStatement(_) => {
            let span = statement.span();
            (false, span.start <= at.start && at.end <= span.end)
        }
        Statement::ForStatement(_) | Statement::ForInStatement(_) | Statement::ForOfStatement(_)
            | Statement::WhileStatement(_) | Statement::DoWhileStatement(_) =>
                (discovery.opaque_loop_completes(statement), false),
        _ => (true, replacement_activation_supported(statement, at)),
    }
}

fn replacement_control_supported(statement: &Statement<'_>, at: Span) -> bool {
    if statement.span().start > at.start { return true; }
    match statement {
        Statement::BlockStatement(b) => b.body.iter().all(|s| replacement_control_supported(s, at)),
        Statement::IfStatement(b) => replacement_control_supported(&b.consequent, at)
            && b.alternate.as_ref().is_none_or(|s| replacement_control_supported(s, at)),
        Statement::TryStatement(t) if t.finalizer.is_none() =>
            t.block.body.iter().all(|s| replacement_control_supported(s, at))
                && t.handler.as_ref().is_none_or(|h| h.body.body.iter().all(|s| replacement_control_supported(s, at))),
        Statement::ExpressionStatement(_) | Statement::VariableDeclaration(_) | Statement::ReturnStatement(_)
        | Statement::ThrowStatement(_) | Statement::FunctionDeclaration(_) | Statement::ClassDeclaration(_)
        | Statement::EmptyStatement(_)
        | Statement::DebuggerStatement(_) | Statement::ForStatement(_) | Statement::ForInStatement(_)
        | Statement::ForOfStatement(_) | Statement::WhileStatement(_) | Statement::DoWhileStatement(_) => true,
        _ => false,
    }
}
