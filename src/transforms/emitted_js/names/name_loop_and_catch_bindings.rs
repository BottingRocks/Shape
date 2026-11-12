//! Give generated loop counters and catch parameters readable names.
//!
//! Linked output names every temporary `$PREFIX<identity>_<serial>`, including
//! the binding a `for` loop increments and the parameter a `catch` binds. This
//! pass renames those two positions, in place, to conventional names:
//!
//! - a `for` counter (a generated local written in the header and read there)
//!   becomes `i`, `j`, `k`, then `i_2`, `j_2`, ... per function;
//! - a `catch` parameter becomes `e`, then `e_2`, ... per function.
//!
//! Renaming is by resolved symbol, so every reference moves with the binding;
//! only names starting with `$` are touched, and a candidate name already used
//! anywhere in the declaring function's subtree is skipped. `with`/`eval`
//! disables the pass.

use std::collections::{BTreeMap, BTreeSet};

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::scope::ScopeFlags;

use crate::analysis;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopCatchNamesResult {
    pub code: String,
    pub renamed_loops: usize,
    pub renamed_catches: usize,
}

/// Rename generated `for` counters and `catch` parameters.
pub fn name_loop_and_catch_bindings(source: &str) -> Result<LoopCatchNamesResult, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut collect = Collect::new(&scoping);
    collect.visit_program(&program);
    if collect.dynamic_scope {
        return Ok(unchanged(source));
    }
    let mut edits: Vec<(Span, String)> = Vec::new();
    let mut renamed: BTreeSet<SymbolId> = BTreeSet::new();
    let mut assigned: BTreeMap<Span, BTreeSet<String>> = BTreeMap::new();
    let mut loop_index: BTreeMap<Span, usize> = BTreeMap::new();
    let mut catch_index: BTreeMap<Span, usize> = BTreeMap::new();
    let mut renamed_loops = 0;
    let mut renamed_catches = 0;

    for loop_ in &collect.loops {
        let Some(function) = loop_.function else {
            continue;
        };
        for symbol in loop_.counters(&scoping, &nodes) {
            if renamed.contains(&symbol) || !starts_generated(&scoping, symbol) {
                continue;
            }
            let Some(name) = allocate(
                function,
                &loop_name,
                &mut loop_index,
                &collect.used,
                &mut assigned,
            ) else {
                continue;
            };
            renamed.insert(symbol);
            collect_symbol_edits(&scoping, &nodes, symbol, &name, &mut edits);
            renamed_loops += 1;
        }
    }

    for catch in &collect.catches {
        let Some(symbol) = catch.symbol else {
            continue;
        };
        let Some(function) = catch.function else {
            continue;
        };
        if renamed.contains(&symbol) || !starts_generated(&scoping, symbol) {
            continue;
        }
        let Some(name) = allocate(
            function,
            &catch_name,
            &mut catch_index,
            &collect.used,
            &mut assigned,
        ) else {
            continue;
        };
        renamed.insert(symbol);
        collect_symbol_edits(&scoping, &nodes, symbol, &name, &mut edits);
        renamed_catches += 1;
    }

    if edits.is_empty() {
        return Ok(unchanged(source));
    }
    edits.sort_by(|a, b| a.0.start.cmp(&b.0.start).then(a.0.end.cmp(&b.0.end)));
    edits.dedup_by(|a, b| a.0 == b.0);
    let edit_list = edits
        .into_iter()
        .map(|(span, name)| analysis::Edit::replace(span, name))
        .collect();
    let applied = analysis::apply_edits(source, edit_list);
    if !applied.rejected.is_empty() {
        return Err("loop/catch rename edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|error| format!("loop/catch rename output: {error}"))?;
    Ok(LoopCatchNamesResult {
        code: applied.output,
        renamed_loops,
        renamed_catches,
    })
}

fn unchanged(source: &str) -> LoopCatchNamesResult {
    LoopCatchNamesResult {
        code: source.into(),
        renamed_loops: 0,
        renamed_catches: 0,
    }
}

fn starts_generated(scoping: &Scoping, symbol: SymbolId) -> bool {
    scoping.symbol_name(symbol).starts_with('$')
}

fn collect_symbol_edits(
    scoping: &Scoping,
    nodes: &AstNodes,
    symbol: SymbolId,
    name: &str,
    edits: &mut Vec<(Span, String)>,
) {
    edits.push((scoping.symbol_span(symbol), name.to_owned()));
    for reference in scoping.get_resolved_references(symbol) {
        edits.push((
            nodes.get_node(reference.node_id()).kind().span(),
            name.to_owned(),
        ));
    }
}

fn loop_name(index: usize) -> String {
    let base = ["i", "j", "k"][index % 3];
    match index / 3 {
        0 => base.to_owned(),
        round => format!("{base}_{}", round + 1),
    }
}

fn catch_name(index: usize) -> String {
    match index {
        0 => "e".into(),
        round => format!("e_{}", round + 1),
    }
}

fn allocate(
    function: Span,
    base: &dyn Fn(usize) -> String,
    index: &mut BTreeMap<Span, usize>,
    used: &BTreeMap<Span, BTreeSet<String>>,
    assigned: &mut BTreeMap<Span, BTreeSet<String>>,
) -> Option<String> {
    let taken = used.get(&function)?;
    let assigned = assigned.entry(function).or_default();
    let mut candidate_index = *index.get(&function).unwrap_or(&0);
    loop {
        let candidate = base(candidate_index);
        candidate_index += 1;
        if taken.contains(&candidate) || assigned.contains(&candidate) {
            continue;
        }
        assigned.insert(candidate.clone());
        index.insert(function, candidate_index);
        return Some(candidate);
    }
}

#[derive(Clone)]
struct LoopRecord {
    function: Option<Span>,
    while_like: bool,
    init: Option<Span>,
    test: Option<Span>,
    update: Option<Span>,
    candidates: Vec<SymbolId>,
}

impl LoopRecord {
    fn counters(&self, scoping: &Scoping, nodes: &AstNodes) -> Vec<SymbolId> {
        let mut candidates = self.candidates.clone();
        candidates.sort_unstable();
        candidates.dedup();
        if self.while_like {
            candidates
                .retain(|symbol| self.write_reference_in(scoping, nodes, *symbol, &[self.test]));
            return candidates;
        }
        candidates.retain(|symbol| {
            let declared_in_header = [self.init, self.update].into_iter().flatten().any(|range| {
                let span = scoping.symbol_span(*symbol);
                span.start >= range.start && span.end <= range.end
            });
            let written = declared_in_header
                || self.write_reference_in(scoping, nodes, *symbol, &[self.init, self.update]);
            let read = self.reference_in(scoping, nodes, *symbol, &[self.test, self.update]);
            written && read
        });
        candidates
    }

    fn write_reference_in(
        &self,
        scoping: &Scoping,
        nodes: &AstNodes,
        symbol: SymbolId,
        spans: &[Option<Span>],
    ) -> bool {
        for reference in scoping.get_resolved_references(symbol) {
            if !reference.is_write() {
                continue;
            }
            let span = nodes.get_node(reference.node_id()).kind().span();
            if spans
                .iter()
                .flatten()
                .any(|range| span.start >= range.start && span.end <= range.end)
            {
                return true;
            }
        }
        false
    }

    fn reference_in(
        &self,
        scoping: &Scoping,
        nodes: &AstNodes,
        symbol: SymbolId,
        spans: &[Option<Span>],
    ) -> bool {
        for reference in scoping.get_resolved_references(symbol) {
            let span = nodes.get_node(reference.node_id()).kind().span();
            if spans
                .iter()
                .flatten()
                .any(|range| span.start >= range.start && span.end <= range.end)
            {
                return true;
            }
        }
        false
    }
}

struct CatchRecord {
    function: Option<Span>,
    symbol: Option<SymbolId>,
}

struct ExprSymbols<'s> {
    scoping: &'s Scoping,
    symbols: Vec<SymbolId>,
}

impl<'s, 'a> Visit<'a> for ExprSymbols<'s> {
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) {
            self.symbols.push(symbol);
        }
    }
}

struct Collect<'s> {
    scoping: &'s Scoping,
    functions: Vec<Span>,
    used: BTreeMap<Span, BTreeSet<String>>,
    loops: Vec<LoopRecord>,
    catches: Vec<CatchRecord>,
    dynamic_scope: bool,
}

impl<'s> Collect<'s> {
    fn new(scoping: &'s Scoping) -> Self {
        Self {
            scoping,
            functions: Vec::new(),
            used: BTreeMap::new(),
            loops: Vec::new(),
            catches: Vec::new(),
            dynamic_scope: false,
        }
    }

    fn record(&mut self, name: &str) {
        for function in &self.functions {
            self.used
                .entry(*function)
                .or_default()
                .insert(name.to_owned());
        }
    }

    fn push_function(&mut self, span: Span) {
        self.functions.push(span);
        self.used.entry(span).or_default();
    }

    fn pop_function(&mut self) {
        self.functions.pop();
    }

    fn current_function(&self) -> Option<Span> {
        self.functions.last().copied()
    }

    fn symbols_in(&mut self, expressions: &[Option<&Expression<'_>>]) -> Vec<SymbolId> {
        let mut symbols = Vec::new();
        for expression in expressions.iter().flatten() {
            let mut visitor = ExprSymbols {
                scoping: self.scoping,
                symbols: Vec::new(),
            };
            visitor.visit_expression(expression);
            symbols.extend(visitor.symbols);
        }
        symbols
    }
}

impl<'s, 'a> Visit<'a> for Collect<'s> {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        self.push_function(function.span);
        if let Some(id) = &function.id {
            self.record(id.name.as_str());
        }
        walk::walk_function(self, function, flags);
        self.pop_function();
    }

    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        self.push_function(arrow.span);
        walk::walk_arrow_function_expression(self, arrow);
        self.pop_function();
    }

    fn visit_binding_identifier(&mut self, identifier: &BindingIdentifier<'a>) {
        self.record(identifier.name.as_str());
        walk::walk_binding_identifier(self, identifier);
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        self.record(identifier.name.as_str());
        if identifier.name == "eval" {
            self.dynamic_scope = true;
        }
    }

    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.dynamic_scope = true;
        walk::walk_with_statement(self, statement);
    }

    fn visit_for_statement(&mut self, statement: &ForStatement<'a>) {
        let mut candidates = Vec::new();
        if let Some(ForStatementInit::VariableDeclaration(declaration)) = &statement.init {
            for declarator in &declaration.declarations {
                if let BindingPattern::BindingIdentifier(binding) = &declarator.id {
                    if let Some(symbol) = binding.symbol_id.get() {
                        candidates.push(symbol);
                    }
                }
            }
        }
        let referenced = self.symbols_in(&[statement.test.as_ref(), statement.update.as_ref()]);
        candidates.extend(referenced);
        self.loops.push(LoopRecord {
            function: self.current_function(),
            while_like: false,
            init: statement.init.as_ref().map(GetSpan::span),
            test: statement.test.as_ref().map(GetSpan::span),
            update: statement.update.as_ref().map(GetSpan::span),
            candidates,
        });
        walk::walk_for_statement(self, statement);
    }

    fn visit_while_statement(&mut self, statement: &WhileStatement<'a>) {
        let candidates = self.symbols_in(&[Some(&statement.test)]);
        self.loops.push(LoopRecord {
            function: self.current_function(),
            while_like: true,
            init: None,
            test: Some(statement.test.span()),
            update: None,
            candidates,
        });
        walk::walk_while_statement(self, statement);
    }

    fn visit_do_while_statement(&mut self, statement: &DoWhileStatement<'a>) {
        let candidates = self.symbols_in(&[Some(&statement.test)]);
        self.loops.push(LoopRecord {
            function: self.current_function(),
            while_like: true,
            init: None,
            test: Some(statement.test.span()),
            update: None,
            candidates,
        });
        walk::walk_do_while_statement(self, statement);
    }

    fn visit_catch_clause(&mut self, clause: &CatchClause<'a>) {
        let symbol = clause
            .param
            .as_ref()
            .and_then(|parameter| match &parameter.pattern {
                BindingPattern::BindingIdentifier(binding) => binding.symbol_id.get(),
                _ => None,
            });
        self.catches.push(CatchRecord {
            function: self.current_function(),
            symbol,
        });
        walk::walk_catch_clause(self, clause);
    }
}

#[cfg(test)]
mod tests {
    use super::name_loop_and_catch_bindings;

    #[test]
    fn renames_a_for_counter_and_its_references() {
        let source = "function f(n) { for (let $CARRY_1_2 = 0; $CARRY_1_2 < n; $CARRY_1_2++) { use($CARRY_1_2); } }";
        let result = name_loop_and_catch_bindings(source).unwrap();
        assert_eq!(result.renamed_loops, 1);
        assert!(
            result
                .code
                .contains("for (let i = 0; i < n; i++) { use(i); }"),
            "{}",
            result.code
        );
    }

    #[test]
    fn renames_an_outer_declared_counter() {
        let source = "function f(n) { let $CARRY_1_2; for (; $CARRY_1_2 < n; $CARRY_1_2 = $CARRY_1_2 + 1) { use($CARRY_1_2); } }";
        let result = name_loop_and_catch_bindings(source).unwrap();
        assert_eq!(result.renamed_loops, 1);
        assert!(
            result.code.contains("let i; for (; i < n; i = i + 1)"),
            "{}",
            result.code
        );
    }

    #[test]
    fn names_nested_and_sibling_counters_distinctly() {
        let source = "function f(n) { for (let $CARRY_1 = 0; $CARRY_1 < n; $CARRY_1++) { for (let $CARRY_2 = 0; $CARRY_2 < n; $CARRY_2++) {} } for (let $CARRY_3 = 0; $CARRY_3 < n; $CARRY_3++) {} }";
        let result = name_loop_and_catch_bindings(source).unwrap();
        assert_eq!(result.renamed_loops, 3);
        assert!(result.code.contains("for (let i = 0; i < n; i++)"));
        assert!(result.code.contains("for (let j = 0; j < n; j++)"));
        assert!(result.code.contains("for (let k = 0; k < n; k++)"));
    }

    #[test]
    fn renames_a_do_while_test_update() {
        let source = "function f() { let $CARRY_4_2 = 0; do { use($CARRY_4_2); } while (ok() && ($CARRY_4_2 = next(), true)); }";
        let result = name_loop_and_catch_bindings(source).unwrap();
        assert_eq!(result.renamed_loops, 1);
        assert!(
            result.code.contains("(i = next(), true)"),
            "{}",
            result.code
        );
    }

    #[test]
    fn renames_a_catch_parameter() {
        let source = "function f() { try { g(); } catch ($SSA_2_1) { use($SSA_2_1); } }";
        let result = name_loop_and_catch_bindings(source).unwrap();
        assert_eq!(result.renamed_catches, 1);
        assert!(
            result.code.contains("catch (e) { use(e); }"),
            "{}",
            result.code
        );
    }

    #[test]
    fn skips_a_name_already_used_in_the_function() {
        let source = "function f(n) { let i = 1; for (let $CARRY_1 = 0; $CARRY_1 < n; $CARRY_1++) { use($CARRY_1); } }";
        let result = name_loop_and_catch_bindings(source).unwrap();
        assert_eq!(result.renamed_loops, 1);
        assert!(
            result.code.contains("for (let j = 0; j < n; j++)"),
            "{}",
            result.code
        );
    }

    #[test]
    fn skips_non_counters_non_generated_names_and_dynamic_scope() {
        for source in [
            "function f(flag) { for (; flag; ) { use(flag); } }",
            "function f(n) { for (let idx = 0; idx < n; idx++) { use(idx); } }",
            "function f(n) { for (let $CARRY_1 = 0; $CARRY_1 < n; $CARRY_1++) { eval('x'); } }",
        ] {
            let result = name_loop_and_catch_bindings(source).unwrap();
            assert_eq!(result.renamed_loops, 0, "{source}");
            assert_eq!(result.renamed_catches, 0, "{source}");
            assert_eq!(result.code, source);
        }
    }
}
