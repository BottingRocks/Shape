//! Complete, distinct sparse callback notifier protocols. The closed typed AST
//! projection retains every effect and alpha-renames only scoped local binders.
//! No nested callable or unsupported syntax can donate notifier evidence.
use crate::signals::extract::bindings::{property_key_name, Model};
use crate::signals::analysis::roles::FinderClaim;
use crate::signals::schema::Confidence;
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_parser::Parser;
use oxc_semantic::{Scoping, SemanticBuilder, SymbolId};
use oxc_span::SourceType;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;
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
            None if matches!(id.name.as_str(), "Array" | "Number") => {
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
            Statement::BreakStatement(s) if s.label.is_none() => Some(node("break", vec![])),
            Statement::ForStatement(s) if s.init.is_none() => Some(node(
                "for",
                vec![
                    node(
                        "test",
                        s.test
                            .iter()
                            .map(|e| self.expression(e))
                            .collect::<Option<_>>()?,
                    ),
                    node(
                        "update",
                        s.update
                            .iter()
                            .map(|e| self.expression(e))
                            .collect::<Option<_>>()?,
                    ),
                    self.statement(&s.body)?,
                ],
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
            Statement::VariableDeclaration(s) => {
                // Preserve declaration kind: no var/TDZ or const-write surprises.
                let mut children = vec![atom(format!("{:?}", s.kind))];
                for d in &s.declarations {
                    let binding = self.bind(&d.id)?;
                    children.push(node(
                        "declaration",
                        vec![
                            binding,
                            node(
                                "init",
                                d.init
                                    .iter()
                                    .map(|e| self.expression(e))
                                    .collect::<Option<_>>()?,
                            ),
                        ],
                    ));
                }
                Some(node("variables", children))
            }
            Statement::WhileStatement(s) => Some(node(
                "while",
                vec![self.expression(&s.test)?, self.statement(&s.body)?],
            )),
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
    fn expression(&mut self, e: &Expression<'_>) -> Option<Tree> {
        self.budget = self.budget.checked_sub(1)?;
        self.depth += 1;
        if self.depth > 64 {
            return None;
        }
        let result = match e.without_parentheses() {
            Expression::UpdateExpression(e) => {
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
            Expression::NumericLiteral(v) => Some(node("number", vec![atom(v.value)])),
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
            Expression::AssignmentExpression(a) => Some(node(
                "assign",
                vec![
                    atom(format!("{:?}", a.operator)),
                    self.target(&a.left)?,
                    self.expression(&a.right)?,
                ],
            )),
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
            AssignmentTarget::ComputedMemberExpression(m) if !m.optional => Some(node(
                "index",
                vec![self.expression(&m.object)?, self.expression(&m.expression)?],
            )),
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
    function_body(scoping, function)
}
fn function_body(scoping: &Scoping, function: &Function<'_>) -> Option<Tree> {
    project_function(scoping, function, false)
}
fn project_function(scoping: &Scoping, function: &Function<'_>, strict: bool) -> Option<Tree> {
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
    };
    let mut parameters = Vec::new();
    for p in &function.params.items {
        if p.initializer.is_some() || !p.decorators.is_empty() {
            return None;
        }
        parameters.push(proof.bind(&p.pattern)?);
    }
    let b = function.body.as_ref()?;
    if !(b.directives.is_empty() || strict && b.directives.len() == 1
        && b.directives[0].directive == "use strict") {
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

type Methods = BTreeMap<String, Tree>;
fn methods(scoping: &Scoping, class: &Class<'_>) -> Option<Methods> {
    if class.super_class.is_some() || !class.decorators.is_empty() {
        return None;
    }
    let mut own = BTreeMap::new();
    for element in &class.body.body {
        let ClassElement::MethodDefinition(m) = element else {
            return None;
        };
        if m.computed || m.r#static || !m.decorators.is_empty() {
            return None;
        }
        let key = property_key_name(&m.key)?;
        let kind = if key == "constructor" {
            MethodDefinitionKind::Constructor
        } else {
            MethodDefinitionKind::Method
        };
        if m.kind != kind || own.insert(key, body(scoping, &m.value)?).is_some() {
            return None;
        }
    }
    Some(own)
}

const REVERSE: &str = r#"class Notifier {
    constructor() { this.callbacks = []; }
    register(callback) { this.callbacks.push(callback); return this; }
    unregister(callback) {
        let i = 0;
        for (; i < this.callbacks.length; i++) {
            if (this.callbacks[i] === callback) { this.callbacks[i] = void 0; }
        }
        return this;
    }
    clear() { this.callbacks = []; return this; }
    notify(payload) {
        let snapshot = Array.prototype.slice.call(this.callbacks);
        let count = snapshot.length;
        let accumulator = null;
        while (count - 1 >= 0) {
            if (snapshot[count - 1] != null) {
                try { accumulator = snapshot[count - 1].call(void 0, payload, accumulator); } catch (caught) {}
            }
            --count;
        }
        return accumulator;
    }
}"#;
const FORWARD_NOTIFY: &str = r#"notify(payload) {
    let i = 0;
    let accumulator = null;
    for (; i < this.callbacks.length; i++) {
        if (this.callbacks[i] != null) {
            try { accumulator = this.callbacks[i].call(void 0, payload, accumulator); } catch (caught) {}
        }
    }
    return accumulator;
}"#;
// The cached callback is invoked as a local value, preserving the undefined
// receiver. Number(next) and the break precede the next iteration exactly.
const LOWERED_REVERSE_NOTIFY: &str = r#"notify(payload) {
    let snapshot = Array.prototype.slice.call(this.callbacks);
    let count = snapshot.length;
    let accumulator = null;
    let next;
    for (;; count = Number(next)) {
        next = count - 1;
        if (!(next >= 0)) break;
        let callback = snapshot[next];
        if (callback != null) {
            try { accumulator = callback(payload, accumulator); } catch (caught) {}
        }
    }
    return accumulator;
}"#;
fn lowered_reverse_source() -> String {
    format!("{}{}\n}}", REVERSE.split("notify(payload)").next().unwrap(), LOWERED_REVERSE_NOTIFY)
}
fn forward_source() -> String {
    format!(
        "{}{}\n}}",
        REVERSE.split("notify(payload)").next().unwrap(),
        FORWARD_NOTIFY
    )
}
fn reference(source: &str) -> Methods {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::default()).parse();
    assert!(parsed.errors.is_empty());
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    let Statement::ClassDeclaration(class) = &parsed.program.body[0] else {
        unreachable!()
    };
    methods(semantic.scoping(), class).expect("complete notifier grammar")
}
fn lowered_reverse_grammar() -> &'static Methods {
    static LOWERED: OnceLock<Methods> = OnceLock::new();
    LOWERED.get_or_init(|| reference(&lowered_reverse_source()))
}
fn grammar() -> &'static [(&'static str, Methods)] {
    static GRAMMAR: OnceLock<Vec<(&'static str, Methods)>> = OnceLock::new();
    GRAMMAR.get_or_init(|| {
        vec![
            ("owned_callback_notifier_reverse_class", reference(REVERSE)),
            ("owned_callback_notifier_reverse_class", lowered_reverse_grammar().clone()),
            (
                "owned_callback_notifier_forward_class",
                reference(&forward_source()),
            ),
        ]
    })
}
fn symbol_of(model: &Model<'_, '_>, id: &IdentifierReference<'_>) -> Option<SymbolId> {
    model
        .scoping
        .get_reference(id.reference_id.get()?)
        .symbol_id()
}
fn stable(model: &Model<'_, '_>, symbol: SymbolId) -> bool {
    model.binding_writes(symbol) == 0 && model.scoping.symbol_redeclarations(symbol).is_empty()
}

// This is value hygiene only, not a lazy-subclass purpose proof: an argument may
// enter an owned factory only when its sole use is the superclass operand. The
// lazy family independently proves the returned subclass and register effects.
// Conservatively exclude executable class-definition effects and any method
// syntax that could expose or mutate the superclass through this/super. This is
// a non-escape gate, not a proof of the subclass's purpose or hook installation.
fn subclass_does_not_expose_base(class: &Class<'_>) -> bool {
    if !class.decorators.is_empty() {
        return false;
    }
    class.body.body.iter().all(|element| {
        let ClassElement::MethodDefinition(m) = element else { return false; };
        if m.r#static || m.computed || !m.decorators.is_empty()
            || !matches!(m.kind, MethodDefinitionKind::Constructor | MethodDefinitionKind::Method)
            || m.value.r#async || m.value.generator || m.value.params.rest.is_some()
        { return false; }
        if !m.value.params.items.iter().all(|p| p.initializer.is_none()
            && matches!(&p.pattern, BindingPattern::BindingIdentifier(_))) { return false; }
        let Some(body) = &m.value.body else { return false; };
        body.directives.is_empty() && body.statements.iter().all(|s| match s {
            Statement::ReturnStatement(r) => matches!(&r.argument, Some(Expression::ThisExpression(_))),
            Statement::ExpressionStatement(s) => {
                let Expression::CallExpression(c) = &s.expression else { return false; };
                if c.optional || c.type_arguments.is_some() { return false; }
                match &c.callee {
                    // No base value or receiver reaches the initialization call.
                    Expression::Identifier(_) => c.arguments.is_empty(),
                    Expression::Super(_) => c.arguments.iter().all(|a| match a {
                        Argument::SpreadElement(s) => matches!(&s.argument, Expression::Identifier(i) if i.name == "arguments"),
                        _ => false,
                    }),
                    Expression::StaticMemberExpression(member) => !member.optional
                        && matches!(&member.object, Expression::Super(_))
                        && member.property.name == "register"
                        && c.arguments.iter().all(|a| matches!(a.as_expression(), Some(Expression::Identifier(_)))),
                    _ => false,
                }
            }
            _ => false,
        })
    })
}

fn superclass_transport(
    model: &Model<'_, '_>,
    call: &CallExpression<'_>,
    symbol: SymbolId,
) -> bool {
    if call.optional || call.type_arguments.is_some() || call.arguments.len() != 1 {
        return false;
    }
    if !matches!(call.arguments[0].as_expression(), Some(Expression::Identifier(i)) if symbol_of(model, i) == Some(symbol))
    {
        return false;
    }
    let Expression::Identifier(callee) = &call.callee else {
        return false;
    };
    let Some(callee_symbol) = symbol_of(model, callee) else {
        return false;
    };
    if !stable(model, callee_symbol) {
        return false;
    }
    let AstKind::Function(f) = model
        .nodes
        .kind(model.scoping.symbol_declaration(callee_symbol))
    else {
        return false;
    };
    if f.r#async || f.generator || f.params.rest.is_some() || f.params.items.len() != 1 {
        return false;
    }
    let param = &f.params.items[0];
    if param.initializer.is_some() {
        return false;
    }
    let BindingPattern::BindingIdentifier(id) = &param.pattern else {
        return false;
    };
    let Some(param_symbol) = id.symbol_id.get() else {
        return false;
    };
    if !stable(model, param_symbol) {
        return false;
    }
    let Some(body) = &f.body else {
        return false;
    };
    let [Statement::ClassDeclaration(class), Statement::ReturnStatement(ret)] =
        body.statements.as_slice()
    else {
        return false;
    };
    if !subclass_does_not_expose_base(class) {
        return false;
    }
    let Some(class_symbol) = class.id.as_ref().and_then(|i| i.symbol_id.get()) else {
        return false;
    };
    if !stable(model, class_symbol)
        || !matches!(&ret.argument, Some(Expression::Identifier(i)) if symbol_of(model, i) == Some(class_symbol))
    {
        return false;
    }
    let refs: Vec<_> = model
        .scoping
        .get_resolved_references(param_symbol)
        .collect();
    refs.len() == 1 && refs.iter().all(|r| {
        matches!(model.nodes.kind(r.node_id()), AstKind::IdentifierReference(i)
            if matches!(&class.super_class, Some(Expression::Identifier(base)) if base.span == i.span))
    }) && model.scoping.get_resolved_references(class_symbol).all(|r| {
        matches!(model.nodes.kind(r.node_id()), AstKind::IdentifierReference(i)
            if matches!(&ret.argument, Some(Expression::Identifier(value)) if value.span == i.span))
    })
}
fn readonly_prototype(model: &Model<'_, '_>, call: &CallExpression<'_>, symbol: SymbolId) -> bool {
    if call.optional || call.type_arguments.is_some() || call.arguments.len() != 3 {
        return false;
    }
    let Expression::StaticMemberExpression(callee) = &call.callee else {
        return false;
    };
    let Expression::Identifier(object) = &callee.object else {
        return false;
    };
    if callee.optional
        || callee.property.name != "defineProperty"
        || object.name != "Object"
        || symbol_of(model, object).is_some()
    {
        return false;
    }
    if !matches!(call.arguments[0].as_expression(), Some(Expression::Identifier(i)) if symbol_of(model, i) == Some(symbol))
        || !matches!(call.arguments[1].as_expression(), Some(Expression::StringLiteral(s)) if s.value == "prototype")
    {
        return false;
    }
    let Some(Expression::ObjectExpression(d)) = call.arguments[2].as_expression() else {
        return false;
    };
    let [ObjectPropertyKind::ObjectProperty(p)] = d.properties.as_slice() else {
        return false;
    };
    !p.computed
        && !p.method
        && !p.shorthand
        && p.kind == PropertyKind::Init
        && property_key_name(&p.key).as_deref() == Some("writable")
        && matches!(&p.value, Expression::BooleanLiteral(b) if !b.value)
}
fn constructor_uses(
    model: &Model<'_, '_>,
    symbol: SymbolId,
    seen: &mut BTreeSet<SymbolId>,
) -> bool {
    if !seen.insert(symbol) || seen.len() > 32 || !stable(model, symbol) {
        return false;
    }
    model.scoping.get_resolved_references(symbol).all(|r| {
        let AstKind::IdentifierReference(id) = model.nodes.kind(r.node_id()) else { return false; };
        // Reads before initialization (including aliases) do not transport a value.
        if id.span.start < model.scoping.symbol_span(symbol).end { return false; }
        match model.nodes.parent_kind(r.node_id()) {
            AstKind::NewExpression(n) => matches!(&n.callee, Expression::Identifier(i) if i.span == id.span),
            AstKind::VariableDeclarator(d) if matches!(&d.init, Some(Expression::Identifier(i)) if i.span == id.span) => {
                let BindingPattern::BindingIdentifier(alias) = &d.id else { return false; };
                alias.symbol_id.get().is_some_and(|s| constructor_uses(model, s, seen))
            }
            AstKind::CallExpression(c) => readonly_prototype(model, c, symbol) || superclass_transport(model, c, symbol),
            _ => false,
        }
    })
}
fn native_role(model: &Model<'_, '_>, class: &Class<'_>) -> Option<&'static str> {
    if class.r#type != ClassType::ClassDeclaration {
        return None;
    }
    let symbol = class.id.as_ref()?.symbol_id.get()?;
    let own = methods(model.scoping, class)?;
    let role = grammar().iter().find(|(_, expected)| expected == &own)?.0;
    constructor_uses(model, symbol, &mut BTreeSet::new()).then_some(role)
}

// Declaration-level account only. This grammar is intentionally absent from
// grammar(), native_role() and proves_base(): no helper/base lifetime fact is
// exported by recognizing these own methods.
const INDEXED_DEFINITION_NOTIFY: &str = r#"notify(payload) {
    let snapshot = Array.prototype.slice.call(this.callbacks);
    let index = snapshot.length - 1;
    let accumulator = null;
    for (; index >= 0; index--) {
        if (snapshot[index] != null) {
            try { accumulator = snapshot[index].call(void 0, payload, accumulator); } catch (caught) {}
        }
    }
    return accumulator;
}"#;
fn indexed_definition_grammar() -> &'static Methods {
    static DEFINITION: OnceLock<Methods> = OnceLock::new();
    DEFINITION.get_or_init(|| reference(&format!(
        "{}{}\n}}",
        REVERSE.split("notify(payload)").next().unwrap(),
        INDEXED_DEFINITION_NOTIFY,
    )))
}
fn definition_claim(model: &Model<'_, '_>, class: &Class<'_>) -> Option<FinderClaim> {
    if class.r#type != ClassType::ClassDeclaration {
        return None;
    }
    let symbol = class.id.as_ref()?.symbol_id.get()?;
    if !stable(model, symbol) || methods(model.scoping, class).as_ref() != Some(indexed_definition_grammar()) {
        return None;
    }
    Some(FinderClaim::ClassBinding {
        role: "owned_reverse_snapshot_notifier_definition",
        class_span: class.span,
        confidence: Confidence::High,
        evidence: vec![
            "complete owned five-method sparse callback lifecycle and indexed reverse snapshot fold as defined; exact source-resolved global Array access, void-zero callback receiver, payload/accumulator order, empty catch and returned accumulator; client mutation, inherited runtime behavior and callback/intrinsic purity are not established; definition-only claim, not a proven initialized base protocol".into(),
        ],
    })
}

/// Complete own contracts only. Does not claim aliases, functions or methods.
pub(crate) fn claims(model: &Model<'_, '_>) -> Vec<FinderClaim> {
    model.nodes.iter().filter_map(|n| {
        let AstKind::Class(class) = n.kind() else { return None; };
        // Preserve accepted042 priority and metadata. Only an unclaimed native
        // class can receive the weaker, separate definition description.
        let Some(role) = native_role(model, class) else {
            return definition_claim(model, class);
        };
        Some(FinderClaim::ClassBinding {
            role, class_span: class.span, confidence: Confidence::High,
            evidence: vec![if role == "owned_callback_notifier_reverse_class"
                && methods(model.scoping, class).as_ref() == Some(lowered_reverse_grammar()) {
                "complete owned sparse callback lifecycle; exact slice snapshot, descending traversal, undefined callback receiver, payload/accumulator order, swallowed callback failure, returned accumulator; stable constructor aliases and closed uses"
            } else if role == "owned_callback_notifier_reverse_class" {
                "complete owned sparse callback lifecycle; exact slice snapshot, descending count-minus-one reads, void-zero receiver, payload/accumulator order, swallowed callback failure, returned accumulator; stable constructor aliases and closed uses"
            } else {
                "complete owned sparse callback lifecycle; same live callbacks list on ascending bounds and reads (reentrant mutation preserved), void-zero receiver, payload/accumulator order, swallowed callback failure, returned accumulator; stable constructor aliases and closed uses"
            }.into()],
        })
    }).collect()
}

// An ordinary base is a constructor plus four installations, never a name or a
// property bag. Every constructor reference must be an installation, a stable
// alias, or a consumer following all installations in the same statement list.
fn ordinary_base(model: &Model<'_, '_>, symbol: SymbolId) -> bool {
    use oxc_semantic::NodeId;
    fn statement_owner(model: &Model<'_, '_>, id: NodeId) -> Option<NodeId> {
        if !matches!(model.nodes.kind(id), AstKind::ExpressionStatement(_) | AstKind::VariableDeclaration(_) | AstKind::Function(_)) {
            return None;
        }
        let parent = model.nodes.parent_id(id);
        matches!(model.nodes.kind(parent), AstKind::FunctionBody(_) | AstKind::Program(_)).then_some(parent)
    }
    fn strict_tree(model: &Model<'_, '_>, symbol: SymbolId, owner: NodeId) -> Option<Tree> {
        if !stable(model, symbol) { return None; }
        let declaration = model.scoping.symbol_declaration(symbol);
        if statement_owner(model, declaration) != Some(owner) { return None; }
        let AstKind::Function(f) = model.nodes.kind(declaration) else { return None; };
        // Strictness is essential for the callback receiver and constructor
        // behavior to match a native class method.
        let b = f.body.as_ref()?;
        if b.directives.len() != 1 || b.directives[0].directive != "use strict" { return None; }
        project_function(model.scoping, f, true)
    }
    fn constructed(model: &Model<'_, '_>, symbol: SymbolId, seen: &mut BTreeSet<SymbolId>) -> bool {
        if !seen.insert(symbol) || seen.len() > 32 { return false; }
        model.scoping.get_resolved_references(symbol).any(|r| match model.nodes.parent_kind(r.node_id()) {
            AstKind::NewExpression(n) => matches!(&n.callee, Expression::Identifier(i) if symbol_of(model, i) == Some(symbol)),
            AstKind::VariableDeclarator(d) => match &d.id {
                BindingPattern::BindingIdentifier(id) => id.symbol_id.get().is_some_and(|s| constructed(model, s, seen)),
                _ => false,
            },
            _ => false,
        })
    }
    let declaration = model.scoping.symbol_declaration(symbol);
    let Some(owner) = statement_owner(model, declaration) else { return false; };
    let Some(constructor) = strict_tree(model, symbol, owner) else { return false; };
    let mut own = BTreeMap::from([("constructor".to_string(), constructor)]);
    let mut aliases = BTreeSet::new();
    let mut pending = vec![symbol];
    let mut installed_end = 0;
    let mut consumers = Vec::new();
    let mut method_symbols = BTreeMap::new();
    while let Some(current) = pending.pop() {
        if !stable(model, current) || !aliases.insert(current) || aliases.len() > 32 { return false; }
        for r in model.scoping.get_resolved_references(current) {
            let AstKind::IdentifierReference(id) = model.nodes.kind(r.node_id()) else { return false; };
            if id.span.start < model.scoping.symbol_span(current).end { return false; }
            let parent = model.nodes.parent_id(r.node_id());
            match model.nodes.kind(parent) {
                AstKind::VariableDeclarator(d) if matches!(&d.init, Some(Expression::Identifier(i)) if i.span == id.span) => {
                    let BindingPattern::BindingIdentifier(binding) = &d.id else { return false; };
                    let Some(alias) = binding.symbol_id.get() else { return false; };
                    let statement = model.nodes.parent_id(parent);
                    if statement_owner(model, statement) != Some(owner)
                        || id.span.start < model.scoping.symbol_span(current).end { return false; }
                    pending.push(alias);
                }
                AstKind::StaticMemberExpression(prototype) if !prototype.optional && prototype.property.name == "prototype" => {
                    let member_id = model.nodes.parent_id(parent);
                    let (value, key, end, statement) = match model.nodes.kind(member_id) {
                        AstKind::StaticMemberExpression(member) if !member.optional => {
                            let assignment_id = model.nodes.parent_id(member_id);
                            let AstKind::AssignmentExpression(assignment) = model.nodes.kind(assignment_id) else { return false; };
                            if assignment.operator != oxc_syntax::operator::AssignmentOperator::Assign
                                || !matches!(&assignment.left, AssignmentTarget::StaticMemberExpression(m) if m.span == member.span) { return false; }
                            let Expression::Identifier(value) = &assignment.right else { return false; };
                            (value, member.property.name.to_string(), assignment.span.end, model.nodes.parent_id(assignment_id))
                        }
                        AstKind::CallExpression(call) if !call.optional && call.type_arguments.is_none() && call.arguments.len() == 3 => {
                            let Expression::StaticMemberExpression(callee) = &call.callee else { return false; };
                            let Expression::Identifier(object) = &callee.object else { return false; };
                            if callee.optional || callee.property.name != "defineProperty" || object.name != "Object"
                                || symbol_of(model, object).is_some()
                                || !matches!(call.arguments[0].as_expression(), Some(Expression::StaticMemberExpression(p)) if p.span == prototype.span) { return false; }
                            let Some(Expression::StringLiteral(key)) = call.arguments[1].as_expression() else { return false; };
                            let Some(Expression::ObjectExpression(descriptor)) = call.arguments[2].as_expression() else { return false; };
                            let mut fields = BTreeSet::new();
                            let mut value = None;
                            for field in &descriptor.properties {
                                let ObjectPropertyKind::ObjectProperty(field) = field else { return false; };
                                if field.computed || field.method || field.shorthand || field.kind != PropertyKind::Init { return false; }
                                let Some(name) = property_key_name(&field.key) else { return false; };
                                if !fields.insert(name.clone()) { return false; }
                                match name.as_str() {
                                    "value" => {
                                        let Expression::Identifier(id) = &field.value else { return false; };
                                        value = Some(id);
                                    }
                                    "writable" | "configurable" if matches!(&field.value, Expression::BooleanLiteral(b) if b.value) => {}
                                    "enumerable" if matches!(&field.value, Expression::BooleanLiteral(_)) => {}
                                    _ => return false,
                                }
                            }
                            if fields.len() != 4 { return false; }
                            let Some(value) = value else { return false; };
                            (value, key.value.to_string(), call.span.end, model.nodes.parent_id(member_id))
                        }
                        _ => return false,
                    };
                    if statement_owner(model, statement) != Some(owner) { return false; }
                    let Some(method) = symbol_of(model, value) else { return false; };
                    let Some(tree) = strict_tree(model, method, owner) else { return false; };
                    if !matches!(key.as_str(), "register" | "unregister" | "clear" | "notify")
                        || own.insert(key, tree).is_some()
                        || method_symbols.insert(method, value.span).is_some() { return false; }
                    installed_end = installed_end.max(end);
                }
                AstKind::CallExpression(call) if superclass_transport(model, call, current) => {
                    let call_parent = model.nodes.parent_id(parent);
                    let AstKind::VariableDeclarator(d) = model.nodes.kind(call_parent) else { return false; };
                    let BindingPattern::BindingIdentifier(binding) = &d.id else { return false; };
                    let Some(child) = binding.symbol_id.get() else { return false; };
                    if !constructor_uses(model, child, &mut BTreeSet::new())
                        || !constructed(model, child, &mut BTreeSet::new())
                        || statement_owner(model, model.nodes.parent_id(call_parent)) != Some(owner) { return false; }
                    consumers.push(call.span.start);
                }
                AstKind::NewExpression(n) if matches!(&n.callee, Expression::Identifier(i) if i.span == id.span) => {
                    // Only a direct statement or declaration initializer is
                    // accepted; conditional/nested constructions are unsupported.
                    let outer = model.nodes.parent_id(parent);
                    let statement = if matches!(model.nodes.kind(outer), AstKind::VariableDeclarator(_)) {
                        model.nodes.parent_id(outer)
                    } else { outer };
                    if statement_owner(model, statement) != Some(owner) { return false; }
                    consumers.push(n.span.start);
                }
                _ => return false,
            }
        }
    }
    // An implementation function cannot also be a mutable donor or escape via
    // another carrier. Only its single exact installation is owned here.
    for (method, span) in method_symbols {
        let refs: Vec<_> = model.scoping.get_resolved_references(method).collect();
        if refs.len() != 1 || !matches!(model.nodes.kind(refs[0].node_id()), AstKind::IdentifierReference(i) if i.span == span) { return false; }
    }
    if consumers.is_empty() || consumers.iter().any(|start| *start <= installed_end) { return false; }
    let last_consumer = *consumers.iter().max().unwrap();
    // A dead installation after an unconditional abrupt exit proves nothing.
    let statements = match model.nodes.kind(owner) {
        AstKind::FunctionBody(body) => &body.statements,
        AstKind::Program(program) => &program.body,
        _ => return false,
    };
    for statement in statements {
        match statement {
            Statement::ReturnStatement(r) if r.span.start < last_consumer => return false,
            Statement::ThrowStatement(t) if t.span.start < last_consumer => return false,
            _ => {}
        }
    }
    grammar().iter().any(|(_, expected)| expected == &own)
}

/// Stable aliases resolve to the complete native or ordinary owned contract.
/// Ordinary bases are dependency proofs only; existing function names survive.
pub(crate) fn proves_base(model: &Model<'_, '_>, symbol: SymbolId) -> bool {
    fn resolve(model: &Model<'_, '_>, symbol: SymbolId, seen: &mut BTreeSet<SymbolId>) -> bool {
        if !stable(model, symbol) || !seen.insert(symbol) || seen.len() > 32 {
            return false;
        }
        match model.nodes.kind(model.scoping.symbol_declaration(symbol)) {
            AstKind::Class(c) => native_role(model, c).is_some(),
            AstKind::Function(_) => ordinary_base(model, symbol),
            AstKind::VariableDeclarator(d) => {
                let Some(Expression::Identifier(id)) = &d.init else {
                    return false;
                };
                let Some(origin) = symbol_of(model, id) else {
                    return false;
                };
                model.scoping.symbol_span(origin).end < id.span.start
                    && resolve(model, origin, seen)
            }
            _ => false,
        }
    }
    resolve(model, symbol, &mut BTreeSet::new())
}

#[cfg(test)]
#[path = "callback_notifier_tests.rs"]
mod tests;
