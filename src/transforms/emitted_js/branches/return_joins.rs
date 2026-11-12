//! Remove private join storage used only by an immediately following consumer.
//! Returns, expression statements and generated local initializers may sink
//! into every fallthrough branch. Stores retain their RHS evaluation order;
//! delaying a nonterminal store additionally requires an inert stable value.
//! Other consumer names must resolve identically in each branch, and lifted
//! locals must preserve initialization, capture and scope behavior.
use super::super::{adjacent_ssa::LocalReads, index::SpanIndex};
use crate::analysis;
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{Scoping, SymbolId};
use oxc_span::GetSpan;
use oxc_syntax::scope::ScopeFlags;
use std::collections::BTreeSet;

#[cfg(test)]
pub(crate) fn eliminate_tail_joins(
    source: &str,
    protected: &BTreeSet<String>,
) -> Result<(String, usize), String> {
    eliminate_thread_tail_joins(source, protected, false)
}
pub(crate) fn eliminate_thread_tail_joins(
    source: &str,
    protected: &BTreeSet<String>,
    checked_captures: bool,
) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut facts = Facts::default();
    facts.visit_program(&program);
    facts.owners.build();
    if facts.dynamic {
        return Ok((source.into(), 0));
    }
    let mut locals = LocalReads::default();
    locals.visit_program(&program);
    locals.build(&scoping, &nodes, protected);
    let mut scan = Scan {
        source,
        protected,
        scoping: &scoping,
        nodes: &nodes,
        owners: &facts.owners,
        locals: &locals,
        edits: vec![],
        count: 0,
        checked_captures,
        pending: facts.pending,
    };
    super::super::support::for_each_list(&program, |list| scan.body(list));
    let result = analysis::apply_edits(source, scan.edits);
    if !result.rejected.is_empty() {
        return Err("return join edits overlap".into());
    }
    Ok((result.output, scan.count))
}
#[derive(Default)]
struct Facts {
    owners: SpanIndex,
    dynamic: bool,
    pending: bool,
}
impl<'a> Visit<'a> for Facts {
    fn visit_variable_declaration(&mut self, d: &VariableDeclaration<'a>) {
        // Scope-exit disposal must not observe an earlier destination write.
        self.dynamic |= !matches!(
            d.kind,
            VariableDeclarationKind::Var
                | VariableDeclarationKind::Let
                | VariableDeclarationKind::Const
        );
        walk::walk_variable_declaration(self, d);
    }
    fn visit_function(&mut self, f: &Function<'a>, flags: ScopeFlags) {
        self.pending |=
            f.id.as_ref()
                .is_some_and(|id| id.name.starts_with("$THREAD_"))
                && f.body
                    .as_ref()
                    .is_some_and(|b| b.statements.is_empty() && b.directives.is_empty());
        self.owners.push(f.span);
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
}
struct Scan<'s, 'a> {
    source: &'s str,
    protected: &'s BTreeSet<String>,
    scoping: &'s Scoping,
    nodes: &'s oxc_semantic::AstNodes<'a>,
    owners: &'s SpanIndex,
    locals: &'s LocalReads,
    edits: Vec<analysis::Edit>,
    count: usize,
    checked_captures: bool,
    pending: bool,
}
/// A joined read and the consumer it feeds.
type Candidate<'c> = (&'c IdentifierReference<'c>, Continuation<'c, 'c>);
enum Continuation<'s, 'a> {
    Return(&'s Expression<'a>, &'s IdentifierReference<'a>),
    Copy(&'s IdentifierReference<'a>),
    Evaluate(&'s Expression<'a>, &'s IdentifierReference<'a>),
    Initialize(&'s Expression<'a>, &'s IdentifierReference<'a>, SymbolId),
}

// Moving the continuation into a branch must not resolve any of its names
// against that branch's declarations. Nested environments need a separate
// scope proof, so leave those continuations in place.
struct ReturnScope<'s> {
    scoping: &'s Scoping,
    scope: oxc_semantic::ScopeId,
    target: oxc_span::Span,
    safe: bool,
    bare_callee: bool,
}
impl<'a> Visit<'a> for ReturnScope<'_> {
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        if id.span != self.target {
            self.safe &= self.scoping.find_binding(self.scope, id.name)
                == analysis::identifier_symbol(id, self.scoping);
        }
    }
    fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {
        self.safe = false;
    }
    fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {
        self.safe = false;
    }
    fn visit_class(&mut self, _: &Class<'a>) {
        self.safe = false;
    }
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        let mut callee = &call.callee;
        while let Expression::ParenthesizedExpression(p) = callee {
            callee = &p.expression;
        }
        self.bare_callee |= matches!(callee, Expression::Identifier(id) if id.span == self.target);
        walk::walk_call_expression(self, call);
    }
    fn visit_tagged_template_expression(&mut self, tag: &TaggedTemplateExpression<'a>) {
        let mut value = &tag.tag;
        while let Expression::ParenthesizedExpression(p) = value {
            value = &p.expression;
        }
        self.bare_callee |= matches!(value, Expression::Identifier(id) if id.span == self.target);
        walk::walk_tagged_template_expression(self, tag);
    }
    fn visit_unary_expression(&mut self, unary: &UnaryExpression<'a>) {
        let mut value = &unary.argument;
        while let Expression::ParenthesizedExpression(p) = value {
            value = &p.expression;
        }
        // A cached value has already performed GetValue; typeof must not
        // turn an unresolved RHS read into a nonthrowing identifier query.
        self.bare_callee |= unary.operator == UnaryOperator::Typeof
            && matches!(value, Expression::Identifier(id) if id.span == self.target);
        if unary.operator == UnaryOperator::Delete
            && matches!(value, Expression::Identifier(id) if id.span == self.target)
        {
            self.safe = false;
        }
        walk::walk_unary_expression(self, unary);
    }
    fn visit_object_property(&mut self, property: &ObjectProperty<'a>) {
        if property.shorthand && property.value.span() == self.target {
            self.safe = false;
        }
        walk::walk_object_property(self, property);
    }
}

#[derive(Default)]
struct ConsumerReads {
    ids: Vec<oxc_syntax::reference::ReferenceId>,
}
impl<'a> Visit<'a> for ConsumerReads {
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        if let Some(reference) = id.reference_id.get() {
            self.ids.push(reference);
        }
    }
    fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {}
    fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}
    fn visit_class(&mut self, _: &Class<'a>) {}
}
impl<'a> Scan<'_, 'a> {
    fn stable_rhs(&self, e: &Expression<'_>) -> bool {
        if super::super::purity::expression_is_pure(e) {
            return true;
        }
        match e {
            Expression::ParenthesizedExpression(p) => self.stable_rhs(&p.expression),
            Expression::Identifier(id) => analysis::identifier_symbol(id, self.scoping)
                .is_some_and(|symbol| {
                    !self.protected.contains(self.scoping.symbol_name(symbol))
                        && (!self.pending
                            || self.checked_captures
                                && ["$SSA_", "$CARRY_", "$ARRAY_", "$OBJECT_"]
                                    .iter()
                                    .any(|prefix| id.name.starts_with(prefix)))
                        && self
                            .scoping
                            .symbol_flags(symbol)
                            .contains(oxc_syntax::symbol::SymbolFlags::BlockScopedVariable)
                        && self.locals.initialized(symbol, id.span)
                        && self
                            .scoping
                            .get_resolved_references(symbol)
                            .all(|r| !r.is_write())
                }),
            _ => false,
        }
    }
    fn terminal(
        &self,
        s: &Statement<'_>,
        target: SymbolId,
        continuation: &Continuation<'_, '_>,
        edits: &mut Vec<analysis::Edit>,
    ) -> bool {
        match s {
            Statement::BlockStatement(b) => {
                let checkpoint = edits.len();
                if b.body
                    .iter()
                    .rev()
                    .find(|s| !matches!(s, Statement::EmptyStatement(_)))
                    .is_some_and(|s| self.terminal(s, target, continuation, edits))
                {
                    return true;
                }
                edits.truncate(checkpoint);
                self.delayed_store(b, target, continuation, edits)
            }
            Statement::IfStatement(i) => i.alternate.as_ref().is_some_and(|alt| {
                self.terminal(&i.consequent, target, continuation, edits)
                    && self.terminal(alt, target, continuation, edits)
            }),
            Statement::ReturnStatement(_) | Statement::ThrowStatement(_) => true,
            Statement::ExpressionStatement(s) => self.sunk_store(s, target, continuation, edits),
            _ => false,
        }
    }
    /// A block whose last store to `target` has later work after it.
    fn delayed_store(
        &self,
        b: &BlockStatement<'_>,
        target: SymbolId,
        continuation: &Continuation<'_, '_>,
        edits: &mut Vec<analysis::Edit>,
    ) -> bool {
        // Parallel join channels can have later work after this
        // store. Delay only an inert, stable RHS read; keep the
        // destination write after that work, at the original handoff.
        for statement in b.body.iter().rev() {
            let Statement::ExpressionStatement(s) = statement else {
                continue;
            };
            let Expression::AssignmentExpression(a) = &s.expression else {
                continue;
            };
            let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else {
                continue;
            };
            if analysis::identifier_symbol(id, self.scoping) != Some(target) {
                continue;
            }
            if !self.stable_rhs(&a.right)
                || self.scoping.get_resolved_references(target).any(|r| {
                    let span = self.nodes.get_node(r.node_id()).kind().span();
                    r.is_write() && s.span.end <= span.start && span.end <= b.span.end
                })
            {
                return false;
            }
            let mut store = vec![];
            if !self.terminal(statement, target, continuation, &mut store) {
                return false;
            }
            let Some(store) = store.pop() else {
                return false;
            };
            edits.push(analysis::Edit::delete(s.span));
            edits.push(analysis::Edit::replace(
                oxc_span::Span::new(b.span.end - 1, b.span.end - 1),
                format!("\n{}\n", store.replacement),
            ));
            return true;
        }
        false
    }
    /// A branch's final `target = rhs;` becomes the continuation with `rhs`
    /// in place of the joined read.
    fn sunk_store(
        &self,
        s: &ExpressionStatement<'_>,
        target: SymbolId,
        continuation: &Continuation<'_, '_>,
        edits: &mut Vec<analysis::Edit>,
    ) -> bool {
        let Expression::AssignmentExpression(a) = &s.expression else {
            return false;
        };
        let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else {
            return false;
        };
        if a.operator != AssignmentOperator::Assign
            || analysis::identifier_symbol(id, self.scoping) != Some(target)
            || !self.locals.initialized(target, s.span)
        {
            return false;
        }
        let mut rhs = &a.right;
        while let Expression::ParenthesizedExpression(p) = rhs {
            rhs = &p.expression;
        }
        if matches!(
            rhs,
            Expression::FunctionExpression(_)
                | Expression::ArrowFunctionExpression(_)
                | Expression::ClassExpression(_)
        ) {
            return false;
        }
        let text = &self.source[s.span.start as usize..s.span.end as usize];
        if text.contains("/*") || text.contains("//") {
            return false;
        }
        let Some(replacement) = self.sunk_continuation(a, id, continuation) else {
            return false;
        };
        edits.push(analysis::Edit::replace(s.span, replacement));
        true
    }
    /// The continuation's text with the store `id = a.right` substituted for
    /// its read, when its names resolve the same at the store.
    fn sunk_continuation(
        &self,
        a: &AssignmentExpression<'_>,
        id: &IdentifierReference<'_>,
        continuation: &Continuation<'_, '_>,
    ) -> Option<String> {
        let rhs = &self.source[a.right.span().start as usize..a.right.span().end as usize];
        let replacement = match continuation {
            Continuation::Return(expression, read)
            | Continuation::Evaluate(expression, read)
            | Continuation::Initialize(expression, read, _) => {
                if !self.stable_rhs(&a.right)
                    && !super::super::adjacent_ssa::first_evaluated_identifier(expression)
                        .is_some_and(|first| first.span == read.span)
                {
                    return None;
                }
                let reference = id.reference_id.get()?;
                let mut check = ReturnScope {
                    scoping: self.scoping,
                    scope: self.scoping.get_reference(reference).scope_id(),
                    target: read.span,
                    safe: true,
                    bare_callee: false,
                };
                check.visit_expression(expression);
                if !check.safe {
                    return None;
                }
                let value = if check.bare_callee {
                    format!("(0, ({rhs}))")
                } else {
                    format!("({rhs})")
                };
                let before =
                    &self.source[expression.span().start as usize..read.span.start as usize];
                let after = &self.source[read.span.end as usize..expression.span().end as usize];
                let expression = format!("({before}{value}{after})");
                match continuation {
                    Continuation::Return(..) => format!("return {expression};"),
                    Continuation::Evaluate(..) => format!("{expression};"),
                    Continuation::Initialize(_, _, destination) => {
                        let scope = self.scoping.get_reference(reference).scope_id();
                        if self
                            .scoping
                            .find_binding(scope, self.scoping.symbol_ident(*destination))
                            != Some(*destination)
                        {
                            return None;
                        }
                        format!("{} = {expression};", self.scoping.symbol_name(*destination))
                    }
                    Continuation::Copy(_) => unreachable!(),
                }
            }
            Continuation::Copy(destination) => {
                let reference = id.reference_id.get()?;
                let scope = self.scoping.get_reference(reference).scope_id();
                if self.scoping.find_binding(scope, destination.name)
                    != analysis::identifier_symbol(destination, self.scoping)
                {
                    return None;
                }
                format!("{} = ({rhs});", destination.name)
            }
        };
        Some(replacement)
    }
    fn body(&mut self, body: &[Statement<'_>]) {
        let executable = body
            .iter()
            .filter(|s| !matches!(s, Statement::EmptyStatement(_)))
            .collect::<Vec<_>>();
        for pair in executable.windows(2) {
            if !matches!(pair[0], Statement::IfStatement(_)) {
                continue;
            }
            let Some((candidates, lifted)) = self.candidates(pair[1]) else {
                continue;
            };
            for (id, continuation) in candidates {
                if let Some(proposal) =
                    self.proposal(pair[0], pair[1], id, &continuation, lifted.as_ref())
                {
                    self.edits.extend(proposal);
                    self.count += 1;
                    break;
                }
            }
        }
    }
    /// The joined reads `consumer` could take from each branch instead, with
    /// the declaration to lift ahead of the branch when it initializes a local.
    fn candidates<'c>(
        &self,
        consumer: &'c Statement<'c>,
    ) -> Option<(Vec<Candidate<'c>>, Option<(SymbolId, String)>)>
    where
        'a: 'c,
    {
        let mut candidates = Vec::new();
        let mut lifted = None;
        let expression = match consumer {
            Statement::ReturnStatement(r) => {
                let expression = r.argument.as_ref()?;
                if let Some(id) = super::super::adjacent_ssa::first_evaluated_identifier(expression)
                {
                    candidates.push((id, Continuation::Return(expression, id)));
                }
                Some(expression)
            }
            Statement::ExpressionStatement(s) => {
                if let Expression::AssignmentExpression(a) = &s.expression {
                    if let (
                        AssignmentTarget::AssignmentTargetIdentifier(destination),
                        Expression::Identifier(id),
                    ) = (&a.left, &a.right)
                    {
                        if a.operator == AssignmentOperator::Assign
                            && (analysis::identifier_symbol(destination, self.scoping).is_some()
                                || self.protected.contains(destination.name.as_str()))
                        {
                            candidates.push((id.as_ref(), Continuation::Copy(destination)));
                        }
                    }
                }
                Some(&s.expression)
            }
            Statement::VariableDeclaration(d)
                if d.declarations.len() == 1
                    && matches!(
                        d.kind,
                        VariableDeclarationKind::Let | VariableDeclarationKind::Var
                    ) =>
            {
                let (BindingPattern::BindingIdentifier(id), Some(init)) =
                    (&d.declarations[0].id, &d.declarations[0].init)
                else {
                    return None;
                };
                let destination = id.symbol_id.get()?;
                if !(id.name.starts_with("$SSA_") || id.name.starts_with("$CARRY_"))
                    || self.protected.contains(id.name.as_str())
                    || self.pending && !self.checked_captures
                    || !self.scoping.symbol_redeclarations(destination).is_empty()
                    || self.scoping.symbol_span(destination) != id.span
                    || self.scoping.get_resolved_references(destination).any(|r| {
                        let at = self.nodes.get_node(r.node_id()).kind().span();
                        at.start < d.span.end
                            || self.owners.enclosing(at) != self.owners.enclosing(d.span)
                    })
                {
                    return None;
                }
                lifted = Some((destination, format!("{} {};\n", d.kind.as_str(), id.name)));
                Some(init)
            }
            _ => return None,
        };
        if let Some(expression) = expression
            .filter(|e| e.span().size() <= 512 && (!self.pending || self.checked_captures))
        {
            let mut reads = ConsumerReads::default();
            reads.visit_expression(expression);
            for reference in reads.ids {
                let r = self.scoping.get_reference(reference);
                if !r.is_read() || r.is_write() {
                    continue;
                }
                let oxc_ast::AstKind::IdentifierReference(id) = self.nodes.kind(r.node_id()) else {
                    continue;
                };
                if !(id.name.starts_with("$SSA_") || id.name.starts_with("$CARRY_")) {
                    continue;
                }
                let continuation = match consumer {
                    Statement::ReturnStatement(_) => Continuation::Return(expression, id),
                    Statement::VariableDeclaration(_) => {
                        Continuation::Initialize(expression, id, lifted.as_ref().unwrap().0)
                    }
                    _ => Continuation::Evaluate(expression, id),
                };
                candidates.push((id, continuation));
            }
        }
        Some((candidates, lifted))
    }
    /// The edits that sink `consumer`'s read of `id` into every arm of
    /// `branch`, when `id` is a private join cell and the result stays small
    /// and clear of earlier edits.
    fn proposal(
        &self,
        branch: &Statement<'_>,
        consumer: &Statement<'_>,
        id: &IdentifierReference<'_>,
        continuation: &Continuation<'_, '_>,
        lifted: Option<&(SymbolId, String)>,
    ) -> Option<Vec<analysis::Edit>> {
        let span = consumer.span();
        let target = analysis::identifier_symbol(id, self.scoping)?;
        if self.protected.contains(self.scoping.symbol_name(target))
            || self.scoping.symbol_flags(target).is_const_variable()
            || self.scoping.symbol_span(target).end >= branch.span().start
            || !self.scoping.symbol_redeclarations(target).is_empty()
            || self
                .scoping
                .get_resolved_references(target)
                .any(|reference| {
                    let at = self.nodes.get_node(reference.node_id()).kind().span();
                    (reference.is_read() && at != id.span)
                        || self.owners.enclosing(at) != self.owners.enclosing(span)
                })
        {
            return None;
        }
        let mut proposal = vec![];
        if !self.terminal(branch, target, continuation, &mut proposal) || proposal.is_empty() {
            return None;
        }
        if self.source[span.start as usize..span.end as usize].contains("/*")
            || self.source[span.start as usize..span.end as usize].contains("//")
        {
            return None;
        }
        proposal.push(analysis::Edit::delete(span));
        if matches!(continuation, Continuation::Initialize(..)) {
            proposal.push(analysis::Edit::replace(
                oxc_span::Span::new(branch.span().start, branch.span().start),
                lifted.unwrap().1.clone(),
            ));
        }
        // Bound consumer duplication in multi-arm joins. Later dead-cell
        // cleanup removes the now-unread handoff declaration as well.
        let old_bytes: usize = proposal.iter().map(|e| (e.end - e.start) as usize).sum();
        let new_bytes: usize = proposal.iter().map(|e| e.replacement.len()).sum();
        if matches!(
            continuation,
            Continuation::Evaluate(..) | Continuation::Initialize(..)
        ) && (proposal.len() > 26 || new_bytes > old_bytes + 192)
        {
            return None;
        }
        if proposal.iter().any(|e| {
            self.edits
                .iter()
                .any(|p| e.start < p.end && p.start < e.end)
        }) {
            return None;
        }
        Some(proposal)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn initializer_and_effect_consumers_remove_branch_handoff_stores() {
        for consumer in [
            "let $SSA_result=$CARRY_pick*2;return $SSA_result;",
            "consume($CARRY_pick);",
        ] {
            let source=format!("function f(flag){{let $CARRY_pick;if(flag){{const left=3;effect();$CARRY_pick=left;}}else{{const right=7;effect();$CARRY_pick=right;}}{consumer}}}");
            let (code, count) = super::eliminate_tail_joins(&source, &Default::default()).unwrap();
            assert_eq!(count, 1, "{code}");
            assert!(!code.contains("$CARRY_pick="), "{code}");
            if consumer.starts_with("let") {
                assert!(code.contains("let $SSA_result;"), "{code}");
            }
        }
    }

    #[test]
    fn lifted_initializers_require_private_scope_and_capture_facts() {
        for source in [
            "function f(flag){let $CARRY_pick;if(flag){let $SSA_result=99;$CARRY_pick=3;}else{$CARRY_pick=7;}let $SSA_result=$CARRY_pick*2;return $SSA_result;}",
            "function f(flag){let $CARRY_pick;if(flag){read();$CARRY_pick=3;}else{$CARRY_pick=7;}let $SSA_result=$CARRY_pick*2;function read(){return $SSA_result;}return $SSA_result;}",
            "function f(flag){function $THREAD_1(){}let $CARRY_pick;if(flag){$CARRY_pick=3;}else{$CARRY_pick=7;}let $SSA_result=$CARRY_pick*2;return $SSA_result;}",
            "function f(flag,$SSA_result){let $CARRY_pick;if(flag){$CARRY_pick=3;}else{$CARRY_pick=7;}var $SSA_result=$CARRY_pick*2;return $SSA_result;}",
        ]{
            let (code,count)=super::eliminate_tail_joins(source,&Default::default()).unwrap();
            assert_eq!(count,0,"{code}");
            assert_eq!(code,source);
        }
    }

    #[test]
    fn return_expressions_consume_the_join_before_any_other_evaluation() {
        for tail in [
            "result ? 1 : other()",
            "!result",
            "result.value",
            "(result)()",
        ] {
            let source = format!("function f(flag){{let result;if(flag){{result=left.value;}}else{{result=right.value;}}return {tail};}}");
            let (code, count) = super::eliminate_tail_joins(&source, &Default::default()).unwrap();
            assert_eq!(count, 1, "{code}");
            assert!(!code.contains("result="), "{code}");
            if tail == "(result)()" {
                assert!(code.contains("(0, (left.value))"), "{code}");
            }
        }
    }

    #[test]
    fn return_expressions_retain_scope_effect_and_snapshot_boundaries() {
        for source in [
            "function f(flag){let result;if(flag){let other=1;result=left();}else{result=right();}return result+other;}",
            "function f(flag){let result;if(flag){result=left();}else{result=right();}return effect()+result;}",
            "function f(flag){let result;if(flag){result=left();}else{result=right();}return result+result;}",
            "function f(flag){let result;if(flag){result=left();}else{result=right();}return result?()=>other:1;}",
            "function f(flag){let result;if(flag){result=left();}else{result=right();}return result;let read=()=>result;}",
        ] {
            let (code, count) = super::eliminate_tail_joins(source, &Default::default()).unwrap();
            assert_eq!(count, 0, "{code}");
            assert_eq!(code, source);
        }
    }

    #[test]
    fn resource_scope_exit_keeps_the_handoff_outside_the_scope() {
        let source="function f(){let result;let destination;if(flag){using resource=acquire();result=1;}else{result=2;}destination=result;return destination;}";
        let (code, count) = super::eliminate_tail_joins(source, &Default::default()).unwrap();
        assert_eq!(count, 0);
        assert_eq!(code, source);
    }
}
