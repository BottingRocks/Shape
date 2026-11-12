//! Full request-URL / route-pattern matcher grammar.
//!
//! The owned function is a self-contained request matcher. It
//! prefers a cached URL object, otherwise parses the request URL through an
//! anchor element, decodes percent escapes, builds the canonical URL object,
//! runs the two-copy `xn--` punycode decoder, then tests static and
//! provider-supplied method/protocol/host/path/port regex patterns and caches
//! the parsed object for the next call.
//!
//! The proof is a closed typed-AST reference grammar, following the accepted
//! `thread_class_*` convention: the reference body is parsed with oxc and the
//! actual function body is projected to the same closed tree. Local symbols are
//! alpha-renamed by declaration order; captures by resolved first occurrence,
//! and unresolved globals retain exact names. Every
//! branch, assignment, receiver, argument, return, throw, numeric constant and
//! regex pattern/flags stays exact. No source text, body hash, offset, vendor
//! name or sibling body participates. Captured symbols must be declared outside
//! the owned body, immutable and free of redeclarations; only true JavaScript
//! globals may be unresolved.

use crate::naming::*;
use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_semantic::{Scoping, SemanticBuilder, SymbolId};
use oxc_span::SourceType;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

const REFERENCE: &str = include_str!("route_pattern_canonicalizer.reference.js");

/// Only these unresolved spellings are accepted as true host globals. Every
/// other unresolved identifier is refused instead of being invented as native.
const HOST_GLOBALS: &[&str] = &[
    "window",
    "document",
    "Array",
    "Math",
    "Number",
    "RegExp",
    "decodeURIComponent",
    "Error",
    "TypeError",
    "Promise",
    "Symbol",
    "Object",
];

#[derive(Clone, Debug, PartialEq, Eq)]
enum Tree {
    Node(&'static str, Vec<Tree>),
    Atom(String),
    Local(usize),
    Capture(usize),
    Global(String),
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
    outer: BTreeMap<String, usize>,
    captures: Vec<SymbolId>,
    globals: BTreeSet<String>,
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
        if self.locals.contains_key(&symbol)
            || !self.scoping.symbol_redeclarations(symbol).is_empty()
        {
            return None;
        }
        let index = self.locals.len();
        self.locals.insert(symbol, index);
        Some(Tree::Local(index))
    }
    fn outer_index(&mut self, key: String) -> usize {
        if let Some(index) = self.outer.get(&key) {
            return *index;
        }
        let index = self.outer.len();
        self.outer.insert(key, index);
        index
    }
    fn reference(&mut self, id: &IdentifierReference<'_>) -> Option<Tree> {
        let symbol = id
            .reference_id
            .get()
            .and_then(|reference| self.scoping.get_reference(reference).symbol_id());
        match symbol {
            Some(symbol) if self.locals.contains_key(&symbol) => {
                Some(Tree::Local(*self.locals.get(&symbol)?))
            }
            Some(symbol) => {
                // A bound identifier outside the owned body is a capture. The
                // caller post-checks that it is immutable and non-redeclared.
                let index = self.outer_index(format!("#{}", symbol.index()));
                self.captures.push(symbol);
                Some(Tree::Capture(index))
            }
            None => {
                let name = id.name.as_str();
                if !HOST_GLOBALS.contains(&name) {
                    return None;
                }
                self.globals.insert(name.to_string());
                Some(Tree::Global(name.to_string()))
            }
        }
    }
    fn statements(&mut self, statements: &[Statement<'_>]) -> Option<Tree> {
        Some(node(
            "body",
            statements
                .iter()
                .map(|statement| self.statement(statement))
                .collect::<Option<_>>()?,
        ))
    }
    fn statement(&mut self, statement: &Statement<'_>) -> Option<Tree> {
        self.budget = self.budget.checked_sub(1)?;
        self.depth += 1;
        if self.depth > 96 {
            return None;
        }
        let result = match statement {
            Statement::BlockStatement(block) => self.statements(&block.body),
            Statement::EmptyStatement(_) => Some(node("empty", vec![])),
            Statement::ExpressionStatement(expression) => Some(node(
                "expression",
                vec![self.expression(&expression.expression)?],
            )),
            Statement::ReturnStatement(returned) => Some(node(
                "return",
                returned
                    .argument
                    .iter()
                    .map(|value| self.expression(value))
                    .collect::<Option<_>>()?,
            )),
            Statement::ThrowStatement(thrown) => {
                Some(node("throw", vec![self.expression(&thrown.argument)?]))
            }
            Statement::BreakStatement(broken) => Some(node(
                "break",
                broken
                    .label
                    .iter()
                    .map(|label| atom(label.name.as_str()))
                    .collect(),
            )),
            Statement::ContinueStatement(continued) => Some(node(
                "continue",
                continued
                    .label
                    .iter()
                    .map(|label| atom(label.name.as_str()))
                    .collect(),
            )),
            Statement::VariableDeclaration(declaration) => self.variables(declaration),
            Statement::IfStatement(branch) => Some(node(
                "if",
                vec![
                    self.expression(&branch.test)?,
                    self.statement(&branch.consequent)?,
                    node(
                        "else",
                        branch
                            .alternate
                            .iter()
                            .map(|alternate| self.statement(alternate))
                            .collect::<Option<_>>()?,
                    ),
                ],
            )),
            Statement::ForStatement(loop_) => {
                let init = match &loop_.init {
                    None => node("init", vec![]),
                    Some(ForStatementInit::VariableDeclaration(declaration)) => {
                        node("init", vec![self.variables(declaration)?])
                    }
                    _ => return None,
                };
                Some(node(
                    "for",
                    vec![
                        init,
                        node(
                            "test",
                            loop_
                                .test
                                .iter()
                                .map(|test| self.expression(test))
                                .collect::<Option<_>>()?,
                        ),
                        node(
                            "update",
                            loop_
                                .update
                                .iter()
                                .map(|update| self.expression(update))
                                .collect::<Option<_>>()?,
                        ),
                        self.statement(&loop_.body)?,
                    ],
                ))
            }
            Statement::WhileStatement(loop_) => Some(node(
                "while",
                vec![self.expression(&loop_.test)?, self.statement(&loop_.body)?],
            )),
            Statement::DoWhileStatement(loop_) => Some(node(
                "do_while",
                vec![self.statement(&loop_.body)?, self.expression(&loop_.test)?],
            )),
            Statement::TryStatement(try_) => {
                let block = self.statements(&try_.block.body)?;
                let handler = if let Some(handler) = &try_.handler {
                    let param = handler
                        .param
                        .as_ref()
                        .map(|param| self.bind(&param.pattern))
                        .transpose_option()?;
                    node(
                        "catch",
                        vec![
                            node("param", param.into_iter().collect()),
                            self.statements(&handler.body.body)?,
                        ],
                    )
                } else {
                    node("no catch", vec![])
                };
                let finalizer = try_
                    .finalizer
                    .iter()
                    .map(|block| self.statements(&block.body))
                    .collect::<Option<_>>()?;
                Some(node(
                    "try",
                    vec![block, handler, node("finally", finalizer)],
                ))
            }
            Statement::FunctionDeclaration(function) => {
                if function.r#async
                    || function.generator
                    || function.params.rest.is_some()
                    || function.type_parameters.is_some()
                    || function.return_type.is_some()
                {
                    return None;
                }
                let name = self.bind_symbol(function.id.as_ref()?.symbol_id.get()?)?;
                let mut parameters = vec![name];
                for parameter in &function.params.items {
                    if parameter.initializer.is_some() || !parameter.decorators.is_empty() {
                        return None;
                    }
                    parameters.push(self.bind(&parameter.pattern)?);
                }
                let body = function.body.as_ref()?;
                if !body.directives.is_empty() {
                    return None;
                }
                Some(node(
                    "function",
                    vec![
                        node("parameters", parameters),
                        self.statements(&body.statements)?,
                    ],
                ))
            }
            _ => None,
        };
        self.depth -= 1;
        result
    }
    fn variables(&mut self, declaration: &VariableDeclaration<'_>) -> Option<Tree> {
        let mut children = vec![atom(format!("{:?}", declaration.kind))];
        for declarator in &declaration.declarations {
            let binding = self.bind(&declarator.id)?;
            let init = node(
                "init",
                declarator
                    .init
                    .iter()
                    .map(|value| self.expression(value))
                    .collect::<Option<_>>()?,
            );
            children.push(node("declaration", vec![binding, init]));
        }
        Some(node("variables", children))
    }
    fn expression(&mut self, expression: &Expression<'_>) -> Option<Tree> {
        self.budget = self.budget.checked_sub(1)?;
        self.depth += 1;
        if self.depth > 96 {
            return None;
        }
        let result = match expression.without_parentheses() {
            Expression::ThisExpression(_) => Some(node("this", vec![])),
            Expression::Identifier(identifier) => self.reference(identifier),
            Expression::NullLiteral(_) => Some(node("null", vec![])),
            Expression::BooleanLiteral(value) => Some(node("bool", vec![atom(value.value)])),
            Expression::StringLiteral(value) => {
                Some(node("string", vec![atom(value.value.as_str())]))
            }
            // Exact numeric identity, including 9007199254740991 and
            // 257348550135456.88; no "large constant" erasure.
            Expression::NumericLiteral(value) => {
                Some(node("number", vec![atom(value.value.to_bits())]))
            }
            Expression::RegExpLiteral(literal) => Some(node(
                "regexp",
                vec![
                    atom(literal.regex.pattern.text.as_str()),
                    atom(literal.regex.flags.bits()),
                ],
            )),
            Expression::StaticMemberExpression(member) if !member.optional => Some(node(
                "member",
                vec![
                    self.expression(&member.object)?,
                    atom(member.property.name.as_str()),
                ],
            )),
            Expression::ComputedMemberExpression(member) if !member.optional => Some(node(
                "index",
                vec![
                    self.expression(&member.object)?,
                    self.expression(&member.expression)?,
                ],
            )),
            Expression::CallExpression(call) if !call.optional && call.type_arguments.is_none() => {
                let mut children = vec![self.expression(&call.callee)?];
                for argument in &call.arguments {
                    children.push(self.expression(argument.as_expression()?)?);
                }
                Some(node("call", children))
            }
            Expression::NewExpression(created) if created.type_arguments.is_none() => {
                let mut children = vec![self.expression(&created.callee)?];
                for argument in &created.arguments {
                    children.push(self.expression(argument.as_expression()?)?);
                }
                Some(node("new", children))
            }
            Expression::BinaryExpression(binary) => Some(node(
                "binary",
                vec![
                    atom(format!("{:?}", binary.operator)),
                    self.expression(&binary.left)?,
                    self.expression(&binary.right)?,
                ],
            )),
            Expression::LogicalExpression(logical) => Some(node(
                "logical",
                vec![
                    atom(format!("{:?}", logical.operator)),
                    self.expression(&logical.left)?,
                    self.expression(&logical.right)?,
                ],
            )),
            Expression::UnaryExpression(unary) => Some(node(
                "unary",
                vec![
                    atom(format!("{:?}", unary.operator)),
                    self.expression(&unary.argument)?,
                ],
            )),
            Expression::UpdateExpression(update) => Some(node(
                "update",
                vec![
                    atom(format!("{:?}", update.operator)),
                    atom(update.prefix),
                    self.simple_target(&update.argument)?,
                ],
            )),
            Expression::ConditionalExpression(conditional) => Some(node(
                "conditional",
                vec![
                    self.expression(&conditional.test)?,
                    self.expression(&conditional.consequent)?,
                    self.expression(&conditional.alternate)?,
                ],
            )),
            Expression::AssignmentExpression(assignment) => Some(node(
                "assign",
                vec![
                    atom(format!("{:?}", assignment.operator)),
                    self.target(&assignment.left)?,
                    self.expression(&assignment.right)?,
                ],
            )),
            Expression::ArrayExpression(array) => Some(node(
                "array",
                array
                    .elements
                    .iter()
                    .map(|element| self.expression(element.as_expression()?))
                    .collect::<Option<_>>()?,
            )),
            Expression::ObjectExpression(object) => {
                let mut properties = Vec::new();
                for property in &object.properties {
                    let ObjectPropertyKind::ObjectProperty(property) = property else {
                        return None;
                    };
                    if property.computed
                        || property.method
                        || property.shorthand
                        || property.kind != PropertyKind::Init
                    {
                        return None;
                    }
                    properties.push(node(
                        "property",
                        vec![
                            atom(property_key_name(&property.key)?),
                            self.expression(&property.value)?,
                        ],
                    ));
                }
                Some(node("object", properties))
            }
            Expression::SequenceExpression(sequence) => Some(node(
                "sequence",
                sequence
                    .expressions
                    .iter()
                    .map(|expression| self.expression(expression))
                    .collect::<Option<_>>()?,
            )),
            Expression::ArrowFunctionExpression(arrow) => {
                if arrow.r#async
                    || arrow.type_parameters.is_some()
                    || arrow.return_type.is_some()
                    || arrow.params.rest.is_some()
                {
                    return None;
                }
                let mut parameters = Vec::new();
                for parameter in &arrow.params.items {
                    if parameter.initializer.is_some() || !parameter.decorators.is_empty() {
                        return None;
                    }
                    parameters.push(self.bind(&parameter.pattern)?);
                }
                let body = arrow.body.as_ref();
                if !body.directives.is_empty() {
                    return None;
                }
                Some(node(
                    "arrow",
                    vec![
                        atom(arrow.expression),
                        node("parameters", parameters),
                        self.statements(&body.statements)?,
                    ],
                ))
            }
            _ => None,
        };
        self.depth -= 1;
        result
    }
    fn target(&mut self, target: &AssignmentTarget<'_>) -> Option<Tree> {
        match target {
            AssignmentTarget::AssignmentTargetIdentifier(identifier) => self.reference(identifier),
            AssignmentTarget::StaticMemberExpression(member) if !member.optional => Some(node(
                "member",
                vec![
                    self.expression(&member.object)?,
                    atom(member.property.name.as_str()),
                ],
            )),
            AssignmentTarget::ComputedMemberExpression(member) if !member.optional => Some(node(
                "index",
                vec![
                    self.expression(&member.object)?,
                    self.expression(&member.expression)?,
                ],
            )),
            _ => None,
        }
    }
    fn simple_target(&mut self, target: &SimpleAssignmentTarget<'_>) -> Option<Tree> {
        match target {
            SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) => {
                self.reference(identifier)
            }
            SimpleAssignmentTarget::StaticMemberExpression(member) if !member.optional => {
                Some(node(
                    "member",
                    vec![
                        self.expression(&member.object)?,
                        atom(member.property.name.as_str()),
                    ],
                ))
            }
            SimpleAssignmentTarget::ComputedMemberExpression(member) if !member.optional => {
                Some(node(
                    "index",
                    vec![
                        self.expression(&member.object)?,
                        self.expression(&member.expression)?,
                    ],
                ))
            }
            _ => None,
        }
    }
}

trait OptionalNode<T> {
    fn transpose_option(self) -> Option<Option<T>>;
}
impl<T> OptionalNode<T> for Option<Option<T>> {
    fn transpose_option(self) -> Option<Option<T>> {
        match self {
            Some(value) => value.map(Some),
            None => Some(None),
        }
    }
}

struct Projection {
    tree: Tree,
    captures: Vec<SymbolId>,
    globals: BTreeSet<String>,
}

fn method_tree(scoping: &Scoping, function: &Function<'_>) -> Option<Projection> {
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
        outer: BTreeMap::new(),
        captures: Vec::new(),
        globals: BTreeSet::new(),
        depth: 0,
        budget: 20048,
    };
    let mut parameters = Vec::new();
    for parameter in &function.params.items {
        if parameter.initializer.is_some() || !parameter.decorators.is_empty() {
            return None;
        }
        parameters.push(proof.bind(&parameter.pattern)?);
    }
    let body = function.body.as_ref()?;
    if !body.directives.is_empty() {
        return None;
    }
    let tree = node(
        "method",
        vec![
            node("parameters", parameters),
            proof.statements(&body.statements)?,
        ],
    );
    Some(Projection {
        tree,
        captures: proof.captures,
        globals: proof.globals,
    })
}

fn reference_tree() -> Tree {
    static TREE: OnceLock<Tree> = OnceLock::new();
    TREE.get_or_init(|| {
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, REFERENCE, SourceType::default()).parse();
        assert!(
            parsed.errors.is_empty() && !parsed.panicked,
            "network_url_pattern_matcher reference grammar must parse: {:?}",
            parsed.errors
        );
        let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
        let Statement::FunctionDeclaration(scope) = &parsed.program.body[0] else {
            panic!("network_url_pattern_matcher reference grammar must start with a function declaration")
        };
        let Statement::FunctionDeclaration(function) =
            &scope.body.as_ref().expect("reference scope").statements[1]
        else {
            panic!("reference matcher declaration");
        };
        method_tree(semantic.scoping(), function)
            .expect("network_url_pattern_matcher reference grammar must project")
            .tree
    })
    .clone()
}

fn url_pattern_matcher(model: &Model<'_, '_>, site: FunctionSite) -> Option<()> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return None;
    };
    // A named expression id is private, not its externally exposed binding.
    if function.r#type != FunctionType::FunctionDeclaration {
        return None;
    }
    let projection = method_tree(model.scoping, function)?;
    if projection.tree != reference_tree() {
        return None;
    }
    // Captured data and helpers are consumer input: plain lexical declarators
    // declared outside the owned body. Each capture must be initialized by its
    // own initializer or by exactly one executed `=` store that is ordered
    // before every exposure of this matcher, following alias exposure through
    // the shared callable-origin proof. No capture may be a write target after
    // setup or a redeclaration.
    let matcher_symbol = function.id.as_ref()?.symbol_id.get()?;
    if model.binding_writes(matcher_symbol) != 0
        || !model
            .scoping
            .symbol_redeclarations(matcher_symbol)
            .is_empty()
    {
        return None;
    }
    let exposures: Vec<oxc_semantic::NodeId> = model
        .scoping
        .get_resolved_references(matcher_symbol)
        .map(|reference| reference.node_id())
        .collect();
    if exposures.is_empty() {
        return None;
    }
    for symbol in projection.captures {
        if !model.scoping.symbol_redeclarations(symbol).is_empty()
            || site
                .span
                .contains_inclusive(model.scoping.symbol_span(symbol))
        {
            return None;
        }
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
        {
            return None;
        }
        let container = model.nodes.parent_id(model.nodes.parent_id(declaration));
        if !matches!(
            model.nodes.kind(container),
            AstKind::FunctionBody(_) | AstKind::Program(_)
        ) {
            return None;
        }
        let origin_end = if variable.init.is_some() {
            if model.binding_writes(symbol) != 0 {
                return None;
            }
            variable.span.end
        } else {
            if model.binding_writes(symbol) != 1 {
                return None;
            }
            let write = model
                .scoping
                .get_resolved_references(symbol)
                .find(|reference| reference.is_write())?;
            let node = write.node_id();
            let AstKind::AssignmentExpression(assignment) = model.nodes.parent_kind(node) else {
                return None;
            };
            if assignment.operator.as_str() != "="
                || !matches!(&assignment.left, AssignmentTarget::AssignmentTargetIdentifier(target)
                    if crate::analysis::identifier_symbol(target, model.scoping) == Some(symbol))
            {
                return None;
            }
            let statement = model.nodes.parent_id(model.nodes.parent_id(node));
            if !matches!(model.nodes.kind(statement), AstKind::ExpressionStatement(_))
                || model.nodes.parent_id(statement) != container
            {
                return None;
            }
            assignment.span.end
        };
        if exposures.iter().any(|exposure| {
            !crate::naming::shared::callable_origin_precedes(
                model, container, origin_end, *exposure, None, 0, true,
            )
        }) {
            return None;
        }
    }
    // Unresolved identifiers must be real host globals from the closed set.
    if projection
        .globals
        .iter()
        .any(|name| !HOST_GLOBALS.contains(&name.as_str()))
    {
        return None;
    }
    Some(())
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    facts: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    if url_pattern_matcher(model, site).is_none() {
        return Vec::new();
    }
    vec![proposal(
        "matchesRequestUrlPatterns",
        "network_url_pattern_matcher",
        "complete owned body: cached URL fast path, anchor-based URL parse with exact percent-escape regexes, default-port normalization, canonical protocol/hostname/port/pathname/search/hash/href object, both xn-- punycode decoder copies including the cache-miss port write, and static plus provider method/protocol/host/path/port pattern predicates that cache the parsed object; local symbols are alpha-renamed and every literal, regex pattern/flags, branch, assignment, receiver, argument, return and throw is compared by typed AST; captured helpers are immutable lexical bindings and unresolved names are host globals only",
        facts,
    )]
}
