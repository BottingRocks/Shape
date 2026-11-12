//! Exact software arithmetic protocol; isolated from the accepted header numeric rule.
use crate::naming::*;
use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_semantic::{Scoping, SemanticBuilder, SymbolId};
use oxc_span::SourceType;
use std::sync::OnceLock;
mod reference;
mod tagged_arithmetic;
pub(in crate::naming) use tagged_arithmetic::evaluate as evaluate_tagged_arithmetic;
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Port {
    Tags,
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
            None if matches!(id.name.as_str(), "Math") => {
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

struct Projection { tree: Tree, table: SymbolId }
fn project(scoping: &Scoping, f: &Function<'_>) -> Option<Projection> {
    if f.r#type != FunctionType::FunctionDeclaration || f.r#async || f.generator
        || f.params.rest.is_some() || f.params.items.len()!=2
        || f.type_parameters.is_some() || f.return_type.is_some() { return None; }
    let body=f.body.as_ref()?;
    if !body.directives.is_empty() { return None; }
    // The sole port must be the object of the DOUBLE64 read. Projection below
    // checks every later use against this exact resolved identity.
    let table=body.statements.iter().find_map(|s| {
        let Statement::VariableDeclaration(d)=s else {return None};
        d.declarations.iter().find_map(|v| {
            let Expression::StaticMemberExpression(m)=v.init.as_ref()? else {return None};
            if m.optional || m.property.name!="DOUBLE64" {return None;}
            let Expression::Identifier(id)=m.object.without_parentheses() else{return None};
            id.reference_id.get().and_then(|r|scoping.get_reference(r).symbol_id())
        })
    })?;
    let mut proof=Body{scoping, locals:BTreeMap::new(), ports:BTreeMap::from([(table,Port::Tags)]),
        seen:BTreeMap::new(), self_binding:f.id.as_ref()?.symbol_id.get()?,depth:0,budget:20048};
    for p in &f.params.items {
        if p.initializer.is_some() || !p.decorators.is_empty() {return None;}
        proof.bind(&p.pattern)?;
    }
    let tree=proof.statements(&body.statements)?;
    if proof.seen.get(&Port::Tags)!=Some(&table) {return None;}
    Some(Projection{tree,table})
}
fn references()-> &'static Vec<Tree> {
    static REFS:OnceLock<Vec<Tree>>=OnceLock::new();
    REFS.get_or_init(||reference::protocols().iter().map(|body| {
        let source=format!("let tags; function writer(value, output){{{body}}}");
        let allocator=Allocator::default();
        let parsed=Parser::new(&allocator,&source,SourceType::default()).parse();
        assert!(!parsed.panicked && parsed.errors.is_empty(),"float grammar parse: {:?}",parsed.errors);
        let semantic=SemanticBuilder::new().build(&parsed.program).semantic;
        let Statement::FunctionDeclaration(f)=&parsed.program.body[1] else{unreachable!()};
        project(semantic.scoping(),f).expect("float grammar projects").tree
    }).collect())
}
pub(in crate::naming) fn evaluate(model:&Model<'_, '_>,site:FunctionSite,f:&Facts<'_>)->Vec<Proposal>{
    let AstKind::Function(function)=model.nodes.kind(site.node_id) else{return Vec::new()};
    if !matches!(model.nodes.parent_kind(site.node_id),AstKind::FunctionBody(_)|AstKind::BlockStatement(_))
        || function.id.as_ref().and_then(|id|id.symbol_id.get())!=site.binding_symbol {return Vec::new();}
    let Some(p)=project(model.scoping,function) else{return Vec::new()};
    if !references().contains(&p.tree) {return Vec::new();}
    if crate::naming::functions::encoding::numeric_byte_writers::software_float_capture_ready(model,site,p.table).is_none(){return Vec::new();}
    vec![proposal("writeTaggedArithmeticFloatBytes", "shared_software_float_byte_writer_protocol",
        "whole owned ordered numeric protocol: reciprocal signed-zero test, abs/log/LN2 normalization, exact half/parity ties, zero/subnormal/overflow paths and source NaN/infinity fallthrough; exact 536870911 mask AND divisor, narrow exponent adjustment placement, 8/23 and 11/52 widths, low-bit extraction and MSB-first eight-bit output; global Math and resolved symbols; all numeric tag stores and callable exposure readiness independently proved; arithmetic behavior only, no native IEEE/superpack validity asserted",f)]
}

#[cfg(test)]
mod protocol_tests {
    use super::*;
    #[test]
    fn actual_software_float_whole_model_guard() {
        let source=include_str!("../../../../tests/fixtures/software_float_byte_writer/whole-scope.js");
        let allocator=Allocator::default();
        let program=crate::analysis::parse_program(&allocator,source).unwrap();
        let (scoping,nodes)=crate::analysis::analyze(&program);
        let model=Model::new(source,&program,&scoping,&nodes,ProgramPropertiesLimits::default()).unwrap();
        for start in [405638,601302] {
            let site=model.site_by_span(oxc_span::Span::new(start,start)).unwrap();
            let AstKind::Function(function)=model.nodes.kind(site.node_id) else {panic!()};
            let p=project(&scoping,function).expect("actual entire body projects");
            assert!(!evaluate(&model,site,&Facts::default()).is_empty(),"full evaluator accepts {start}");
            assert!(references().contains(&p.tree),"whole numeric AST grammar {start}");
            assert!(crate::naming::functions::encoding::numeric_byte_writers::software_float_capture_ready(&model,site,p.table).is_some(),"captured table/exposure protocol {start}");
        }
    }
}
