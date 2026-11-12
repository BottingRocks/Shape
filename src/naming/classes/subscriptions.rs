//! Independent native subscription class proof. Entire own bodies must match
//! the ordered lifecycle grammar below. This deliberately does not widen p03.
//!
//! The grammar is expressed as readable reference methods and compared as typed
//! AST trees, not text or hashes. Only local SymbolIds are alpha-renamed. Every
//! branch, assignment, receiver, argument and exception edge is retained. The
//! method projection has no nested-callable production, so donors cannot
//! contribute. Descriptor setup accepts only the separately proved own getter.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_parser::Parser;
use oxc_semantic::{Scoping, SemanticBuilder, SymbolId};
use oxc_span::{GetSpan, SourceType, Span};

use crate::signals::extract::bindings::{property_key_name, Model};
use crate::signals::analysis::roles::FinderClaim;
use crate::signals::schema::Confidence;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Tree {
    Node(&'static str, Vec<Tree>),
    Atom(String),
    Local(usize),
}
fn node(tag: &'static str, children: Vec<Tree>) -> Tree {
    Tree::Node(tag, children)
}
fn atom(value: impl ToString) -> Tree {
    Tree::Atom(value.to_string())
}

/// A closed projection, not an AST subtree search. Unrecognized syntax fails.
struct Body<'a> {
    scoping: &'a Scoping,
    locals: BTreeMap<SymbolId, usize>,
    depth: usize,
    budget: usize,
    // Enabled only while checking the complete descriptor setup, never methods.
    setup_getter: Option<Tree>,
    // Enabled only for a full singleton installer grammar comparison. The
    // matched prefix proves this exact indexed descriptor key is "closed".
    closed_key: Option<Tree>,
    closed_locals: Vec<Tree>,
}
impl<'a> Body<'a> {
    fn bind(&mut self, pattern: &BindingPattern<'_>) -> Option<Tree> {
        let BindingPattern::BindingIdentifier(id) = pattern else {
            return None;
        };
        self.bind_symbol(id.symbol_id.get()?)
    }
    fn bind_symbol(&mut self, symbol: SymbolId) -> Option<Tree> {
        if !self.scoping.symbol_redeclarations(symbol).is_empty()
            || self.locals.contains_key(&symbol)
        {
            return None;
        }
        let index = self.locals.len();
        self.locals.insert(symbol, index);
        Some(Tree::Local(index))
    }
    fn reference(&self, id: &IdentifierReference<'_>) -> Option<Tree> {
        let symbol = self
            .scoping
            .get_reference(id.reference_id.get()?)
            .symbol_id();
        match symbol {
            Some(symbol) => Some(Tree::Local(*self.locals.get(&symbol)?)),
            None if self.setup_getter.is_some()
                && matches!(
                    id.name.as_str(),
                    "Object" | "Symbol" | "String" | "TypeError"
                ) =>
            {
                Some(node("global", vec![atom(id.name.as_str())]))
            }
            _ => None,
        }
    }
    fn statements(&mut self, statements: &[Statement<'_>]) -> Option<Tree> {
        Some(node(
            "body",
            statements
                .iter()
                .map(|s| self.statement(s))
                .collect::<Option<_>>()?,
        ))
    }
    fn statement(&mut self, s: &Statement<'_>) -> Option<Tree> {
        self.budget = self.budget.checked_sub(1)?;
        self.depth += 1;
        if self.depth > 64 {
            return None;
        }
        let result = match s {
            Statement::FunctionDeclaration(f) if self.setup_getter.is_some() => {
                let expected = self.setup_getter.as_ref()?;
                if &function_body(self.scoping, f, true)? != expected {
                    return None;
                }
                Some(node(
                    "closed getter",
                    vec![self.bind_symbol(f.id.as_ref()?.symbol_id.get()?)?],
                ))
            }
            Statement::ForStatement(s) if self.setup_getter.is_some() => {
                let init = match &s.init {
                    None => None,
                    Some(ForStatementInit::VariableDeclaration(d)) => Some(self.variables(d)?),
                    _ => return None,
                };
                let mut children = Vec::new();
                if let Some(init) = init {
                    children.push(node("init", vec![init]));
                }
                children.extend([
                    node("test", s.test.iter().map(|e| self.expression(e)).collect::<Option<_>>()?),
                    node("update", s.update.iter().map(|e| self.expression(e)).collect::<Option<_>>()?),
                    self.statement(&s.body)?,
                ]);
                Some(node(
                    "for",
                    children,
                ))
            }
            Statement::WhileStatement(s) if self.setup_getter.is_some() => Some(node(
                "while", vec![self.expression(&s.test)?, self.statement(&s.body)?],
            )),
            Statement::BlockStatement(s) => self.statements(&s.body),
            Statement::ExpressionStatement(s) => {
                Some(node("expression", vec![self.expression(&s.expression)?]))
            }
            Statement::ReturnStatement(s) => Some(node(
                "return",
                s.argument
                    .iter()
                    .map(|e| self.expression(e))
                    .collect::<Option<_>>()?,
            )),
            Statement::ThrowStatement(s) => {
                Some(node("throw", vec![self.expression(&s.argument)?]))
            }
            Statement::VariableDeclaration(s) => self.variables(s),
            Statement::IfStatement(s) if self.closed_key.is_some() => {
                let test = self.expression(&s.test)?;
                if let Some(value) = self.closed_condition(&test) {
                    let selected = if value {
                        self.statement(&s.consequent)?
                    } else if let Some(alternate) = &s.alternate {
                        self.statement(alternate)?
                    } else {
                        node("no branch", vec![])
                    };
                    Some(node("finite if", vec![test, selected]))
                } else {
                    Some(node("if", vec![test, self.statement(&s.consequent)?,
                        node("else", s.alternate.iter().map(|s| self.statement(s)).collect::<Option<_>>()?)]))
                }
            }
            Statement::IfStatement(s) => Some(node(
                "if",
                vec![
                    self.expression(&s.test)?,
                    self.statement(&s.consequent)?,
                    node(
                        "else",
                        s.alternate
                            .iter()
                            .map(|s| self.statement(s))
                            .collect::<Option<_>>()?,
                    ),
                ],
            )),
            Statement::TryStatement(s) => {
                let block = self.statements(&s.block.body)?;
                let handler = if let Some(h) = &s.handler {
                    let param = h
                        .param
                        .as_ref()
                        .map(|p| self.bind(&p.pattern))
                        .transpose_option()?;
                    node(
                        "catch",
                        vec![
                            node("param", param.into_iter().collect()),
                            self.statements(&h.body.body)?,
                        ],
                    )
                } else {
                    node("no catch", vec![])
                };
                let finalizer = s
                    .finalizer
                    .iter()
                    .map(|b| self.statements(&b.body))
                    .collect::<Option<_>>()?;
                Some(node(
                    "try",
                    vec![block, handler, node("finally", finalizer)],
                ))
            }
            _ => None,
        };
        self.depth -= 1;
        result
    }
    fn variables(&mut self, s: &VariableDeclaration<'_>) -> Option<Tree> {
        // Preserve declaration kind and every initializer, including for-init.
        let mut children = vec![atom(format!("{:?}", s.kind))];
        for d in &s.declarations {
            let binding = self.bind(&d.id)?;
            children.push(node("declaration", vec![binding, node("init",
                d.init.iter().map(|e| self.expression(e)).collect::<Option<_>>()?)]));
        }
        Some(node("variables", children))
    }
    fn expression(&mut self, e: &Expression<'_>) -> Option<Tree> {
        self.budget = self.budget.checked_sub(1)?;
        self.depth += 1;
        if self.depth > 64 {
            return None;
        }
        let result = match e.without_parentheses() {
            Expression::ObjectExpression(o) if self.setup_getter.is_some() => {
                let mut properties = Vec::new();
                for property in &o.properties {
                    let ObjectPropertyKind::ObjectProperty(p) = property else {
                        return None;
                    };
                    if p.computed || p.method || p.shorthand || p.kind != PropertyKind::Init {
                        return None;
                    }
                    properties.push(node(
                        "property",
                        vec![atom(property_key_name(&p.key)?), self.expression(&p.value)?],
                    ));
                }
                Some(node("object", properties))
            }
            Expression::NewExpression(n)
                if self.setup_getter.is_some() && n.type_arguments.is_none() =>
            {
                let mut children = vec![self.expression(&n.callee)?];
                for argument in &n.arguments {
                    children.push(self.expression(argument.as_expression()?)?);
                }
                Some(node("new", children))
            }
            Expression::LogicalExpression(e) if self.setup_getter.is_some() => Some(node(
                "logical",
                vec![
                    atom(format!("{:?}", e.operator)),
                    self.expression(&e.left)?,
                    self.expression(&e.right)?,
                ],
            )),
            Expression::UpdateExpression(e) if self.setup_getter.is_some() => {
                let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &e.argument else {
                    return None;
                };
                Some(node(
                    "update",
                    vec![
                        atom(format!("{:?}", e.operator)),
                        atom(e.prefix),
                        self.reference(id)?,
                    ],
                ))
            }
            Expression::ThisExpression(_) => Some(node("this", vec![])),
            Expression::Identifier(id) => self.reference(id),
            Expression::NullLiteral(_) => Some(node("null", vec![])),
            Expression::BooleanLiteral(v) => Some(node("bool", vec![atom(v.value)])),
            Expression::StringLiteral(v) => Some(node("string", vec![atom(v.value.as_str())])),
            // Values above one occur only in explicit dead-store alternatives below.
            // Index 0/1 and void 0 remain exact; no state/call operand is erased.
            Expression::NumericLiteral(v) => Some(node(
                "number",
                vec![if v.value >= 2.0 {
                    atom("finite >= 2")
                } else {
                    atom(v.value)
                }],
            )),
            Expression::StaticMemberExpression(m) if !m.optional => Some(node(
                "member",
                vec![self.expression(&m.object)?, atom(m.property.name.as_str())],
            )),
            Expression::ComputedMemberExpression(m) if !m.optional => Some(node(
                "index",
                vec![self.expression(&m.object)?, self.expression(&m.expression)?],
            )),
            Expression::CallExpression(c) if !c.optional && c.type_arguments.is_none() => {
                let mut children = vec![self.expression(&c.callee)?];
                for argument in &c.arguments {
                    children.push(self.expression(argument.as_expression()?)?);
                }
                Some(node("call", children))
            }
            Expression::BinaryExpression(b) => Some(node(
                "binary",
                vec![
                    atom(format!("{:?}", b.operator)),
                    self.expression(&b.left)?,
                    self.expression(&b.right)?,
                ],
            )),
            Expression::UnaryExpression(u) => Some(node(
                "unary",
                vec![
                    atom(format!("{:?}", u.operator)),
                    self.expression(&u.argument)?,
                ],
            )),
            Expression::ConditionalExpression(c) => Some(node(
                "conditional",
                vec![
                    self.expression(&c.test)?,
                    self.expression(&c.consequent)?,
                    self.expression(&c.alternate)?,
                ],
            )),
            Expression::AssignmentExpression(a) => {
                let target = self.target(&a.left)?;
                let value = self.expression(&a.right)?;
                if self.closed_key.is_some() && matches!(target, Tree::Local(_)) {
                    let closed = self.is_closed_string(&value) && format!("{:?}", a.operator) == "Assign";
                    self.closed_locals.retain(|local| local != &target);
                    if closed { self.closed_locals.push(target.clone()); }
                }
                Some(node("assign", vec![atom(format!("{:?}", a.operator)), target, value]))
            }
            Expression::ArrayExpression(a) => Some(node(
                "array",
                a.elements
                    .iter()
                    .map(|e| self.expression(e.as_expression()?))
                    .collect::<Option<_>>()?,
            )),
            _ => None,
        };
        self.depth -= 1;
        result
    }
    fn target(&mut self, t: &AssignmentTarget<'_>) -> Option<Tree> {
        match t {
            AssignmentTarget::AssignmentTargetIdentifier(id) => self.reference(id),
            AssignmentTarget::StaticMemberExpression(m) if !m.optional => Some(node(
                "member",
                vec![self.expression(&m.object)?, atom(m.property.name.as_str())],
            )),
            _ => None,
        }
    }
    fn is_closed_string(&self, value: &Tree) -> bool {
        self.closed_key.as_ref() == Some(value) || self.closed_locals.contains(value)
    }
    fn closed_condition(&self, test: &Tree) -> Option<bool> {
        let Tree::Node(tag, children) = test else { return None };
        match (*tag, children.as_slice()) {
            ("binary", [Tree::Atom(operator), Tree::Node("unary", unary), Tree::Node("string", kind)])
                if matches!(unary.as_slice(), [Tree::Atom(op), value] if op == "Typeof" && self.is_closed_string(value)) => {
                let [Tree::Atom(kind)] = kind.as_slice() else { return None };
                match operator.as_str() {
                    "Equality" | "StrictEquality" => Some(kind == "string"),
                    "Inequality" | "StrictInequality" => Some(kind != "string"),
                    _ => None,
                }
            }
            ("logical", [Tree::Atom(operator), left, right]) if operator == "Or" => {
                if self.closed_condition(left)? { Some(true) } else { self.closed_condition(right) }
            }
            ("conditional", [test, yes, no]) => {
                self.closed_condition(if self.closed_condition(test)? { yes } else { no })
            }
            _ => None,
        }
    }
}

// Option<Option<T>> with a fallible inner value but an optional outer node.
trait OptionalNode<T> {
    fn transpose_option(self) -> Option<Option<T>>;
}
impl<T> OptionalNode<T> for Option<Option<T>> {
    fn transpose_option(self) -> Option<Option<T>> {
        match self {
            Some(x) => x.map(Some),
            None => Some(None),
        }
    }
}

fn body(scoping: &Scoping, function: &Function<'_>) -> Option<Tree> {
    function_body(scoping, function, false)
}
fn function_body(scoping: &Scoping, function: &Function<'_>, strict_getter: bool) -> Option<Tree> {
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || function.type_parameters.is_some()
        || function.return_type.is_some()
    {
        return None;
    }
    let mut proof = Body {
        scoping,
        locals: BTreeMap::new(),
        depth: 0,
        budget: 1024,
        setup_getter: None,
        closed_key: None,
        closed_locals: Vec::new(),
    };
    let mut parameters = Vec::new();
    for p in &function.params.items {
        if p.initializer.is_some() || !p.decorators.is_empty() {
            return None;
        }
        parameters.push(proof.bind(&p.pattern)?);
    }
    let b = function.body.as_ref()?;
    if !b.directives.is_empty()
        && !(strict_getter && b.directives.len() == 1 && b.directives[0].directive == "use strict")
    {
        return None;
    }
    Some(node(
        "method",
        vec![
            node("parameters", parameters),
            proof.statements(&b.statements)?,
        ],
    ))
}

// References to a class/its direct aliases must remain plain constructor-value
// uses, the proved descriptor interval, or the exact readonly-prototype call.
// Other prototype/member writes, delete and escaping calls are refused.
fn constructor_uses(
    model: &Model<'_, '_>,
    symbol: SymbolId,
    seen: &mut BTreeSet<SymbolId>,
    setup: Option<Span>,
) -> bool {
    if !seen.insert(symbol)
        || seen.len() > 32
        || model.binding_writes(symbol) != 0
        || !model.scoping.symbol_redeclarations(symbol).is_empty()
    {
        return false;
    }
    model.scoping.get_resolved_references(symbol).all(|r| {
        let AstKind::IdentifierReference(id) = model.nodes.kind(r.node_id()) else { return false };
        if setup.is_some_and(|span| span.contains_inclusive(id.span)) { return true; }
        match model.nodes.parent_kind(r.node_id()) {
            AstKind::NewExpression(n) => matches!(&n.callee, Expression::Identifier(i) if i.span == id.span),
            AstKind::ReturnStatement(s) => matches!(&s.argument, Some(Expression::Identifier(i)) if i.span == id.span),
            AstKind::VariableDeclarator(d) if matches!(&d.init, Some(Expression::Identifier(i)) if i.span == id.span) => {
                let BindingPattern::BindingIdentifier(alias) = &d.id else { return false };
                alias.symbol_id.get().is_some_and(|s| constructor_uses(model, s, seen, None))
            }
            AstKind::CallExpression(c) => readonly_prototype(model, c, symbol),
            _ => false,
        }
    })
}

fn readonly_prototype(model: &Model<'_, '_>, call: &CallExpression<'_>, class: SymbolId) -> bool {
    if call.optional || call.type_arguments.is_some() || call.arguments.len() != 3 {
        return false;
    }
    let Expression::StaticMemberExpression(callee) = call.callee.without_parentheses() else {
        return false;
    };
    let Expression::Identifier(object) = callee.object.without_parentheses() else {
        return false;
    };
    if callee.optional
        || callee.property.name != "defineProperty"
        || object.name != "Object"
        || object
            .reference_id
            .get()
            .is_none_or(|r| model.scoping.get_reference(r).symbol_id().is_some())
    {
        return false;
    }
    let Some(Expression::Identifier(receiver)) = call.arguments[0].as_expression() else {
        return false;
    };
    if receiver
        .reference_id
        .get()
        .and_then(|r| model.scoping.get_reference(r).symbol_id())
        != Some(class)
    {
        return false;
    }
    if !matches!(call.arguments[1].as_expression(), Some(Expression::StringLiteral(s)) if s.value == "prototype")
    {
        return false;
    }
    let Some(Expression::ObjectExpression(descriptor)) = call.arguments[2].as_expression() else {
        return false;
    };
    let [ObjectPropertyKind::ObjectProperty(p)] = descriptor.properties.as_slice() else {
        return false;
    };
    !p.computed
        && !p.method
        && !p.shorthand
        && p.kind == PropertyKind::Init
        && property_key_name(&p.key).as_deref() == Some("writable")
        && matches!(&p.value, Expression::BooleanLiteral(b) if !b.value)
}

/// Entire adjacent, directly owned installer, in one of the closed layouts.
/// Six-statement layouts retain either the for-init declaration and readonly
/// prototype or the unbounded loop's terminal increment without readonly setup.
/// The latter really is unbounded: no implicit test or exit is invented.
/// No setup-local reference may escape the consumed interval.
fn descriptor_setup(model: &Model<'_, '_>, class: &Class<'_>, getter: &Tree) -> Option<Span> {
    let statements = match model.nodes.parent_kind(class.node_id.get()) {
        AstKind::FunctionBody(b) => b.statements.as_slice(),
        AstKind::BlockStatement(b) => b.body.as_slice(),
        AstKind::Program(p) => p.body.as_slice(),
        _ => return None,
    };
    let index = statements
        .iter()
        .position(|s| matches!(s, Statement::ClassDeclaration(c) if c.span == class.span))?;
    let class_symbol = class.id.as_ref()?.symbol_id.get()?;
    for count in [7, 6] {
        let Some(setup) = statements.get(index + 1..index + 1 + count) else { continue };
        let mut original = setup_projection(model.scoping, class_symbol, getter);
        let projection = if original.statements(setup).is_some_and(|tree| setup_grammar(getter).contains(&tree)) {
            original
        } else if count == 7 {
            let Some((tree, finite)) = finite_setup_projection(model.scoping, class_symbol, getter, setup) else { continue };
            if !finite_setup_grammar(getter).contains(&tree) { continue; }
            finite
        } else { continue };
        let span = Span::new(setup.first()?.span().start, setup.last()?.span().end);
        if projection.locals.keys().copied().filter(|s| *s != class_symbol).any(|symbol| {
            model.scoping.get_resolved_references(symbol)
                .any(|r| !span.contains_inclusive(model.nodes.kind(r.node_id()).span()))
        }) {
            return None;
        }
        // New layouts do not inherit function-style hoisting. Check aliases
        // separately without widening the existing constructor-use grammar.
        if count == 6 && !eager_alias_uses(model, class_symbol, class.span.end,
            model.nodes.parent_id(class.node_id.get()), &mut BTreeSet::new()) {
            return None;
        }
        return Some(span);
    }
    None
}

fn eager_alias_uses(model: &Model<'_, '_>, symbol: SymbolId, ready: u32,
    owner: oxc_semantic::NodeId, seen: &mut BTreeSet<SymbolId>) -> bool {
    if !seen.insert(symbol) || seen.len() > 32 { return false; }
    let callable = model.owning_site(owner).map(|site| site.span);
    model.scoping.get_resolved_references(symbol).all(|r| {
        let AstKind::IdentifierReference(id) = model.nodes.kind(r.node_id()) else { return false };
        // Source order alone does not prove a hoisted callable runs after the
        // class or alias initializer. Those captures need a separate epoch proof.
        if id.span.start < ready || model.owning_site(r.node_id()).map(|site| site.span) != callable {
            return false;
        }
        if let AstKind::VariableDeclarator(d) = model.nodes.parent_kind(r.node_id()) {
            let declaration = model.nodes.parent_id(model.nodes.parent_id(r.node_id()));
            // A conditional var initializer is not an unconditional publication.
            if model.nodes.parent_id(declaration) != owner { return false; }
            let BindingPattern::BindingIdentifier(alias) = &d.id else { return false };
            return alias.symbol_id.get().is_some_and(|s| eager_alias_uses(model, s, d.span.end, owner, seen));
        }
        true
    })
}

// Do not discard a failed owner proof and then accept the same declaration as
// a plain class when a foreign receiver removed its prototype reference. This
// recognizes the adjacent descriptor prefix, not an unrelated Object call.
fn descriptor_prefix(model: &Model<'_, '_>, class: &Class<'_>) -> bool {
    let statements = match model.nodes.parent_kind(class.node_id.get()) {
        AstKind::FunctionBody(b) => b.statements.as_slice(),
        AstKind::BlockStatement(b) => b.body.as_slice(),
        AstKind::Program(p) => p.body.as_slice(),
        _ => return false,
    };
    let Some(index) = statements
        .iter()
        .position(|s| matches!(s, Statement::ClassDeclaration(c) if c.span == class.span))
    else {
        return false;
    };
    let Some(Statement::VariableDeclaration(d)) = statements.get(index + 1) else {
        return false;
    };
    d.declarations.iter().any(|d| matches!(&d.init, Some(Expression::ObjectExpression(o))
        if o.properties.iter().any(|p| matches!(p, ObjectPropertyKind::ObjectProperty(p) if property_key_name(&p.key).as_deref() == Some("key")))))
        && matches!(statements.get(index + 2), Some(Statement::FunctionDeclaration(_)))
        && [index + 5, index + 6].into_iter().any(|i|
            matches!(statements.get(i), Some(Statement::ForStatement(_) | Statement::WhileStatement(_))))
}

fn setup_projection<'a>(scoping: &'a Scoping, class: SymbolId, getter: &Tree) -> Body<'a> {
    Body {
        scoping,
        locals: BTreeMap::from([(class, 0)]),
        depth: 0,
        budget: 1024,
        setup_getter: Some(getter.clone()),
        closed_key: None,
        closed_locals: Vec::new(),
    }
}

// Facts are usable only as part of a successful whole-installer comparison.
// In particular the prefix must still equal the reference's literal descriptor,
// own getter attachment, singleton list and zero index. No facts reach methods.
fn finite_setup_projection<'a>(
    scoping: &'a Scoping, class: SymbolId, getter: &Tree, setup: &[Statement<'_>],
) -> Option<(Tree, Body<'a>)> {
    let mut proof = setup_projection(scoping, class, getter);
    let Tree::Node("body", mut prefix) = proof.statements(setup.get(..5)?)? else { return None };
    let list = match setup.get(3)? {
        Statement::ExpressionStatement(s) => {
            let Expression::AssignmentExpression(a) = s.expression.without_parentheses() else { return None };
            let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else { return None };
            proof.reference(id)?
        }
        Statement::VariableDeclaration(d) => {
            let [d] = d.declarations.as_slice() else { return None };
            let BindingPattern::BindingIdentifier(id) = &d.id else { return None };
            Tree::Local(*proof.locals.get(&id.symbol_id.get()?)?)
        }
        _ => return None,
    };
    let Statement::VariableDeclaration(d) = setup.get(4)? else { return None };
    let [d] = d.declarations.as_slice() else { return None };
    let BindingPattern::BindingIdentifier(id) = &d.id else { return None };
    let index = Tree::Local(*proof.locals.get(&id.symbol_id.get()?)?);
    proof.closed_key = Some(node("member", vec![node("index", vec![list, index]), atom("key")]));
    let Tree::Node("body", tail) = proof.statements(setup.get(5..)?)? else { return None };
    prefix.extend(tail);
    Some((node("body", prefix), proof))
}

fn finite_setup_grammar(getter: &Tree) -> Vec<Tree> {
    static GRAMMAR: OnceLock<Vec<(Tree, Vec<Tree>)>> = OnceLock::new();
    let all = GRAMMAR.get_or_init(|| {
        [SUBSCRIPTION, OBSERVER].into_iter().map(|class| {
            let closed = reference(class)["closed"].clone();
            let path = if class == SUBSCRIPTION { "this._observer" } else { "this._subscription._observer" };
            let mut shapes = Vec::new();
            for conditional in [false, true] {
                for symbol_guard in [false, true] {
                    let mut source = DESCRIPTOR_SETUP.replace("CLOSED_STATE", path);
                    if conditional {
                        source = source.replace("typeof descriptors[index].key != \"object\" || !descriptors[index].key", "typeof descriptors[index].key != \"object\" ? typeof descriptors[index].key != \"object\" : !descriptors[index].key");
                    }
                    let install = "Object.defineProperty(Class.prototype, typeof key == \"symbol\" ? key : key + \"\", descriptors[index]);";
                    let guard = if symbol_guard {
                        "if (typeof key == \"symbol\") { unreachable(); } else { key = key + \"\"; }"
                    } else {
                        "if (typeof key != \"symbol\") { key = key + \"\"; }"
                    };
                    source = source.replace(install, &format!("{guard} Object.defineProperty(Class.prototype, key, descriptors[index]);"));
                    let allocator = Allocator::default();
                    let parsed = Parser::new(&allocator, &source, SourceType::default()).parse();
                    assert!(parsed.errors.is_empty());
                    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
                    let Statement::FunctionDeclaration(f) = &parsed.program.body[0] else { unreachable!() };
                    let BindingPattern::BindingIdentifier(c) = &f.params.items[0].pattern else { unreachable!() };
                    let (tree, _) = finite_setup_projection(semantic.scoping(), c.symbol_id.get().unwrap(), &closed, &f.body.as_ref().unwrap().statements).expect("finite descriptor grammar");
                    shapes.push(tree);
                }
            }
            (closed, shapes)
        }).collect()
    });
    all.iter().find(|(closed, _)| closed == getter).map(|(_, shapes)| shapes.clone()).unwrap_or_default()
}

fn setup_grammar(getter: &Tree) -> Vec<Tree> {
    // Only two closed-field paths exist. Cache both structural grammars once.
    static GRAMMAR: OnceLock<Vec<(Tree, Vec<Tree>)>> = OnceLock::new();
    let all = GRAMMAR.get_or_init(|| {
        [SUBSCRIPTION, OBSERVER].into_iter().map(|class| {
            let closed = reference(class)["closed"].clone();
            let path = if class == SUBSCRIPTION { "this._observer" } else { "this._subscription._observer" };
            let mut alternatives = Vec::new();
            // The literal singleton descriptor needs no property-key coercion.
            // Keep the complete installer, including flags and exact receiver.
            let direct = DESCRIPTOR_SETUP.replace("CLOSED_STATE", path);
            let coercion_start = direct.find("        let key;").unwrap();
            let coercion_end = direct.find("\n    }\n    Object.defineProperty").unwrap();
            let direct = format!("{}        Object.defineProperty(Class.prototype, descriptors[index].key, descriptors[index]);{}", &direct[..coercion_start], &direct[coercion_end..]);
            // These are complete alternative installers, not normalizations.
            let for_init = direct.replace("let index = 0;\n    for (;", "for (let index = 0;");
            let terminal = DESCRIPTOR_SETUP.replace("CLOSED_STATE", path)
                .replace("for (; index < descriptors.length; index++)", "for (;;)")
                .replace("key : key + \"\"", "key : String(key)")
                .replace("\n    }\n    Object.defineProperty(Class, \"prototype\", { writable: false });", "\n        ++index;\n    }");
            for direct in [direct, for_init, terminal] {
                let allocator = Allocator::default();
                let parsed = Parser::new(&allocator, &direct, SourceType::default()).parse();
                assert!(parsed.errors.is_empty());
                let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
                let Statement::FunctionDeclaration(f) = &parsed.program.body[0] else { unreachable!() };
                let BindingPattern::BindingIdentifier(c) = &f.params.items[0].pattern else { unreachable!() };
                let mut proof = setup_projection(semantic.scoping(), c.symbol_id.get().unwrap(), &closed);
                alternatives.push(proof.statements(&f.body.as_ref().unwrap().statements).expect("direct descriptor grammar"));
            }
            for conditional in [false, true] {
                for bundled in [false, true] {
                    for string_call in [false, true] {
                        let mut source = DESCRIPTOR_SETUP.replace("CLOSED_STATE", path);
                        if conditional { source = source.replace("typeof descriptors[index].key != \"object\" || !descriptors[index].key", "typeof descriptors[index].key != \"object\" ? typeof descriptors[index].key != \"object\" : !descriptors[index].key"); }
                        if bundled { source = source.replace("key = descriptors[index].key[Symbol.toPrimitive].call(descriptors[index].key, \"string\");", "let pair = [descriptors[index].key[Symbol.toPrimitive], descriptors[index].key[Symbol.toPrimitive].call]; key = pair[1].call(pair[0], descriptors[index].key, \"string\");"); }
                        if string_call { source = source.replace("key : key + \"\"", "key : String(key)"); }
                        for source in descriptor_forms(&source) {
                        let allocator = Allocator::default();
                        let parsed = Parser::new(&allocator, &source, SourceType::default()).parse();
                        assert!(parsed.errors.is_empty());
                        let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
                        let Statement::FunctionDeclaration(f) = &parsed.program.body[0] else { unreachable!() };
                        let BindingPattern::BindingIdentifier(c) = &f.params.items[0].pattern else { unreachable!() };
                        let mut proof = setup_projection(semantic.scoping(), c.symbol_id.get().unwrap(), &closed);
                        alternatives.push(proof.statements(&f.body.as_ref().unwrap().statements).expect("descriptor grammar"));
                        }
                    }
                }
            }
            (closed, alternatives)
        }).collect()
    });
    all.iter()
        .find(|(closed, _)| closed == getter)
        .map(|(_, shapes)| shapes.clone())
        .unwrap_or_default()
}

// Each alternative retains its entire reachable installer. Separate-list
// references resolve to one local singleton and the same class parameter;
// while loops retain their terminal increment; tuple indices remain exact.
fn descriptor_forms(source: &str) -> Vec<String> {
    let separate = source.replace("descriptors = [descriptors];", "let list = [descriptors];")
        .replace("descriptors[index]", "list[index]")
        .replace("index < descriptors.length", "index < list.length");
    let while_loop = source.replace("for (; index < descriptors.length; index++)", "while (index < descriptors.length)")
        .replace("\n    }\n    Object.defineProperty", "\n        ++index;\n    }\n    Object.defineProperty");
    let start = source.find("        Object.defineProperty(Class.prototype,").unwrap();
    let end = source[start..].find('\n').unwrap() + start;
    let tuple = format!("{}        key = [key, key];\n        Object.defineProperty(Class.prototype, typeof key[1] == \"symbol\" ? key[0] : key[0] + \"\", descriptors[index]);{}", &source[..start], &source[end..]);
    let mut saved_symbol = source.replace("let key;", "let symbol, key;")
        .replace("key = descriptors[index].key;", "symbol = typeof descriptors[index].key == \"symbol\"; key = descriptors[index].key;");
    // Preserve the nested else grouping and the assignment after coercion.
    saved_symbol = saved_symbol.replace("} else if (descriptors[index].key[Symbol.toPrimitive]", "} else { if (descriptors[index].key[Symbol.toPrimitive]")
        .replace("} else { key = String(descriptors[index].key); }", "} else { key = String(descriptors[index].key); } symbol = typeof key == \"symbol\"; }")
        .replace("typeof key == \"symbol\" ? key : key + \"\"", "symbol ? key : String(key)")
        .replace("typeof key == \"symbol\" ? key : String(key)", "symbol ? key : String(key)");
    vec![source.to_string(), separate, while_loop, tuple, saved_symbol]
}

const DESCRIPTOR_SETUP: &str = r#"function setup(Class) {
    let descriptors = { key: "closed" };
    function getClosed() { "use strict"; return CLOSED_STATE === void 0; }
    descriptors.get = getClosed;
    descriptors = [descriptors];
    let index = 0;
    for (; index < descriptors.length; index++) {
        descriptors[index].enumerable = descriptors[index].enumerable || false;
        descriptors[index].configurable = true;
        if ("value" in descriptors[index]) { descriptors[index].writable = true; }
        let key;
        if (typeof descriptors[index].key != "object" || !descriptors[index].key) {
            key = descriptors[index].key;
        } else if (descriptors[index].key[Symbol.toPrimitive] !== void 0) {
            key = descriptors[index].key[Symbol.toPrimitive].call(descriptors[index].key, "string");
            if (typeof key == "object") { throw new TypeError("@@toPrimitive must return a primitive value."); }
        } else { key = String(descriptors[index].key); }
        Object.defineProperty(Class.prototype, typeof key == "symbol" ? key : key + "", descriptors[index]);
    }
    Object.defineProperty(Class, "prototype", { writable: false });
}"#;

type Methods = BTreeMap<String, Tree>;
fn methods(scoping: &Scoping, class: &Class<'_>) -> Option<Methods> {
    if class.super_class.is_some() || !class.decorators.is_empty() {
        return None;
    }
    let mut methods = BTreeMap::new();
    for e in &class.body.body {
        let ClassElement::MethodDefinition(m) = e else {
            return None;
        };
        if m.computed || m.r#static || !m.decorators.is_empty() {
            return None;
        }
        let key = property_key_name(&m.key)?;
        let kind = match key.as_str() {
            "constructor" => MethodDefinitionKind::Constructor,
            "closed" => MethodDefinitionKind::Get,
            _ => MethodDefinitionKind::Method,
        };
        if m.kind != kind || methods.insert(key, body(scoping, &m.value)?).is_some() {
            return None;
        }
    }
    Some(methods)
}

/// Only selected native declaration claims; class binding hygiene belongs to
/// the integrating route. Does not claim function sites or constructor aliases.
pub(crate) fn claims(model: &Model<'_, '_>) -> Vec<FinderClaim> {
    let grammar = grammar();
    let mut result = Vec::new();
    for n in model.nodes.iter() {
        let AstKind::Class(class) = n.kind() else {
            continue;
        };
        if class.r#type != ClassType::ClassDeclaration {
            continue;
        }
        let Some(symbol) = class.id.as_ref().and_then(|id| id.symbol_id.get()) else {
            continue;
        };
        // Cheap API gate avoids projecting unrelated classes.
        if !class.body.body.iter().any(|e| matches!(e, ClassElement::MethodDefinition(m) if property_key_name(&m.key).is_some_and(|k| k == "unsubscribe" || k == "next"))) { continue; }
        let Some(mut own) = methods(model.scoping, class) else {
            continue;
        };
        let closed = own.remove("closed");
        for (role, alternatives, closed_body) in grammar {
            if closed.as_ref().is_some_and(|c| c != closed_body) {
                continue;
            }
            if !alternatives.iter().any(|g| g == &own) {
                continue;
            }
            let setup = descriptor_setup(model, class, closed_body);
            if setup.is_none() && descriptor_prefix(model, class) {
                continue;
            }
            if !constructor_uses(model, symbol, &mut BTreeSet::new(), setup) {
                continue;
            }
            result.push(FinderClaim::ClassBinding {
                role, class_span: class.span, confidence: Confidence::High,
                evidence: vec!["same native class owns the complete ordered subscription lifecycle; exact scoped formal/local flows, guarded callback receiver/arguments and cleanup capture-clear-call; unique instance API and validated optional closed getter".into()],
            });
        }
    }
    result
}

type Grammar = Vec<(&'static str, Vec<Methods>, Tree)>;
fn grammar() -> &'static Grammar {
    static GRAMMAR: OnceLock<Grammar> = OnceLock::new();
    GRAMMAR.get_or_init(|| {
        let sub = reference(SUBSCRIPTION);
        let obs = reference(OBSERVER);
        let mut alternatives = Vec::new();
        for error_form in [0, 1, 2] {
            for swap in [false, true] {
                for callback in [0, 1, 2, 3] {
                    let mut source = OBSERVER.to_string();
                    if error_form != 0 { source = source.replace("observer.error(value);", "let pair = [observer.error, observer.error.call]; pair[1].call(pair[0], observer, value);"); }
                    if error_form == 2 {
                        source = source.replace("sub._observer = void 0;\n        try {\n            let cleanup = sub._cleanup;\n            let value;", "sub._observer = void 0; let cleanup;\n        try {\n            cleanup = sub._cleanup;\n            let value;")
                            .replace("if (observer) { if (observer.error)", "cleanup = 2; if (observer) { if (observer.error)");
                    }
                    if swap { source = source.replace("let failure, failed;", "let failed, failure;").replace("failure = void 0; failed = false;", "failed = false; failure = void 0;").replace("failure = caught; failed = true;", "failed = true; failure = caught;"); }
                    if callback == 1 {
                        source = source.replace("sub._observer = void 0;\n        try {\n            if (typeof (observer ? observer.complete : void 0)", "let done; if (observer) { done = observer.complete; } else { done = void 0; }\n        sub._observer = void 0;\n        try {\n            if (typeof done");
                        source = source.replace("(observer ? observer.complete : void 0).call(observer);", "done.call(observer);");
                    } else if callback == 2 {
                        source = source.replace("sub._observer = void 0;\n        try {\n            if (typeof (observer ? observer.complete : void 0)", "sub._observer = void 0; observer ? observer.complete : void 0;\n        try {\n            if (typeof (observer ? observer.complete : void 0)");
                    }
                    if callback == 3 {
                        source = source.replace("sub._observer = void 0;\n        try {\n            if (typeof (observer ? observer.complete : void 0)", "const done = observer ? observer.complete : void 0;\n        sub._observer = void 0;\n        try {\n            if (typeof done")
                            .replace("(observer ? observer.complete : void 0).call(observer);", "done.call(observer);")
                            .replace("let failure, failed;", "let cleanup = done; let failure, failed;")
                            .replace("let failed, failure;", "let cleanup = done; let failed, failure;")
                            .replace("try {\n            let cleanup = sub._cleanup;\n            if (cleanup !== void 0)", "try {\n            cleanup = sub._cleanup;\n            if (cleanup !== void 0)")
                            .replace("failure = void 0; failed = false;", "cleanup = 2; failure = void 0; failed = false;")
                            .replace("failed = false; failure = void 0;", "cleanup = 2; failed = false; failure = void 0;");
                    }
                    alternatives.push(reference(&source));
                }
            }
        }
        vec![("subscription_class", vec![without_closed(sub.clone())], sub["closed"].clone()),
             ("subscription_observer_class", alternatives.into_iter().map(without_closed).collect(), obs["closed"].clone())]
    })
}
fn without_closed(mut m: Methods) -> Methods {
    m.remove("closed");
    m
}
fn reference(source: &str) -> Methods {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::default()).parse();
    assert!(parsed.errors.is_empty());
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    let Statement::ClassDeclaration(class) = &parsed.program.body[0] else {
        unreachable!()
    };
    methods(semantic.scoping(), class).expect("reference lifecycle grammar")
}

// These are full behavioral patterns, not examples mined by subtree membership.
// Local spellings are irrelevant; the projection resolves every use to its binder.
const SUBSCRIPTION: &str = r#"class Subscription {
    constructor(observer) { this._cleanup = void 0; this._observer = observer; }
    unsubscribe() {
        if (this.isComplete()) { return void 0; } else {
            this._observer = void 0;
            let cleanup = this._cleanup;
            if (cleanup !== void 0) {
                this._cleanup = void 0;
                try { cleanup(); } catch (caught) {}
            }
            return void 0;
        }
    }
    isComplete() { return this._observer === void 0; }
    get closed() { return this._observer === void 0; }
}"#;
const OBSERVER: &str = r#"class SubscriptionObserver {
    constructor(subscription) { this._subscription = subscription; }
    next(input) {
        try {
            if (this._subscription._observer == null) { return void 0; }
            if (this._subscription._observer.next) {
                this._subscription._observer.next(input); return void 0;
            } else { return void 0; }
        } catch (caught) { return void 0; }
    }
    error(input) {
        let sub = this._subscription;
        let observer = sub._observer;
        sub._observer = void 0;
        try {
            let cleanup = sub._cleanup;
            let value;
            if (cleanup === void 0) { value = input; } else {
                sub._cleanup = void 0;
                try { cleanup(); value = input; } catch (caught) { value = input; }
            }
            if (observer) { if (observer.error) { observer.error(value); } }
        } catch (caught) {}
    }
    complete() {
        let sub = this._subscription;
        let observer = sub._observer;
        if (sub._observer === void 0) { return void 0; }
        sub._observer = void 0;
        try {
            if (typeof (observer ? observer.complete : void 0) === "function") {
                (observer ? observer.complete : void 0).call(observer);
            }
        } catch (caught) {}
        let failure, failed;
        try {
            let cleanup = sub._cleanup;
            if (cleanup !== void 0) {
                sub._cleanup = void 0;
                try { cleanup(); } catch (caught) {}
            }
            failure = void 0; failed = false;
        } catch (caught) { failure = caught; failed = true; }
        if (failed) { throw failure; } else { return void 0; }
    }
    isComplete() { return this._subscription.isComplete(); }
    get closed() { return this._subscription._observer === void 0; }
}"#;

#[cfg(test)]
#[path = "subscriptions_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "subscriptions_descriptor_tests.rs"]
mod descriptor_tests;
