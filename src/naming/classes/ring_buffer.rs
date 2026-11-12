//! Closed, additive owned RingBuffer proof. Complete method trees preserve every
//! receiver, operand, branch and statement order. Nested callables have no
//! production. Only scoped formal/local names are alpha-renamed.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_parser::Parser;
use oxc_semantic::{Scoping, SemanticBuilder, SymbolId};
use oxc_span::SourceType;

use crate::signals::extract::bindings::{property_key_name, Model};
use crate::signals::analysis::roles::FinderClaim;
use crate::signals::schema::Confidence;

#[derive(Debug, Clone, PartialEq, Eq)]
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

struct Body<'a> {
    scoping: &'a Scoping,
    locals: BTreeMap<SymbolId, usize>,
    budget: usize,
    depth: usize,
}
impl Body<'_> {
    fn bind(&mut self, pattern: &BindingPattern<'_>) -> Option<Tree> {
        let BindingPattern::BindingIdentifier(id) = pattern else {
            return None;
        };
        let symbol = id.symbol_id.get()?;
        if self.locals.contains_key(&symbol)
            || !self.scoping.symbol_redeclarations(symbol).is_empty()
        {
            return None;
        }
        let index = self.locals.len();
        self.locals.insert(symbol, index);
        Some(Tree::Local(index))
    }
    fn reference(&self, id: &IdentifierReference<'_>) -> Option<Tree> {
        match self
            .scoping
            .get_reference(id.reference_id.get()?)
            .symbol_id()
        {
            Some(s) => Some(Tree::Local(*self.locals.get(&s)?)),
            None if matches!(id.name.as_str(), "Infinity" | "Number") => {
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
    fn statement(&mut self, statement: &Statement<'_>) -> Option<Tree> {
        self.budget = self.budget.checked_sub(1)?;
        self.depth += 1;
        if self.depth > 64 {
            return None;
        }
        let result = match statement {
            Statement::BlockStatement(b) => self.statements(&b.body),
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
            Statement::VariableDeclaration(d) => {
                let mut values = vec![atom(format!("{:?}", d.kind))];
                for d in &d.declarations {
                    let binding = self.bind(&d.id)?;
                    values.push(node(
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
                Some(node("variables", values))
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
            _ => None,
        };
        self.depth -= 1;
        result
    }
    fn expression(&mut self, expression: &Expression<'_>) -> Option<Tree> {
        self.budget = self.budget.checked_sub(1)?;
        self.depth += 1;
        if self.depth > 64 {
            return None;
        }
        let result = match expression.without_parentheses() {
            Expression::Identifier(id) => self.reference(id),
            Expression::ThisExpression(_) => Some(node("this", vec![])),
            Expression::NumericLiteral(n) => Some(node("number", vec![atom(n.value)])),
            Expression::BooleanLiteral(b) => Some(node("boolean", vec![atom(b.value)])),
            Expression::StringLiteral(s) => Some(node("string", vec![atom(s.value.as_str())])),
            Expression::ArrayExpression(a) if a.elements.is_empty() => {
                Some(node("empty array", vec![]))
            }
            Expression::StaticMemberExpression(m) if !m.optional => Some(node(
                "member",
                vec![self.expression(&m.object)?, atom(m.property.name.as_str())],
            )),
            Expression::ComputedMemberExpression(m) if !m.optional => Some(node(
                "index",
                vec![self.expression(&m.object)?, self.expression(&m.expression)?],
            )),
            Expression::BinaryExpression(b) => Some(node(
                "binary",
                vec![
                    atom(format!("{:?}", b.operator)),
                    self.expression(&b.left)?,
                    self.expression(&b.right)?,
                ],
            )),
            Expression::LogicalExpression(b) => Some(node(
                "logical",
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
            Expression::CallExpression(c) if !c.optional && c.type_arguments.is_none() => {
                let mut args = vec![self.expression(&c.callee)?];
                for arg in &c.arguments {
                    args.push(self.expression(arg.as_expression()?)?);
                }
                Some(node("call", args))
            }
            _ => None,
        };
        self.depth -= 1;
        result
    }
    fn target(&mut self, target: &AssignmentTarget<'_>) -> Option<Tree> {
        match target {
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
fn body(scoping: &Scoping, function: &Function<'_>) -> Option<Tree> {
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
        budget: 512,
        depth: 0,
    };
    let mut params = Vec::new();
    for p in &function.params.items {
        if p.initializer.is_some() || !p.decorators.is_empty() {
            return None;
        }
        params.push(proof.bind(&p.pattern)?);
    }
    let b = function.body.as_ref()?;
    if !b.directives.is_empty() {
        return None;
    }
    Some(node(
        "method",
        vec![node("parameters", params), proof.statements(&b.statements)?],
    ))
}

type Methods = BTreeMap<String, Tree>;
fn methods(scoping: &Scoping, class: &Class<'_>) -> Option<Methods> {
    if class.super_class.is_some() || !class.decorators.is_empty() {
        return None;
    }
    let mut result = BTreeMap::new();
    for element in &class.body.body {
        let ClassElement::MethodDefinition(m) = element else {
            return None;
        };
        let key = property_key_name(&m.key)?;
        if m.r#static
            || m.computed
            || !m.decorators.is_empty()
            || (key == "constructor" && m.kind != MethodDefinitionKind::Constructor)
            || (key != "constructor" && m.kind != MethodDefinitionKind::Method)
            || result.insert(key, body(scoping, &m.value)?).is_some()
        {
            return None;
        }
    }
    Some(result)
}

// Constructor values may only flow through stable, ordered plain aliases and
// into `new`. Member writes, calls, return escapes and use before origin fail.
fn constructor_uses(
    model: &Model<'_, '_>,
    symbol: SymbolId,
    ready: u32,
    seen: &mut BTreeSet<SymbolId>,
    constructed: &mut bool,
) -> bool {
    if !seen.insert(symbol)
        || seen.len() > 64
        || model.binding_writes(symbol) != 0
        || !model.scoping.symbol_redeclarations(symbol).is_empty()
    {
        return false;
    }
    model.scoping.get_resolved_references(symbol).all(|r| {
        let AstKind::IdentifierReference(id) = model.nodes.kind(r.node_id()) else { return false };
        if id.span.start < ready { return false; }
        match model.nodes.parent_kind(r.node_id()) {
            AstKind::NewExpression(n) => { let valid = matches!(&n.callee, Expression::Identifier(i) if i.span == id.span); *constructed |= valid; valid },
            AstKind::VariableDeclarator(d) if matches!(&d.init, Some(Expression::Identifier(i)) if i.span == id.span) => {
                let BindingPattern::BindingIdentifier(alias) = &d.id else { return false };
                alias.symbol_id.get().is_some_and(|s| constructor_uses(model, s, d.span.end, seen, constructed))
            }
            _ => false,
        }
    })
}

// Separate from the legacy transport above: every writer belongs to one closed
// constructor family. Readiness is a structural dominance obligation, including
// every publication/call of a hoisted consumer, rather than a body text offset.
#[derive(Clone, Copy)]
struct EpochWrite {
    node: oxc_semantic::NodeId,
    source: Option<SymbolId>,
}
fn identifier_symbol(model: &Model<'_, '_>, id: &IdentifierReference<'_>) -> Option<SymbolId> {
    model
        .scoping
        .get_reference(id.reference_id.get()?)
        .symbol_id()
}
fn copy_source(model: &Model<'_, '_>, expression: &Expression<'_>) -> Option<SymbolId> {
    let Expression::Identifier(id) = expression else {
        return None;
    };
    identifier_symbol(model, id)
}
fn copy_destination(model: &Model<'_, '_>, node: oxc_semantic::NodeId) -> Option<SymbolId> {
    match model.nodes.kind(node) {
        AstKind::AssignmentExpression(a)
            if a.operator == oxc_syntax::operator::AssignmentOperator::Assign =>
        {
            let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else {
                return None;
            };
            identifier_symbol(model, id)
        }
        AstKind::VariableDeclarator(d) => {
            let BindingPattern::BindingIdentifier(id) = &d.id else {
                return None;
            };
            id.symbol_id.get()
        }
        _ => None,
    }
}
fn copy_rhs<'a>(model: &Model<'_, 'a>, node: oxc_semantic::NodeId) -> Option<&'a Expression<'a>> {
    match model.nodes.kind(node) {
        AstKind::AssignmentExpression(a)
            if a.operator == oxc_syntax::operator::AssignmentOperator::Assign =>
        {
            Some(&a.right)
        }
        AstKind::VariableDeclarator(d) => d.init.as_ref(),
        _ => None,
    }
}

/// Only unconditional, effect-free standalone copies and class declarations can
/// establish readiness. A writer in one branch never dominates a sibling or a
/// join; a writer inside a callable never dominates its declaration/publication.
fn writer_statement(
    model: &Model<'_, '_>,
    node: oxc_semantic::NodeId,
) -> Option<(oxc_semantic::NodeId, oxc_semantic::NodeId)> {
    let statement = match model.nodes.kind(node) {
        AstKind::Class(c) if c.is_declaration() => node,
        AstKind::AssignmentExpression(_)
            if matches!(
                model.nodes.parent_kind(node),
                AstKind::ExpressionStatement(_)
            ) =>
        {
            model.nodes.parent_id(node)
        }
        AstKind::VariableDeclarator(_) => {
            let parent = model.nodes.parent_id(node);
            let AstKind::VariableDeclaration(d) = model.nodes.kind(parent) else {
                return None;
            };
            // Multiple declarators have their own left-to-right evaluation; this
            // bounded path deliberately requires one declaration per statement.
            if d.declarations.len() != 1 {
                return None;
            }
            parent
        }
        _ => return None,
    };
    let container = model.nodes.parent_id(statement);
    matches!(
        model.nodes.kind(container),
        AstKind::BlockStatement(_) | AstKind::FunctionBody(_) | AstKind::Program(_)
    )
    .then_some((statement, container))
}

// Lexical declaration activation is an epoch boundary too. `var` is initialized
// on callable entry and its bare declaration does not reset an installed value.
fn assignment_destination_live(
    model: &Model<'_, '_>,
    declaration: oxc_semantic::NodeId,
    site: oxc_semantic::NodeId,
) -> bool {
    use oxc_span::GetSpan;
    let parent = model.nodes.parent_id(declaration);
    let AstKind::VariableDeclaration(d) = model.nodes.kind(parent) else {
        return false;
    };
    if d.kind.is_var() {
        return true;
    }
    if d.kind != VariableDeclarationKind::Let {
        return false;
    }
    let container = model.nodes.parent_id(parent);
    if !matches!(
        model.nodes.kind(container),
        AstKind::BlockStatement(_) | AstKind::FunctionBody(_) | AstKind::Program(_)
    ) {
        return false;
    }
    let mut child = site;
    for ancestor in model.nodes.ancestor_ids(site) {
        if ancestor == container {
            return d.span.end <= model.nodes.kind(child).span().start;
        }
        if matches!(
            model.nodes.kind(ancestor),
            AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) | AstKind::Class(_)
        ) {
            return false;
        }
        child = ancestor;
    }
    false
}

struct Epochs<'m, 's, 'a> {
    model: &'m Model<'s, 'a>,
    writers: BTreeMap<SymbolId, Vec<EpochWrite>>,
}
impl Epochs<'_, '_, '_> {
    /// A function declaration does not execute its body. Follow its actual
    /// entries/publications, including stable local callback copies. An unused
    /// local callable has no entry obligation; an exported/global binding does
    /// not acquire that exemption merely because scoped references are absent.
    fn consumer_entries_ready(
        &self,
        constructor: SymbolId,
        callable: SymbolId,
        visiting: &mut BTreeSet<(SymbolId, oxc_semantic::NodeId)>,
        aliases: &mut BTreeSet<SymbolId>,
    ) -> bool {
        let model = self.model;
        if aliases.len() >= 64
            || !aliases.insert(callable)
            || model.binding_writes(callable) != 0
            || !model.scoping.symbol_redeclarations(callable).is_empty()
        {
            return false;
        }
        let declaration = model.scoping.symbol_declaration(callable);
        let local = model.nodes.ancestor_ids(declaration).any(|id| {
            matches!(
                model.nodes.kind(id),
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
            )
        });
        // Resolved references do not close a program-scope/public callable's
        // implicit entry set. In particular, a copy ending in an unused local
        // alias cannot erase the original public entry obligation. This bounded
        // proof supports only callable bindings enclosed by a local function.
        if !local {
            return false;
        }
        let valid = model
            .scoping
            .get_resolved_references(callable)
            .all(|reference| {
                let site = reference.node_id();
                // A callback alias must itself be initialized before every copy,
                // call or publication. This is independent of constructor readiness.
                if matches!(
                    model.nodes.kind(declaration),
                    AstKind::VariableDeclarator(_)
                ) {
                    let activation = Epochs {
                        model,
                        writers: BTreeMap::from([(
                            callable,
                            vec![EpochWrite {
                                node: declaration,
                                source: None,
                            }],
                        )]),
                    };
                    if !activation.ready(callable, site, visiting) {
                        return false;
                    }
                }
                let parent = model.nodes.parent_id(site);
                if let AstKind::VariableDeclarator(d) = model.nodes.kind(parent) {
                    if d.init.as_ref().and_then(|e| copy_source(model, e)) == Some(callable) {
                        let Some(alias) = copy_destination(model, parent) else {
                            return false;
                        };
                        let AstKind::VariableDeclaration(decl) = model.nodes.parent_kind(parent)
                        else {
                            return false;
                        };
                        if decl.kind.is_using() || writer_statement(model, parent).is_none() {
                            return false;
                        }
                        return self.consumer_entries_ready(constructor, alias, visiting, aliases);
                    }
                }
                self.ready(constructor, site, visiting)
            });
        aliases.remove(&callable);
        valid
    }

    fn ready(
        &self,
        symbol: SymbolId,
        site: oxc_semantic::NodeId,
        visiting: &mut BTreeSet<(SymbolId, oxc_semantic::NodeId)>,
    ) -> bool {
        if visiting.len() >= 128 || !visiting.insert((symbol, site)) {
            return false;
        }
        let result = self.ready_inner(symbol, site, visiting);
        visiting.remove(&(symbol, site));
        result
    }
    fn ready_inner(
        &self,
        symbol: SymbolId,
        site: oxc_semantic::NodeId,
        visiting: &mut BTreeSet<(SymbolId, oxc_semantic::NodeId)>,
    ) -> bool {
        use oxc_span::GetSpan;
        let model = self.model;
        let mut child = site;
        for parent in model.nodes.ancestor_ids(site) {
            match model.nodes.kind(parent) {
                AstKind::Function(f) => {
                    // A declaration is hoisted. Validate every resolved call or
                    // publication at its actual execution site, including ones
                    // textually before the declaration. Anonymous callbacks and
                    // methods have no closed publication protocol here.
                    if !f.is_declaration() || f.r#async || f.generator {
                        return false;
                    }
                    let Some(function) = f.id.as_ref().and_then(|i| i.symbol_id.get()) else {
                        return false;
                    };
                    return self.consumer_entries_ready(
                        symbol,
                        function,
                        visiting,
                        &mut BTreeSet::new(),
                    );
                }
                AstKind::ArrowFunctionExpression(_) | AstKind::Class(_) => return false,
                AstKind::BlockStatement(_) | AstKind::FunctionBody(_) | AstKind::Program(_) => {
                    if self.writers.get(&symbol).is_some_and(|writers| {
                        writers.iter().any(|write| {
                            writer_statement(model, write.node).is_some_and(
                                |(statement, container)| {
                                    container == parent
                                        && model.nodes.kind(statement).span().end
                                            <= model.nodes.kind(child).span().start
                                },
                            )
                        })
                    }) {
                        return true;
                    }
                }
                _ => {}
            }
            child = parent;
        }
        false
    }
}

fn epoch_constructor_uses(model: &Model<'_, '_>, origin: SymbolId) -> bool {
    // Dynamic scope can introduce writers not represented by SymbolId. This is
    // an additive proof, so unsupported dynamic scope refuses the entire path.
    if model.nodes.iter().any(|n| match n.kind() {
        AstKind::WithStatement(_) => true,
        AstKind::CallExpression(c) => {
            matches!(c.callee.without_parentheses(), Expression::Identifier(i) if i.name == "eval")
        }
        _ => false,
    }) {
        return false;
    }
    let mut family = BTreeSet::from([origin]);
    let mut writers = BTreeMap::new();
    let mut reads = Vec::new();
    let mut pending = vec![origin];
    let mut constructed = false;
    while let Some(symbol) = pending.pop() {
        if family.len() > 64 || !model.scoping.symbol_redeclarations(symbol).is_empty() {
            return false;
        }
        let declaration = model.scoping.symbol_declaration(symbol);
        let mut own_writers = Vec::new();
        match model.nodes.kind(declaration) {
            AstKind::Class(c)
                if c.is_declaration()
                    && methods(model.scoping, c).is_some_and(|m| grammar().contains(&m)) =>
            {
                own_writers.push(EpochWrite {
                    node: declaration,
                    source: None,
                });
            }
            AstKind::VariableDeclarator(d)
                if matches!(&d.id, BindingPattern::BindingIdentifier(_)) =>
            {
                let AstKind::VariableDeclaration(decl) = model.nodes.parent_kind(declaration)
                else {
                    return false;
                };
                if decl.kind.is_using() {
                    return false;
                }
                if let Some(init) = &d.init {
                    let Some(source) = copy_source(model, init) else {
                        return false;
                    };
                    own_writers.push(EpochWrite {
                        node: declaration,
                        source: Some(source),
                    });
                }
            }
            _ => return false,
        }
        for reference in model.scoping.get_resolved_references(symbol) {
            let site = reference.node_id();
            let AstKind::IdentifierReference(id) = model.nodes.kind(site) else {
                return false;
            };
            let parent = model.nodes.parent_id(site);
            if reference.is_write() {
                if !assignment_destination_live(model, declaration, parent) {
                    return false;
                }
                let Some(rhs) = copy_rhs(model, parent) else {
                    return false;
                };
                if copy_destination(model, parent) != Some(symbol) || reference.is_read() {
                    return false;
                }
                let Some(source) = copy_source(model, rhs) else {
                    return false;
                };
                own_writers.push(EpochWrite {
                    node: parent,
                    source: Some(source),
                });
            } else {
                match model.nodes.kind(parent) {
                    AstKind::NewExpression(n) if matches!(&n.callee, Expression::Identifier(i) if i.span == id.span) =>
                    {
                        constructed = true;
                        reads.push((symbol, site));
                    }
                    _ if copy_rhs(model, parent).is_some_and(
                        |e| matches!(e, Expression::Identifier(i) if i.span == id.span),
                    ) =>
                    {
                        let Some(destination) = copy_destination(model, parent) else {
                            return false;
                        };
                        if family.insert(destination) {
                            pending.push(destination);
                        }
                    }
                    _ => return false,
                }
            }
        }
        for write in &own_writers {
            if writer_statement(model, write.node).is_none() {
                return false;
            }
            if let Some(source) = write.source {
                if family.insert(source) {
                    pending.push(source);
                }
            }
        }
        writers.insert(symbol, own_writers);
    }
    let epochs = Epochs { model, writers };
    // Enumerating every resolved writer above is essential: no last-writer
    // shortcut, no foreign epoch, and no unreachable-producer grammar exemption.
    constructed
        && epochs.writers.values().flatten().all(|w| {
            w.source
                .is_none_or(|source| epochs.ready(source, w.node, &mut BTreeSet::new()))
        })
        && reads
            .into_iter()
            .all(|(symbol, site)| epochs.ready(symbol, site, &mut BTreeSet::new()))
}

pub(crate) fn claims(model: &Model<'_, '_>) -> Vec<FinderClaim> {
    let mut result = Vec::new();
    for n in model.nodes.iter() {
        let AstKind::Class(class) = n.kind() else {
            continue;
        };
        if !class.is_declaration() {
            continue;
        }
        let Some(symbol) = class.id.as_ref().and_then(|id| id.symbol_id.get()) else {
            continue;
        };
        let Some(own) = methods(model.scoping, class) else {
            continue;
        };
        let mut constructed = false;
        if !grammar().contains(&own) {
            continue;
        }
        let legacy = constructor_uses(
            model,
            symbol,
            class.span.end,
            &mut BTreeSet::new(),
            &mut constructed,
        ) && constructed;
        if !legacy && !epoch_constructor_uses(model, symbol) {
            continue;
        }
        result.push(FinderClaim::ClassBinding {
            role: "owned_ring_buffer_class", class_span: class.span, confidence: Confidence::High,
            evidence: vec![if legacy {
                "complete owned RingBuffer contract: exact zero/non-number Infinity default, scoped exhaustive capacity origin, same receiver backing array/cursor/capacity/length, saturated put, oldest-first slice/concat and full reset; stable ordered constructor aliases"
            } else {
                "complete owned RingBuffer contract: exact zero/non-number Infinity default, scoped exhaustive capacity origin, same receiver backing array/cursor/capacity/length, saturated put, oldest-first slice/concat and full reset; every scoped constructor producer has the complete method grammar, every writer is a plain initialized copy, and structural dominance proves each copy, construction and hoisted consumer publication across all constructor epochs"
            }.into()],
        });
    }
    result
}

fn grammar() -> &'static Vec<Methods> {
    static GRAMMAR: OnceLock<Vec<Methods>> = OnceLock::new();
    GRAMMAR.get_or_init(|| {
        [INLINE_CAPACITY, SPLIT_CAPACITY, SPLIT_PREDICATE_CAPACITY]
            .into_iter()
            .map(|capacity| {
                let source = REFERENCE.replace("CAPACITY", capacity);
                let allocator = Allocator::default();
                let parsed = Parser::new(&allocator, &source, SourceType::default()).parse();
                assert!(parsed.errors.is_empty());
                let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
                let (scoping, nodes) = semantic.into_scoping_and_nodes();
                let reference = nodes
                    .iter()
                    .find_map(|n| match n.kind() {
                        AstKind::Class(c) => {
                            Some(methods(&scoping, c).expect("valid ring grammar"))
                        }
                        _ => None,
                    })
                    .expect("reference class");
                reference
            })
            .collect()
    })
}
const INLINE_CAPACITY: &str =
    "this.n = value === 0 || typeof value !== 'number' ? Infinity : value;";
const SPLIT_CAPACITY: &str = "let capacity; if (value === 0 || typeof value !== 'number') { capacity = Infinity; } else { capacity = value; } this.n = capacity;";
// Both branches must define the same scoped predicate before its sole capacity
// use. Exact trees retain the zero test, typeof polarity and branch order.
const SPLIT_PREDICATE_CAPACITY: &str = "let invalid; if (value === 0) { invalid = true; } else { invalid = typeof value !== 'number'; } this.n = invalid ? Infinity : value;";
const REFERENCE: &str = r#"
class Reference {
    constructor(value) { this.i = 0; this.length = 0; CAPACITY this.arr = []; }
    put(value) {
        this.arr[this.i] = value;
        this.i = Number(this.i) + 1;
        if (this.length < this.n) { this.length = Number(this.length) + 1; }
        if (this.i >= this.n) { this.i = 0; }
    }
    toArray() {
        if (this.length < this.n) { return this.arr.slice(0, this.length); }
        else { return this.arr.slice(this.i, this.n).concat(this.arr.slice(0, this.i)); }
    }
    clear() { this.arr = []; this.length = 0; this.i = 0; }
}
"#;

#[cfg(test)]
#[path = "ring_buffer_tests.rs"]
mod tests;
