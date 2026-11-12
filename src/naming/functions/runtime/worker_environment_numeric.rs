//! Complete owned numeric-body protocol. The typed AST projector is adapted
//! from probe09's reviewed pure projection component; no other finder changes.
//! Exact global names, resolved local/self/capture identities, node/operator
//! kinds, numeric bits, property names, regex flags and control flow are kept.
use super::*;
use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_semantic::{Scoping, SemanticBuilder, SymbolId};
use oxc_span::SourceType;
use std::sync::OnceLock;
#[path = "worker_environment_numeric_reference.rs"]
mod reference;
#[path = "worker_environment_numeric_variants.rs"]
mod variants;
#[path = "worker_environment_writer_variants.rs"]
mod writer_variants;
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Port {
    FloatScratch,
    ByteConstructor,
    NanData,
    CharacterCode,
    NumberWriter,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Tree {
    Node(&'static str, Vec<Tree>),
    Atom(String),
    Local(usize),
    Capture(Port),
    SelfBinding,
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
    ports: BTreeMap<SymbolId, Port>,
    seen: BTreeMap<Port, SymbolId>,
    self_binding: SymbolId,
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
    fn reference(&mut self, id: &IdentifierReference<'_>) -> Option<Tree> {
        let symbol = id
            .reference_id
            .get()
            .and_then(|r| self.scoping.get_reference(r).symbol_id());
        match symbol {
            Some(s) if s == self.self_binding => Some(Tree::SelfBinding),
            Some(s) if self.locals.contains_key(&s) => Some(Tree::Local(*self.locals.get(&s)?)),
            Some(s) => {
                let role = *self.ports.get(&s)?;
                if self.seen.get(&role).is_some_and(|old| *old != s) {
                    return None;
                }
                self.seen.insert(role, s);
                Some(Tree::Capture(role))
            }
            None if matches!(id.name.as_str(), "Math" | "Number" | "Object") => {
                Some(Tree::Global(id.name.to_string()))
            }
            _ => None,
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
    integer: Tree,
    float: Tree,
    ports: BTreeMap<Port, SymbolId>,
}
fn project(
    scoping: &Scoping,
    f: &Function<'_>,
    ports: BTreeMap<SymbolId, Port>,
) -> Option<Projection> {
    if f.r#type != FunctionType::FunctionDeclaration
        || f.r#async
        || f.generator
        || f.params.rest.is_some()
        || f.type_parameters.is_some()
        || f.return_type.is_some()
        || f.params.items.len() != 4
    {
        return None;
    }
    let body = f.body.as_ref()?;
    if !body.directives.is_empty() {
        return None;
    }
    let [Statement::VariableDeclaration(d), Statement::IfStatement(branch), Statement::ReturnStatement(ret)] =
        body.statements.as_slice()
    else {
        return None;
    };
    if d.kind != VariableDeclarationKind::Let {
        return None;
    }
    let [cursor] = d.declarations.as_slice() else {
        return None;
    };
    if cursor.init.is_some() {
        return None;
    }
    let self_binding = f.id.as_ref()?.symbol_id.get()?;
    let mut proof = Body {
        scoping,
        locals: BTreeMap::new(),
        ports,
        seen: BTreeMap::new(),
        self_binding,
        depth: 0,
        budget: 20048,
    };
    for param in &f.params.items {
        if param.initializer.is_some() || !param.decorators.is_empty() {
            return None;
        }
        proof.bind(&param.pattern)?;
    }
    let Tree::Local(4) = proof.bind(&cursor.id)? else {
        return None;
    };
    let header = proof.expression(&branch.test)?;
    let expected = node(
        "binary",
        vec![
            atom("StrictEquality"),
            node(
                "binary",
                vec![
                    atom("BitwiseOR"),
                    Tree::Local(1),
                    node("number", vec![atom(0f64.to_bits())]),
                ],
            ),
            Tree::Local(1),
        ],
    );
    if header != expected || proof.expression(ret.argument.as_ref()?)? != Tree::Local(4) {
        return None;
    }
    let locals = proof.locals.clone();
    let integer = proof.statement(&branch.consequent)?;
    if !proof.seen.is_empty() {
        return None;
    }
    proof.locals = locals;
    let float = proof.statement(branch.alternate.as_ref()?)?;
    Some(Projection {
        integer,
        float,
        ports: proof.seen,
    })
}
struct References {
    integers: Vec<Tree>,
    floats: Vec<Tree>,
}
fn references() -> &'static References {
    static REFS: OnceLock<References> = OnceLock::new();
    REFS.get_or_init(||{
        fn parse(integer:&str,float:&str)->Projection{
            let src=format!("function ports(){{let floatScratch,byteConstructor,nanData;function numeric(mask,value,output,position){{let cursor;if((value|0)===value){integer}else{float}return cursor;}}}}");
            let allocator=Allocator::default();let parsed=Parser::new(&allocator,&src,SourceType::default()).parse();
            assert!(!parsed.panicked&&parsed.errors.is_empty(),"numeric reference must parse: {:?}",parsed.errors);
            let semantic=SemanticBuilder::new().build(&parsed.program).semantic;
            let Statement::FunctionDeclaration(scope)=&parsed.program.body[0]else{unreachable!()};
            let scope=scope.body.as_ref().unwrap();
            let Statement::VariableDeclaration(ports)=&scope.statements[0]else{unreachable!()};
            let port_map=ports.declarations.iter().zip([Port::FloatScratch,Port::ByteConstructor,Port::NanData])
                .map(|(d,p)|{let BindingPattern::BindingIdentifier(id)=&d.id else{unreachable!()};(id.symbol_id.get().unwrap(),p)}).collect();
            let Statement::FunctionDeclaration(f)=&scope.statements[1]else{unreachable!()};
            project(semantic.scoping(),f,port_map).expect("complete numeric reference must project")
        }
        References{
            integers:reference::integers().iter().map(|s|parse(s,"{}").integer).collect(),
            floats:reference::floats().iter().map(|s|parse("{}",s).float).collect(),
        }
    })
}
// Alternative complete bodies retain their real branch-local return and cursor
// layouts. Compare the entire typed tree instead of erasing those distinctions.
fn whole(
    scoping: &Scoping,
    f: &Function<'_>,
    ports: BTreeMap<SymbolId, Port>,
    arity: usize,
) -> Option<(Tree, BTreeMap<Port, SymbolId>)> {
    if f.r#type != FunctionType::FunctionDeclaration || f.r#async || f.generator
        || f.params.rest.is_some() || f.params.items.len() != arity
        || f.type_parameters.is_some() || f.return_type.is_some()
    { return None; }
    let body = f.body.as_ref()?;
    if !body.directives.is_empty() { return None; }
    let mut proof = Body {
        scoping, locals: BTreeMap::new(), ports, seen: BTreeMap::new(),
        self_binding: f.id.as_ref()?.symbol_id.get()?, depth: 0, budget: 20048,
    };
    for p in &f.params.items {
        if p.initializer.is_some() || !p.decorators.is_empty() { return None; }
        proof.bind(&p.pattern)?;
    }
    Some((proof.statements(&body.statements)?, proof.seen))
}
fn complete_references() -> &'static Vec<Tree> {
    static REFS: OnceLock<Vec<Tree>> = OnceLock::new();
    REFS.get_or_init(|| variants::FORMS.iter().map(|body| {
        let src = format!("function ports(){{let floatScratch,byteConstructor,nanData;{body}}}");
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &src, SourceType::default()).parse();
        assert!(!parsed.panicked && parsed.errors.is_empty());
        let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
        let Statement::FunctionDeclaration(scope) = &parsed.program.body[0] else { unreachable!() };
        let statements = &scope.body.as_ref().unwrap().statements;
        let Statement::VariableDeclaration(ports) = &statements[0] else { unreachable!() };
        let ports = ports.declarations.iter()
            .zip([Port::FloatScratch, Port::ByteConstructor, Port::NanData])
            .map(|(d, p)| {
                let BindingPattern::BindingIdentifier(id) = &d.id else { unreachable!() };
                (id.symbol_id.get().unwrap(), p)
            }).collect();
        let Statement::FunctionDeclaration(f) = &statements[1] else { unreachable!() };
        whole(semantic.scoping(), f, ports, 4).expect("complete numeric grammar").0
    }).collect())
}
fn complete_proof(
    model: &Model<'_, '_>, f: &Function<'_>, ports: BTreeMap<SymbolId, Port>,
) -> Option<NumericPorts> {
    let (tree, seen) = whole(model.scoping, f, ports, 4)?;
    if !complete_references().contains(&tree) { return None; }
    Some(NumericPorts {
        scratch: *seen.get(&Port::FloatScratch)?,
        byte_constructor: *seen.get(&Port::ByteConstructor)?,
        nan_data: *seen.get(&Port::NanData)?,
    })
}
pub(super) struct NumericPorts {

    pub(super) scratch: SymbolId,
    pub(super) byte_constructor: SymbolId,
    pub(super) nan_data: SymbolId,
}
pub(super) fn prove(
    model: &Model<'_, '_>,
    site: FunctionSite,
    kinds: &BTreeMap<SymbolId, SetupValue>,
) -> Option<NumericPorts> {
    let AstKind::Function(f) = model.nodes.kind(site.node_id) else {
        return None;
    };
    if !plain_parameters(model, f, 4) {
        return None;
    }
    let ports: BTreeMap<SymbolId, Port> = kinds
        .iter()
        .filter_map(|(&s, kind)| match kind {
            SetupValue::Float64 => Some((s, Port::FloatScratch)),
            SetupValue::Uint8 => Some((s, Port::ByteConstructor)),
            SetupValue::Nan => Some((s, Port::NanData)),
            _ => None,
        })
        .collect();
    let Some(projection) = project(model.scoping, f, ports.clone()) else {
        return complete_proof(model, f, ports);
    };
    let refs = references();
    if !refs.integers.contains(&projection.integer) || !refs.floats.contains(&projection.float) {
        return complete_proof(model, f, ports);
    }
    Some(NumericPorts {
        scratch: *projection.ports.get(&Port::FloatScratch)?,
        byte_constructor: *projection.ports.get(&Port::ByteConstructor)?,
        nan_data: *projection.ports.get(&Port::NanData)?,
    })
}

pub(super) fn prove_writer(
    model: &Model<'_, '_>, factory: FunctionSite, setup: &WrapperSetup,
    number: FunctionSite, proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Option<()> {
    let AstKind::Function(f) = model.nodes.kind(factory.node_id) else { return None; };
    let children = direct_children(model, factory);
    let [writer] = children.as_slice() else { return None; };
    if role_at(proposals, writer.span.start) != Some("writeInternedString") { return None; }
    let ports = setup.kinds.iter().filter_map(|(&b, kind)| match kind {
        SetupValue::CharacterCode => Some((b, Port::CharacterCode)),
        SetupValue::Callable(start) if *start == number.span.start => Some((b, Port::NumberWriter)),
        _ => None,
    }).collect();
    let (tree, seen) = whole(model.scoping, f, ports, 1)?;
    if seen.len() != 2 { return None; }
    static REFS: OnceLock<Vec<Tree>> = OnceLock::new();
    let refs = REFS.get_or_init(|| writer_variants::FORMS.iter().map(|body| {
        let src = format!("function ports(){{let characterCode,numberWriter;{body}}}");
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &src, SourceType::default()).parse();
        assert!(!parsed.panicked && parsed.errors.is_empty());
        let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
        let Statement::FunctionDeclaration(scope) = &parsed.program.body[0] else { unreachable!() };
        let statements = &scope.body.as_ref().unwrap().statements;
        let Statement::VariableDeclaration(ports) = &statements[0] else { unreachable!() };
        let ports = ports.declarations.iter().zip([Port::CharacterCode, Port::NumberWriter])
            .map(|(d, p)| {
                let BindingPattern::BindingIdentifier(id) = &d.id else { unreachable!() };
                (id.symbol_id.get().unwrap(), p)
            }).collect();
        let Statement::FunctionDeclaration(f) = &statements[1] else { unreachable!() };
        whole(semantic.scoping(), f, ports, 1).expect("complete string writer grammar").0
    }).collect());
    refs.contains(&tree).then_some(())
}
