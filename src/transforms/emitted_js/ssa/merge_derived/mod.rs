//! Merge a value derived from a local back into that local.
//!
//! The VM computes a new value of a variable into a fresh stack slot, so one
//! source variable comes out as a chain of names:
//!
//! ```js
//! let $SSA_2 = $CARRY_1 - 1;
//! if ($SSA_2 >= 0) { use(a[$SSA_2]); $CARRY_1 = Number($SSA_2); continue; }
//! ```
//!
//! When no path from the declaration reads the local's old value before
//! writing it again, the derived binding can be the local itself:
//!
//! ```js
//! $CARRY_1 = $CARRY_1 - 1;
//! if ($CARRY_1 >= 0) { use(a[$CARRY_1]); $CARRY_1 = Number($CARRY_1); continue; }
//! ```
//!
//! `let r = f(L);` becomes `L = f(L);` and every read of `r` reads `L`. This
//! requires:
//!
//! - `r` is a generated (`$`) `let`/`const`, never written, read only by its
//!   own function and never as a shorthand property;
//! - `L` is a `let`, `var` or parameter (one declaration, not `const`) that
//!   only its own function references, so no call can read or write it; a
//!   parameter only when the function never mentions `arguments`;
//! - nothing writes `L` between the declaration and a read of `r`, and no read
//!   of `r` sits in a loop after the declaration that writes `L`;
//! - the old value is dead: walking forward from the declaration — through
//!   the rest of its block, out of enclosing blocks and branches, and around
//!   enclosing loops — no read of `L` is reachable before a plain store
//!   `L = v` (or a `let` re-entry) kills it. Jumps follow `break`/`continue`
//!   to their loop; `return`/`throw` leave the function, where a local is
//!   dead; inside a `try` block a throw from any later point enters the
//!   handler, which must not read the old value either (a finalizer
//!   declines).
//!
//! Each binding takes part in one merge per call; callers repeat until
//! nothing merges, so a chain folds one link at a time. `with`/`eval`
//! decline.

use std::collections::BTreeSet;

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, NodeId, Scoping, SymbolFlags, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::AssignmentOperator;

use crate::analysis;
use super::super::support::whole_line;

mod merger;
mod statements;
#[cfg(test)]
mod tests;

use statements::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MergeDerivedResult {
    pub code: String,
    pub merged: usize,
}

pub(crate) fn merge_derived_values(source: &str) -> Result<MergeDerivedResult, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let mut dynamic = Dynamic::default();
    dynamic.visit_program(&program);
    if dynamic.found {
        return Ok(unchanged(source));
    }
    let (scoping, nodes) = analysis::analyze(&program);
    let merger = Merger {
        source,
        scoping: &scoping,
        nodes: &nodes,
        arguments_at: source.match_indices("arguments").map(|(at, _)| at as u32).collect(),
    };
    let mut selected: BTreeSet<SymbolId> = BTreeSet::new();
    let mut edits = Vec::new();
    let mut merged = 0;
    for derived in scoping.symbol_ids() {
        if selected.contains(&derived) {
            continue;
        }
        let Some(candidate) = merger.candidate(derived) else {
            continue;
        };
        for local in merger.sources(&candidate) {
            if selected.contains(&local) || local == derived {
                continue;
            }
            let Some(proposal) = merger.merge(&candidate, local) else {
                continue;
            };
            // Two names leaving one declaration edit next to each other: the
            // second waits for the next call.
            if proposal.iter().any(|edit| {
                edits.iter().any(|other: &analysis::Edit| {
                    edit.start < other.end && other.start < edit.end
                        || (edit.start == edit.end && other.start < edit.start && edit.start < other.end)
                })
            }) {
                break;
            }
            selected.insert(derived);
            selected.insert(local);
            edits.extend(proposal);
            merged += 1;
            break;
        }
    }
    if merged == 0 {
        return Ok(unchanged(source));
    }
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err("derived value merges overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|error| format!("derived value merge output: {error}"))?;
    Ok(MergeDerivedResult {
        code: applied.output,
        merged,
    })
}

/// A value stored into a local and read once, by the store that replaces it
/// (`v = E; …; v = f(v);`), becomes its own single-use binding
/// (`let t = E; …; v = f(t);`) for forwarding to place: the reverse of a
/// merge, for the one value whose only reader is the next store.
pub(crate) fn split_single_reads(source: &str) -> Result<MergeDerivedResult, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let mut dynamic = Dynamic::default();
    dynamic.visit_program(&program);
    if dynamic.found {
        return Ok(unchanged(source));
    }
    let (scoping, nodes) = analysis::analyze(&program);
    let merger = Merger {
        source,
        scoping: &scoping,
        nodes: &nodes,
        arguments_at: source.match_indices("arguments").map(|(at, _)| at as u32).collect(),
    };
    let mut fresh = 0usize;
    let taken = super::super::inline_tuple_helpers::taken_numbers(source, "$SSA_s");
    let mut name = || loop {
        fresh += 1;
        if !taken.contains(&fresh.to_string()) {
            return format!("$SSA_s{fresh}");
        }
    };
    let mut edits = Vec::new();
    let mut split = 0;
    for symbol in scoping.symbol_ids() {
        let declaration = scoping.symbol_declaration(symbol);
        let flags = scoping.symbol_flags(symbol);
        if flags.contains(SymbolFlags::ConstVariable)
            || !matches!(nodes.kind(declaration), AstKind::VariableDeclarator(_))
            || scoping.symbol_declarations(symbol).count() != 1
        {
            continue;
        }
        let home = merger.function_of(declaration);
        let mut references: Vec<(u32, NodeId, bool)> = Vec::new();
        let mut local = true;
        for reference in scoping.get_resolved_references(symbol) {
            if merger.function_of(reference.node_id()) != home || (reference.is_read() && reference.is_write()) {
                local = false;
                break;
            }
            // A store takes effect once its value is computed.
            let at = if reference.is_write() {
                merger.write_at(reference.node_id())
            } else {
                merger.span_of(reference.node_id()).start
            };
            references.push((at, reference.node_id(), reference.is_write()));
        }
        if !local {
            continue;
        }
        references.sort_by_key(|(at, _, _)| *at);
        let name_text = scoping.symbol_name(symbol);
        // A store whose read was just renamed waits for the next round.
        let mut renamed_in: Option<NodeId> = None;
        for window in references.windows(3) {
            let [(_, write, true), (_, read, false), (_, next, true)] = window else {
                continue;
            };
            // `v = E;` then `v = f(v);` in the same statement list.
            let Some(first) = merger.store_statement(*write) else {
                continue;
            };
            let Some(second) = merger.store_statement(*next) else {
                continue;
            };
            let second_span = merger.span_of(second);
            let read_span = merger.span_of(*read);
            if !(second_span.start <= read_span.start && read_span.end <= second_span.end)
                || nodes.parent_id(first) != nodes.parent_id(second)
            {
                continue;
            }
            if renamed_in == Some(first) {
                continue;
            }
            let AstKind::ExpressionStatement(store) = nodes.kind(first) else {
                continue;
            };
            let Expression::AssignmentExpression(assignment) = &store.expression else {
                continue;
            };
            let value = assignment.right.span();
            renamed_in = Some(second);
            let temp = name();
            let text = &source[value.start as usize..value.end as usize];
            let _ = name_text;
            edits.push(analysis::Edit::replace(store.span, format!("let {temp} = {text};")));
            edits.push(analysis::Edit::replace(read_span, temp));
            split += 1;
        }
    }
    if split == 0 {
        return Ok(unchanged(source));
    }
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err("single-read splits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output).map_err(|error| format!("single-read split output: {error}"))?;
    Ok(MergeDerivedResult {
        code: applied.output,
        merged: split,
    })
}

fn unchanged(source: &str) -> MergeDerivedResult {
    MergeDerivedResult {
        code: source.into(),
        merged: 0,
    }
}

#[derive(Default)]
struct Dynamic {
    found: bool,
}

impl<'a> Visit<'a> for Dynamic {
    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.found = true;
        walk::walk_with_statement(self, statement);
    }
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if identifier.name == "eval" {
            self.found = true;
        }
    }
}

/// `let r = init;` with its statement and function.
struct Candidate<'s, 'a> {
    symbol: SymbolId,
    statement: NodeId,
    statement_span: Span,
    /// `None` for `let r;` that the next statement stores on every path.
    init: Option<&'s Expression<'a>>,
    /// What to delete when the declaration goes: the whole statement, or
    /// this name and a comma.
    removal: Span,
    function: Option<NodeId>,
    reads: Vec<NodeId>,
    /// Later `r = v;` statements, with the written identifier.
    writes: Vec<(NodeId, NodeId)>,
}

/// Where control can go from a point without passing a store to the local.
#[derive(Default)]
struct Flow {
    /// A read of the old value is reachable.
    read: bool,
    /// Control can complete normally.
    falls: bool,
    jumps: Vec<Jump>,
}

#[derive(Clone, PartialEq, Eq)]
enum Jump {
    Break(Option<String>),
    Continue(Option<String>),
}

struct Merger<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    /// Where the text `arguments` starts, in order.
    arguments_at: Vec<u32>,
}

struct Walk<'m, 's, 'a> {
    merger: &'m Merger<'s, 'a>,
    local: SymbolId,
    /// Starts of every read of the local, sorted.
    reads: &'m [u32],
}

/// The name a property path (`x.p`, `x[k]?.q`) starts from.
fn path_root<'s, 'a>(expression: &'s Expression<'a>) -> Option<&'s IdentifierReference<'a>> {
    fn root<'s, 'a>(expression: &'s Expression<'a>) -> Option<&'s IdentifierReference<'a>> {
        match expression.without_parentheses() {
            Expression::Identifier(identifier) => Some(&**identifier),
            Expression::StaticMemberExpression(member) => root(&member.object),
            Expression::ComputedMemberExpression(member) => root(&member.object),
            Expression::ChainExpression(chain) => match &chain.expression {
                ChainElement::StaticMemberExpression(member) => root(&member.object),
                ChainElement::ComputedMemberExpression(member) => root(&member.object),
                _ => None,
            },
            _ => None,
        }
    }
    match expression.without_parentheses() {
        Expression::Identifier(_) => None,
        other => root(other),
    }
}
