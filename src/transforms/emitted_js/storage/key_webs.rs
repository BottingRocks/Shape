//! A slot the VM reuses for an unrelated value inside one block gets its
//! own binding there: `let $KEY_n` at the top of that block.
//!
//! The VM's register allocator puts unrelated values in one slot when their
//! lifetimes do not overlap (an interleaved signal body writes `$KEY_35`
//! after the Intl counts it held are hashed). One `$KEY_35` variable for
//! both makes every read of the later value look like it may see the
//! earlier one. When a block `B` of the slot's function
//!
//! - writes the slot before any read in it can see the value it entered
//!   with (each path through `B` writes first),
//! - and nothing outside `B` reads or writes the slot after `B` runs: every
//!   other use is earlier in the source and none shares a loop with `B`,
//!
//! the uses in `B` are one value of their own, and a `let` of the same name
//! at the top of `B` makes them so without renaming the slot. No use may be
//! in a nested function: a created thread could run at any time.

use std::collections::{BTreeMap, BTreeSet};

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::scope::ScopeFlags;

use crate::analysis;

pub(crate) fn split_block_key_webs(source: &str) -> Result<(String, usize), String> {
    if !source.contains("$KEY_") {
        return Ok((source.into(), 0));
    }
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut dynamic = Dynamic(false);
    dynamic.visit_program(&program);
    if dynamic.0 {
        return Ok((source.into(), 0));
    }
    let slots = slots(&scoping, &nodes);
    if slots.is_empty() {
        return Ok((source.into(), 0));
    }
    let mut chosen: BTreeMap<SymbolId, Vec<Span>> = BTreeMap::new();
    for _ in 0..16 {
        let mut scan = Scan { slots: &slots, chosen: &chosen, loops: Vec::new(), found: Vec::new() };
        scan.visit_program(&program);
        if scan.found.is_empty() {
            break;
        }
        for (symbol, block) in scan.found {
            chosen.entry(symbol).or_default().push(block);
        }
    }
    if chosen.is_empty() {
        return Ok((source.into(), 0));
    }
    // One `let` per block, its names in slot order.
    let mut by_block: BTreeMap<u32, Vec<&str>> = BTreeMap::new();
    let mut count = 0;
    for (symbol, blocks) in &chosen {
        for block in blocks {
            by_block.entry(block.start).or_default().push(scoping.symbol_name(*symbol));
            count += 1;
        }
    }
    let edits = by_block
        .into_iter()
        .map(|(start, names)| {
            // Just inside the block's `{`, on a line of its own (the
            // declaration placement after it moves or drops it whole).
            let at = start + 1;
            analysis::Edit::replace(Span::new(at, at), format!("\nlet {};", names.join(", ")))
        })
        .collect();
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err(format!("key web edits overlap: {}", analysis::describe_rejected_edits(&applied.rejected)));
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output).map_err(|error| format!("key web output: {error}"))?;
    Ok((applied.output, count))
}

/// A slot variable's declarations and uses, all in its own function.
struct Slot {
    declarations: Vec<Span>,
    /// Each use: its span and whether it reads.
    uses: Vec<(Span, bool)>,
}

fn slots(scoping: &Scoping, nodes: &AstNodes<'_>) -> BTreeMap<SymbolId, Slot> {
    let mut slots = BTreeMap::new();
    'symbols: for symbol in scoping.symbol_ids() {
        if !scoping.symbol_name(symbol).starts_with("$KEY_") {
            continue;
        }
        let owner = scoping.symbol_scope_id(symbol);
        let mut uses = Vec::new();
        for reference in scoping.get_resolved_references(symbol) {
            let span = nodes.get_node(reference.node_id()).kind().span();
            // A use in a nested function can run whenever it is called.
            for scope in scoping.scope_ancestors(reference.scope_id()) {
                if scope == owner {
                    break;
                }
                if scoping.scope_flags(scope).intersects(ScopeFlags::Function | ScopeFlags::Arrow) {
                    continue 'symbols;
                }
            }
            uses.push((span, reference.is_read()));
        }
        if uses.is_empty() {
            continue;
        }
        let mut declarations = vec![scoping.symbol_span(symbol)];
        declarations.extend(scoping.symbol_redeclarations(symbol).iter().map(|r| r.span));
        slots.insert(symbol, Slot { declarations, uses });
    }
    slots
}

fn inside(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

struct Scan<'s> {
    slots: &'s BTreeMap<SymbolId, Slot>,
    chosen: &'s BTreeMap<SymbolId, Vec<Span>>,
    /// The loops around the visit, innermost last; a function resets them.
    loops: Vec<Span>,
    found: Vec<(SymbolId, Span)>,
}

impl Scan<'_> {
    fn consider(&mut self, block: &BlockStatement<'_>) {
        let span = block.span;
        for (&symbol, slot) in self.slots {
            let chosen = self.chosen.get(&symbol).map(Vec::as_slice).unwrap_or(&[]);
            if chosen.iter().any(|outer| inside(*outer, span))
                || self.found.iter().any(|(s, outer)| *s == symbol && inside(*outer, span))
            {
                continue;
            }
            if !slot.uses.iter().any(|(use_, _)| inside(span, *use_)) {
                continue;
            }
            if slot.declarations.iter().any(|d| inside(span, *d)) {
                continue;
            }
            let outside: Vec<Span> = slot
                .uses
                .iter()
                .map(|(use_, _)| *use_)
                .filter(|use_| !inside(span, *use_) && !chosen.iter().any(|c| inside(*c, *use_)))
                .collect();
            if outside.is_empty() {
                continue;
            }
            // Every other use runs before the block: earlier, and in no
            // loop the block is in.
            if outside.iter().any(|use_| use_.start >= span.start) {
                continue;
            }
            if self.loops.iter().any(|loop_| outside.iter().any(|use_| inside(*loop_, *use_))) {
                continue;
            }
            let uses: Vec<(Span, bool)> =
                slot.uses.iter().copied().filter(|(use_, _)| inside(span, *use_)).collect();
            let first = WritesFirst { symbol_uses: &uses };
            if first.statements(&block.body, true).is_some() {
                self.found.push((symbol, span));
            }
        }
    }
}

impl<'a> Visit<'a> for Scan<'_> {
    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        self.consider(block);
        walk::walk_block_statement(self, block);
    }
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        let loops = std::mem::take(&mut self.loops);
        walk::walk_function(self, function, flags);
        self.loops = loops;
    }
    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        let loops = std::mem::take(&mut self.loops);
        walk::walk_arrow_function_expression(self, arrow);
        self.loops = loops;
    }
    fn visit_for_statement(&mut self, it: &ForStatement<'a>) {
        self.loops.push(it.span);
        walk::walk_for_statement(self, it);
        self.loops.pop();
    }
    fn visit_for_in_statement(&mut self, it: &ForInStatement<'a>) {
        self.loops.push(it.span);
        walk::walk_for_in_statement(self, it);
        self.loops.pop();
    }
    fn visit_for_of_statement(&mut self, it: &ForOfStatement<'a>) {
        self.loops.push(it.span);
        walk::walk_for_of_statement(self, it);
        self.loops.pop();
    }
    fn visit_while_statement(&mut self, it: &WhileStatement<'a>) {
        self.loops.push(it.span);
        walk::walk_while_statement(self, it);
        self.loops.pop();
    }
    fn visit_do_while_statement(&mut self, it: &DoWhileStatement<'a>) {
        self.loops.push(it.span);
        walk::walk_do_while_statement(self, it);
        self.loops.pop();
    }
}

/// Whether each path through statements writes the slot before a read can
/// see the value they started with.
struct WritesFirst<'u> {
    /// The slot's uses in the block.
    symbol_uses: &'u [(Span, bool)],
}

impl WritesFirst<'_> {
    fn uses_in(&self, span: Span) -> bool {
        self.symbol_uses.iter().any(|(use_, _)| inside(span, *use_))
    }

    /// `Some(live)`: whether the entry value may still be the slot's value
    /// after them; `None`: a use may see it.
    fn statements(&self, statements: &[Statement<'_>], mut live: bool) -> Option<bool> {
        for statement in statements {
            live = self.statement(statement, live)?;
        }
        Some(live)
    }

    fn statement(&self, statement: &Statement<'_>, live: bool) -> Option<bool> {
        if !live || !self.uses_in(statement.span()) {
            return Some(live);
        }
        match statement {
            Statement::ExpressionStatement(expression) => match &expression.expression {
                Expression::AssignmentExpression(assign)
                    if assign.operator == oxc_syntax::operator::AssignmentOperator::Assign
                        && matches!(&assign.left, AssignmentTarget::AssignmentTargetIdentifier(id)
                            if self.uses_in(id.span))
                        && !self.uses_in(assign.right.span()) =>
                {
                    Some(false)
                }
                _ => None,
            },
            Statement::BlockStatement(block) => self.statements(&block.body, live),
            Statement::IfStatement(branch) => {
                if self.uses_in(branch.test.span()) {
                    return None;
                }
                let then = self.statement(&branch.consequent, true)?;
                let otherwise = match &branch.alternate {
                    Some(alternate) => self.statement(alternate, true)?,
                    None => true,
                };
                Some(then || otherwise)
            }
            _ => None,
        }
    }
}

struct Dynamic(bool);

impl<'a> Visit<'a> for Dynamic {
    fn visit_with_statement(&mut self, _: &WithStatement<'a>) {
        self.0 = true;
    }
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        if id.name == "eval" {
            self.0 = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::split_block_key_webs;

    fn split(source: &str) -> (String, usize) {
        split_block_key_webs(source).unwrap()
    }

    #[test]
    fn gives_a_block_that_writes_first_its_own_binding() {
        let source = "function f(a, b) {\n  let $KEY_35 = a.length;\n  use($KEY_35);\n  if (b != null) {\n    if (typeof b.f === \"function\") {\n      $KEY_35 = b.f();\n    } else {\n      $KEY_35 = [];\n    }\n    use($KEY_35.length);\n  }\n}\n";
        let (code, count) = split(source);
        assert_eq!(count, 1, "{code}");
        assert!(code.contains("if (b != null) {\nlet $KEY_35;"), "{code}");
    }

    #[test]
    fn keeps_one_binding_when_the_block_may_read_its_entry_value() {
        for source in [
            // Read before the write.
            "function f(a) { let $KEY_1 = a; if (a) { use($KEY_1); $KEY_1 = 2; use($KEY_1); } }",
            // Written on one path only.
            "function f(a) { let $KEY_1 = a; if (a) { if (g()) { $KEY_1 = 2; } use($KEY_1); } }",
            // Read after the block.
            "function f(a) { let $KEY_1 = a; use($KEY_1); if (a) { $KEY_1 = 2; use($KEY_1); } use($KEY_1); }",
            // A loop runs the earlier use again after the block.
            "function f(a) { let $KEY_1 = a; while (g()) { use($KEY_1); if (a) { $KEY_1 = 2; use($KEY_1); } } }",
            // A nested function could read it any time.
            "function f(a) { let $KEY_1 = a; h(() => $KEY_1); if (a) { $KEY_1 = 2; use($KEY_1); } }",
        ] {
            let (code, count) = split(source);
            assert_eq!(count, 0, "{source} → {code}");
        }
    }

    #[test]
    fn splits_each_later_block_of_its_own() {
        let source = "function f(a) { let $KEY_1 = a; use($KEY_1); if (a) { $KEY_1 = 2; use($KEY_1); } if (g()) { $KEY_1 = 3; use($KEY_1); } }";
        let (code, count) = split(source);
        assert_eq!(count, 2, "{code}");
        assert_eq!(code.matches("let $KEY_1;").count(), 2, "{code}");
    }
}
