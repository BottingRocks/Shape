//! Relational proof for the *selected* writer's native backing-byte path.
//! Runtime initialization and host byte order remain explicit prerequisites.
use super::super::{algorithms, AlgorithmProof};
use crate::signals::schema::SourceSpan;
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_parser::Parser;
use oxc_semantic::{AstNodes, NodeId, Scoping, SemanticBuilder, SymbolId};
use oxc_span::{GetSpan, SourceType, Span};
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, LogicalOperator, UnaryOperator};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProofSite {
    pub role: String,
    pub span: SourceSpan,
    pub sha256: String,
}

fn site(source: &str, role: &str, span: Span) -> ProofSite {
    ProofSite {
        role: role.into(),
        span: span.into(),
        sha256: algorithms::digest(source[span.start as usize..span.end as usize].as_bytes()),
    }
}
fn required_intrinsic(name: &str) -> bool {
    matches!(name, "Float64Array" | "Uint8Array" | "Number")
}
fn intrinsic_member(member: &StaticMemberExpression<'_>, scopes: &Scoping) -> bool {
    required_intrinsic(member.property.name.as_str())
        && matches!(member.object.without_parentheses(),Expression::Identifier(id) if id.name=="globalThis" && symbol(scopes,id).is_none())
}
fn contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}
fn symbol(scopes: &Scoping, id: &IdentifierReference<'_>) -> Option<SymbolId> {
    id.reference_id
        .get()
        .and_then(|r| scopes.get_reference(r).symbol_id())
}
fn native_branch<'b, 'a>(function: &'b Function<'a>) -> Option<&'b IfStatement<'a>> {
    let outer = function.body.as_ref()?.statements.iter().find_map(|s| {
        if let Statement::IfStatement(i) = s {
            Some(i)
        } else {
            None
        }
    })?;
    let Statement::BlockStatement(float) = outer.alternate.as_ref()? else {
        return None;
    };
    float.body.iter().find_map(|s| {
        if let Statement::IfStatement(i) = s {
            Some(i.as_ref())
        } else {
            None
        }
    })
}

/// Replace ONLY the native guard's alternate, retaining the complete integer
/// path, native store/view, marker, loop direction and all byte writes.
fn native_text(source: &str, function: &Function<'_>) -> Option<String> {
    let alternate = native_branch(function)?.alternate.as_ref()?.span();
    let span = function.span;
    let mut text = source
        .get(span.start as usize..alternate.start as usize)?
        .to_string();
    text.push_str("{throw 0;}");
    text.push_str(source.get(alternate.end as usize..span.end as usize)?);
    if function.id.is_none() {
        text = text.replacen("function", "function nativeWriter", 1);
    }
    Some(text)
}

#[derive(Clone, Copy)]
enum Definition<'a> {
    Value(&'a Expression<'a>, Span, NodeId),
    Invalid,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Origin {
    Float64,
    Bytes,
    Scratch,
}

struct Origins<'s, 'a> {
    source: &'s str,
    scopes: &'a Scoping,
    nodes: &'a AstNodes<'a>,
    defs: BTreeMap<SymbolId, Vec<Definition<'a>>>,
    evidence: Vec<ProofSite>,
    symbols: BTreeSet<SymbolId>,
}
impl<'s, 'a> Origins<'s, 'a> {
    fn owner(&self, mut id: NodeId) -> Option<NodeId> {
        loop {
            let parent = self.nodes.parent_id(id);
            if parent == id {
                return None;
            }
            id = parent;
            if matches!(
                self.nodes.kind(id),
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
            ) {
                return Some(id);
            }
        }
    }
    fn definition(&mut self, sym: SymbolId, depth: usize) -> Result<Origin, &'static str> {
        if depth > 24 || !self.scopes.symbol_redeclarations(sym).is_empty() {
            return Err("cyclic or redeclared native dependency");
        }
        let (value, span, node) = match self.defs.get(&sym).map(Vec::as_slice) {
            Some([Definition::Value(value, span, node)]) => (*value, *span, *node),
            Some([Definition::Value(a, sa, na), Definition::Value(b, sb, nb)]) => {
                // The only admitted two-definition origin is successful native
                // allocation versus null on exception, in one simple try/catch.
                let (value, init, node, null, null_node) =
                    if matches!(b.without_parentheses(), Expression::NullLiteral(_)) {
                        (*a, *sa, *na, *sb, *nb)
                    } else if matches!(a.without_parentheses(), Expression::NullLiteral(_)) {
                        (*b, *sb, *nb, *sa, *na)
                    } else {
                        return Err("native dependency is overwritten");
                    };
                let allocation = self
                    .nodes
                    .ancestors(node)
                    .find_map(|parent| match parent.kind() {
                        AstKind::TryStatement(t)
                            if contains(t.block.span, init)
                                && t.finalizer.is_none()
                                && t.block.body.len() == 1
                                && t.handler.as_ref().is_some_and(|c| {
                                    contains(c.body.span, null) && c.body.body.len() == 1
                                }) =>
                        {
                            Some(t)
                        }
                        _ => None,
                    })
                    .ok_or(
                        "multiple native definitions are not a simple allocation/null try-catch",
                    )?;
                if self.owner(null_node) != self.owner(node)
                    || !matches!(value.without_parentheses(), Expression::NewExpression(_))
                {
                    return Err("conditional allocation origin is unresolved");
                }
                self.evidence.push(site(
                    self.source,
                    "conditional_native_allocation",
                    allocation.span,
                ));
                (value, allocation.span, node)
            }
            _ => return Err("native dependency is overwritten or has no single origin"),
        };
        if self.owner(node) != self.owner(self.scopes.symbol_declaration(sym)) {
            return Err("native dependency is initialized by a different enclosing function");
        }
        let origin = self.expression(value, depth + 1)?;
        self.evidence.push(site(
            self.source,
            match origin {
                Origin::Float64 => "float64_constructor_origin",
                Origin::Bytes => "uint8_constructor_origin",
                Origin::Scratch => "scratch_origin",
            },
            span,
        ));
        self.symbols.insert(sym);
        Ok(origin)
    }
    fn expression(
        &mut self,
        value: &'a Expression<'a>,
        depth: usize,
    ) -> Result<Origin, &'static str> {
        if depth > 24 {
            return Err("native dependency alias depth exceeded");
        }
        match value.without_parentheses() {
            Expression::Identifier(id) => {
                if let Some(sym) = symbol(self.scopes, id) {
                    return self.definition(sym, depth + 1);
                }
                self.intrinsic(id.name.as_str(), id.span)
            }
            Expression::StaticMemberExpression(member) if matches!(member.object.without_parentheses(), Expression::Identifier(id) if id.name=="globalThis" && symbol(self.scopes,id).is_none()) => {
                self.intrinsic(member.property.name.as_str(), member.span)
            }
            Expression::NewExpression(new)
                if new.arguments.len() == 1
                    && matches!(new.arguments[0].as_expression().map(Expression::without_parentheses),Some(Expression::NumericLiteral(n)) if n.value==1.0) =>
            {
                if self.expression(&new.callee, depth + 1)? != Origin::Float64 {
                    return Err("scratch constructor is not Float64Array");
                }
                Ok(Origin::Scratch)
            }
            Expression::ConditionalExpression(c)
                if matches!(
                    c.alternate.without_parentheses(),
                    Expression::NullLiteral(_)
                ) =>
            {
                let origin = self.expression(&c.consequent, depth + 1)?;
                match origin {
                    Origin::Float64 | Origin::Bytes => {
                        if self.type_guard(&c.test, origin, depth + 1)? {
                            Ok(origin)
                        } else {
                            Err("native constructor guard does not test its own intrinsic")
                        }
                    }
                    Origin::Scratch => {
                        let Expression::LogicalExpression(test) = c.test.without_parentheses()
                        else {
                            return Err("native scratch guard is unresolved");
                        };
                        if test.operator == LogicalOperator::And
                            && self.expression(&test.left, depth + 1)? == Origin::Bytes
                            && self.type_guard(&test.right, Origin::Float64, depth + 1)?
                        {
                            Ok(origin)
                        } else {
                            Err("native scratch guard is not byte-constructor and Float64 availability")
                        }
                    }
                }
            }
            _ => Err("native class/value/coercion path is not certified"),
        }
    }
    fn intrinsic(&mut self, name: &str, span: Span) -> Result<Origin, &'static str> {
        let origin = match name { "Float64Array"=>Origin::Float64,"Uint8Array"=>Origin::Bytes,_=>return Err("native intrinsic must be Float64Array or Uint8Array; signed/wide views need separate narrowing proof") };
        self.evidence
            .push(site(self.source, "trusted_intrinsic_reference", span));
        Ok(origin)
    }
    fn type_guard(
        &mut self,
        expression: &'a Expression<'a>,
        expected: Origin,
        depth: usize,
    ) -> Result<bool, &'static str> {
        let Expression::BinaryExpression(b) = expression.without_parentheses() else {
            return Ok(false);
        };
        if b.operator != BinaryOperator::StrictEquality {
            return Ok(false);
        }
        let (operand, text) = match (b.left.without_parentheses(), b.right.without_parentheses()) {
            (Expression::UnaryExpression(u), Expression::StringLiteral(s))
            | (Expression::StringLiteral(s), Expression::UnaryExpression(u)) => (u, s),
            _ => return Ok(false),
        };
        Ok(operand.operator == UnaryOperator::Typeof
            && text.value == "function"
            && self.expression(&operand.argument, depth + 1)? == expected)
    }
}

// Trace only concrete single-origin writer exports/calls. In particular an
// alias initialization is not itself evidence that a later return still
// contains this function after that alias was overwritten. Unresolved alias
// paths contribute no witness; they do not erase a separate proven direct
// call/return of the selected function. Constructor/scratch origins stay strict.
fn writer_link<'a>(
    origins: &Origins<'_, 'a>,
    initial: SymbolId,
    function: Span,
    evidence: &mut Vec<ProofSite>,
    visited: &mut BTreeSet<SymbolId>,
    depth: usize,
) -> Result<bool, &'static str> {
    if depth > 24 || !visited.insert(initial) {
        return Err("writer alias cycle or depth limit");
    }
    if !origins.scopes.symbol_redeclarations(initial).is_empty() {
        return Err("writer binding is redeclared");
    }
    let allowed = origins.defs.get(&initial).map(Vec::as_slice);
    if allowed.is_some_and(|d| !matches!(d, [Definition::Value(_, _, _)])) {
        return Err("selected writer alias is overwritten");
    }
    let mut linked = false;
    for reference in origins.scopes.get_resolved_references(initial) {
        let span = origins.nodes.get_node(reference.node_id()).span();
        if reference.is_write()
            && !allowed.is_some_and(|d| matches!(d,[Definition::Value(_,s,_)] if contains(*s,span)))
        {
            return Err("selected writer binding has an unaccounted write");
        }
        if !reference.is_read() || contains(function, span) {
            continue;
        }
        match origins.nodes.parent_kind(reference.node_id()) {
            AstKind::CallExpression(c) if c.callee.span() == span => {
                evidence.push(site(origins.source, "selected_writer_call", c.span));
                linked = true;
            }
            AstKind::ReturnStatement(r)
                if r.argument.as_ref().is_some_and(|e| e.span() == span) =>
            {
                evidence.push(site(origins.source, "selected_writer_return", r.span));
                linked = true;
            }
            AstKind::VariableDeclarator(d) if d.init.as_ref().is_some_and(|e| e.span() == span) => {
                let BindingPattern::BindingIdentifier(id) = &d.id else {
                    continue;
                };
                let sym = id
                    .symbol_id
                    .get()
                    .ok_or("writer alias binding unresolved")?;
                let mut alias_evidence =
                    vec![site(origins.source, "selected_writer_alias", d.span)];
                if writer_link(
                    origins,
                    sym,
                    function,
                    &mut alias_evidence,
                    &mut visited.clone(),
                    depth + 1,
                )
                .unwrap_or(false)
                {
                    evidence.extend(alias_evidence);
                    linked = true;
                }
            }
            AstKind::AssignmentExpression(a)
                if a.operator == AssignmentOperator::Assign && a.right.span() == span =>
            {
                let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else {
                    continue;
                };
                let sym = symbol(origins.scopes, id).ok_or("writer assignment unresolved")?;
                let mut alias_evidence =
                    vec![site(origins.source, "selected_writer_alias", a.span)];
                if writer_link(
                    origins,
                    sym,
                    function,
                    &mut alias_evidence,
                    &mut visited.clone(),
                    depth + 1,
                )
                .unwrap_or(false)
                {
                    evidence.extend(alias_evidence);
                    linked = true;
                }
            }
            _ => {}
        }
    }
    visited.remove(&initial);
    Ok(linked)
}

pub(super) fn certify(
    source: &str,
    selected: SourceSpan,
    selected_writer: &mut AlgorithmProof,
) -> Result<(AlgorithmProof, Vec<ProofSite>), &'static str> {
    let arena = Allocator::default();
    let parsed = Parser::new(&arena, source, SourceType::mjs()).parse();
    if !parsed.errors.is_empty() {
        return Err("source parse failed");
    }
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    let scopes = semantic.scoping();
    let nodes = semantic.nodes();
    let selected_span = Span::new(selected.start, selected.end);
    let (writer_node, function) = nodes
        .iter()
        .find_map(|n| match n.kind() {
            AstKind::Function(f)
                if f.span.start == selected_span.start
                    && f.span.end <= selected_span.end
                    && source
                        .get(f.span.end as usize..selected_span.end as usize)
                        .is_some_and(|tail| tail.trim().is_empty()) =>
            {
                Some((n.id(), f))
            }
            _ => None,
        })
        .ok_or("selected number function is not a concrete source function")?;
    let native = native_branch(function).ok_or("selected writer has no native float guard")?;
    let Expression::Identifier(scratch) = native.test.without_parentheses() else {
        return Err("native guard is not the captured scratch");
    };
    let scratch_sym = symbol(scopes, scratch).ok_or("native scratch is unresolved")?;
    let Statement::BlockStatement(block) = &native.consequent else {
        return Err("native body is not a block");
    };
    let byte_new = block
        .body
        .iter()
        .find_map(|s| match s {
            Statement::ExpressionStatement(e) => match e.expression.without_parentheses() {
                Expression::AssignmentExpression(a) => match a.right.without_parentheses() {
                    Expression::NewExpression(n) => Some(n),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        })
        .ok_or("native body has no backing byte view")?;
    let candidate = native_text(source, function).ok_or("native branch boundary is unresolved")?;
    let reference = include_str!("reference/number.js");
    let reference_arena = Allocator::default();
    let reference_parsed = Parser::new(&reference_arena, reference, SourceType::mjs()).parse();
    let Some(Statement::FunctionDeclaration(reference_function)) =
        reference_parsed.program.body.first()
    else {
        return Err("reference function unavailable");
    };
    let reference_native =
        native_text(reference, reference_function).ok_or("reference native branch unavailable")?;
    let mut writer = algorithms::proof(
        Some((selected, &candidate)),
        &reference_native,
        "ieee64-number-native-conditional/2",
        false,
    );
    writer.sha256 = Some(algorithms::digest(
        source[selected.start as usize..selected.end as usize].as_bytes(),
    ));
    if writer.codec.is_none() || !algorithms::unshadowed(source, selected, &["Number"]) {
        return Err("whole integer/native writer body or Number dependency is incompatible");
    }
    *selected_writer = writer;
    let mut origins = Origins {
        source,
        scopes,
        nodes,
        defs: BTreeMap::new(),
        evidence: vec![],
        symbols: BTreeSet::new(),
    };
    for node in nodes.iter() {
        match node.kind() {
            AstKind::VariableDeclarator(d) => {
                if let BindingPattern::BindingIdentifier(b) = &d.id {
                    if let (Some(sym), Some(value)) = (b.symbol_id.get(), &d.init) {
                        origins.defs.entry(sym).or_default().push(Definition::Value(
                            value,
                            d.span,
                            node.id(),
                        ));
                    }
                }
            }
            AstKind::AssignmentExpression(a) => {
                if matches!(&a.left,AssignmentTarget::StaticMemberExpression(m) if intrinsic_member(m,scopes))
                    || matches!(&a.left,AssignmentTarget::AssignmentTargetIdentifier(id) if symbol(scopes,id).is_none() && required_intrinsic(id.name.as_str()))
                {
                    return Err("source writes a required native intrinsic");
                }
                if let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left {
                    if let Some(sym) = symbol(scopes, id) {
                        origins.defs.entry(sym).or_default().push(
                            if a.operator == AssignmentOperator::Assign {
                                Definition::Value(&a.right, a.span, node.id())
                            } else {
                                Definition::Invalid
                            },
                        );
                    }
                }
            }
            AstKind::UpdateExpression(u) => {
                if matches!(&u.argument,SimpleAssignmentTarget::StaticMemberExpression(m) if intrinsic_member(m,scopes))
                    || matches!(&u.argument,SimpleAssignmentTarget::AssignmentTargetIdentifier(id) if symbol(scopes,id).is_none() && required_intrinsic(id.name.as_str()))
                {
                    return Err("source updates a required native intrinsic");
                }
                if let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &u.argument {
                    if let Some(sym) = symbol(scopes, id) {
                        origins
                            .defs
                            .entry(sym)
                            .or_default()
                            .push(Definition::Invalid);
                    }
                }
            }
            AstKind::UnaryExpression(u)
                if u.operator == UnaryOperator::Delete
                    && matches!(u.argument.without_parentheses(),Expression::StaticMemberExpression(m) if intrinsic_member(m,scopes)) =>
            {
                return Err("source deletes a required native intrinsic")
            }
            AstKind::WithStatement(_) => {
                return Err("with prevents native capture provenance proof")
            }
            AstKind::CallExpression(c) if matches!(c.callee.without_parentheses(),Expression::Identifier(id) if id.name=="eval" && symbol(scopes,id).is_none()) => {
                return Err("direct eval prevents native capture provenance proof")
            }
            _ => {}
        }
    }
    if origins.definition(scratch_sym, 0)? != Origin::Scratch {
        return Err("guard does not reference a one-element Float64Array scratch");
    }
    if origins.expression(&byte_new.callee, 0)? != Origin::Bytes {
        return Err("native backing view is not Uint8Array");
    }
    // Whole-body comparison binds guard/store/buffer to one identifier. Verify
    // the concrete scope relationship too; a parameter or unrelated scratch
    // cannot borrow a neighboring factory's constructor certificate.
    let scratch_decl = scopes.symbol_span(scratch_sym);
    if contains(function.span, scratch_decl) {
        return Err("native scratch is not an enclosing capture");
    }
    let mut evidence = std::mem::take(&mut origins.evidence);
    evidence.push(site(source, "selected_writer", function.span));
    evidence.push(site(source, "native_scratch_guard", native.test.span()));
    evidence.push(site(
        source,
        "native_store_and_same_buffer_view",
        native.consequent.span(),
    ));
    evidence.push(site(
        source,
        "software_alternate_not_certified",
        native.alternate.as_ref().unwrap().span(),
    ));
    // Single-origin aliases must not escape to unknown mutation sites. Reads
    // in approved initialization expressions and the certified writer are safe.
    let approved = evidence
        .iter()
        .map(|e| Span::new(e.span.start, e.span.end))
        .collect::<Vec<_>>();
    for sym in &origins.symbols {
        for reference in scopes.get_resolved_references(*sym) {
            let span = nodes.get_node(reference.node_id()).span();
            if reference.is_write() && !origins.defs.get(sym).is_some_and(|defs| defs.iter().any(|d| matches!(d, Definition::Value(_, definition, _) if contains(*definition,span)))) {
                return Err("native dependency has an unaccounted destructuring/update write");
            }
            if !reference.is_read() {
                continue;
            }
            if !contains(function.span, span) && !approved.iter().any(|a| contains(*a, span)) {
                // A null-test for the software NaN fallback does not expose
                // or mutate scratch contents; recognize only that exact use.
                let parent = nodes.parent_kind(reference.node_id());
                if !matches!(parent,AstKind::ConditionalExpression(c) if c.test.span()==span && matches!(c.consequent.without_parentheses(),Expression::NullLiteral(_)) && matches!(c.alternate.without_parentheses(),Expression::ArrayExpression(_)))
                {
                    return Err(
                        "native dependency escapes its certified writer/initialization uses",
                    );
                }
            }
        }
    }
    // The selected function is a declaration, an assigned literal or a directly
    // returned literal. Link it to actual uses of its own binding, not to an
    // arbitrary compatible function found somewhere in an ancestor.
    let linked = if function.r#type == FunctionType::FunctionDeclaration {
        let sym = function
            .id
            .as_ref()
            .and_then(|id| id.symbol_id.get())
            .ok_or("writer declaration binding missing")?;
        if origins.defs.contains_key(&sym) {
            return Err("selected writer declaration is overwritten");
        }
        writer_link(
            &origins,
            sym,
            function.span,
            &mut evidence,
            &mut BTreeSet::new(),
            0,
        )?
    } else {
        let parent = nodes.parent_id(writer_node);
        let binding = match nodes.kind(parent) {
            AstKind::VariableDeclarator(d)
                if d.init.as_ref().is_some_and(|e| e.span() == function.span) =>
            {
                match &d.id {
                    BindingPattern::BindingIdentifier(id) => id.symbol_id.get(),
                    _ => None,
                }
            }
            AstKind::AssignmentExpression(a)
                if a.operator == AssignmentOperator::Assign && a.right.span() == function.span =>
            {
                match &a.left {
                    AssignmentTarget::AssignmentTargetIdentifier(id) => symbol(scopes, id),
                    _ => None,
                }
            }
            AstKind::ReturnStatement(r)
                if r.argument
                    .as_ref()
                    .is_some_and(|e| e.span() == function.span) =>
            {
                evidence.push(site(source, "selected_writer_return", r.span));
                None
            }
            _ => return Err("selected writer is not a directly assigned or returned literal"),
        };
        if let Some(sym) = binding {
            writer_link(
                &origins,
                sym,
                function.span,
                &mut evidence,
                &mut BTreeSet::new(),
                0,
            )?
        } else {
            matches!(nodes.kind(parent), AstKind::ReturnStatement(_))
        }
    };
    if !linked {
        return Err("selected writer has no concrete returned/assigned/called linkage");
    }
    evidence.push(site(source, "selected_writer_binding_link", function.span));
    evidence.sort_by_key(|s| (s.span.start, s.span.end, s.role.clone()));
    evidence.dedup();
    let enclosing = origins
        .owner(writer_node)
        .map(|id| nodes.get_node(id).span())
        .unwrap_or(parsed.program.span);
    let proof = AlgorithmProof {
        codec: Some("native-f64-single-origin-backing-bytes/2".into()),
        span: Some(enclosing.into()),
        sha256: Some(algorithms::digest(
            source[enclosing.start as usize..enclosing.end as usize].as_bytes(),
        )),
        reason: None,
    };
    Ok((proof, evidence))
}
