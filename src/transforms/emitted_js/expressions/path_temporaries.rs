//! Write a property read where it is used instead of holding it in a
//! variable the next statement consumes.
//!
//! ```js
//! let $BN = $KEY_9.Intl?.supportedValuesOf;
//! $BN = ($BN && typeof $BN === "function" && $KEY_52($BN)) | 0;
//! ```
//!
//! becomes `let $BN = ($KEY_9.Intl?.supportedValuesOf && typeof
//! $KEY_9.Intl?.supportedValuesOf === "function" && …) | 0;`. Cleanup kept
//! such a value in a variable because reading a property could run a
//! getter; here property reads are taken to run no code and to give the
//! same value each time (the fingerprint probes read host objects only).
//!
//! The value is a path: a name and property names (`?.` allowed), no
//! call. Every read of the variable is in the next statement and outside
//! any function it creates. The path must still hold the same value at each
//! read: it is a name never stored again (or a global), or nothing in the
//! statement that could store anything — a call, `new`, an assignment, an
//! update, a `delete` — runs before the last read. When that statement stores the variable again
//! (`x = E;`), the two merge into `let x = E`; otherwise the variable must
//! have no other use and its declaration goes. A read used as an object
//! takes the path in parentheses when it has `?.`, which would otherwise
//! extend the chain; a read that is called declines for a member path,
//! which would call with a receiver.

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping};
use oxc_span::{GetSpan, Span};

use crate::analysis;
use super::super::support::whole_line;

pub(crate) fn inline_path_temporaries(source: &str) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut scan = Scan { source, scoping: &scoping, nodes: &nodes, edits: Vec::new(), inlined: 0 };
    super::super::support::for_each_list(&program, |list| scan.list(list));
    if scan.inlined == 0 {
        return Ok((source.into(), 0));
    }
    let output = super::super::support::apply_checked(source, scan.edits, "path temporary")?;
    Ok((output, scan.inlined))
}

struct Scan<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    edits: Vec<analysis::Edit>,
    inlined: usize,
}

/// A name and property names, `?.` allowed; whether it has a `?.`.
fn path(expression: &Expression<'_>) -> Option<bool> {
    match expression.without_parentheses() {
        Expression::Identifier(_) | Expression::ThisExpression(_) => Some(false),
        Expression::StaticMemberExpression(member) => path(&member.object).map(|optional| optional || member.optional),
        Expression::ComputedMemberExpression(member) if matches!(member.expression, Expression::StringLiteral(_)) => {
            path(&member.object).map(|optional| optional || member.optional)
        }
        Expression::ChainExpression(chain) => match &chain.expression {
            ChainElement::StaticMemberExpression(member) => path(&member.object).map(|_| true),
            ChainElement::ComputedMemberExpression(member) if matches!(member.expression, Expression::StringLiteral(_)) => {
                path(&member.object).map(|_| true)
            }
            _ => None,
        },
        _ => None,
    }
}


impl Scan<'_, '_> {
    fn text(&self, span: Span) -> &str {
        &self.source[span.start as usize..span.end as usize]
    }

    fn list(&mut self, statements: &[Statement<'_>]) {
        let mut taken = 0;
        for (index, statement) in statements.iter().enumerate() {
            if index < taken {
                continue;
            }
            if let Some(next) = statements.get(index + 1) {
                if self.inline(statement, next) {
                    taken = index + 2;
                }
            }
        }
    }

    fn inline(&mut self, statement: &Statement<'_>, next: &Statement<'_>) -> bool {
        let Statement::VariableDeclaration(declaration) = statement else {
            return false;
        };
        let [declarator] = declaration.declarations.as_slice() else {
            return false;
        };
        let (BindingPattern::BindingIdentifier(id), Some(value)) = (&declarator.id, &declarator.init) else {
            return false;
        };
        let Some(optional) = path(value) else {
            return false;
        };
        if declaration.kind == VariableDeclarationKind::Var || !self.source[statement.span().end as usize..next.span().start as usize].trim().is_empty() {
            return false;
        }
        let symbol = id.symbol_id();
        let member_path = !matches!(value.without_parentheses(), Expression::Identifier(_) | Expression::ThisExpression(_));
        // `x = E;` storing the variable again, E holding the reads.
        let restore = match next {
            Statement::ExpressionStatement(expression) => match &expression.expression {
                Expression::AssignmentExpression(assignment)
                    if assignment.operator == AssignmentOperator::Assign
                        && matches!(&assignment.left, AssignmentTarget::AssignmentTargetIdentifier(target)
                            if self.scoping.get_reference(target.reference_id()).symbol_id() == Some(symbol)) =>
                {
                    Some((assignment.right.span(), expression.span))
                }
                _ => None,
            },
            _ => None,
        };
        let within = |outer: Span, span: Span| outer.start <= span.start && span.end <= outer.end;
        let mut reads = Vec::new();
        for reference in self.scoping.get_resolved_references(symbol) {
            let node = reference.node_id();
            let span = self.nodes.kind(node).span();
            match restore {
                Some((right, store)) => {
                    if reference.is_write() {
                        // Only the restore itself, which keeps the variable.
                        if !(within(store, span) && !within(right, span)) {
                            return false;
                        }
                        continue;
                    }
                    if within(right, span) {
                        reads.push((span, node));
                    } else if span.start < store.end {
                        return false;
                    }
                }
                None => {
                    if reference.is_write() || !within(next.span(), span) {
                        return false;
                    }
                    reads.push((span, node));
                }
            }
        }
        if reads.is_empty() {
            return false;
        }
        let last_read = reads.iter().map(|(span, _)| span.start).max().unwrap_or(0);
        let scope = match restore {
            Some((right, _)) => right,
            None => next.span(),
        };
        if !self.fixed_name(value) && self.effect_before(next, scope, last_read) {
            return false;
        }
        let path_text = self.text(value.span()).to_string();
        let mut replaced = Vec::new();
        for (span, node) in &reads {
            if self.in_function(*node, next.span()) {
                return false;
            }
            let text = match self.nodes.parent_kind(*node) {
                AstKind::CallExpression(call) if call.callee.span() == *span => {
                    if member_path {
                        return false;
                    }
                    path_text.clone()
                }
                AstKind::NewExpression(new) if new.callee.span() == *span && member_path => return false,
                AstKind::StaticMemberExpression(_) | AstKind::ComputedMemberExpression(_) if optional => format!("({path_text})"),
                _ => path_text.clone(),
            };
            replaced.push((*span, text));
        }
        match restore {
            Some((right, store)) => {
                let mut merged = self.text(right).to_string();
                for (span, text) in replaced.iter().rev() {
                    merged.replace_range((span.start - right.start) as usize..(span.end - right.start) as usize, text);
                }
                let kind = declaration.kind.as_str();
                self.edits.push(analysis::Edit::replace(Span::new(statement.span().start, store.end), format!("{kind} {} = {merged};", id.name)));
            }
            None => {
                self.edits.push(analysis::Edit::delete(whole_line(self.source, statement.span())));
                for (span, text) in replaced {
                    self.edits.push(analysis::Edit::replace(span, text));
                }
            }
        }
        self.inlined += 1;
        true
    }

    /// A bare name no store reaches after its declaration, or a global.
    fn fixed_name(&self, value: &Expression<'_>) -> bool {
        match value.without_parentheses() {
            Expression::Identifier(name) => match self.scoping.get_reference(name.reference_id()).symbol_id() {
                None => true,
                Some(symbol) => self.scoping.get_resolved_references(symbol).all(|reference| !reference.is_write()),
            },
            _ => false,
        }
    }

    /// Something in `statement` (within `scope`) that could store a value
    /// completes before `before`: a call, `new`, assignment, update or
    /// `delete` ending ahead of a read. Functions it creates do not run.
    fn effect_before(&self, statement: &Statement<'_>, scope: Span, before: u32) -> bool {
        struct Effects {
            scope: Span,
            before: u32,
            found: bool,
        }
        impl Effects {
            fn note(&mut self, span: Span) {
                self.found |= self.scope.start <= span.start && span.end <= self.before;
            }
        }
        impl<'b> Visit<'b> for Effects {
            fn visit_call_expression(&mut self, call: &CallExpression<'b>) {
                self.note(call.span);
                walk::walk_call_expression(self, call);
            }
            fn visit_new_expression(&mut self, new: &NewExpression<'b>) {
                self.note(new.span);
                walk::walk_new_expression(self, new);
            }
            fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'b>) {
                self.note(assignment.span);
                walk::walk_assignment_expression(self, assignment);
            }
            fn visit_update_expression(&mut self, update: &UpdateExpression<'b>) {
                self.note(update.span);
                walk::walk_update_expression(self, update);
            }
            fn visit_unary_expression(&mut self, unary: &UnaryExpression<'b>) {
                if unary.operator == UnaryOperator::Delete {
                    self.note(unary.span);
                }
                walk::walk_unary_expression(self, unary);
            }
            fn visit_function(&mut self, _: &Function<'b>, _: oxc_syntax::scope::ScopeFlags) {}
            fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'b>) {}
        }
        let mut effects = Effects { scope, before, found: false };
        effects.visit_statement(statement);
        effects.found
    }

    /// The read sits in a function created inside `statement`.
    fn in_function(&self, node: oxc_semantic::NodeId, statement: Span) -> bool {
        self.nodes.ancestor_ids(node).take_while(|&ancestor| statement.start <= self.nodes.kind(ancestor).span().start).any(|ancestor| {
            matches!(self.nodes.kind(ancestor), AstKind::Function(_) | AstKind::ArrowFunctionExpression(_))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inline(source: &str) -> String {
        inline_path_temporaries(source).unwrap().0
    }

    #[test]
    fn merges_a_probe_into_its_restore() {
        let source = "function f(g, k) {\n\tlet b = g.Intl?.supportedValuesOf;\n\tb = (b && typeof b === \"function\" && k(b)) | 0;\n\treturn b;\n}\n";
        assert_eq!(
            inline(source),
            "function f(g, k) {\n\tlet b = (g.Intl?.supportedValuesOf && typeof g.Intl?.supportedValuesOf === \"function\" && k(g.Intl?.supportedValuesOf)) | 0;\n\treturn b;\n}\n"
        );
    }

    #[test]
    fn writes_a_value_the_next_statement_reads() {
        let source = "function f(k, s) {\n\tlet m = navigator;\n\ts = s | (m && k(m)) << 5;\n\tlet o = g?.h;\n\treturn o.length;\n}\n";
        assert_eq!(inline(source), "function f(k, s) {\n\ts = s | (navigator && k(navigator)) << 5;\n\treturn (g?.h).length;\n}\n");
    }

    #[test]
    fn keeps_a_path_a_call_could_change_before_a_read() {
        // `g()` runs before the second read of `a`; `n` is a name stored
        // again, read after a call.
        let source = "function f(o, g) {\n\tlet a = o.p;\n\tif (g(a)) h(a);\n\tlet m = 0;\n\tlet b = n;\n\tif (g()) h(b);\n\tn = 2;\n\treturn m;\n}\nlet n = 1;\n";
        assert_eq!(inline(source), source);
    }

    #[test]
    fn keeps_a_value_read_later_called_or_read_by_a_closure() {
        let source = "function f(o) {\n\tlet a = o.p;\n\tg(a);\n\th(a);\n\tlet b = o.m;\n\tb(1);\n\tlet c = o.q;\n\tq(() => c);\n\treturn 0;\n}\n";
        assert_eq!(inline(source), source);
    }
}
