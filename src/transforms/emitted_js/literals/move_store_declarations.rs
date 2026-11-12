//! Move a declaration that splits a run of property stores to one object.
//!
//! The tracer can emit a helper function — and a binding aliasing it —
//! between two halves of an object's property stores:
//!
//! ```js
//! let table = Object.create(null);
//! table.a = 1;
//! function helper(value) { … }
//! let formatter = helper;
//! table.b = 2;
//! ```
//!
//! A function declaration is hoisted, so moving it to the top of the same
//! block changes nothing. The alias is moved with it only when no store value
//! in the run reads the alias (it is initialized earlier, never later). The
//! declarations then sit at the top of the block, where the other declarations
//! are, and the store run reads straight through.
//!
//! The pass only moves statements that sit *between* two stores of the same
//! receiver, and only whole statements with no comments. Anything else, or an
//! alias the run mentions, keeps the declarations where they are.

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::AssignmentOperator;

use super::super::support::whole_line;
use crate::analysis;

pub(crate) fn move_store_declarations(source: &str) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut scan = Scan {
        source,
        scoping: &scoping,
        nodes: &nodes,
        comments: program.comments.iter().map(|comment| comment.span).collect(),
        edits: Vec::new(),
        moved: 0,
    };
    super::super::support::for_each_list(&program, |list| scan.list(list));
    if scan.moved == 0 {
        return Ok((source.into(), 0));
    }
    let output = super::super::support::apply_checked(source, scan.edits, "store declarations")?;
    Ok((output, scan.moved))
}

/// The declaration that can move past the stores.
struct Movable {
    statement: Span,
    /// Index of the declaration in the statement list.
    at: usize,
    /// The binding the declaration defines.
    symbol: SymbolId,
    /// For `let $X = $F;`, the `$F` bound by a function declared in the run.
    init: Option<SymbolId>,
}

struct Run {
    /// Index one past the last statement of the run.
    end: usize,
    /// The declarations between two stores, in order.
    interior: Vec<Movable>,
}

struct Scan<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    comments: Vec<Span>,
    edits: Vec<analysis::Edit>,
    moved: usize,
}

impl Scan<'_, '_> {
    fn list(&mut self, statements: &[Statement<'_>]) {
        let Some(first) = statements.first() else {
            return;
        };
        let mut moved: Vec<Span> = Vec::new();
        let mut index = 0;
        while index < statements.len() {
            let Some(run) = self.run(statements, index) else {
                index += 1;
                continue;
            };
            moved.extend(run.interior.iter().map(|movable| movable.statement));
            index = run.end;
        }
        if moved.is_empty() {
            return;
        }
        let mut text = String::new();
        for span in &moved {
            let line = whole_line(self.source, *span);
            text.push_str(&self.source[line.start as usize..line.end as usize]);
            self.edits.push(analysis::Edit::delete(line));
        }
        let top = whole_line(self.source, first.span()).start;
        self.edits.push(analysis::Edit::replace(Span::new(top, top), text));
        self.moved += moved.len();
    }

    fn run(&self, statements: &[Statement<'_>], start: usize) -> Option<Run> {
        let symbol = self.store_target(&statements[start])?;
        let mut stores = vec![start];
        let mut movables: Vec<Movable> = Vec::new();
        let mut index = start + 1;
        while let Some(statement) = statements.get(index) {
            if self.store_target(statement) == Some(symbol) {
                stores.push(index);
            } else if let Some(movable) = self.movable(statement, index) {
                movables.push(movable);
            } else {
                break;
            }
            index += 1;
        }
        let last_store = statements[*stores.last()?].span();
        // Only declarations with a store on both sides split the run.
        let interior: Vec<Movable> = movables
            .into_iter()
            .filter(|movable| movable.statement.start < last_store.start)
            .collect();
        if interior.is_empty() {
            return None;
        }
        // Functions declared anywhere in the run may be the aliases' target.
        let functions: Vec<SymbolId> = statements[start..index]
            .iter()
            .filter_map(|statement| match statement {
                Statement::FunctionDeclaration(function) => function.id.as_ref()?.symbol_id.get(),
                _ => None,
            })
            .collect();
        let run_start = statements[start].span().start;
        let run_end = last_store.end;
        for movable in &interior {
            let previous = statements[movable.at - 1].span().end;
            if self.has_comment(self.whole_line(movable.statement))
                || self.has_comment_between(previous, movable.statement.start)
            {
                return None;
            }
            if let Some(init) = movable.init {
                if !functions.contains(&init) {
                    return None;
                }
                // The run must not read the alias before its new position.
                if self.scoping.get_resolved_references(movable.symbol).any(|reference| {
                    let span = self.nodes.kind(reference.node_id()).span();
                    run_start <= span.start && span.end <= run_end
                }) {
                    return None;
                }
            }
        }
        Some(Run { end: index, interior })
    }

    /// `$X.k = value;` / `$X[k] = value;`: the receiver.
    fn store_target(&self, statement: &Statement<'_>) -> Option<SymbolId> {
        let Statement::ExpressionStatement(expression) = statement else {
            return None;
        };
        let Expression::AssignmentExpression(assignment) = &expression.expression else {
            return None;
        };
        if assignment.operator != AssignmentOperator::Assign {
            return None;
        }
        let object = match &assignment.left {
            AssignmentTarget::StaticMemberExpression(member) if !member.optional => &member.object,
            AssignmentTarget::ComputedMemberExpression(member) if !member.optional => &member.object,
            _ => return None,
        };
        let Expression::Identifier(receiver) = object else {
            return None;
        };
        self.scoping.get_reference(receiver.reference_id()).symbol_id()
    }

    fn movable(&self, statement: &Statement<'_>, at: usize) -> Option<Movable> {
        match statement {
            Statement::FunctionDeclaration(function) => Some(Movable {
                statement: function.span,
                at,
                symbol: function.id.as_ref()?.symbol_id.get()?,
                init: None,
            }),
            Statement::VariableDeclaration(declaration) if declaration.kind != VariableDeclarationKind::Var => {
                let [declarator] = declaration.declarations.as_slice() else {
                    return None;
                };
                let BindingPattern::BindingIdentifier(binding) = &declarator.id else {
                    return None;
                };
                let Expression::Identifier(init) = declarator.init.as_ref()? else {
                    return None;
                };
                Some(Movable {
                    statement: declaration.span,
                    at,
                    symbol: binding.symbol_id.get()?,
                    init: self.scoping.get_reference(init.reference_id()).symbol_id(),
                })
            }
            _ => None,
        }
    }

    fn whole_line(&self, span: Span) -> Span {
        whole_line(self.source, span)
    }

    fn has_comment(&self, span: Span) -> bool {
        self.comments
            .iter()
            .any(|comment| span.start <= comment.start && comment.end <= span.end)
    }

    fn has_comment_between(&self, start: u32, end: u32) -> bool {
        self.comments
            .iter()
            .any(|comment| start <= comment.start && comment.end <= end)
    }
}

#[cfg(test)]
mod tests {
    use super::move_store_declarations;

    fn moved(source: &str) -> String {
        move_store_declarations(source).unwrap().0
    }

    #[test]
    fn moves_a_function_and_its_alias_to_the_top() {
        let source = "function f() {\n  let t = Object.create(null);\n  t.a = 1;\n  function helper(v) {\n    return v;\n  }\n  let alias = helper;\n  t.b = 2;\n  return [t, alias];\n}\n";
        assert_eq!(
            moved(source),
            "function f() {\n  function helper(v) {\n    return v;\n  }\n  let alias = helper;\n  let t = Object.create(null);\n  t.a = 1;\n  t.b = 2;\n  return [t, alias];\n}\n"
        );
    }

    #[test]
    fn moves_each_run_once_to_the_top_in_source_order() {
        let source = "function f() {\n  let t = {};\n  t.a = 1;\n  function one() {}\n  t.b = 2;\n  let u = {};\n  u.a = 1;\n  function two() {}\n  u.b = 2;\n  return [t, u];\n}\n";
        assert_eq!(
            moved(source),
            "function f() {\n  function one() {}\n  function two() {}\n  let t = {};\n  t.a = 1;\n  t.b = 2;\n  let u = {};\n  u.a = 1;\n  u.b = 2;\n  return [t, u];\n}\n"
        );
    }

    #[test]
    fn moves_a_function_used_by_a_store_value() {
        // The function is hoisted, so a store may still call it after the move.
        let source = "function f() {\n  let t = {};\n  t.a = 1;\n  function helper() {\n    return 2;\n  }\n  t.b = helper();\n  return t;\n}\n";
        assert!(
            moved(source).contains("function helper() {\n    return 2;\n  }\n  let t = {};\n  t.a = 1;\n  t.b = helper();"),
            "{}",
            moved(source)
        );
    }

    #[test]
    fn keeps_an_alias_the_run_reads() {
        let source = "function f() {\n  let t = {};\n  t.a = 1;\n  function helper() {\n    return 2;\n  }\n  let alias = helper;\n  t.b = alias();\n  return t;\n}\n";
        assert_eq!(moved(source), source);
    }

    #[test]
    fn keeps_declarations_at_the_edges_or_with_comments() {
        // The declaration is already after the run.
        let after = "function f() {\n  let t = {};\n  t.a = 1;\n  function helper() {}\n  return t;\n}\n";
        assert_eq!(moved(after), after);
        // A comment on the declaration keeps it in place.
        let commented = "function f() {\n  let t = {};\n  t.a = 1;\n  // keep\n  function helper() {}\n  t.b = 2;\n  return t;\n}\n";
        assert_eq!(moved(commented), commented);
    }

    #[test]
    fn only_moves_statements_between_the_same_receiver() {
        // `other.c` breaks the run, so the declaration is not interior to it.
        let source = "function f() {\n  let t = {};\n  let other = {};\n  t.a = 1;\n  function helper() {}\n  other.c = 2;\n  t.b = 3;\n  return t;\n}\n";
        assert_eq!(moved(source), source);
    }
}
