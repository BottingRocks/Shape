//! Move computed SSA values only while their lexical dependencies are stable.
//!
//! Each straight-line statement list has a version table for initialized
//! lexical locals. A candidate captures those versions, not just their names.
//! Calls and control flow end a region. Expressions that can invoke getters,
//! coercion hooks, or throw are retained. The adjacent pass separately handles
//! effectful expressions when it can prove their exact evaluation position.

use std::collections::{BTreeMap, BTreeSet};

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::scope::ScopeFlags;

use super::super::index::SpanIndex;
use crate::analysis;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Number,
    String,
    Boolean,
    Null,
    Undefined,
    BigInt,
    Unknown,
}

#[derive(Clone, Copy)]
struct Value {
    version: usize,
    kind: Kind,
}
type Values = BTreeMap<SymbolId, Value>;

struct Snapshot {
    declaration: Span,
    value: Span,
    use_span: Span,
    use_scope: oxc_semantic::ScopeId,
    dependencies: Vec<(SymbolId, usize)>,
}

pub(crate) fn inline_snapshot_ssa(
    source: &str,
    protected: &BTreeSet<String>,
) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut facts = Facts::default();
    facts.visit_program(&program);
    facts.functions.build();
    if facts.dynamic_scope {
        return Ok((source.into(), 0));
    }
    let mut scan = Scan {
        protected,
        source,
        scoping: &scoping,
        nodes: &nodes,
        facts: &facts,
        function: None,
        edits: Vec::new(),
        count: 0,
    };
    scan.visit_program(&program);
    if scan.edits.is_empty() {
        return Ok((source.into(), 0));
    }
    let applied = analysis::apply_edits(source, scan.edits);
    if !applied.rejected.is_empty() {
        return Err("snapshot SSA edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)?;
    Ok((applied.output, scan.count))
}

#[derive(Default)]
struct Facts {
    functions: SpanIndex,
    dynamic_scope: bool,
}
impl<'a> Visit<'a> for Facts {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        self.functions.push(function.span);
        walk::walk_function(self, function, flags);
    }
    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        self.functions.push(arrow.span);
        walk::walk_arrow_function_expression(self, arrow);
    }
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        self.dynamic_scope |= id.name == "eval";
    }
    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.dynamic_scope = true;
        walk::walk_with_statement(self, statement);
    }
}

struct Scan<'s, 'a> {
    protected: &'s BTreeSet<String>,
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    facts: &'s Facts,
    function: Option<Span>,
    edits: Vec<analysis::Edit>,
    count: usize,
}

impl Scan<'_, '_> {
    fn body(&mut self, statements: &[Statement<'_>]) {
        let mut values = Values::new();
        let mut pending = BTreeMap::<SymbolId, Snapshot>::new();
        let mut version = 0;
        for statement in statements {
            // Do not move a snapshot into a branch, loop, try, switch, or
            // nested function. Their statement lists are visited separately.
            if !matches!(
                statement,
                Statement::VariableDeclaration(_)
                    | Statement::ExpressionStatement(_)
                    | Statement::ReturnStatement(_)
                    | Statement::ThrowStatement(_)
                    | Statement::EmptyStatement(_)
            ) {
                values.clear();
                pending.clear();
                continue;
            }
            let mut effects = Effects {
                scoping: self.scoping,
                reads: Vec::new(),
                writes: Vec::new(),
                barrier: false,
                non_value_reads: Vec::new(),
            };
            effects.visit_statement(statement);
            let changed = !effects.barrier && self.inline_pending(&effects, &values, &mut pending);

            // Classify against the versions on entry, before applying stores.
            let initialized = self.declared(statement, &values, &effects, changed, &mut pending);
            let assignment = self.assigned(statement, &values);
            if effects.barrier {
                values.clear();
                pending.clear();
            }
            for symbol in effects.writes {
                version += 1;
                if let Some(value) = values.get_mut(&symbol) {
                    *value = Value {
                        version,
                        kind: Kind::Unknown,
                    };
                }
            }
            if let Some((symbol, kind)) = assignment {
                if let Some(value) = values.get_mut(&symbol) {
                    value.kind = kind;
                }
            }
            if let Some((symbol, kind)) = initialized {
                version += 1;
                values.insert(symbol, Value { version, kind });
            }
        }
    }

    /// Inline each pending snapshot this statement reads at its one use,
    /// while its dependencies still hold the captured versions. Reports
    /// whether any was inlined.
    fn inline_pending(
        &mut self,
        effects: &Effects<'_>,
        values: &Values,
        pending: &mut BTreeMap<SymbolId, Snapshot>,
    ) -> bool {
        let mut changed = false;
        for (symbol, span) in &effects.reads {
            let Some(snapshot) = pending.remove(symbol) else {
                continue;
            };
            if snapshot.use_span != *span || effects.non_value_reads.contains(span) {
                continue;
            }
            let valid =
                snapshot
                    .dependencies
                    .iter()
                    .all(|(dependency, captured_version)| {
                        values
                            .get(dependency)
                            .is_some_and(|value| value.version == *captured_version)
                            && !effects.writes.contains(dependency)
                            && self.scoping.find_binding(
                                snapshot.use_scope,
                                self.scoping.symbol_ident(*dependency),
                            ) == Some(*dependency)
                    });
            if !valid {
                continue;
            }
            let value =
                &self.source[snapshot.value.start as usize..snapshot.value.end as usize];
            self.edits
                .push(analysis::Edit::delete(snapshot.declaration));
            self.edits
                .push(analysis::Edit::replace(*span, format!("({value})")));
            self.count += 1;
            changed = true;
        }
        changed
    }

    /// The binding a single `let`/`const` declares and its value's kind; a
    /// generated temporary read once later becomes a pending snapshot.
    fn declared(
        &self,
        statement: &Statement<'_>,
        values: &Values,
        effects: &Effects<'_>,
        changed: bool,
        pending: &mut BTreeMap<SymbolId, Snapshot>,
    ) -> Option<(SymbolId, Kind)> {
        let declaration = match statement {
            Statement::VariableDeclaration(d)
                if matches!(
                    d.kind,
                    VariableDeclarationKind::Let | VariableDeclarationKind::Const
                ) && d.declarations.len() == 1 =>
            {
                Some(d)
            }
            _ => None,
        };
        declaration.and_then(|d| {
            let binding = &d.declarations[0];
            let BindingPattern::BindingIdentifier(id) = &binding.id else {
                return None;
            };
            let symbol = id.symbol_id.get()?;
            let mut dependencies = Vec::new();
            let kind = match &binding.init {
                Some(init) => self.classify(init, values, &mut dependencies),
                None => Some(Kind::Undefined),
            };
            if !changed
                && !effects.barrier
                && (id.name.starts_with("$SSA_") || id.name.starts_with("$CARRY_"))
                && kind.is_some()
                && binding.init.as_ref().is_some_and(|init| {
                    !matches!(
                        init,
                        Expression::Identifier(_)
                            | Expression::NumericLiteral(_)
                            | Expression::StringLiteral(_)
                            | Expression::BooleanLiteral(_)
                            | Expression::NullLiteral(_)
                            | Expression::BigIntLiteral(_)
                    )
                })
            {
                let mut references = self.scoping.get_resolved_references(symbol);
                if let Some(reference) = references.next() {
                    let use_span = self.nodes.get_node(reference.node_id()).kind().span();
                    let text = &self.source[d.span.start as usize..d.span.end as usize];
                    if references.next().is_none()
                        && !reference.is_write()
                        && use_span.start >= d.span.end
                        && !text.contains("//")
                        && !text.contains("/*")
                    {
                        pending.insert(
                            symbol,
                            Snapshot {
                                declaration: d.span,
                                value: binding.init.as_ref().unwrap().span(),
                                use_span,
                                use_scope: reference.scope_id(),
                                dependencies,
                            },
                        );
                    }
                }
            }
            Some((symbol, kind.unwrap_or(Kind::Unknown)))
        })
    }

    /// The local a plain `x = value;` statement stores and the value's kind.
    fn assigned(&self, statement: &Statement<'_>, values: &Values) -> Option<(SymbolId, Kind)> {
        match statement {
            Statement::ExpressionStatement(s) => match &s.expression {
                Expression::AssignmentExpression(a)
                    if a.operator == AssignmentOperator::Assign =>
                {
                    match &a.left {
                        AssignmentTarget::AssignmentTargetIdentifier(id) => {
                            analysis::identifier_symbol(id, self.scoping).map(|symbol| {
                                (
                                    symbol,
                                    self.classify(&a.right, values, &mut Vec::new())
                                        .unwrap_or(Kind::Unknown),
                                )
                            })
                        }
                        _ => None,
                    }
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// Classify only expressions that neither allocate identities, throw, nor
    /// invoke user code. Unknown objects can be read/compared, but cannot be
    /// passed through arithmetic coercion. No guesses based on `$KEY_` names.
    fn classify(
        &self,
        expression: &Expression<'_>,
        values: &Values,
        dependencies: &mut Vec<(SymbolId, usize)>,
    ) -> Option<Kind> {
        let kind = match expression {
            Expression::NumericLiteral(_) => Kind::Number,
            Expression::StringLiteral(_) => Kind::String,
            Expression::BooleanLiteral(_) => Kind::Boolean,
            Expression::NullLiteral(_) => Kind::Null,
            Expression::BigIntLiteral(_) => Kind::BigInt,
            Expression::TemplateLiteral(t) if t.expressions.is_empty() => Kind::String,
            Expression::Identifier(id) => {
                if self.protected.contains(id.name.as_str()) {
                    return None;
                }
                let symbol = analysis::identifier_symbol(id, self.scoping)?;
                let value = values.get(&symbol)?;
                // A closure/setter/callback can change a lexical local even
                // without a visible store at its invocation site.
                if self.scoping.get_resolved_references(symbol).any(|r| {
                    r.is_write()
                        && self
                            .facts
                            .functions
                            .enclosing(self.nodes.get_node(r.node_id()).kind().span())
                            != self.function
                }) {
                    return None;
                }
                dependencies.push((symbol, value.version));
                value.kind
            }
            Expression::UnaryExpression(u) => {
                let argument = self.classify(&u.argument, values, dependencies)?;
                match u.operator {
                    UnaryOperator::LogicalNot => Kind::Boolean,
                    UnaryOperator::Typeof => Kind::String,
                    UnaryOperator::Void => Kind::Undefined,
                    UnaryOperator::UnaryPlus
                    | UnaryOperator::UnaryNegation
                    | UnaryOperator::BitwiseNot
                        if !matches!(argument, Kind::Unknown | Kind::BigInt) =>
                    {
                        Kind::Number
                    }
                    _ => return None,
                }
            }
            Expression::BinaryExpression(b) => {
                let left = self.classify(&b.left, values, dependencies)?;
                let right = self.classify(&b.right, values, dependencies)?;
                if matches!(
                    b.operator,
                    BinaryOperator::StrictEquality | BinaryOperator::StrictInequality
                ) {
                    Kind::Boolean
                } else {
                    if matches!(left, Kind::Unknown | Kind::BigInt)
                        || matches!(right, Kind::Unknown | Kind::BigInt)
                        || b.operator.is_in()
                        || b.operator.is_instance_of()
                    {
                        return None;
                    }
                    match b.operator {
                        BinaryOperator::Equality
                        | BinaryOperator::Inequality
                        | BinaryOperator::LessThan
                        | BinaryOperator::LessEqualThan
                        | BinaryOperator::GreaterThan
                        | BinaryOperator::GreaterEqualThan => Kind::Boolean,
                        BinaryOperator::Addition
                            if left == Kind::String || right == Kind::String =>
                        {
                            Kind::String
                        }
                        _ => Kind::Number,
                    }
                }
            }
            Expression::LogicalExpression(l) => {
                let a = self.classify(&l.left, values, dependencies)?;
                let b = self.classify(&l.right, values, dependencies)?;
                if a == b {
                    a
                } else {
                    Kind::Unknown
                }
            }
            Expression::ConditionalExpression(c) => {
                self.classify(&c.test, values, dependencies)?;
                let a = self.classify(&c.consequent, values, dependencies)?;
                let b = self.classify(&c.alternate, values, dependencies)?;
                if a == b {
                    a
                } else {
                    Kind::Unknown
                }
            }
            Expression::ParenthesizedExpression(p) => {
                self.classify(&p.expression, values, dependencies)?
            }
            _ => return None,
        };
        Some(kind)
    }
}

impl<'a> Visit<'a> for Scan<'_, 'a> {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        let previous = self.function.replace(function.span);
        if let Some(body) = &function.body {
            self.body(&body.statements);
        }
        walk::walk_function(self, function, flags);
        self.function = previous;
    }
    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        let previous = self.function.replace(arrow.span);
        self.body(&arrow.body.statements);
        walk::walk_arrow_function_expression(self, arrow);
        self.function = previous;
    }
    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        self.body(&block.body);
        walk::walk_block_statement(self, block);
    }
}

struct Effects<'s> {
    scoping: &'s Scoping,
    reads: Vec<(SymbolId, Span)>,
    writes: Vec<SymbolId>,
    non_value_reads: Vec<Span>,
    barrier: bool,
}
impl<'a> Visit<'a> for Effects<'_> {
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        let Some(reference) = id.reference_id.get().map(|r| self.scoping.get_reference(r)) else {
            return;
        };
        if let Some(symbol) = reference.symbol_id() {
            if reference.is_read() {
                self.reads.push((symbol, id.span));
            }
            if reference.is_write() {
                self.writes.push(symbol);
            }
        }
    }
    fn visit_expression(&mut self, expression: &Expression<'a>) {
        self.barrier |= matches!(
            expression,
            Expression::CallExpression(_)
                | Expression::NewExpression(_)
                | Expression::StaticMemberExpression(_)
                | Expression::ComputedMemberExpression(_)
                | Expression::PrivateFieldExpression(_)
                | Expression::AwaitExpression(_)
                | Expression::YieldExpression(_)
                | Expression::TaggedTemplateExpression(_)
                | Expression::ObjectExpression(_)
        );
        if let Expression::ArrayExpression(array) = expression {
            self.barrier |= array
                .elements
                .iter()
                .any(|e| matches!(e, ArrayExpressionElement::SpreadElement(_)));
        }
        walk::walk_expression(self, expression);
    }
    fn visit_object_property(&mut self, property: &ObjectProperty<'a>) {
        if property.shorthand {
            self.non_value_reads.push(property.value.span());
        }
        walk::walk_object_property(self, property);
    }
    fn visit_unary_expression(&mut self, unary: &UnaryExpression<'a>) {
        if unary.operator == UnaryOperator::Delete {
            if let Expression::Identifier(id) = &unary.argument {
                self.non_value_reads.push(id.span);
            }
        }
        walk::walk_unary_expression(self, unary);
    }
    fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {}
    fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}
}
