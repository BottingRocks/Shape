//! Turn a two-arm carry assignment into one immutable branch result.
//!
//! The shape is `let $CARRY_x; if (test) { $CARRY_x = a; } else { ... }` with
//! assignment-only arms (nested diamonds included). Every write must resolve
//! to the carry and the leaf count must match. It becomes
//! `const $MERGE_x = (test) ? (a) : (b);`. A read at or before the replaced span
//! declines because the old code could observe the pre-initialized
//! `undefined`/TDZ value. The generated `$MERGE_` name must be fresh, deleted
//! regions must be comment-free, and test-first/one-arm evaluation is
//! preserved. The pass removes mutable join scaffolding and is run to a
//! bounded fixpoint.
//!
//! Per-thread cleanup retains the original carry name and also accepts
//! expression prefixes in each arm, preserving their order in comma expressions.
//! Captured cells and function/class RHS values decline: their previous value
//! or assignment-inferred name can be observable.

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::{operator::AssignmentOperator, scope::ScopeFlags};

use super::super::index::{SourceNames, SpanIndex};
use crate::analysis;
use crate::transforms::emitted_js::support::indent_before;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BranchValueResult {
    pub code: String,
    pub replaced_carries: usize,
}

/// `let $CARRY_x; if (test) { $CARRY_x = yes; } else { $CARRY_x = no; }`
/// becomes `const $MERGE_x = test ? yes : no`, with later resolved reads
/// renamed. Both arms must consist solely of one assignment to the carry.
pub(crate) fn fold_branch_values(source: &str) -> Result<BranchValueResult, String> {
    fold_values(source, false)
}

pub(crate) fn fold_thread_branch_values(source: &str) -> Result<BranchValueResult, String> {
    fold_values(source, true)
}

/// Normalize assignment diamonds independently of where the destination was
/// declared. A lexical LHS reference can be formed before the test without
/// reading or writing its value; PutValue still follows the chosen RHS.
pub(crate) fn fold_branch_assignments(source: &str) -> Result<(String, usize), String> {
    fold_assignments(source, true)
}

/// Like `fold_branch_assignments`, but only for arms that hold nothing but
/// the store: other statements would become comma prefixes.
pub(crate) fn fold_plain_branch_assignments(source: &str) -> Result<(String, usize), String> {
    fold_assignments(source, false)
}

fn fold_assignments(source: &str, prefixes: bool) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, _) = analysis::analyze(&program);
    struct Assignments<'s> {
        source: &'s str,
        scoping: &'s Scoping,
        edits: Vec<analysis::Edit>,
        dynamic: bool,
        prefixes: bool,
    }
    impl<'a> Visit<'a> for Assignments<'_> {
        fn visit_if_statement(&mut self, branch: &IfStatement<'a>) {
            fn terminal<'s, 'a>(mut s: &'s Statement<'a>) -> Option<&'s IdentifierReference<'a>> {
                loop {
                    match s {
                        Statement::BlockStatement(b) => {
                            s = b.body.last()?;
                        }
                        Statement::IfStatement(i) => s = &i.consequent,
                        Statement::ExpressionStatement(s) => {
                            let Expression::AssignmentExpression(a) = &s.expression else {
                                return None;
                            };
                            let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else {
                                return None;
                            };
                            return Some(id);
                        }
                        _ => return None,
                    }
                }
            }
            let candidate = terminal(&branch.consequent).and_then(|id| {
                let symbol = analysis::identifier_symbol(id, self.scoping)?;
                // Function/class declarations, imports and globals can have
                // other reference semantics; only lexical variable cells.
                if !self
                    .scoping
                    .symbol_flags(symbol)
                    .contains(oxc_syntax::symbol::SymbolFlags::BlockScopedVariable)
                {
                    return None;
                }
                let yes =
                    assigned_value(&branch.consequent, symbol, self.scoping, self.source, self.prefixes)?;
                let no = assigned_value(
                    branch.alternate.as_ref()?,
                    symbol,
                    self.scoping,
                    self.source,
                    self.prefixes,
                )?;
                let text = |span: Span| &self.source[span.start as usize..span.end as usize];
                let original = text(branch.span);
                if original.contains("/*") || original.contains("//") {
                    return None;
                }
                Some(analysis::Edit::replace(
                    branch.span,
                    format!(
                        "{} = ({}) ? ({}) : ({});",
                        text(id.span),
                        text(branch.test.span()),
                        yes.0,
                        no.0
                    ),
                ))
            });
            if let Some(edit) = candidate {
                self.edits.push(edit);
            }
            walk::walk_if_statement(self, branch);
        }
        fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
            self.dynamic |= id.name == "eval";
        }
        fn visit_with_statement(&mut self, s: &WithStatement<'a>) {
            self.dynamic = true;
            walk::walk_with_statement(self, s);
        }
    }
    let mut scan = Assignments {
        source,
        scoping: &scoping,
        edits: vec![],
        dynamic: false,
        prefixes,
    };
    scan.visit_program(&program);
    if scan.dynamic {
        return Ok((source.into(), 0));
    }
    // A normalized outer diamond includes its nested diamonds. Inner rewrites
    // can run next round without overlapping replacements in this one.
    scan.edits
        .sort_by_key(|e| (e.start, std::cmp::Reverse(e.end)));
    let mut end = 0;
    scan.edits.retain(|e| {
        if e.start < end {
            false
        } else {
            end = e.end;
            true
        }
    });
    let count = scan.edits.len();
    let applied = analysis::apply_edits(source, scan.edits);
    if !applied.rejected.is_empty() {
        return Err("branch assignment edits overlap".into());
    }
    Ok((applied.output, count))
}

fn fold_values(source: &str, keep_name: bool) -> Result<BranchValueResult, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut owners = Owners::default();
    owners.visit_program(&program);
    owners.0.build();
    let mut scan = Scan {
        source,
        scoping: &scoping,
        nodes: &nodes,
        edits: Vec::new(),
        dynamic_scope: false,
        count: 0,
        names: SourceNames::new(source, "$MERGE_"),
        keep_name,
        owners: &owners.0,
    };
    scan.visit_program(&program);
    if scan.dynamic_scope || scan.edits.is_empty() {
        return Ok(BranchValueResult {
            code: source.into(),
            replaced_carries: 0,
        });
    }
    let applied = analysis::apply_edits(source, scan.edits);
    if !applied.rejected.is_empty() {
        return Err(format!(
            "branch value edits overlap: {}",
            analysis::describe_rejected_edits(&applied.rejected)
        ));
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|error| format!("branch value output: {error}"))?;
    Ok(BranchValueResult {
        code: applied.output,
        replaced_carries: scan.count,
    })
}

struct Scan<'s, 'a> {
    owners: &'s SpanIndex,
    keep_name: bool,
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    edits: Vec<analysis::Edit>,
    dynamic_scope: bool,
    count: usize,
    names: SourceNames<'s>,
}
impl Scan<'_, '_> {
    fn body(&mut self, statements: &[Statement<'_>]) {
        for pair in statements.windows(2) {
            let Statement::VariableDeclaration(declaration) = &pair[0] else {
                continue;
            };
            if declaration.kind != VariableDeclarationKind::Let
                || declaration.declarations.len() != 1
            {
                continue;
            }
            let declarator = &declaration.declarations[0];
            let BindingPattern::BindingIdentifier(binding) = &declarator.id else {
                continue;
            };
            if !binding.name.starts_with("$CARRY_") || declarator.init.is_some() {
                continue;
            }
            let Some(symbol) = binding.symbol_id.get() else {
                continue;
            };
            let Statement::IfStatement(branch) = &pair[1] else {
                continue;
            };
            let (Some(yes), Some(no)) = (
                assigned_value(
                    &branch.consequent,
                    symbol,
                    self.scoping,
                    self.source,
                    self.keep_name,
                ),
                branch.alternate.as_ref().and_then(|arm| {
                    assigned_value(arm, symbol, self.scoping, self.source, self.keep_name)
                }),
            ) else {
                continue;
            };
            let replacement_span = Span::new(declaration.span.start, branch.span.end);
            let old = &self.source[replacement_span.start as usize..replacement_span.end as usize];
            if old.contains("/*") || old.contains("//") {
                continue;
            }
            let new_name = if self.keep_name {
                binding.name.to_string()
            } else {
                format!("$MERGE_{}", binding.name.strip_prefix("$CARRY_").unwrap())
            };
            if !self.keep_name && self.names.contains(&new_name) {
                continue;
            }
            let mut refs = Vec::new();
            let mut writes = 0;
            let mut valid = true;
            for reference in self.scoping.get_resolved_references(symbol) {
                let span = self.nodes.get_node(reference.node_id()).kind().span();
                if self.owners.enclosing(span) != self.owners.enclosing(declaration.span) {
                    valid = false;
                    break;
                }
                if reference.is_write() {
                    writes += 1;
                    if span.start < branch.span.start || span.end > branch.span.end {
                        valid = false;
                        break;
                    }
                } else if span.start < replacement_span.end {
                    // An arm that reads the old `undefined` value cannot be
                    // converted to a const initializer without changing TDZ.
                    valid = false;
                    break;
                } else {
                    refs.push(span);
                }
            }
            if !valid || writes != yes.1 + no.1 {
                continue;
            }
            let text = |span: Span| &self.source[span.start as usize..span.end as usize];
            let replacement = format!(
                "const {new_name} = ({}) ? ({}) : ({});",
                text(branch.test.span()),
                yes.0,
                no.0,
            );
            let mut proposal = vec![analysis::Edit::replace(replacement_span, replacement)];
            for read in refs {
                if !self.keep_name {
                    proposal.push(analysis::Edit::replace(read, new_name.clone()));
                }
            }
            if proposal.iter().any(|edit| {
                self.edits
                    .iter()
                    .any(|other| edit.start < other.end && other.start < edit.end)
            }) {
                continue;
            }
            self.edits.extend(proposal);
            self.count += 1;
        }
    }
}
fn assigned_value(
    arm: &Statement<'_>,
    symbol: SymbolId,
    scoping: &Scoping,
    source: &str,
    prefixes: bool,
) -> Option<(String, usize)> {
    let Statement::BlockStatement(block) = arm else {
        return None;
    };
    let (last, before) = block.body.split_last()?;
    if !prefixes && !before.is_empty() {
        return None;
    }
    let mut sequence = vec![];
    for s in before {
        match s {
            Statement::EmptyStatement(_) => {}
            Statement::ExpressionStatement(s) => {
                let span = s.expression.span();
                sequence.push(format!(
                    "({})",
                    &source[span.start as usize..span.end as usize]
                ));
            }
            _ => return None,
        }
    }
    let (value, writes) = match last {
        Statement::ExpressionStatement(statement) => {
            let Expression::AssignmentExpression(assignment) = &statement.expression else {
                return None;
            };
            if assignment.operator != AssignmentOperator::Assign {
                return None;
            }
            let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
                return None;
            };
            let mut rhs = &assignment.right;
            while let Expression::ParenthesizedExpression(p) = rhs {
                rhs = &p.expression;
            }
            if matches!(
                rhs,
                Expression::FunctionExpression(_)
                    | Expression::ArrowFunctionExpression(_)
                    | Expression::ClassExpression(_)
            ) {
                return None;
            }
            (analysis::identifier_symbol(target, scoping) == Some(symbol)).then(|| {
                let rhs = assignment.right.span();
                (source[rhs.start as usize..rhs.end as usize].to_string(), 1)
            })
        }
        Statement::IfStatement(nested) => {
            let yes = assigned_value(&nested.consequent, symbol, scoping, source, prefixes)?;
            let no = assigned_value(
                nested.alternate.as_ref()?,
                symbol,
                scoping,
                source,
                prefixes,
            )?;
            let test = nested.test.span();
            Some((
                format!(
                    "({}) ? ({}) : ({})",
                    &source[test.start as usize..test.end as usize],
                    yes.0,
                    no.0
                ),
                yes.1 + no.1,
            ))
        }
        _ => None,
    }?;
    if sequence.is_empty() {
        Some((value, writes))
    } else {
        sequence.push(format!("({value})"));
        Some((format!("({})", sequence.join(", ")), writes))
    }
}

#[derive(Default)]
struct Owners(SpanIndex);
impl<'a> Visit<'a> for Owners {
    fn visit_function(&mut self, f: &Function<'a>, flags: ScopeFlags) {
        self.0.push(f.span);
        walk::walk_function(self, f, flags);
    }
    fn visit_arrow_function_expression(&mut self, f: &ArrowFunctionExpression<'a>) {
        self.0.push(f.span);
        walk::walk_arrow_function_expression(self, f);
    }
    fn visit_class(&mut self, c: &Class<'a>) {
        self.0.push(c.span);
        walk::walk_class(self, c);
    }
}
impl<'a> Visit<'a> for Scan<'_, 'a> {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        if let Some(body) = &function.body {
            self.body(&body.statements);
        }
        walk::walk_function(self, function, flags);
    }
    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        self.body(&block.body);
        walk::walk_block_statement(self, block);
    }
    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.dynamic_scope = true;
        walk::walk_with_statement(self, statement);
    }
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if identifier.name == "eval" {
            self.dynamic_scope = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fold_branch_values;
    #[test]
    fn assignment_normalization_preserves_lexical_stores_with_old_value_reads() {
        let source = "function f(flag){let result=0;effect();if(flag){observe(result);result=next();}else{result=result+1;}return result;}";
        let (code, count) = super::fold_branch_assignments(source).unwrap();
        assert_eq!(count, 1);
        assert!(code.contains("result = (flag) ?"), "{code}");
        assert!(code.contains("(observe(result)), (next())"), "{code}");
        assert!(code.contains("(result+1)"), "{code}");
    }

    #[test]
    fn assignment_normalization_rejects_globals_local_declarations_names_and_eval() {
        for source in [
            "function f(flag){if(flag){outside=1;}else{outside=2;}}",
            "function f(flag){let result;if(flag){let local=read();result=local;}else{result=2;}}",
            "function f(flag){let result;if(flag){result=()=>{};}else{result=function(){};}return result.name;}",
            "function f(flag){let result;if(flag){eval('result=4');result=1;}else{result=2;}return result;}",
            "function f(flag){let result;if(flag){return;result=1;}else{result=2;}return result;}",
        ] {
            let (code, count) = super::fold_branch_assignments(source).unwrap();
            assert_eq!(count, 0, "{source}");
            assert_eq!(code, source);
        }
    }
    #[test]
    fn folds_effectful_arms_without_deferring_evaluation() {
        let source = "function f(p) { let $CARRY_1; if (p()) { $CARRY_1 = yes(); } else { $CARRY_1 = no(); } observe(); return $CARRY_1; }";
        let result = fold_branch_values(source).unwrap();
        assert_eq!(result.replaced_carries, 1);
        assert!(!result.code.contains("$CARRY_1"), "{}", result.code);
        assert!(
            result
                .code
                .contains("const $MERGE_1 = (p()) ? (yes()) : (no());"),
            "{}",
            result.code
        );
        assert!(result.code.contains("return $MERGE_1"));
    }
    #[test]
    fn folds_nested_assignment_diamond() {
        let source = "function f(a,b) { let $CARRY_1; if (a()) { if (b()) { $CARRY_1 = 1; } else { $CARRY_1 = 2; } } else { $CARRY_1 = 3; } return $CARRY_1; }";
        let result = fold_branch_values(source).unwrap();
        assert_eq!(result.replaced_carries, 1, "{}", result.code);
        assert!(!result.code.contains("$CARRY_1"));
        assert!(result.code.contains("(a()) ? ((b()) ? (1) : (2)) : (3)"));
    }
    #[test]
    fn retains_old_value_read_and_extra_arm_effects() {
        for source in [
            "function f(p) { let $CARRY_1; if (p) { $CARRY_1 = $CARRY_1; } else { $CARRY_1 = 1; } return $CARRY_1; }",
            "function f(p) { let $CARRY_1; if (p) { effect(); $CARRY_1 = 1; } else { $CARRY_1 = 2; } return $CARRY_1; }",
        ] { assert_eq!(fold_branch_values(source).unwrap().replaced_carries, 0); }
    }
}

/// One `let` per carry for an `if` tree that assigns several carries at
/// once:
///
/// ```js
/// let $CARRY_a, $CARRY_b;
/// if (t) { $CARRY_a = x; $CARRY_b = y; } else { $CARRY_a = z; $CARRY_b = w; }
/// // -> let $CARRY_a = t ? x : z; let $CARRY_b = t ? y : w;
/// ```
///
/// The bare declaration must come right before the `if`; every leaf assigns
/// the same carries in the same order and nothing else; an arm is a leaf or
/// exactly one nested tree; the carries are written nowhere else. Each test
/// is then evaluated once per carry, so every nested test must be repeatable
/// — never written names, literals, `!`, `typeof`, `void`, `===`/`!==` and
/// `== null` — and a leaf's values keep their order (carry declarations
/// follow it), so effects in the values are preserved. A top test that is not
/// repeatable always runs first anyway: it is evaluated once into a fresh
/// `$SSA_t…` temporary that the carries test instead.
pub(crate) fn fold_multi_branch_values(source: &str) -> Result<BranchValueResult, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut scan = MultiScan {
        source,
        scoping: &scoping,
        nodes: &nodes,
        edits: Vec::new(),
        folded: 0,
        dynamic: false,
        taken_tests: std::cell::RefCell::new(super::super::inline_tuple_helpers::taken_numbers(source, "$SSA_t")),
    };
    scan.visit_program(&program);
    if scan.dynamic || scan.edits.is_empty() {
        return Ok(BranchValueResult {
            code: source.into(),
            replaced_carries: 0,
        });
    }
    let applied = analysis::apply_edits(source, scan.edits);
    if !applied.rejected.is_empty() {
        return Err("multi-carry branch edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|error| format!("multi-carry branch output: {error}"))?;
    Ok(BranchValueResult {
        code: applied.output,
        replaced_carries: scan.folded,
    })
}

/// A carry tree: a leaf's values in carry order, or a test with two arms.
enum CarryTree {
    Leaf(Vec<Span>),
    Branch(Span, Box<CarryTree>, Box<CarryTree>),
}

struct MultiScan<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    edits: Vec<analysis::Edit>,
    folded: usize,
    dynamic: bool,
    /// Numbers `$SSA_t…` names already use, in the source or this pass's
    /// edits (see `taken_numbers`).
    taken_tests: std::cell::RefCell<std::collections::BTreeSet<String>>,
}

impl MultiScan<'_, '_> {
    fn list(&mut self, statements: &[Statement<'_>]) {
        for pair in statements.windows(2) {
            let (Statement::VariableDeclaration(declaration), Statement::IfStatement(branch)) = (&pair[0], &pair[1])
            else {
                continue;
            };
            if declaration.kind != VariableDeclarationKind::Let || declaration.declarations.len() < 2 {
                continue;
            }
            let mut carries = Vec::new();
            for declarator in &declaration.declarations {
                let (BindingPattern::BindingIdentifier(id), None) = (&declarator.id, &declarator.init) else {
                    carries.clear();
                    break;
                };
                let Some(symbol) = id.symbol_id.get() else {
                    carries.clear();
                    break;
                };
                carries.push((symbol, id.span));
            }
            if carries.is_empty() {
                continue;
            }
            let symbols: Vec<SymbolId> = carries.iter().map(|(symbol, _)| *symbol).collect();
            // The top test runs exactly once, first: a non-repeatable one is
            // held in a temporary.
            let top_repeatable = self.repeatable(&branch.test);
            let Some((tree, order)) = self.tree_with(branch, &symbols, true) else {
                continue;
            };
            // Every write is one of the tree's leaf assignments.
            let leaves = count_leaves(&tree);
            if symbols
                .iter()
                .any(|symbol| self.scoping.get_resolved_references(*symbol).filter(|r| r.is_write()).count() != leaves)
            {
                continue;
            }
            // A read of a carry inside the tree would read a `let` before its
            // initializer once split.
            if symbols.iter().any(|symbol| {
                self.scoping.get_resolved_references(*symbol).any(|reference| {
                    reference.is_read() && {
                        let at = self.reference_start(reference.node_id());
                        branch.span.start <= at && at < branch.span.end
                    }
                })
            }) {
                continue;
            }
            let span = Span::new(declaration.span.start, branch.span.end);
            let text = &self.source[span.start as usize..span.end as usize];
            if text.contains("//") || text.contains("/*") {
                continue;
            }
            let indent = indent_before(self.source, declaration.span.start);
            let mut lines: Vec<String> = Vec::new();
            let test_name = (!top_repeatable).then(|| self.fresh_test_name());
            if let Some(name) = &test_name {
                lines.push(format!("let {name} = {};", self.source_text(branch.test.span())));
            }
            lines.extend(order.iter().map(|index| {
                let (_, name_span) = carries[*index];
                let name = &self.source[name_span.start as usize..name_span.end as usize];
                format!("let {name} = {};", self.render_top(&tree, *index, test_name.as_deref()))
            }));
            self.edits.push(analysis::Edit::replace(span, lines.join(&format!("\n{indent}"))));
            self.folded += carries.len();
        }
    }

    /// The tree under `branch`, and the carry order every leaf shares.
    fn tree(&self, branch: &IfStatement<'_>, carries: &[SymbolId]) -> Option<(CarryTree, Vec<usize>)> {
        self.tree_with(branch, carries, false)
    }

    fn tree_with(&self, branch: &IfStatement<'_>, carries: &[SymbolId], top: bool) -> Option<(CarryTree, Vec<usize>)> {
        if !top && !self.repeatable(&branch.test) {
            return None;
        }
        let (yes, yes_order) = self.arm(&branch.consequent, carries)?;
        let (no, no_order) = self.arm(branch.alternate.as_ref()?, carries)?;
        (yes_order == no_order).then(|| {
            (
                CarryTree::Branch(branch.test.span(), Box::new(yes), Box::new(no)),
                yes_order,
            )
        })
    }

    fn arm(&self, arm: &Statement<'_>, carries: &[SymbolId]) -> Option<(CarryTree, Vec<usize>)> {
        let statements: &[Statement] = match arm {
            Statement::BlockStatement(block) => &block.body,
            other => std::slice::from_ref(other),
        };
        if let [Statement::IfStatement(nested)] = statements {
            return self.tree(nested, carries);
        }
        let mut values = vec![None; carries.len()];
        let mut order = Vec::new();
        for statement in statements {
            let Statement::ExpressionStatement(expression) = statement else {
                return None;
            };
            let Expression::AssignmentExpression(assignment) = &expression.expression else {
                return None;
            };
            let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
                return None;
            };
            if assignment.operator != AssignmentOperator::Assign {
                return None;
            }
            let symbol = analysis::identifier_symbol(target, self.scoping)?;
            let index = carries.iter().position(|carry| *carry == symbol)?;
            if values[index].is_some() {
                return None;
            }
            values[index] = Some(assignment.right.span());
            order.push(index);
        }
        let values: Option<Vec<Span>> = values.into_iter().collect();
        Some((CarryTree::Leaf(values?), order))
    }

    fn reference_start(&self, node: oxc_semantic::NodeId) -> u32 {
        self.nodes.kind(node).span().start
    }

    fn source_text(&self, span: Span) -> &str {
        &self.source[span.start as usize..span.end as usize]
    }

    /// A `$SSA_t…` name that occurs nowhere in the source or earlier edits.
    fn fresh_test_name(&self) -> String {
        let mut n = self.edits.len() + 1;
        let mut taken = self.taken_tests.borrow_mut();
        loop {
            if !taken.contains(&n.to_string()) {
                // The name goes into this pass's next edit.
                super::super::inline_tuple_helpers::take_number(&mut taken, n);
                return format!("$SSA_t{n}");
            }
            n += 1;
        }
    }

    /// `render` with the top test read from its temporary.
    fn render_top(&self, tree: &CarryTree, index: usize, test: Option<&str>) -> String {
        match (tree, test) {
            (CarryTree::Branch(_, yes, no), Some(name)) => {
                format!("{name} ? {} : {}", self.render(yes, index), self.render(no, index))
            }
            _ => self.render(tree, index),
        }
    }

    fn render(&self, tree: &CarryTree, index: usize) -> String {
        let text = |span: &Span| &self.source[span.start as usize..span.end as usize];
        match tree {
            CarryTree::Leaf(values) => format!("({})", text(&values[index])),
            CarryTree::Branch(test, yes, no) => {
                format!("({}) ? {} : {}", text(test), self.render(yes, index), self.render(no, index))
            }
        }
    }

    fn repeatable(&self, expression: &Expression<'_>) -> bool {
        use oxc_syntax::operator::{BinaryOperator, UnaryOperator};
        match expression {
            Expression::NumericLiteral(_)
            | Expression::StringLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_) => true,
            Expression::Identifier(identifier) => analysis::identifier_symbol(identifier, self.scoping)
                .is_some_and(|symbol| !self.scoping.symbol_is_mutated(symbol)),
            Expression::UnaryExpression(unary) => {
                matches!(unary.operator, UnaryOperator::LogicalNot | UnaryOperator::Typeof | UnaryOperator::Void)
                    && self.repeatable(&unary.argument)
            }
            Expression::BinaryExpression(binary) => match binary.operator {
                BinaryOperator::StrictEquality | BinaryOperator::StrictInequality => {
                    self.repeatable(&binary.left) && self.repeatable(&binary.right)
                }
                BinaryOperator::Equality | BinaryOperator::Inequality => {
                    (matches!(binary.left, Expression::NullLiteral(_))
                        || matches!(binary.right, Expression::NullLiteral(_)))
                        && self.repeatable(&binary.left)
                        && self.repeatable(&binary.right)
                }
                _ => false,
            },
            _ => false,
        }
    }
}

fn count_leaves(tree: &CarryTree) -> usize {
    match tree {
        CarryTree::Leaf(_) => 1,
        CarryTree::Branch(_, yes, no) => count_leaves(yes) + count_leaves(no),
    }
}

impl<'a> Visit<'a> for MultiScan<'_, '_> {
    fn visit_function_body(&mut self, body: &FunctionBody<'a>) {
        self.list(&body.statements);
        walk::walk_function_body(self, body);
    }
    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        self.list(&block.body);
        walk::walk_block_statement(self, block);
    }
    fn visit_switch_case(&mut self, case: &SwitchCase<'a>) {
        self.list(&case.consequent);
        walk::walk_switch_case(self, case);
    }
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        self.dynamic |= id.name == "eval";
    }
    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.dynamic = true;
        walk::walk_with_statement(self, statement);
    }
}

#[cfg(test)]
mod multi_tests {
    use super::fold_multi_branch_values;

    #[test]
    fn folds_a_tree_that_assigns_several_carries() {
        let source = "function f(t, u, x) {\n  let $CARRY_a, $CARRY_b;\n  if (t) {\n    if (u === null) {\n      $CARRY_a = x.p;\n      $CARRY_b = 1;\n    } else {\n      $CARRY_a = 2;\n      $CARRY_b = x.q;\n    }\n  } else {\n    $CARRY_a = 3;\n    $CARRY_b = 4;\n  }\n  return [$CARRY_a, $CARRY_b];\n}";
        let result = fold_multi_branch_values(source).unwrap();
        assert_eq!(result.replaced_carries, 2, "{}", result.code);
        assert!(result.code.contains("let $CARRY_a = (t) ? (u === null) ? (x.p) : (2) : (3);"), "{}", result.code);
        assert!(result.code.contains("let $CARRY_b = (t) ? (u === null) ? (1) : (x.q) : (4);"), "{}", result.code);
    }

    #[test]
    fn keeps_trees_it_cannot_split() {
        // a top test that is not repeatable runs once into a temporary
        let top = "function f(t, x) {\n  let $CARRY_a, $CARRY_b;\n  if (t()) {\n    $CARRY_a = 1;\n    $CARRY_b = 2;\n  } else {\n    $CARRY_a = 3;\n    $CARRY_b = 4;\n  }\n}";
        let result = fold_multi_branch_values(top).unwrap();
        assert!(result.code.contains("let $SSA_t1 = t();\n  let $CARRY_a = $SSA_t1 ? (1) : (3);"), "{}", result.code);
        for source in [
            // a nested test that is not repeatable
            "function f(t, x) {\n  let $CARRY_a, $CARRY_b;\n  if (t) {\n    if (x()) {\n      $CARRY_a = 1;\n      $CARRY_b = 2;\n    } else {\n      $CARRY_a = 5;\n      $CARRY_b = 6;\n    }\n  } else {\n    $CARRY_a = 3;\n    $CARRY_b = 4;\n  }\n}",
            // the arms assign in different orders
            "function f(t, x) {\n  let $CARRY_a, $CARRY_b;\n  if (t) {\n    $CARRY_a = x.p;\n    $CARRY_b = x.q;\n  } else {\n    $CARRY_b = x.r;\n    $CARRY_a = x.s;\n  }\n}",
            // a leaf misses a carry
            "function f(t) {\n  let $CARRY_a, $CARRY_b;\n  if (t) {\n    $CARRY_a = 1;\n  } else {\n    $CARRY_a = 3;\n    $CARRY_b = 4;\n  }\n}",
            // a value reads a carry of the tree
            "function f(t) {\n  let $CARRY_a, $CARRY_b;\n  if (t) {\n    $CARRY_a = 1;\n    $CARRY_b = $CARRY_b;\n  } else {\n    $CARRY_a = 3;\n    $CARRY_b = 4;\n  }\n}",
            // a carry is written elsewhere too
            "function f(t) {\n  let $CARRY_a, $CARRY_b;\n  if (t) {\n    $CARRY_a = 1;\n    $CARRY_b = 2;\n  } else {\n    $CARRY_a = 3;\n    $CARRY_b = 4;\n  }\n  $CARRY_a = 5;\n}",
        ] {
            let result = fold_multi_branch_values(source).unwrap();
            assert_eq!(result.replaced_carries, 0, "{source} -> {}", result.code);
        }
    }
}
