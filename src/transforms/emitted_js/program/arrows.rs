//! Write single-use callback threads as arrow functions where they are
//! passed.
//!
//! ```js
//! function $THREAD_3() { return $KEY_0["next"](arguments[0]); }
//! $KEY_7["forEach"]($THREAD_3);
//! // →
//! $KEY_7["forEach"]($ARG_0 => $KEY_0["next"]($ARG_0));
//! ```
//!
//! A thread declaration becomes an arrow at its one use when:
//!
//! - the use is an argument of a call or `new`, run at most once per run of
//!   the declaring function (not in a loop or nested function there), so
//!   one function object stays one: a listener added twice stays added once;
//! - nothing in it reads `this` (an arrow has none of its own), and it reads
//!   its own `arguments` only as `arguments[n]`, which become parameters;
//! - every name it reads means the same at the use as at the declaration.
//!
//! A trailing `return void 0;` goes; a body that only returns a value
//! becomes an expression body. A `"use strict"` directive stays unless the
//! use is strict already. Callbacks inside callbacks go first; the pass
//! repeats until nothing changes. Parameters are named `$ARG_n`, for the
//! positional naming after it.

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, NodeId, Scoping};
use oxc_span::{GetSpan, Span};

use crate::analysis;
use super::super::support::whole_line;
use crate::transforms::emitted_js::support::is_void_zero;

pub(crate) fn arrow_callbacks(source: &str) -> Result<(String, usize), String> {
    let mut code = source.to_string();
    let mut total = 0;
    for _ in 0..12 {
        let (next, count) = round(&code)?;
        if count == 0 {
            break;
        }
        code = next;
        total += count;
    }
    Ok((code, total))
}

fn round(source: &str) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let scan = Scan { source, scoping: &scoping, nodes: &nodes };
    let mut candidates: Vec<Candidate> = Vec::new();
    for node in nodes.iter() {
        if let AstKind::Function(function) = node.kind() {
            if let Some(candidate) = scan.candidate(node.id(), function) {
                candidates.push(candidate);
            }
        }
    }
    // Innermost first: a callback holding another one's declaration or use
    // waits a round.
    let ready: Vec<&Candidate> = candidates
        .iter()
        .filter(|outer| {
            !candidates.iter().any(|inner| {
                inner.declaration != outer.declaration
                    && (outer.declaration.contains_inclusive(inner.declaration) || outer.declaration.contains_inclusive(inner.use_span))
            })
        })
        .collect();
    if ready.is_empty() {
        return Ok((source.into(), 0));
    }
    let mut edits = Vec::new();
    for candidate in &ready {
        edits.push(analysis::Edit::delete(whole_line(source, candidate.declaration)));
        edits.push(analysis::Edit::replace(candidate.use_span, candidate.arrow.clone()));
    }
    let output = super::super::support::apply_checked(source, edits, "arrow callback")?;
    Ok((output, ready.len()))
}

struct Candidate {
    declaration: Span,
    use_span: Span,
    arrow: String,
}

struct Scan<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
}

/// What the body does with `this` and its own `arguments`.
struct Body<'a> {
    /// Nested ordinary functions: their `this`/`arguments` are their own.
    depth: usize,
    this: bool,
    /// `arguments[n]` spans and `n`.
    indexed: Vec<(Span, usize)>,
    /// Any other use of `arguments`, or `arguments[n]` inside a nested
    /// arrow, whose own parameters could take the new name.
    other_arguments: bool,
    /// Nested arrows around the visit.
    arrows: usize,
    /// References to check at the use site.
    references: Vec<(oxc_semantic::ReferenceId, oxc_span::Ident<'a>)>,
}

impl<'a> Visit<'a> for Body<'a> {
    fn visit_function(&mut self, function: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        self.depth += 1;
        walk::walk_function(self, function, flags);
        self.depth -= 1;
    }
    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        self.arrows += 1;
        walk::walk_arrow_function_expression(self, arrow);
        self.arrows -= 1;
    }
    fn visit_this_expression(&mut self, _: &ThisExpression) {
        if self.depth == 0 {
            self.this = true;
        }
    }
    fn visit_meta_property(&mut self, _: &MetaProperty<'a>) {
        if self.depth == 0 {
            self.this = true;
        }
    }
    fn visit_super(&mut self, _: &Super) {
        self.this = true;
    }
    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'a>) {
        if self.depth == 0 {
            if let (Expression::Identifier(object), Expression::NumericLiteral(index)) = (&member.object, &member.expression) {
                if object.name == "arguments" && index.value.fract() == 0.0 && index.value >= 0.0 && index.value < 64.0 {
                    if self.arrows > 0 {
                        self.other_arguments = true;
                    }
                    self.indexed.push((member.span, index.value as usize));
                    return;
                }
            }
        }
        walk::walk_computed_member_expression(self, member);
    }
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if self.depth == 0 && identifier.name == "arguments" {
            self.other_arguments = true;
        }
        self.references.push((identifier.reference_id(), identifier.name));
    }
}

impl<'s, 'a> Scan<'s, 'a> {
    fn candidate(&self, node: NodeId, function: &'a Function<'a>) -> Option<Candidate> {
        let id = function.id.as_ref()?;
        if !function.is_declaration() || !id.name.starts_with("$THREAD_") || function.r#async || function.generator {
            return None;
        }
        if !function.params.items.is_empty() || function.params.rest.is_some() {
            return None;
        }
        let symbol = id.symbol_id();
        let mut uses = self.scoping.get_resolved_references(symbol);
        let use_node = uses.next()?.node_id();
        if uses.next().is_some() {
            return None;
        }
        let use_span = self.nodes.get_node(use_node).span();
        // An argument of a call or `new`.
        let argument = match self.nodes.parent_kind(use_node) {
            AstKind::CallExpression(call) => call.arguments.iter().any(|argument| argument.span() == use_span) && call.callee.span() != use_span,
            AstKind::NewExpression(new) => new.arguments.iter().any(|argument| argument.span() == use_span),
            _ => false,
        };
        if !argument {
            return None;
        }
        // Run at most once per run of the declaring function: no loop or
        // nested function between the use and that function.
        let home = self.nodes.ancestor_ids(node).find(|&ancestor| matches!(self.nodes.kind(ancestor), AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) | AstKind::Program(_)))?;
        for ancestor in self.nodes.ancestor_ids(use_node) {
            if ancestor == home {
                break;
            }
            if matches!(
                self.nodes.kind(ancestor),
                AstKind::ForStatement(_)
                    | AstKind::ForInStatement(_)
                    | AstKind::ForOfStatement(_)
                    | AstKind::WhileStatement(_)
                    | AstKind::DoWhileStatement(_)
                    | AstKind::Function(_)
                    | AstKind::ArrowFunctionExpression(_)
                    | AstKind::Class(_)
            ) {
                return None;
            }
        }
        // `this` and `arguments`.
        let body = function.body.as_ref()?;
        let mut uses_of = Body { depth: 0, arrows: 0, this: false, indexed: Vec::new(), other_arguments: false, references: Vec::new() };
        uses_of.visit_function_body(body);
        if uses_of.this || uses_of.other_arguments {
            return None;
        }
        // Each name means the same at the use.
        let use_scope = self.nodes.get_node(use_node).scope_id();
        for &(reference, name) in &uses_of.references {
            let resolved = self.scoping.get_reference(reference).symbol_id();
            if let Some(resolved) = resolved {
                if function.span.contains_inclusive(self.scoping.symbol_span(resolved)) {
                    continue;
                }
            }
            if self.scoping.find_binding(use_scope, name) != resolved {
                return None;
            }
        }
        let arrow = self.arrow(function, body, &uses_of.indexed, use_node, use_span)?;
        Some(Candidate { declaration: function.span, use_span, arrow })
    }

    /// The use runs in strict code already.
    fn strict_at(&self, node: NodeId) -> bool {
        self.nodes.ancestor_ids(node).any(|ancestor| match self.nodes.kind(ancestor) {
            AstKind::Function(function) => function.body.as_ref().is_some_and(|body| body.directives.iter().any(|d| d.directive == "use strict")),
            AstKind::Class(_) => true,
            AstKind::Program(program) => program.directives.iter().any(|d| d.directive == "use strict"),
            _ => false,
        })
    }

    fn arrow(&self, function: &Function<'a>, body: &FunctionBody<'a>, indexed: &[(Span, usize)], use_node: NodeId, use_span: Span) -> Option<String> {
        let parameters: Vec<String> = match indexed.iter().map(|(_, index)| *index).max() {
            Some(last) => (0..=last).map(|index| format!("$ARG_{index}")).collect(),
            None => Vec::new(),
        };
        let head = match parameters.as_slice() {
            [single] => single.clone(),
            many => format!("({})", many.join(", ")),
        };
        let strict_directive = body.directives.iter().any(|d| d.directive == "use strict");
        let keep_directive = strict_directive && !self.strict_at(use_node);
        let mut statements: Vec<&Statement<'a>> = body.statements.iter().collect();
        if let Some(Statement::ReturnStatement(last)) = statements.last() {
            if last.argument.as_ref().is_none_or(is_void_zero) {
                statements.pop();
            }
        }
        // `arguments[n]` → `$ARG_n`, inside `span`.
        let render = |span: Span| -> String {
            let mut out = String::new();
            let mut at = span.start;
            let mut inside: Vec<&(Span, usize)> = indexed.iter().filter(|(read, _)| span.contains_inclusive(*read)).collect();
            inside.sort_by_key(|(read, _)| read.start);
            for (read, index) in inside {
                out.push_str(&self.source[at as usize..read.start as usize]);
                out.push_str(&format!("$ARG_{index}"));
                at = read.end;
            }
            out.push_str(&self.source[at as usize..span.end as usize]);
            out
        };
        // An expression body.
        if !keep_directive {
            if let [Statement::ReturnStatement(returned)] = statements.as_slice() {
                let value = returned.argument.as_ref()?;
                let text = render(value.span());
                let wrapped = matches!(value.get_inner_expression(), Expression::ObjectExpression(_) | Expression::SequenceExpression(_));
                if !text.contains('\n') {
                    return Some(if wrapped { format!("{head} => ({text})") } else { format!("{head} => {text}") });
                }
            }
        }
        // A block body, re-indented from the declaration to the use.
        let indent_of = |at: u32| {
            let line_start = self.source[..at as usize].rfind('\n').map_or(0, |p| p + 1);
            self.source[line_start..].chars().take_while(|c| *c == '\t').count()
        };
        let from = indent_of(function.span.start) + 1;
        let to = indent_of(use_span.start) + 1;
        let mut out = format!("{head} => {{\n");
        if keep_directive {
            out.push_str(&"\t".repeat(to));
            out.push_str("\"use strict\";\n");
        }
        for statement in statements {
            for (index, line) in render(statement.span()).split('\n').enumerate() {
                let line = if index == 0 {
                    line
                } else {
                    let tabs = line.chars().take_while(|c| *c == '\t').count().min(from);
                    &line[tabs..]
                };
                out.push_str(&"\t".repeat(to));
                out.push_str(line);
                out.push('\n');
            }
        }
        out.push_str(&"\t".repeat(to - 1));
        out.push('}');
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arrows(source: &str) -> (String, usize) {
        arrow_callbacks(source).unwrap()
    }

    #[test]
    fn writes_callbacks_as_arrows_with_parameters() {
        let source = "function f(o) {\n\tfunction $THREAD_1() {\n\t\treturn o[\"next\"](arguments[0]);\n\t}\n\to[\"forEach\"]($THREAD_1);\n\tfunction $THREAD_2() {\n\t\to[\"a\"] = arguments[1];\n\t\tg(arguments[0]);\n\t\treturn void 0;\n\t}\n\tif (o) {\n\t\to[\"map\"]($THREAD_2);\n\t}\n}\n";
        let (code, count) = arrows(source);
        assert_eq!(count, 2);
        assert_eq!(code, "function f(o) {\n\to[\"forEach\"]($ARG_0 => o[\"next\"]($ARG_0));\n\tif (o) {\n\t\to[\"map\"](($ARG_0, $ARG_1) => {\n\t\t\to[\"a\"] = $ARG_1;\n\t\t\tg($ARG_0);\n\t\t});\n\t}\n}\n");
    }

    #[test]
    fn nested_callbacks_go_inside_out() {
        let source = "function f(o) {\n\tfunction $THREAD_1() {\n\t\tfunction $THREAD_2() {\n\t\t\treturn arguments[0] + 1;\n\t\t}\n\t\treturn arguments[0][\"map\"]($THREAD_2);\n\t}\n\treturn o[\"then\"]($THREAD_1);\n}\n";
        let (code, count) = arrows(source);
        assert_eq!(count, 2, "{code}");
        assert_eq!(code, "function f(o) {\n\treturn o[\"then\"]($ARG_0 => $ARG_0[\"map\"]($ARG_0 => $ARG_0 + 1));\n}\n");
    }

    #[test]
    fn keeps_functions_an_arrow_would_change() {
        for source in [
            // reads this
            "function f(o) {\n\tfunction $THREAD_1() {\n\t\treturn this;\n\t}\n\to[\"forEach\"]($THREAD_1);\n}\n",
            // reads arguments as a whole
            "function f(o) {\n\tfunction $THREAD_1() {\n\t\treturn arguments[\"length\"];\n\t}\n\to[\"forEach\"]($THREAD_1);\n}\n",
            // passed in a loop: one listener, not one per pass
            "function f(o) {\n\tfunction $THREAD_1() {\n\t\treturn 1;\n\t}\n\tfor (;;) {\n\t\to[\"addEventListener\"](\"x\", $THREAD_1);\n\t}\n}\n",
            // used twice
            "function f(o) {\n\tfunction $THREAD_1() {\n\t\treturn 1;\n\t}\n\to[\"a\"]($THREAD_1);\n\to[\"b\"]($THREAD_1);\n}\n",
            // a name shadowed at the use
            "function f(o) {\n\tlet x = 1;\n\tfunction $THREAD_1() {\n\t\treturn x;\n\t}\n\tif (o) {\n\t\tlet x = 2;\n\t\to[\"a\"]($THREAD_1);\n\t}\n}\n",
            // stored, not passed
            "function f(o) {\n\tfunction $THREAD_1() {\n\t\treturn 1;\n\t}\n\to[\"a\"] = $THREAD_1;\n}\n",
        ] {
            let (code, count) = arrows(source);
            assert_eq!(count, 0, "{source} -> {code}");
        }
    }
}
