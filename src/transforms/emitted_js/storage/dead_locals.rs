//! Thread-local unused stores and values. Shared cells and captured bindings
//! are pinned; throwing RHS evaluations survive removal of their destination.
use std::collections::{BTreeMap, BTreeSet};

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::{
    operator::{AssignmentOperator, BinaryOperator, UnaryOperator},
    scope::ScopeFlags,
};

use super::super::{index::SpanIndex, purity::expression_is_pure};
use crate::analysis;

pub(crate) fn remove_dead_locals(
    source: &str,
    protected: &BTreeSet<String>,
) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut facts = Facts::default();
    facts.comments = program.comments.iter().map(|comment| comment.span).collect();
    facts.visit_program(&program);
    if facts.dynamic {
        return Ok((source.into(), 0));
    }
    facts.functions.build();
    facts.classes.build();
    for span in &facts.tries {
        if let Some(owner) = facts.functions.enclosing(*span) {
            facts.exception_functions.insert((owner.start, owner.end));
        }
    }
    let mut scan = Scan {
        source,
        protected,
        scoping: &scoping,
        nodes: &nodes,
        facts: &facts,
        edits: Vec::new(),
        dead: BTreeSet::new(),
    };
    scan.dead = dead_value_graph(&program, &scan);
    scan.visit_program(&program);
    scan.edits
        .sort_by_key(|e| (e.start, std::cmp::Reverse(e.end)));
    let mut end = 0;
    scan.edits.retain(|e| {
        if e.start < end {
            return false;
        }
        end = e.end;
        true
    });
    let count = scan.edits.len();
    let applied = analysis::apply_edits(source, scan.edits);
    if !applied.rejected.is_empty() {
        return Err("dead local edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|e| format!("dead locals: {e}: {}", applied.output))?;
    Ok((applied.output, count))
}

#[derive(Default)]
struct Facts {
    functions: SpanIndex,
    classes: SpanIndex,
    parameters: BTreeSet<SymbolId>,
    dynamic: bool,
    has_nondirective_strings: bool,
    expression_bodies: BTreeSet<(u32, u32)>,
    initialized_regions: BTreeMap<SymbolId, Span>,
    tries: Vec<Span>,
    exception_functions: BTreeSet<(u32, u32)>,
    comments: Vec<Span>,
}
impl Facts {
    /// Whether a real comment lies in `span`; a `//` in a string is not one.
    fn commented(&self, span: Span) -> bool {
        let at = self.comments.partition_point(|comment| comment.end <= span.start);
        self.comments.get(at).is_some_and(|comment| comment.start < span.end)
    }
    fn declarations(&mut self, body: &[Statement<'_>], end: u32) {
        for s in body {
            if let Statement::VariableDeclaration(d) = s {
                if d.kind != VariableDeclarationKind::Var {
                    for item in &d.declarations {
                        if let BindingPattern::BindingIdentifier(id) = &item.id {
                            if let Some(symbol) = id.symbol_id.get() {
                                self.initialized_regions
                                    .insert(symbol, Span::new(d.span.end, end));
                            }
                        }
                    }
                }
            }
        }
    }
}
impl<'a> Visit<'a> for Facts {
    fn visit_expression_statement(&mut self, s: &ExpressionStatement<'a>) {
        self.has_nondirective_strings |= matches!(s.expression, Expression::StringLiteral(_));
        walk::walk_expression_statement(self, s);
    }
    fn visit_try_statement(&mut self, s: &TryStatement<'a>) {
        self.tries.push(s.span);
        walk::walk_try_statement(self, s);
    }
    fn visit_function_body(&mut self, b: &FunctionBody<'a>) {
        self.declarations(&b.statements, b.span.end);
        walk::walk_function_body(self, b);
    }
    fn visit_block_statement(&mut self, b: &BlockStatement<'a>) {
        self.declarations(&b.body, b.span.end);
        walk::walk_block_statement(self, b);
    }
    fn visit_function(&mut self, f: &Function<'a>, flags: ScopeFlags) {
        self.functions.push(f.span);
        walk::walk_function(self, f, flags);
    }
    fn visit_arrow_function_expression(&mut self, f: &ArrowFunctionExpression<'a>) {
        self.functions.push(f.span);
        if f.expression {
            self.expression_bodies
                .insert((f.body.span.start, f.body.span.end));
        }
        walk::walk_arrow_function_expression(self, f);
    }
    fn visit_class(&mut self, c: &Class<'a>) {
        self.classes.push(c.span);
        walk::walk_class(self, c);
    }
    fn visit_formal_parameter(&mut self, p: &FormalParameter<'a>) {
        struct Bindings<'s>(&'s mut BTreeSet<SymbolId>);
        impl<'a> Visit<'a> for Bindings<'_> {
            fn visit_binding_identifier(&mut self, id: &BindingIdentifier<'a>) {
                if let Some(s) = id.symbol_id.get() {
                    self.0.insert(s);
                }
            }
        }
        Bindings(&mut self.parameters).visit_binding_pattern(&p.pattern);
        walk::walk_formal_parameter(self, p);
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
    nodes: &'s AstNodes<'a>,
    facts: &'s Facts,
    edits: Vec<analysis::Edit>,
    dead: BTreeSet<SymbolId>,
}
impl Scan<'_, '_> {
    fn eligible(&self, symbol: SymbolId) -> bool {
        !self.protected.contains(self.scoping.symbol_name(symbol))
    }
    fn local(&self, symbol: SymbolId, at: Span) -> bool {
        let owner = self
            .facts
            .functions
            .enclosing(self.scoping.symbol_span(symbol));
        owner.is_some()
            && owner == self.facts.functions.enclosing(at)
            && !self.facts.parameters.contains(&symbol)
            && self.scoping.get_resolved_references(symbol).all(|r| {
                let span = self.nodes.get_node(r.node_id()).kind().span();
                self.facts.functions.enclosing(span) == owner
                    && self.facts.classes.enclosing(span)
                        == self
                            .facts
                            .classes
                            .enclosing(self.scoping.symbol_span(symbol))
            })
    }
    fn initialized(&self, symbol: SymbolId, initialized: &BTreeSet<SymbolId>, at: Span) -> bool {
        initialized.contains(&symbol)
            || self
                .scoping
                .symbol_flags(symbol)
                .is_function_scoped_declaration()
            || self.scoping.symbol_flags(symbol).is_function()
            || self.facts.parameters.contains(&symbol)
            || self
                .facts
                .initialized_regions
                .get(&symbol)
                .is_some_and(|region| {
                    region.start <= at.start
                        && at.end <= region.end
                        && self.facts.functions.enclosing(at)
                            == self
                                .facts
                                .functions
                                .enclosing(self.scoping.symbol_span(symbol))
                        && self.facts.classes.enclosing(at)
                            == self
                                .facts
                                .classes
                                .enclosing(self.scoping.symbol_span(symbol))
                })
    }
    fn harmless(&self, e: &Expression<'_>, initialized: &BTreeSet<SymbolId>) -> bool {
        if expression_is_pure(e) {
            return true;
        }
        match e {
            Expression::Identifier(id) => analysis::identifier_symbol(id, self.scoping)
                .is_some_and(|s| self.initialized(s, initialized, id.span)),
            Expression::ParenthesizedExpression(p) => self.harmless(&p.expression, initialized),
            Expression::UnaryExpression(u)
                if matches!(
                    u.operator,
                    UnaryOperator::LogicalNot | UnaryOperator::Void | UnaryOperator::Typeof
                ) =>
            {
                self.harmless(&u.argument, initialized)
            }
            Expression::BinaryExpression(b)
                if matches!(
                    b.operator,
                    BinaryOperator::StrictEquality | BinaryOperator::StrictInequality
                ) =>
            {
                self.harmless(&b.left, initialized) && self.harmless(&b.right, initialized)
            }
            Expression::LogicalExpression(l) => {
                self.harmless(&l.left, initialized) && self.harmless(&l.right, initialized)
            }
            Expression::ConditionalExpression(c) => {
                self.harmless(&c.test, initialized)
                    && self.harmless(&c.consequent, initialized)
                    && self.harmless(&c.alternate, initialized)
            }
            Expression::SequenceExpression(s) => {
                s.expressions.iter().all(|e| self.harmless(e, initialized))
            }
            _ => false,
        }
    }
    fn discard(
        &mut self,
        statement: Span,
        value: Option<&Expression<'_>>,
        initialized: &BTreeSet<SymbolId>,
    ) {
        if self.facts.commented(statement) {
            return;
        }
        let mut replacement = value
            .filter(|v| !self.harmless(v, initialized))
            .map_or_else(String::new, |v| {
                format!(
                    "({});",
                    &self.source[v.span().start as usize..v.span().end as usize]
                )
            });
        // An empty statement keeps a later ordinary string expression from
        // becoming a directive after earlier statements have been removed.
        if replacement.is_empty() && self.facts.has_nondirective_strings {
            replacement.push(';');
        }
        self.edits
            .push(analysis::Edit::replace(statement, replacement));
    }
    fn body(&mut self, statements: &[Statement<'_>]) {
        let mut initialized = self.dead_stores(statements);
        // A later non-throwing store kills an earlier store only within this
        // straight-line list. In a function without handlers, a throw leaves
        // an uncaptured local unobservable. Otherwise unknown evaluations stop
        // the proof, so catch/finally and loop backedges see the old value.
        let mut overwritten = BTreeSet::new();
        for s in statements.iter().rev() {
            let no_handler = self
                .facts
                .functions
                .enclosing(s.span())
                .is_some_and(|owner| {
                    !self
                        .facts
                        .exception_functions
                        .contains(&(owner.start, owner.end))
                });
            let Statement::ExpressionStatement(e) = s else {
                self.passed_declaration(s, no_handler, &mut initialized, &mut overwritten);
                continue;
            };
            self.overwritten_store(s, e, no_handler, &initialized, &mut overwritten);
        }
    }
    /// Discard declarations and stores of cells whose values are dead; the
    /// cells this list declares, in order, count as initialized.
    fn dead_stores(&mut self, statements: &[Statement<'_>]) -> BTreeSet<SymbolId> {
        let mut initialized = BTreeSet::new();
        for s in statements {
            match s {
                Statement::VariableDeclaration(d) => {
                    if d.declarations.len() == 1 {
                        let item = &d.declarations[0];
                        if let BindingPattern::BindingIdentifier(id) = &item.id {
                            if let Some(symbol) = id.symbol_id.get() {
                                if self.eligible(symbol)
                                    && self.local(symbol, s.span())
                                    && self.dead.contains(&symbol)
                                {
                                    self.discard(s.span(), item.init.as_ref(), &initialized);
                                }
                            }
                        }
                    } else if d.declarations.iter().all(|item| {
                        item.init.is_none()
                            && matches!(item.id, BindingPattern::BindingIdentifier(_))
                    }) {
                        self.dead_group(d);
                    }
                    for item in &d.declarations {
                        if let BindingPattern::BindingIdentifier(id) = &item.id {
                            if let Some(symbol) = id.symbol_id.get() {
                                initialized.insert(symbol);
                            }
                        }
                    }
                }
                Statement::ExpressionStatement(e) => {
                    if self.harmless(&e.expression, &initialized) {
                        self.discard(s.span(), None, &initialized);
                        continue;
                    }
                    if let Expression::AssignmentExpression(a) = &e.expression {
                        if a.operator == AssignmentOperator::Assign {
                            if let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left {
                                if let Some(symbol) = analysis::identifier_symbol(id, self.scoping)
                                {
                                    if self.eligible(symbol)
                                        && self.local(symbol, s.span())
                                        && !self.scoping.symbol_flags(symbol).is_const_variable()
                                        && self.initialized(symbol, &initialized, s.span())
                                        && self.dead.contains(&symbol)
                                    {
                                        self.discard(s.span(), Some(&a.right), &initialized);
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        initialized
    }
    /// Drop the dead bindings of an uninitialized declaration group.
    fn dead_group(&mut self, d: &VariableDeclaration<'_>) {
        let keep = d
            .declarations
            .iter()
            .filter(|item| {
                let BindingPattern::BindingIdentifier(id) = &item.id else {
                    return true;
                };
                !id.symbol_id.get().is_some_and(|symbol| {
                    self.eligible(symbol)
                        && self.local(symbol, d.span)
                        && self.dead.contains(&symbol)
                })
            })
            .collect::<Vec<_>>();
        if keep.len() != d.declarations.len() {
            let replacement = if keep.is_empty() {
                String::new()
            } else {
                format!(
                    "{} {};",
                    d.kind.as_str(),
                    keep.iter()
                        .map(|item| &self.source
                            [item.span.start as usize..item.span.end as usize])
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            self.edits
                .push(analysis::Edit::replace(d.span, replacement));
        }
    }
    /// Walking back past a statement other than an expression: a handler-free
    /// declaration ends its bindings' pending stores; anything else ends all.
    fn passed_declaration(
        &mut self,
        s: &Statement<'_>,
        no_handler: bool,
        initialized: &mut BTreeSet<SymbolId>,
        overwritten: &mut BTreeSet<SymbolId>,
    ) {
        if no_handler {
            if let Statement::VariableDeclaration(d) = s {
                // `var slot = constant; slot = call();` has no reader
                // of the first value. Keep the hoisted cell, discard
                // only a harmless initializer. Captures and handlers
                // are excluded by the same local-store proof.
                if d.kind == VariableDeclarationKind::Var && d.declarations.len() == 1 {
                    let item = &d.declarations[0];
                    if let (BindingPattern::BindingIdentifier(id), Some(init)) =
                        (&item.id, &item.init)
                    {
                        if let Some(symbol) = id.symbol_id.get() {
                            if overwritten.contains(&symbol)
                                && self.eligible(symbol)
                                && self.local(symbol, s.span())
                                && self.harmless(init, initialized)
                            {
                                self.edits.push(analysis::Edit::replace(
                                    d.span,
                                    format!("var {};", id.name),
                                ));
                            }
                        }
                    }
                }
                struct Read<'s> {
                    scoping: &'s Scoping,
                    pending: &'s mut BTreeSet<SymbolId>,
                }
                impl<'a> Visit<'a> for Read<'_> {
                    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
                        if let Some(symbol) = analysis::identifier_symbol(id, self.scoping) {
                            self.pending.remove(&symbol);
                        }
                    }
                }
                Read {
                    scoping: self.scoping,
                    pending: overwritten,
                }
                .visit_variable_declaration(d);
                for item in &d.declarations {
                    if let BindingPattern::BindingIdentifier(id) = &item.id {
                        if let Some(symbol) = id.symbol_id.get() {
                            initialized.remove(&symbol);
                            overwritten.remove(&symbol);
                        }
                    }
                }
                return;
            }
        }
        overwritten.clear();
        if let Statement::VariableDeclaration(d) = s {
            for item in &d.declarations {
                if let BindingPattern::BindingIdentifier(id) = &item.id {
                    if let Some(symbol) = id.symbol_id.get() {
                        initialized.remove(&symbol);
                    }
                }
            }
        }
    }
    /// Walking back past an expression statement: discard a store that a
    /// later store overwrites unread, then record this store and its reads.
    fn overwritten_store(
        &mut self,
        s: &Statement<'_>,
        e: &ExpressionStatement<'_>,
        no_handler: bool,
        initialized: &BTreeSet<SymbolId>,
        overwritten: &mut BTreeSet<SymbolId>,
    ) {
        let mut value = &e.expression;
        if let Expression::AssignmentExpression(a) = value {
            if a.operator == AssignmentOperator::Assign {
                if let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left {
                    if let Some(symbol) = analysis::identifier_symbol(id, self.scoping) {
                        if self.eligible(symbol)
                            && self.local(symbol, s.span())
                            && !self.scoping.symbol_flags(symbol).is_const_variable()
                            && self.initialized(symbol, initialized, s.span())
                        {
                            if overwritten.contains(&symbol) {
                                self.discard(s.span(), Some(&a.right), initialized);
                            }
                            if no_handler || self.harmless(&a.right, initialized) {
                                overwritten.insert(symbol);
                            } else {
                                overwritten.clear();
                            }
                            value = &a.right;
                        }
                    }
                }
            }
        }
        if !no_handler && !self.harmless(value, initialized) {
            overwritten.clear();
        }
        struct Reads<'s> {
            scoping: &'s Scoping,
            overwritten: &'s mut BTreeSet<SymbolId>,
        }
        impl<'a> Visit<'a> for Reads<'_> {
            fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
                if let Some(symbol) = analysis::identifier_symbol(id, self.scoping) {
                    self.overwritten.remove(&symbol);
                }
            }
        }
        Reads {
            scoping: self.scoping,
            overwritten,
        }
        .visit_expression(value);
    }
}
impl<'a> Visit<'a> for Scan<'_, '_> {
    fn visit_if_statement(&mut self, s: &IfStatement<'a>) {
        let empty = |s: &Statement<'_>| {
            matches!(s,Statement::BlockStatement(b)
            if b.body.iter().all(|s|matches!(s,Statement::EmptyStatement(_))))
        };
        if empty(&s.consequent) && s.alternate.as_ref().is_none_or(empty) {
            if !self.facts.commented(s.span) {
                let test = &self.source[s.test.span().start as usize..s.test.span().end as usize];
                self.edits
                    .push(analysis::Edit::replace(s.span, format!("({test});")));
            }
        }
        walk::walk_if_statement(self, s);
    }
    fn visit_function_body(&mut self, b: &FunctionBody<'a>) {
        if !self
            .facts
            .expression_bodies
            .contains(&(b.span.start, b.span.end))
        {
            self.body(&b.statements);
        }
        walk::walk_function_body(self, b);
    }
    fn visit_block_statement(&mut self, b: &BlockStatement<'a>) {
        self.body(&b.body);
        walk::walk_block_statement(self, b);
    }
}

/// A private cell is live only when an observable evaluation needs its value.
/// Copy edges do not make one another live: this breaks dead loop/phi cycles.
/// Effectful RHS evaluations remain roots even when their destination is dead.
fn dead_value_graph<'a>(program: &Program<'a>, scan: &Scan<'_, 'a>) -> BTreeSet<SymbolId> {
    struct Refs<'s> {
        scoping: &'s Scoping,
        symbols: BTreeSet<SymbolId>,
    }
    impl<'a> Visit<'a> for Refs<'_> {
        fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
            if let Some(symbol) = analysis::identifier_symbol(id, self.scoping) {
                self.symbols.insert(symbol);
            }
        }
        fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {}
        fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}
    }
    struct Graph<'s, 't, 'a> {
        scan: &'s Scan<'t, 'a>,
        edges: BTreeMap<SymbolId, BTreeSet<SymbolId>>,
        roots: BTreeSet<SymbolId>,
        candidates: BTreeSet<SymbolId>,
        suppressed: bool,
    }
    impl Graph<'_, '_, '_> {
        fn store(&mut self, target: SymbolId, rhs: Option<&Expression<'_>>, can_drop: bool) {
            let mut refs = Refs {
                scoping: self.scan.scoping,
                symbols: BTreeSet::new(),
            };
            if let Some(rhs) = rhs {
                refs.visit_expression(rhs);
            }
            if !can_drop || !self.candidates.contains(&target) {
                self.roots.insert(target);
            }
            if rhs.is_none_or(|rhs| self.scan.harmless(rhs, &BTreeSet::new())) {
                self.edges.entry(target).or_default().extend(refs.symbols);
            } else {
                self.roots.extend(refs.symbols);
            }
            // An anonymous class can observe its inferred binding name from
            // a static initializer before anyone reads the destination cell.
            if let Some(mut rhs) = rhs {
                while let Expression::ParenthesizedExpression(p) = rhs {
                    rhs = &p.expression;
                }
                if matches!(rhs, Expression::ClassExpression(_)) {
                    self.roots.insert(target);
                }
            }
        }
    }
    impl<'a> Visit<'a> for Graph<'_, '_, 'a> {
        fn visit_variable_declarator(&mut self, d: &VariableDeclarator<'a>) {
            let BindingPattern::BindingIdentifier(id) = &d.id else {
                walk::walk_variable_declarator(self, d);
                return;
            };
            let Some(symbol) = id.symbol_id.get() else {
                return;
            };
            self.store(symbol, d.init.as_ref(), true);
            let previous = std::mem::replace(&mut self.suppressed, true);
            walk::walk_variable_declarator(self, d);
            self.suppressed = previous;
        }
        fn visit_expression_statement(&mut self, s: &ExpressionStatement<'a>) {
            if let Expression::AssignmentExpression(a) = &s.expression {
                if let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left {
                    if a.operator == AssignmentOperator::Assign {
                        if let Some(symbol) = analysis::identifier_symbol(id, self.scan.scoping) {
                            // A commented store stays, so its binding must too.
                            let can_drop =
                                !self.scan.scoping.symbol_flags(symbol).is_const_variable()
                                    && !self.scan.facts.commented(s.span)
                                    && self.scan.initialized(symbol, &BTreeSet::new(), s.span);
                            self.store(symbol, Some(&a.right), can_drop);
                            let previous = std::mem::replace(&mut self.suppressed, true);
                            walk::walk_expression_statement(self, s);
                            self.suppressed = previous;
                            return;
                        }
                    }
                }
            }
            walk::walk_expression_statement(self, s);
        }
        fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
            if !self.suppressed {
                if let Some(symbol) = analysis::identifier_symbol(id, self.scan.scoping) {
                    self.roots.insert(symbol);
                }
            }
        }
        fn visit_function(&mut self, f: &Function<'a>, flags: ScopeFlags) {
            let previous = std::mem::replace(&mut self.suppressed, false);
            walk::walk_function(self, f, flags);
            self.suppressed = previous;
        }
        fn visit_arrow_function_expression(&mut self, f: &ArrowFunctionExpression<'a>) {
            let previous = std::mem::replace(&mut self.suppressed, false);
            walk::walk_arrow_function_expression(self, f);
            self.suppressed = previous;
        }
    }
    let candidates = scan
        .scoping
        .symbol_ids()
        .filter(|symbol| {
            scan.scoping.symbol_flags(*symbol).is_variable()
                && scan.eligible(*symbol)
                && scan.local(*symbol, scan.scoping.symbol_span(*symbol))
        })
        .collect();
    let mut graph = Graph {
        scan,
        edges: BTreeMap::new(),
        roots: BTreeSet::new(),
        candidates,
        suppressed: false,
    };
    graph.visit_program(program);
    let mut pending: Vec<_> = graph.roots.iter().copied().collect();
    while let Some(symbol) = pending.pop() {
        if let Some(dependencies) = graph.edges.get(&symbol) {
            for dependency in dependencies {
                if graph.roots.insert(*dependency) {
                    pending.push(*dependency);
                }
            }
        }
    }
    graph.candidates.difference(&graph.roots).copied().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean(source: &str) -> String {
        remove_dead_locals(source, &BTreeSet::new()).unwrap().0
    }

    #[test]
    fn a_url_in_a_store_is_not_a_comment() {
        let cleaned = clean(
            "function t(c, d) { let x; if (c) { x = \"https://a.test\"; } else { x = d; } }",
        );
        assert!(!cleaned.contains('x'), "{cleaned}");
    }

    #[test]
    fn a_commented_store_keeps_its_binding() {
        let cleaned =
            clean("function t(c, d) { let x; if (c) { x = /* kept */ 1; } else { x = d; } }");
        assert!(cleaned.contains("let x;"), "{cleaned}");
        assert!(cleaned.contains("/* kept */"), "{cleaned}");
    }
}
