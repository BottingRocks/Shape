//! Branch copies and stores, the facts they give, and immutable producers.

use super::*;

#[derive(Clone, Copy)]
pub(super) struct CopyOut<'a> {
    pub(super) value: Input,
    pub(super) statement: Span,
    pub(super) point: Span,
    pub(super) dest: Span,
    pub(super) name: Ident<'a>,
    pub(super) read: Span,
    pub(super) var: bool,
}
pub(super) struct BranchStore {
    pub(super) assignment: Span,
    pub(super) left: Span,
    pub(super) rhs: Span,
    pub(super) scope: oxc_semantic::ScopeId,
}
pub(super) struct BranchCopy<'a> {
    pub(super) copy: CopyOut<'a>,
    pub(super) leaves: Vec<BranchStore>,
}
pub(super) fn branch_leaves(
    statement: &Statement<'_>,
    symbol: SymbolId,
    scoping: &Scoping,
    leaves: &mut Vec<BranchStore>,
) -> Option<()> {
    match statement {
        Statement::BlockStatement(b) => branch_leaves(
            b.body
                .iter()
                .rev()
                .find(|s| !matches!(s, Statement::EmptyStatement(_)))?,
            symbol,
            scoping,
            leaves,
        ),
        Statement::IfStatement(i) => {
            branch_leaves(&i.consequent, symbol, scoping, leaves)?;
            branch_leaves(i.alternate.as_ref()?, symbol, scoping, leaves)
        }
        Statement::ReturnStatement(_) | Statement::ThrowStatement(_) => Some(()),
        Statement::ExpressionStatement(s) => {
            let Expression::AssignmentExpression(a) = unwrap(&s.expression) else {
                return None;
            };
            let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else {
                return None;
            };
            if a.operator != AssignmentOperator::Assign
                || analysis::identifier_symbol(id, scoping) != Some(symbol)
                || matches!(
                    unwrap(&a.right),
                    Expression::FunctionExpression(_)
                        | Expression::ArrowFunctionExpression(_)
                        | Expression::ClassExpression(_)
                )
            {
                return None;
            }
            leaves.push(BranchStore {
                assignment: a.span,
                left: id.span,
                rhs: a.right.span(),
                scope: scoping.get_reference(id.reference_id.get()?).scope_id(),
            });
            Some(())
        }
        _ => None,
    }
}
#[derive(Default)]
pub(super) struct Facts<'a> {
    pub(super) owners: SpanIndex,
    pub(super) bodies: BTreeMap<Span, &'a FunctionBody<'a>>,
    pub(super) blocks: BTreeMap<Span, &'a BlockStatement<'a>>,
    pub(super) arguments: BTreeSet<Span>,
    pub(super) following: BTreeMap<Span, &'a Statement<'a>>,
    pub(super) previous: BTreeMap<Span, &'a Statement<'a>>,
    pub(super) lists: BTreeMap<Span, Span>,
    pub(super) copies: BTreeMap<SymbolId, Vec<CopyOut<'a>>>,
    pub(super) declarations: BTreeMap<SymbolId, (&'a VariableDeclaration<'a>, &'a VariableDeclarator<'a>)>,
    pub(super) stores: BTreeMap<SymbolId, Vec<(Span, Span, Input)>>,
    pub(super) discarded: BTreeMap<Span, (Span, bool)>,
    pub(super) nonvalues: BTreeSet<Span>,
    pub(super) bare: BTreeSet<Span>,
    pub(super) function_names: BTreeSet<String>,
    pub(super) dynamic: bool,
    pub(super) pending: bool,
}
impl<'a> Facts<'a> {
    pub(super) fn list(&mut self, owner: Span, body: &'a [Statement<'a>], scoping: &Scoping) {
        for pair in body.windows(2) {
            self.following.insert(pair[0].span(), &pair[1]);
            self.previous.insert(pair[1].span(), &pair[0]);
        }
        for statement in body {
            self.lists.insert(statement.span(), owner);
            let copy = match statement {
                Statement::ExpressionStatement(s) => {
                    let Expression::AssignmentExpression(a) = unwrap(&s.expression) else {
                        continue;
                    };
                    if a.operator != AssignmentOperator::Assign {
                        continue;
                    }
                    let (
                        AssignmentTarget::AssignmentTargetIdentifier(to),
                        Expression::Identifier(from),
                    ) = (&a.left, unwrap(&a.right))
                    else {
                        continue;
                    };
                    let Some(source) = analysis::identifier_symbol(from, scoping) else {
                        continue;
                    };
                    let Some(value) = analysis::identifier_symbol(to, scoping)
                        .map(Input::Local)
                        .or_else(|| {
                            to.name
                                .strip_prefix("$KEY_")?
                                .parse()
                                .ok()
                                .map(Input::Incoming)
                        })
                    else {
                        continue;
                    };
                    (
                        source,
                        CopyOut {
                            value,
                            statement: s.span,
                            point: a.span,
                            dest: to.span,
                            name: to.name,
                            read: from.span,
                            var: false,
                        },
                    )
                }
                Statement::VariableDeclaration(d)
                    if d.kind == VariableDeclarationKind::Var && d.declarations.len() == 1 =>
                {
                    let item = &d.declarations[0];
                    let (BindingPattern::BindingIdentifier(to), Some(Expression::Identifier(from))) =
                        (&item.id, item.init.as_ref().map(unwrap))
                    else {
                        continue;
                    };
                    let (Some(source), Some(dest)) = (
                        analysis::identifier_symbol(from, scoping),
                        to.symbol_id.get(),
                    ) else {
                        continue;
                    };
                    (
                        source,
                        CopyOut {
                            value: Input::Local(dest),
                            statement: d.span,
                            point: item.span,
                            dest: to.span,
                            name: to.name,
                            read: from.span,
                            var: true,
                        },
                    )
                }
                _ => continue,
            };
            if copy.1.name.starts_with("$KEY_") {
                self.copies.entry(copy.0).or_default().push(copy.1);
            }
        }
    }
    pub(super) fn adjacent(&self, from: Span, to: Span) -> bool {
        let mut next = self.following.get(&from);
        while let Some(Statement::EmptyStatement(s)) = next {
            next = self.following.get(&s.span);
        }
        next.is_some_and(|s| s.span() == to)
    }
}
impl<'a> Visit<'a> for Facts<'a> {
    fn visit_function(&mut self, f: &Function<'a>, flags: ScopeFlags) {
        self.owners.push(f.span);
        if let Some(id) = &f.id {
            self.function_names.insert(id.name.to_string());
        }
        if super::super::super::dead_ssa::readonly_arguments(f) {
            self.arguments.insert(f.span);
        }
        self.pending |=
            f.id.as_ref()
                .is_some_and(|id| id.name.starts_with("$THREAD_"))
                && f.body
                    .as_ref()
                    .is_some_and(|b| b.statements.is_empty() && b.directives.is_empty());
        walk::walk_function(self, f, flags);
    }
    fn visit_arrow_function_expression(&mut self, f: &ArrowFunctionExpression<'a>) {
        self.owners.push(f.span);
        walk::walk_arrow_function_expression(self, f);
    }
    fn visit_class(&mut self, c: &Class<'a>) {
        self.owners.push(c.span);
        walk::walk_class(self, c);
    }
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        self.dynamic |= id.name == "eval";
    }
    fn visit_with_statement(&mut self, s: &WithStatement<'a>) {
        self.dynamic = true;
        walk::walk_with_statement(self, s);
    }
    fn visit_unary_expression(&mut self, u: &UnaryExpression<'a>) {
        if u.operator == UnaryOperator::Typeof {
            self.bare.insert(unwrap(&u.argument).span());
        }
        if u.operator == UnaryOperator::Delete {
            self.nonvalues.insert(unwrap(&u.argument).span());
        }
        walk::walk_unary_expression(self, u);
    }
    fn visit_object_property(&mut self, p: &ObjectProperty<'a>) {
        if p.shorthand {
            self.nonvalues.insert(p.value.span());
        }
        walk::walk_object_property(self, p);
    }
    fn visit_call_expression(&mut self, c: &CallExpression<'a>) {
        self.bare.insert(unwrap(&c.callee).span());
        walk::walk_call_expression(self, c);
    }
    fn visit_tagged_template_expression(&mut self, t: &TaggedTemplateExpression<'a>) {
        self.bare.insert(unwrap(&t.tag).span());
        walk::walk_tagged_template_expression(self, t);
    }
    fn visit_variable_declaration(&mut self, d: &VariableDeclaration<'a>) {
        self.dynamic |= !matches!(
            d.kind,
            VariableDeclarationKind::Let
                | VariableDeclarationKind::Const
                | VariableDeclarationKind::Var
        );
        walk::walk_variable_declaration(self, d);
    }
}
// A delayed copy keeps its existing destination write at the original point.
// Only an inert producer whose lexical inputs cannot change may move there.
pub(super) fn immutable_producer(
    e: &Expression<'_>,
    scoping: &Scoping,
    nodes: &oxc_semantic::AstNodes<'_>,
    locals: &LocalReads,
    protected: &BTreeSet<String>,
    arguments: bool,
) -> bool {
    fn scalar(e: &Expression<'_>, scoping: &Scoping, locals: &LocalReads) -> bool {
        match unwrap(e) {
            Expression::NumericLiteral(_)
            | Expression::StringLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_) => true,
            Expression::Identifier(id) => analysis::identifier_symbol(id, scoping)
                .is_some_and(|s| locals.primitive_cell(s, id.span)),
            Expression::UnaryExpression(u) => {
                matches!(
                    u.operator,
                    UnaryOperator::LogicalNot
                        | UnaryOperator::Typeof
                        | UnaryOperator::Void
                        | UnaryOperator::UnaryPlus
                ) || scalar(&u.argument, scoping, locals)
            }
            Expression::BinaryExpression(b) => {
                b.operator.is_compare()
                    || b.operator.is_equality()
                    || scalar(&b.left, scoping, locals) && scalar(&b.right, scoping, locals)
            }
            Expression::LogicalExpression(l) => {
                scalar(&l.left, scoping, locals) && scalar(&l.right, scoping, locals)
            }
            Expression::ConditionalExpression(c) => {
                scalar(&c.consequent, scoping, locals) && scalar(&c.alternate, scoping, locals)
            }
            _ => false,
        }
    }
    let inert = |e| immutable_producer(e, scoping, nodes, locals, protected, arguments);
    match unwrap(e) {
        Expression::NumericLiteral(_)|Expression::StringLiteral(_)|Expression::BooleanLiteral(_)
        |Expression::NullLiteral(_)|Expression::BigIntLiteral(_)|Expression::RegExpLiteral(_)=>true,
        Expression::Identifier(id)=>analysis::identifier_symbol(id,scoping).is_some_and(|s|
            !protected.contains(id.name.as_str()) && locals.initialized(s,id.span)
            && scoping.symbol_flags(s).contains(oxc_syntax::symbol::SymbolFlags::BlockScopedVariable)
            && scoping.symbol_redeclarations(s).is_empty()
            && scoping.get_resolved_references(s).all(|r|!r.is_write())
            && matches!(nodes.kind(scoping.symbol_declaration(s)),AstKind::VariableDeclarator(d)if d.init.is_some()&&matches!(d.id,BindingPattern::BindingIdentifier(_)))),
        Expression::ComputedMemberExpression(m) if arguments && super::super::super::argument_snapshots::input(e)=>{
            matches!(unwrap(&m.object),Expression::Identifier(id)if analysis::identifier_symbol(id,scoping).is_none())
        },
        Expression::UnaryExpression(u)=>inert(&u.argument)&&u.operator!=UnaryOperator::Delete
            &&(matches!(u.operator,UnaryOperator::LogicalNot|UnaryOperator::Typeof|UnaryOperator::Void)||scalar(&u.argument,scoping,locals)),
        Expression::BinaryExpression(b)=>inert(&b.left)&&inert(&b.right)
            &&(matches!(b.operator,BinaryOperator::StrictEquality|BinaryOperator::StrictInequality)
                ||matches!(b.operator,BinaryOperator::Equality|BinaryOperator::Inequality)
                    &&(matches!(unwrap(&b.left),Expression::NullLiteral(_))||matches!(unwrap(&b.right),Expression::NullLiteral(_)))
                ||!b.operator.is_in()&&!b.operator.is_instance_of()&&scalar(&b.left,scoping,locals)&&scalar(&b.right,scoping,locals)),
        Expression::LogicalExpression(l)=>inert(&l.left)&&inert(&l.right),
        Expression::ConditionalExpression(c)=>inert(&c.test)&&inert(&c.consequent)&&inert(&c.alternate),
        Expression::SequenceExpression(s)=>s.expressions.iter().all(inert),
        Expression::ArrayExpression(a)=>a.elements.iter().all(|e|e.as_expression().is_some_and(inert)),
        Expression::ObjectExpression(o)=>o.properties.iter().all(|p|matches!(p,ObjectPropertyKind::ObjectProperty(p)
            if !p.computed&&!p.method&&p.kind==PropertyKind::Init&&inert(&p.value))),
        Expression::TemplateLiteral(t)=>t.expressions.iter().all(|e|inert(e)&&scalar(e,scoping,locals)),
        _=>false,
    }
}
