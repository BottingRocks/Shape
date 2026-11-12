//! Build an array with its values instead of placeholders filled after.
//!
//! ```js
//! let $BH = [!!$AX.ctrlKey, void 0, void 0, void 0];
//! $BH[1] = !!$AX.metaKey;
//! $BH[2] = !!$AX.altKey;
//! $BH[3] = !!$AX.shiftKey;
//! ```
//!
//! becomes `let $BH = [!!$AX.ctrlKey, !!$AX.metaKey, !!$AX.altKey,
//! !!$AX.shiftKey];`. The stores follow the declaration directly, with
//! whole-number indexes in ascending order, each replacing a placeholder or
//! adding the next element. The placeholders replaced, and every element
//! after the first of them, run no code (`void 0`, literals), so a stored
//! value runs where it did relative to anything with an effect. A value
//! must not name the array, which does not exist yet, and makes no call
//! unless no function names the array (a call reaches it only through one);
//! a `RegExp` of literal arguments is no call, and is written as a regex
//! literal. Placeholders past the last store stay, keeping the length.
//!
//! The tracer often stores a value through a reused scratch binding instead
//! of writing it in the store:
//!
//! ```js
//! let $AR = [$T];
//! $T = { ctor: null };
//! $AR[1] = $T;
//! $T = { ctor: null };
//! $AR[2] = $T;
//! ```
//!
//! Each `$T = <value>` is immediately followed by `$AR[k] = $T`, so the value
//! can stand in the store and the pair folds like a plain store. The binding
//! may be reused, so the value it holds at the end of the folded run must be
//! dead: the first reference to it after the run is a write (or there is
//! none). A `$T = …` immediately before the declaration folds into the
//! element that reads `$T`, when the other elements run no code. Comments
//! between paired statements keep the run from folding.
//!
//! An element is written on one line when it fits: threads flatten the
//! elements they folded, and the ones folded here must not read differently
//! beside them. Only an object or array literal of one-line values joins, so
//! the line breaks joined are layout, and none with a comment inside does.

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};

use super::super::constant_slots::regexp_literal;
use super::super::purity::expression_is_pure;
use crate::analysis;

/// The column a folded literal stays within, a tab counting 4.
const WIDTH: usize = 120;

pub(crate) fn fold_array_fills(source: &str) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut scan = Scan { source, scoping: &scoping, nodes: &nodes, comments: &program.comments, edits: Vec::new(), folded: 0 };
    super::super::support::for_each_list(&program, |list| scan.list(list));
    if scan.folded == 0 {
        return Ok((source.into(), 0));
    }
    let output = super::super::support::apply_checked(source, scan.edits, "array fill")?;
    Ok((output, scan.folded))
}

struct Scan<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    comments: &'s [Comment],
    edits: Vec<analysis::Edit>,
    folded: usize,
}


/// `A[k] = v;` with a whole-number literal `k`: (k, v).
fn store<'e, 'a>(statement: &'e Statement<'a>, symbol: SymbolId, scoping: &Scoping) -> Option<(usize, &'e Expression<'a>)> {
    let Statement::ExpressionStatement(statement) = statement else {
        return None;
    };
    let Expression::AssignmentExpression(assignment) = &statement.expression else {
        return None;
    };
    let AssignmentTarget::ComputedMemberExpression(member) = &assignment.left else {
        return None;
    };
    let (Expression::Identifier(array), Expression::NumericLiteral(index)) = (&member.object, &member.expression) else {
        return None;
    };
    if assignment.operator != AssignmentOperator::Assign
        || member.optional
        || scoping.get_reference(array.reference_id()).symbol_id() != Some(symbol)
        || index.value.fract() != 0.0
        || index.value < 0.0
        || index.value > 4096.0
    {
        return None;
    }
    Some((index.value as usize, &assignment.right))
}

/// `$T = value;` with a plain identifier target: (symbol, value).
fn scratch_assignment<'e, 'a>(statement: &'e Statement<'a>, scoping: &Scoping) -> Option<(SymbolId, &'e Expression<'a>)> {
    let Statement::ExpressionStatement(statement) = statement else {
        return None;
    };
    let Expression::AssignmentExpression(assignment) = &statement.expression else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
        return None;
    };
    if assignment.operator != AssignmentOperator::Assign {
        return None;
    }
    let symbol = scoping.get_reference(target.reference_id()).symbol_id()?;
    Some((symbol, &assignment.right))
}

/// `expression` is exactly a read of `symbol`.
fn names(expression: &Expression<'_>, symbol: SymbolId, scoping: &Scoping) -> bool {
    match expression {
        Expression::Identifier(identifier) => scoping.get_reference(identifier.reference_id()).symbol_id() == Some(symbol),
        _ => false,
    }
}

impl Scan<'_, '_> {
    fn text(&self, span: Span) -> &str {
        &self.source[span.start as usize..span.end as usize]
    }

    /// Only whitespace between the two spans.
    fn adjacent(&self, end: u32, start: u32) -> bool {
        end <= start && self.source[end as usize..start as usize].trim().is_empty()
    }

    /// The leading whitespace of the line the span starts on.
    fn line_indent(&self, span: Span) -> &str {
        let start = span.start as usize;
        let line = self.source[..start].rfind('\n').map_or(0, |at| at + 1);
        let prefix = &self.source[line..start];
        if prefix.chars().all(|c| c == ' ' || c == '\t') {
            prefix
        } else {
            ""
        }
    }

    /// One indentation step: the shallowest continuation line deeper than the
    /// declaration, relative to it.
    fn indent_unit(&self, filled: &[String], declaration_indent: &str) -> String {
        let mut smallest: Option<(usize, String)> = None;
        for text in filled {
            for line in text.split('\n').skip(1) {
                if line.trim().is_empty() {
                    continue;
                }
                let indent = line.len() - line.trim_start_matches([' ', '\t']).len();
                if indent <= declaration_indent.len() {
                    continue;
                }
                let Some(unit) = line[..indent].strip_prefix(declaration_indent) else {
                    continue;
                };
                if smallest.as_ref().is_none_or(|(at, _)| indent < *at) {
                    smallest = Some((indent, unit.to_owned()));
                }
            }
        }
        smallest.map(|(_, unit)| unit).unwrap_or_else(|| "\t".to_owned())
    }

    fn list(&mut self, statements: &[Statement<'_>]) {
        let mut used_until = 0;
        for (index, statement) in statements.iter().enumerate() {
            if index < used_until {
                continue;
            }
            if let Some(consumed) = self.fold(statements, index, statement) {
                used_until = index + 1 + consumed;
            }
        }
    }

    /// `let $X = […];` or `$X = […];` with a plain identifier target.
    fn array_start<'a>(&self, statement: &'a Statement<'a>) -> Option<(SymbolId, &'a ArrayExpression<'a>, String)> {
        match statement {
            Statement::VariableDeclaration(declaration) => {
                let [declarator] = declaration.declarations.as_slice() else {
                    return None;
                };
                let (BindingPattern::BindingIdentifier(id), Some(Expression::ArrayExpression(array))) = (&declarator.id, &declarator.init) else {
                    return None;
                };
                if declaration.kind == VariableDeclarationKind::Var {
                    return None;
                }
                Some((
                    id.symbol_id(),
                    array,
                    format!("{} {}", declaration.kind.as_str(), id.name),
                ))
            }
            Statement::ExpressionStatement(expression) => {
                let Expression::AssignmentExpression(assignment) = &expression.expression else {
                    return None;
                };
                let (AssignmentTarget::AssignmentTargetIdentifier(target), Expression::ArrayExpression(array)) =
                    (&assignment.left, &assignment.right)
                else {
                    return None;
                };
                if assignment.operator != AssignmentOperator::Assign {
                    return None;
                }
                let symbol = self.scoping.get_reference(target.reference_id.get()?).symbol_id()?;
                Some((symbol, array, target.name.to_string()))
            }
            _ => None,
        }
    }

    fn fold(&mut self, statements: &[Statement<'_>], index: usize, statement: &Statement<'_>) -> Option<usize> {
        let (symbol, array, head) = self.array_start(statement)?;
        let indent = self.line_indent(statement.span()).to_owned();
        let mut elements: Vec<Option<&Expression<'_>>> = Vec::new();
        for element in &array.elements {
            match element {
                ArrayExpressionElement::SpreadElement(_) | ArrayExpressionElement::Elision(_) => return None,
                element => elements.push(element.as_expression()),
            }
        }
        let calls_allowed = self.unclosed(symbol);
        // `at` can take the value without reordering a placeholder or the array.
        let acceptable = |at: usize, value: &Expression<'_>, filled_len: usize, last_index: Option<usize>| {
            last_index.is_none_or(|last| at > last)
                && at <= filled_len
                && (at >= elements.len() || elements[at].is_some_and(|element| expression_is_pure(element)))
                && self.movable(value, symbol, calls_allowed)
        };
        let mut filled: Vec<String> = elements.iter().map(|element| element.map_or(String::new(), |e| self.element_text(e, &indent))).collect();
        let mut first_replaced = None;
        let mut last_index = None;
        let mut count = 0;
        let mut start = statement.span().start;
        // The scratch bindings whose values this run moves into the literal.
        let mut scratch: Vec<SymbolId> = Vec::new();

        // `$T = <value>;\nlet $AR = […, $T, …];` — that element takes the value.
        let mut lookbehind: Option<(usize, SymbolId, String, String, u32)> = None;
        if index > 0 {
            if let Some((temp, value)) = scratch_assignment(&statements[index - 1], self.scoping) {
                let position = elements.iter().position(|element| element.is_some_and(|element| names(element, temp, self.scoping)));
                if let Some(position) = position {
                    // Moving the value into the element keeps its order
                    // relative to the other elements only when they run no code.
                    let others_pure = elements.iter().enumerate().all(|(at, element)| at == position || element.is_some_and(|e| expression_is_pure(e)));
                    if others_pure
                        && self.movable(value, symbol, true)
                        && self.adjacent(statements[index - 1].span().end, statement.span().start)
                    {
                        let text = self.element_text(value, &indent);
                        let original = filled[position].clone();
                        filled[position] = text.clone();
                        lookbehind = Some((position, temp, original, text, statements[index - 1].span().start));
                    }
                }
            }
        }

        loop {
            let Some(later) = statements.get(index + 1 + count) else {
                break;
            };
            // `$AR[k] = value;`
            if let Some((at, value)) = store(later, symbol, self.scoping) {
                if !acceptable(at, value, filled.len(), last_index) {
                    break;
                }
                self.fill(&mut filled, at, self.element_text(value, &indent));
                first_replaced.get_or_insert(at);
                last_index = Some(at);
                count += 1;
                continue;
            }
            // `$T = value;\n$AR[k] = $T;`
            let Some((temp, value)) = scratch_assignment(later, self.scoping) else {
                break;
            };
            let Some(next) = statements.get(index + 2 + count) else {
                break;
            };
            let Some((at, stored)) = store(next, symbol, self.scoping) else {
                break;
            };
            if !names(stored, temp, self.scoping)
                || !self.adjacent(later.span().end, next.span().start)
                || !acceptable(at, value, filled.len(), last_index)
            {
                break;
            }
            self.fill(&mut filled, at, self.element_text(value, &indent));
            first_replaced.get_or_insert(at);
            last_index = Some(at);
            scratch.push(temp);
            count += 2;
        }

        let first = first_replaced?;
        // Everything the stored values now run before has no effect.
        if elements.iter().skip(first).any(|element| !element.is_some_and(|e| expression_is_pure(e))) {
            return None;
        }
        let end = statements[index + count].span().end;
        // The value a scratch binding holds after the run must be overwritten
        // before it is read; otherwise its assignment has to stay.
        if let Some((position, temp, original, text, previous)) = lookbehind {
            if filled[position] != text {
                // A store replaced that element; the assignment stays put.
            } else if self.dead_after(temp, end) {
                scratch.push(temp);
                start = previous;
            } else {
                filled[position] = original;
            }
        }
        if scratch.iter().any(|symbol| !self.dead_after(*symbol, end)) {
            return None;
        }
        let multiline = filled.iter().any(|e| e.contains('\n'));
        // A template literal's newlines are content, not layout.
        if multiline && filled.iter().any(|e| e.contains('`') && e.contains('\n')) {
            return None;
        }
        let one_line = filled.join(", ");
        let literal = if one_line.len() + indent.len() * 4 > WIDTH || multiline {
            let inner = format!("{indent}{}", self.indent_unit(&filled, &indent));
            let body = filled.iter().map(|e| reindent(e, &inner)).collect::<Vec<_>>().join(",\n");
            format!("[\n{body}\n{indent}]")
        } else {
            format!("[{one_line}]")
        };
        self.edits.push(analysis::Edit::replace(
            Span::new(start, end),
            format!("{head} = {literal};"),
        ));
        self.folded += 1;
        Some(count)
    }

    fn fill(&self, filled: &mut Vec<String>, at: usize, text: String) {
        if at == filled.len() {
            filled.push(text);
        } else {
            filled[at] = text;
        }
    }

    /// The element as written, a `RegExp` of literals as a regex literal, on
    /// one line if it fits one inside an array at `indent`.
    fn element_text(&self, expression: &Expression<'_>, indent: &str) -> String {
        if let Some(regex) = regexp_literal(self.scoping, expression) {
            return regex;
        }
        let span = expression.span();
        let text = self.text(span);
        if self.on_one_line(span) || !self.joinable(expression) || self.has_comment(span) {
            return text.to_string();
        }
        let joined = join_lines(text);
        if joined.len() + (indent.len() + 1) * 4 > WIDTH {
            return text.to_string();
        }
        joined
    }

    fn on_one_line(&self, span: Span) -> bool {
        !self.text(span).contains('\n')
    }

    fn has_comment(&self, span: Span) -> bool {
        let at = self.comments.partition_point(|comment| comment.span.start < span.start);
        self.comments.get(at).is_some_and(|comment| comment.span.start < span.end)
    }

    /// Only the object and array literals of `expression` break lines.
    fn joinable(&self, expression: &Expression<'_>) -> bool {
        match expression {
            Expression::ObjectExpression(object) => object.properties.iter().all(|property| match property {
                ObjectPropertyKind::ObjectProperty(property) => self.on_one_line(property.key.span()) && self.joinable(&property.value),
                ObjectPropertyKind::SpreadProperty(spread) => self.on_one_line(spread.span),
            }),
            Expression::ArrayExpression(array) => array.elements.iter().all(|element| match element.as_expression() {
                Some(element) => self.joinable(element),
                None => self.on_one_line(element.span()),
            }),
            value => self.on_one_line(value.span()),
        }
    }

    /// The value held at `end` is not read before it is written again.
    fn dead_after(&self, symbol: SymbolId, end: u32) -> bool {
        let mut references: Vec<(u32, bool)> = self
            .scoping
            .get_resolved_references(symbol)
            .map(|reference| (self.nodes.get_node(reference.node_id()).span().start, reference.is_read()))
            .filter(|(start, _)| *start >= end)
            .collect();
        references.sort_unstable_by_key(|(start, _)| *start);
        references.first().is_none_or(|(_, read)| !read)
    }

    /// `value` does not name the array and, unless no function names the
    /// array, makes no call (a literal `RegExp` aside).
    fn movable(&self, value: &Expression<'_>, symbol: SymbolId, calls_allowed: bool) -> bool {
        struct Check<'s> {
            scoping: &'s Scoping,
            symbol: SymbolId,
            calls_allowed: bool,
            ok: bool,
        }
        impl<'b> Visit<'b> for Check<'_> {
            fn visit_identifier_reference(&mut self, name: &IdentifierReference<'b>) {
                if self.scoping.get_reference(name.reference_id()).symbol_id() == Some(self.symbol) {
                    self.ok = false;
                }
            }
            fn visit_call_expression(&mut self, call: &CallExpression<'b>) {
                self.ok &= self.calls_allowed;
                walk::walk_call_expression(self, call);
            }
            fn visit_new_expression(&mut self, new: &NewExpression<'b>) {
                self.ok &= self.calls_allowed;
                walk::walk_new_expression(self, new);
            }
            fn visit_tagged_template_expression(&mut self, tagged: &TaggedTemplateExpression<'b>) {
                self.ok &= self.calls_allowed;
                walk::walk_tagged_template_expression(self, tagged);
            }
            fn visit_function(&mut self, _: &Function<'b>, _: oxc_syntax::scope::ScopeFlags) {}
            fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'b>) {}
        }
        if regexp_literal(self.scoping, value).is_some() {
            return true;
        }
        let mut check = Check { scoping: self.scoping, symbol, calls_allowed, ok: true };
        check.visit_expression(value);
        check.ok
    }

    /// Every reference to `symbol` is in the function that declares it.
    fn unclosed(&self, symbol: SymbolId) -> bool {
        super::super::support::unclosed(self.scoping, self.nodes, symbol)
    }
}

/// The lines of a literal as one, spaced like `{ a: 1 }` and `[1, 2]`.
fn join_lines(text: &str) -> String {
    let mut joined = String::with_capacity(text.len());
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if !joined.is_empty() && !joined.ends_with('[') && !line.starts_with(']') {
            joined.push(' ');
        }
        joined.push_str(line);
    }
    joined
}

/// Place `text` at `inner`, shifting its continuation lines by the indentation
/// of its shallowest one (its closing delimiter, in practice).
fn reindent(text: &str, inner: &str) -> String {
    let mut lines = text.split('\n');
    let first = lines.next().unwrap_or_default();
    let rest: Vec<&str> = lines.collect();
    if rest.is_empty() {
        return format!("{inner}{first}");
    }
    let base = rest
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start_matches([' ', '\t']).len())
        .min()
        .unwrap_or(0);
    let mut out = String::with_capacity(text.len() + inner.len() * (rest.len() + 1));
    out.push_str(inner);
    out.push_str(first);
    for line in rest {
        out.push('\n');
        if line.trim().is_empty() {
            continue;
        }
        let indent = line.len() - line.trim_start_matches([' ', '\t']).len();
        out.push_str(inner);
        out.push_str(&line[base.min(indent)..]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_the_array_in_its_literal() {
        let source = "function f(e, b, g) {\n\tlet h = [!!e.ctrlKey, void 0, void 0, void 0];\n\th[1] = !!e.metaKey;\n\th[2] = !!e.altKey;\n\th[3] = b.getParameter(1);\n\treturn h;\n}\n";
        assert_eq!(
            fold_array_fills(source).unwrap().0,
            "function f(e, b, g) {\n\tlet h = [!!e.ctrlKey, !!e.metaKey, !!e.altKey, b.getParameter(1)];\n\treturn h;\n}\n"
        );
    }

    #[test]
    fn appends_and_writes_regexps_as_literals() {
        let source = "function f() {\n\tlet q = [RegExp(\"(?:)\"), void 0, void 0];\n\tq[1] = RegExp(\"^a\\\\/b\", \"i\");\n\tq[2] = RegExp(\"c\");\n\tq[3] = RegExp(\"d\", \"i\");\n\tg(() => q);\n}\n";
        assert_eq!(
            fold_array_fills(source).unwrap().0,
            "function f() {\n\tlet q = [/(?:)/, /^a\\/b/i, /c/, /d/i];\n\tg(() => q);\n}\n"
        );
    }

    #[test]
    fn keeps_fills_that_would_reorder_or_read_the_array() {
        // A call while a closure sees the array; a value naming the array;
        // an index out of order; a gap past the end.
        let source = "function f(b) {\n\tlet a = [1, void 0];\n\ta[1] = b.get();\n\tg(() => a);\n\tlet c = [1, void 0];\n\tc[1] = c;\n\tlet d = [1, void 0, void 0];\n\td[2] = 2;\n\td[1] = 1;\n\tlet e = [1];\n\te[3] = 2;\n\treturn [c, d, e];\n}\n";
        let (code, n) = fold_array_fills(source).unwrap();
        // `d` folds its first store only.
        assert_eq!(n, 1, "{code}");
        assert!(code.contains("let d = [1, void 0, 2];\n\td[1] = 1;"), "{code}");
    }

    #[test]
    fn folds_values_stored_through_a_reused_scratch_binding() {
        // The shape a reused tracer binding leaves behind: the value is
        // written to a scratch binding and then stored in the array.
        let source = "function f() {\n\tlet t;\n\tt = { x: 1 };\n\tlet a = [t];\n\tt = { x: 2 };\n\ta[1] = t;\n\tt = { x: 3 };\n\ta[2] = t;\n\tt = { x: 4 };\n\treturn a;\n}\n";
        assert_eq!(
            fold_array_fills(source).unwrap().0,
            "function f() {\n\tlet t;\n\tlet a = [{ x: 1 }, { x: 2 }, { x: 3 }];\n\tt = { x: 4 };\n\treturn a;\n}\n"
        );
    }

    #[test]
    fn folds_after_an_already_inline_prefix() {
        let source = "function f() {\n\tlet t;\n\tlet a = [{ x: 0 }];\n\tt = { x: 1 };\n\ta[1] = t;\n\tt = { x: 2 };\n\ta[2] = t;\n\tt = { x: 3 };\n\treturn [a, t];\n}\n";
        assert_eq!(
            fold_array_fills(source).unwrap().0,
            "function f() {\n\tlet t;\n\tlet a = [{ x: 0 }, { x: 1 }, { x: 2 }];\n\tt = { x: 3 };\n\treturn [a, t];\n}\n"
        );
    }

    #[test]
    fn folds_a_scratch_element_before_the_fill_run() {
        // The tracer's shared `handler2` shape: one element is a scratch read,
        // the rest are already inline, and the stores start after them.
        let source = "function f() {\n\tlet t;\n\tt = { x: 2 };\n\tlet a = [{ x: 1 }, t, { x: 3 }];\n\tt = { x: 4 };\n\ta[3] = t;\n\tt = { x: 5 };\n\treturn a;\n}\n";
        assert_eq!(
            fold_array_fills(source).unwrap().0,
            "function f() {\n\tlet t;\n\tlet a = [{ x: 1 }, { x: 2 }, { x: 3 }, { x: 4 }];\n\tt = { x: 5 };\n\treturn a;\n}\n"
        );
    }

    #[test]
    fn keeps_a_scratch_element_among_effectful_ones() {
        // The value before the array runs before `g()`; inside the literal it
        // would run after it, so the element stays a read.
        let source = "function f() {\n\tlet t;\n\tt = { x: 2 };\n\tlet a = [{ x: 1 }, g(), t];\n\tt = { x: 4 };\n\ta[3] = t;\n\tt = { x: 5 };\n\treturn a;\n}\n";
        let (code, _) = fold_array_fills(source).unwrap();
        // The store at index 3 is pure and may still fold; the element that
        // would move before `g()` stays a read.
        assert!(code.contains("[{ x: 1 }, g(), t, { x: 4 }]"), "{code}");
    }

    #[test]
    fn folds_a_fill_into_an_assigned_array_literal() {
        // The registry tables are assigned to a binding declared elsewhere.
        let source = "function f() {\n\tlet t;\n\tlet a;\n\ta = [{ x: 0 }];\n\tt = { x: 1 };\n\ta[1] = t;\n\tt = { x: 2 };\n\treturn [a, t];\n}\n";
        assert_eq!(
            fold_array_fills(source).unwrap().0,
            "function f() {\n\tlet t;\n\tlet a;\n\ta = [{ x: 0 }, { x: 1 }];\n\tt = { x: 2 };\n\treturn [a, t];\n}\n"
        );
    }

    #[test]
    fn keeps_a_fill_whose_scratch_value_is_read_after_it() {
        // Dropping the last `t = …; a[1] = t;` pair would leave `t` holding
        // the previous value, which `use(t)` reads.
        let source = "function f() {\n\tlet t;\n\tlet a = [];\n\tt = { x: 1 };\n\ta[0] = t;\n\tt = { x: 2 };\n\ta[1] = t;\n\tuse(t);\n\treturn a;\n}\n";
        assert_eq!(fold_array_fills(source).unwrap().0, source);
    }

    #[test]
    fn keeps_a_pair_with_a_comment_between_it() {
        let source = "function f() {\n\tlet t;\n\tlet a = [];\n\tt = { x: 1 };\n\t// keep\n\ta[0] = t;\n\treturn a;\n}\n";
        assert_eq!(fold_array_fills(source).unwrap().0, source);
    }

    #[test]
    fn indents_multiline_elements_to_the_file_style() {
        let tabs = "function f() {\n\tlet t;\n\tt = {\n\t\tx() {\n\t\t\treturn 1;\n\t\t}\n\t};\n\tlet a = [t];\n\tt = {\n\t\tx() {\n\t\t\treturn 2;\n\t\t}\n\t};\n\ta[1] = t;\n\tt = { x: 3 };\n\treturn a;\n}\n";
        assert_eq!(
            fold_array_fills(tabs).unwrap().0,
            "function f() {\n\tlet t;\n\tlet a = [\n\t\t{\n\t\t\tx() {\n\t\t\t\treturn 1;\n\t\t\t}\n\t\t},\n\t\t{\n\t\t\tx() {\n\t\t\t\treturn 2;\n\t\t\t}\n\t\t}\n\t];\n\tt = { x: 3 };\n\treturn a;\n}\n"
        );

        let spaces = "function f() {\n    let t;\n    t = {\n      x() {\n        return 1;\n      }\n    };\n    let a = [t];\n    t = {\n      x() {\n        return 2;\n      }\n    };\n    a[1] = t;\n    t = { x: 3 };\n    return a;\n}\n";
        assert_eq!(
            fold_array_fills(spaces).unwrap().0,
            "function f() {\n    let t;\n    let a = [\n      {\n        x() {\n          return 1;\n        }\n      },\n      {\n        x() {\n          return 2;\n        }\n      }\n    ];\n    t = { x: 3 };\n    return a;\n}\n"
        );
    }

    #[test]
    fn writes_a_stored_literal_like_the_flat_elements_beside_it() {
        let flat = "{ ctor: null, priority: false }, ".repeat(4);
        let source = format!("function f(w) {{\n\tlet t;\n\tlet a = [{flat}{{ ctor: null, priority: false }}];\n\tt = {{\n\t\tctor: w,\n\t\tpriority: false\n\t}};\n\ta[5] = t;\n\ta[6] = {{\n\t\tctor: null,\n\t\tpriority: [\n\t\t\t1,\n\t\t\t2,\n\t\t\t3\n\t\t]\n\t}};\n\treturn a;\n}}\n");
        let element = "\t\t{ ctor: null, priority: false },\n".repeat(5);
        assert_eq!(
            fold_array_fills(&source).unwrap().0,
            format!("function f(w) {{\n\tlet t;\n\tlet a = [\n{element}\t\t{{ ctor: w, priority: false }},\n\t\t{{ ctor: null, priority: [1, 2, 3] }}\n\t];\n\treturn a;\n}}\n")
        );
    }

    #[test]
    fn leaves_the_inside_of_a_string_alone_when_joining() {
        let source = "function f() {\n\tlet a = [void 0];\n\ta[0] = {\n\t\tx: \"a   b\",\n\t\ty: \"[\"\n\t};\n\treturn a;\n}\n";
        assert_eq!(
            fold_array_fills(source).unwrap().0,
            "function f() {\n\tlet a = [{ x: \"a   b\", y: \"[\" }];\n\treturn a;\n}\n"
        );
    }

    #[test]
    fn keeps_the_lines_of_a_literal_it_cannot_join() {
        let comment = "function f() {\n\tlet a = [void 0];\n\ta[0] = {\n\t\t// why\n\t\tx: 1\n\t};\n\treturn a;\n}\n";
        assert!(fold_array_fills(comment).unwrap().0.contains("// why\n\t\t\tx: 1\n"));

        let wide = format!("function f() {{\n\tlet a = [void 0];\n\ta[0] = {{\n\t\tx: \"{0}\",\n\t\ty: 1\n\t}};\n\treturn a;\n}}\n", "z".repeat(120));
        assert!(fold_array_fills(&wide).unwrap().0.contains("\",\n\t\t\ty: 1\n"));
    }

    #[test]
    fn keeps_a_template_literal_whose_newlines_are_content() {
        let source = "function f() {\n\tlet a = [void 0];\n\ta[0] = `x\ny`;\n\treturn a;\n}\n";
        assert_eq!(fold_array_fills(source).unwrap().0, source);
    }
}
