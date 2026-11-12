//! Bounded scoped syntax extracted from stopped A solely for snapshot proofs.
use super::*;
use oxc_semantic::{Scoping,SymbolId};
#[derive(Clone, Debug, PartialEq)]
enum Tree {
    Node(String, Vec<Tree>),
    Local(SymbolId),
    Global(String),
}
fn node(tag: impl Into<String>, children: Vec<Tree>) -> Tree {
    Tree::Node(tag.into(), children)
}
fn leaf(tag: impl Into<String>) -> Tree {
    node(tag, vec![])
}

struct Syntax<'a> {
    scopes: &'a Scoping,
    remaining: usize,
}
impl Syntax<'_> {
    fn tick(&mut self) -> Option<()> {
        self.remaining = self.remaining.checked_sub(1)?;
        Some(())
    }
    fn identifier(&self, i: &IdentifierReference<'_>) -> Tree {
        crate::analysis::identifier_symbol(i, self.scopes)
            .map(Tree::Local)
            .unwrap_or_else(|| Tree::Global(i.name.to_string()))
    }
    fn binding(&self, p: &BindingPattern<'_>) -> Option<Tree> {
        let BindingPattern::BindingIdentifier(i) = p else {
            return None;
        };
        let s = i.symbol_id.get()?;
        self.scopes
            .symbol_redeclarations(s)
            .is_empty()
            .then_some(Tree::Local(s))
    }
    fn function(&mut self, f: &Function<'_>) -> Option<Tree> {
        if f.r#async || f.generator {
            return None;
        }
        let body = f.body.as_ref()?;
        if !body.directives.is_empty() {
            return None;
        }
        Some(node(
            "function",
            vec![self.params(&f.params)?, self.statements(&body.statements)?],
        ))
    }
    fn params(&self, p: &FormalParameters<'_>) -> Option<Tree> {
        if p.rest.is_some() {
            return None;
        }
        Some(node(
            "params",
            p.items
                .iter()
                .map(|p| {
                    if p.initializer.is_some() {
                        return None;
                    }
                    self.binding(&p.pattern)
                })
                .collect::<Option<_>>()?,
        ))
    }
    fn statements(&mut self, statements: &[Statement<'_>]) -> Option<Tree> {
        Some(node("statements", statements.iter().map(|s|self.statement(s)).collect::<Option<_>>()?))
    }
    fn statement(&mut self, s: &Statement<'_>) -> Option<Tree> {
        self.tick()?;
        Some(match s {
            Statement::VariableDeclaration(d)
                if matches!(
                    d.kind,
                    VariableDeclarationKind::Var
                        | VariableDeclarationKind::Let
                        | VariableDeclarationKind::Const
                ) =>
            {
                node(
                    format!("variable:{:?}", d.kind),
                    d.declarations
                        .iter()
                        .map(|v| {
                            let mut children = vec![self.binding(&v.id)?];
                            if let Some(e) = &v.init {
                                children.push(self.expression(e)?);
                            }
                            Some(node("binding", children))
                        })
                        .collect::<Option<_>>()?,
                )
            }
            Statement::ExpressionStatement(s) => {
                node("expression", vec![self.expression(&s.expression)?])
            }
            Statement::ReturnStatement(s) => node(
                "return",
                s.argument
                    .iter()
                    .map(|e| self.expression(e))
                    .collect::<Option<_>>()?,
            ),
            Statement::BlockStatement(s) => node("block", vec![self.statements(&s.body)?]),
            Statement::WhileStatement(s) => node("while", vec![self.expression(&s.test)?, self.statement(&s.body)?]),
            Statement::ForStatement(s) => {
                let init = match &s.init {
                    None => node("init", vec![]),
                    Some(ForStatementInit::VariableDeclaration(d)) if matches!(d.kind, VariableDeclarationKind::Var | VariableDeclarationKind::Let | VariableDeclarationKind::Const) => {
                        let entries = d.declarations.iter().map(|v| {
                            let mut fields = vec![self.binding(&v.id)?];
                            if let Some(e) = &v.init { fields.push(self.expression(e)?); }
                            Some(node("binding", fields))
                        }).collect::<Option<Vec<_>>>()?;
                        node("init", vec![node(format!("variable:{:?}", d.kind), entries)])
                    },
                    _ => return None,
                };
                node("for", vec![init,
                    node("test", s.test.iter().map(|e|self.expression(e)).collect::<Option<_>>()?),
                    node("update", s.update.iter().map(|e|self.expression(e)).collect::<Option<_>>()?),
                    self.statement(&s.body)?])
            }
            Statement::IfStatement(s) => {
                let mut parts = vec![self.expression(&s.test)?, self.statement(&s.consequent)?];
                if let Some(other) = &s.alternate {
                    parts.push(self.statement(other)?);
                }
                node("if", parts)
            }
            _ => return None,
        })
    }
    fn expression(&mut self, e: &Expression<'_>) -> Option<Tree> {
        self.tick()?;
        Some(match e {
            Expression::Identifier(i) => self.identifier(i),
            Expression::ParenthesizedExpression(p) => return self.expression(&p.expression),
            Expression::StringLiteral(s) => leaf(format!("string:{}", s.value)),
            Expression::NumericLiteral(n) => leaf(format!("number:{}", n.value)),
            Expression::BooleanLiteral(b) => leaf(format!("bool:{}", b.value)),
            Expression::NullLiteral(_) => leaf("null"),
            Expression::StaticMemberExpression(m) => node(
                format!("member:{}:{}", m.optional, m.property.name),
                vec![self.expression(&m.object)?],
            ),
            Expression::ComputedMemberExpression(m) => node(
                format!("index:{}", m.optional),
                vec![self.expression(&m.object)?, self.expression(&m.expression)?],
            ),
            Expression::ChainExpression(c) => {
                let inner = match &c.expression {
                    ChainElement::CallExpression(c) => self.call(c)?,
                    ChainElement::StaticMemberExpression(m) => node(
                        format!("member:{}:{}", m.optional, m.property.name),
                        vec![self.expression(&m.object)?],
                    ),
                    ChainElement::ComputedMemberExpression(m) => node(
                        format!("index:{}", m.optional),
                        vec![self.expression(&m.object)?, self.expression(&m.expression)?],
                    ),
                    _ => return None,
                };
                node("chain", vec![inner])
            }
            Expression::CallExpression(c) => self.call(c)?,
            Expression::NewExpression(n) => {
                let mut parts = vec![self.expression(&n.callee)?];
                parts.extend(
                    n.arguments
                        .iter()
                        .map(|a| self.argument(a))
                        .collect::<Option<Vec<_>>>()?,
                );
                node("new", parts)
            }
            Expression::BinaryExpression(b) => node(
                format!("binary:{}", b.operator.as_str()),
                vec![self.expression(&b.left)?, self.expression(&b.right)?],
            ),
            Expression::LogicalExpression(b) => node(
                format!("logical:{}", b.operator.as_str()),
                vec![self.expression(&b.left)?, self.expression(&b.right)?],
            ),
            Expression::UnaryExpression(u) => node(
                format!("unary:{}", u.operator.as_str()),
                vec![self.expression(&u.argument)?],
            ),
            Expression::ConditionalExpression(c) => node(
                "conditional",
                vec![
                    self.expression(&c.test)?,
                    self.expression(&c.consequent)?,
                    self.expression(&c.alternate)?,
                ],
            ),
            Expression::ArrayExpression(a) => node(
                "array",
                a.elements
                    .iter()
                    .map(|e| self.expression(e.as_expression()?))
                    .collect::<Option<_>>()?,
            ),
            Expression::ObjectExpression(o) => {
                let mut properties = Vec::new();
                let mut keys = BTreeSet::new();
                for p in &o.properties {
                    let ObjectPropertyKind::ObjectProperty(p) = p else {
                        return None;
                    };
                    if p.computed || p.method || p.kind != PropertyKind::Init {
                        return None;
                    }
                    let key = property_key_name(&p.key)?;
                    if !keys.insert(key.clone()) {
                        return None;
                    }
                    properties.push((key, self.expression(&p.value)?));
                }
                node(
                    "object",
                    properties
                        .into_iter()
                        .map(|(k, v)| node(k, vec![v]))
                        .collect(),
                )
            }
            Expression::ArrowFunctionExpression(a) => {
                if a.r#async || !a.body.directives.is_empty() {
                    return None;
                }
                node(
                    format!("arrow:{}", a.expression),
                    vec![
                        self.params(&a.params)?,
                        self.statements(&a.body.statements)?,
                    ],
                )
            }
            Expression::UpdateExpression(u) => {
                let SimpleAssignmentTarget::AssignmentTargetIdentifier(i) = &u.argument else { return None; };
                node(format!("update:{}:{}", u.operator.as_str(), u.prefix), vec![self.identifier(i)])
            }
            Expression::AssignmentExpression(a) => {
                let target = match &a.left {
                    AssignmentTarget::AssignmentTargetIdentifier(i) => self.identifier(i),
                    AssignmentTarget::StaticMemberExpression(m) => node(
                        format!("member:{}:{}", m.optional, m.property.name),
                        vec![self.expression(&m.object)?],
                    ),
                    AssignmentTarget::ComputedMemberExpression(m) => node(
                        format!("index:{}", m.optional),
                        vec![self.expression(&m.object)?, self.expression(&m.expression)?],
                    ),
                    _ => return None,
                };
                node(
                    format!("assign:{}", a.operator.as_str()),
                    vec![target, self.expression(&a.right)?],
                )
            }
            _ => return None,
        })
    }
    fn argument(&mut self, a: &Argument<'_>) -> Option<Tree> {
        self.expression(a.as_expression()?)
    }
    fn call(&mut self, c: &CallExpression<'_>) -> Option<Tree> {
        let mut children = vec![self.expression(&c.callee)?];
        children.extend(
            c.arguments
                .iter()
                .map(|a| self.argument(a))
                .collect::<Option<Vec<_>>>()?,
        );
        Some(node(format!("call:{}", c.optional), children))
    }
}

#[derive(Default)]
struct Match {
    locals: BTreeMap<SymbolId, SymbolId>,
    captures: BTreeMap<String, SymbolId>,
}
impl Match {
    fn tree(
        &mut self,
        expected: &Tree,
        actual: &Tree,
        model: &Model<'_, '_>,
        site: FunctionSite,
    ) -> bool {
        match (expected, actual) {
            (Tree::Local(a), Tree::Local(b)) => {
                if !site.span.contains_inclusive(model.scoping.symbol_span(*b))
                    || !model.scoping.symbol_redeclarations(*b).is_empty()
                {
                    return false;
                }
                if let Some(old) = self.locals.get(a) {
                    return old == b;
                }
                if self.locals.values().any(|s| s == b) {
                    return false;
                }
                self.locals.insert(*a, *b);
                true
            }
            (Tree::Global(a), Tree::Local(b)) if a.starts_with("CAP_") => {
                if site.span.contains_inclusive(model.scoping.symbol_span(*b)) {
                    return false;
                }
                if let Some(old) = self.captures.get(a) {
                    return old == b;
                }
                if self.captures.values().any(|s| s == b) {
                    return false;
                }
                self.captures.insert(a.clone(), *b);
                true
            }
            (Tree::Node(a, ac), Tree::Node(b, bc))
                if matches!(a.as_str(), "string:BUILD_TOKEN" | "string:DESCRIPTOR") => {
                b.starts_with("string:") && ac.is_empty() && bc.is_empty()
            }
            (Tree::Node(a, ac), Tree::Node(b, bc)) => {
                a == b
                    && ac.len() == bc.len()
                    && ac.iter().zip(bc).all(|(a, b)| self.tree(a, b, model, site))
            }
            (Tree::Global(a), Tree::Global(b)) => a == b,
            _ => false,
        }
    }
}

fn compile(source: &str, expression: bool) -> Tree {
    let allocator = Allocator::default();
    let program = crate::analysis::parse_program(&allocator, source)
        .expect("packing proof specification parses");
    let (scopes, _) = crate::analysis::analyze(&program);
    let mut syntax = Syntax {
        scopes: &scopes,
        remaining: 4096,
    };
    let result = if expression {
        let Statement::ExpressionStatement(s) = &program.body[0] else {
            unreachable!()
        };
        syntax.expression(&s.expression)
    } else {
        let Statement::FunctionDeclaration(f) = &program.body[0] else {
            unreachable!()
        };
        syntax.function(f)
    };
    result.expect("packing proof specification uses supported grammar")
}



pub(in crate::naming) struct CompleteSpec(Tree);
pub(in crate::naming) fn compile_complete(source: &str) -> CompleteSpec { CompleteSpec(compile(source, false)) }
pub(in crate::naming) fn complete_capture_specs(
    model: &Model<'_, '_>, site: FunctionSite, specifications: &[CompleteSpec],
) -> bool {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else { return false; };
    let Some(actual) = (Syntax { scopes: model.scoping, remaining: 4096 }).function(function) else { return false; };
    specifications.iter().any(|expected| {
        let mut matched = Match::default();
        if !matched.tree(&expected.0, &actual, model, site) { return false; }
        matched.captures.values().all(|id| {
            if !model.scoping.symbol_redeclarations(*id).is_empty() { return false; }
            let AstKind::VariableDeclarator(v) = model.nodes.kind(model.scoping.symbol_declaration(*id)) else { return false; };
            matches!(&v.id, BindingPattern::BindingIdentifier(i) if i.symbol_id.get() == Some(*id))
        })
    })
}
