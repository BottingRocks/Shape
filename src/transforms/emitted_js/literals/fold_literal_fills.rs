//! Fold the stores that fill a fresh object literal back into the literal.
//!
//! Program mode creates a plain object, then assigns every property:
//!
//! ```js
//! let $OBJECT_3637_31 = {
//!   ["onAfterGet"]: void 0,
//!   ["originals"]: void 0
//! };
//! $OBJECT_3637_31["onAfterGet"] = $SSA_1;
//! $OBJECT_3637_31["originals"] = {};
//! ```
//!
//! The literal's own values are all `void 0`, so replacing one with a later
//! store's value cannot move an effect; the only ordering that matters is the
//! relative order of the store values, which is preserved. The pass rewrites
//! the literal to carry the stores' values and deletes the store statements.
//!
//! The tracer also builds literals through an assignment to an SSA binding
//! (`$SSA = { ["emoji"]: void 0 }; $SSA["emoji"] = …;`) instead of a `let`.
//! The same folds apply there: the assignment target starts the literal, and
//! the reads that would observe it before the first fill (its own target
//! aside) keep it an assignment plus stores.
//!
//! An empty array literal `let $X = [];` filled by `$X[0] = a; $X[1] = b;`
//! (a dense run from index 0, possibly after unrelated statements) becomes
//! `let $X = [a, b];` at the first fill: allocating `[]` has no effect, so
//! moving it down past statements that never mention `$X` changes nothing,
//! and the values keep their order. Index stores on a fresh array assume the
//! standard Array prototype, like the rest of the program cleanup. An empty
//! object literal `let $X = {};` filled by `$X["a"] = a;` stores is moved
//! down the same way, stopping at a repeated key or `__proto__` (whose
//! store runs the inherited setter while a literal key would not).
//!
//! It declines when any of that stops being provable: a store value reads the
//! object being built; the store sequence would have to reorder values (an
//! appended key before a replaced one, or replaced keys out of literal order);
//! a statement sits between the literal and a store; a key is `__proto__`
//! (assignment runs the inherited setter, a computed literal key does not); a
//! key is not a string literal; the literal has a spread or a `void 0`-free
//! value; the region has a comment; or the file has dynamic scope.

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{Scoping, SymbolId};
use oxc_span::{GetSpan, Span};

use crate::analysis;
use super::super::support::whole_line;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiteralFillResult {
    pub code: String,
    pub folded_stores: usize,
}

/// Fold post-construction property stores into a fresh object literal.
pub fn fold_literal_fills(source: &str) -> Result<LiteralFillResult, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut scan = Scan {
        source,
        scoping: &scoping,
        nodes: &nodes,
        edits: Vec::new(),
        folded: 0,
        dynamic: false,
    };
    scan.visit_program(&program);
    if scan.dynamic || scan.edits.is_empty() {
        return Ok(LiteralFillResult {
            code: source.into(),
            folded_stores: 0,
        });
    }
    scan.edits
        .sort_by_key(|edit| (edit.start, std::cmp::Reverse(edit.end)));
    let mut end = 0;
    scan.edits.retain(|edit| {
        if edit.start < end {
            return false;
        }
        end = edit.end;
        true
    });
    let applied = analysis::apply_edits(source, scan.edits);
    if !applied.rejected.is_empty() {
        return Err("literal fill edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|error| format!("literal fill output: {error}"))?;
    Ok(LiteralFillResult {
        code: applied.output,
        folded_stores: scan.folded,
    })
}

struct Store {
    statement: Span,
    key: String,
    /// The key's string literal as written, quotes and escapes included.
    key_text: Span,
    value: Span,
    /// A comma expression, whose parentheses the parser drops from `value`.
    sequence: bool,
}

struct Scan<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s oxc_semantic::AstNodes<'a>,
    edits: Vec<analysis::Edit>,
    folded: usize,
    dynamic: bool,
}

/// The statement that creates a fresh literal: a declaration or an
/// assignment to a plain identifier.
struct LiteralStart<'a> {
    symbol: SymbolId,
    init: &'a Expression<'a>,
    /// `let $X` / `const $X`, or `$X` for an assignment.
    head: String,
    /// The whole statement, for the delete when the literal moves.
    statement: Span,
    /// The assignment target, which is not a read of the fresh value.
    assigned_target: Option<Span>,
}

impl Scan<'_, '_> {
    fn body(&mut self, statements: &[Statement<'_>], indent: &str) {
        for (position, statement) in statements.iter().enumerate() {
            if let Some(edits) = self.array_fill(statements, position) {
                if !edits.iter().any(|edit| {
                    self.edits
                        .iter()
                        .any(|other| edit.start < other.end && other.start < edit.end)
                }) {
                    self.edits.extend(edits);
                }
                continue;
            }
            let Some((symbol, literal)) = self.fresh_literal(statement) else {
                continue;
            };
            let properties = self.literal_properties(literal);
            let Some(properties) = properties else {
                continue;
            };
            let stores = self.stores(&statements[position + 1..], symbol);
            if stores.is_empty() || !self.order_preserved(&properties, &stores) {
                continue;
            }
            let Some(edits) = self.rewrite(literal, &properties, &stores, indent) else {
                continue;
            };
            if edits.iter().any(|edit| {
                self.edits
                    .iter()
                    .any(|other| edit.start < other.end && other.start < edit.end)
            }) {
                continue;
            }
            self.folded += stores.len();
            self.edits.extend(edits);
        }
    }

    /// A literal this statement starts: `let $X = …` or `$X = …` with a plain
    /// identifier target, followed by the stores that fill it.
    fn array_fill(&mut self, statements: &[Statement<'_>], position: usize) -> Option<Vec<analysis::Edit>> {
        let fresh = self.literal_start(&statements[position])?;
        // `[]`, a pre-sized `[void 0, …]` whose placeholders the stores fill
        // in order, or `[a, b]` that the very next stores extend from its
        // length (the literal stays where it is, so `a` and `b` still run
        // first); `{}` and `{ k: a }` likewise.
        let (object, placeholders, mut values) = match fresh.init {
            Expression::ArrayExpression(array)
                if array.elements.iter().all(|element| element.as_expression().is_some_and(is_void_zero)) =>
            {
                (false, array.elements.len(), Vec::new())
            }
            Expression::ArrayExpression(array)
                if array.elements.iter().all(|element| {
                    element.as_expression().is_some_and(|value| !is_void_zero(value))
                }) =>
            {
                let texts: Vec<String> = array
                    .elements
                    .iter()
                    .map(|element| self.text(element.span()).to_owned())
                    .collect();
                (false, 0, texts)
            }
            Expression::ObjectExpression(object) if object.properties.is_empty() => (true, 0, Vec::new()),
            // `{ k: a }` of plain data properties that the very next stores
            // extend: defining a key in the literal stores what assigning it
            // after would (no `__proto__` prototype to hold a setter).
            Expression::ObjectExpression(object)
                if object.properties.iter().all(|property| match property {
                    ObjectPropertyKind::ObjectProperty(property) => {
                        property.kind == PropertyKind::Init
                            && !property.method
                            && !is_void_zero(&property.value)
                            && (property.computed || property_key(&property.key, false).is_some_and(|key| key != "__proto__"))
                    }
                    ObjectPropertyKind::SpreadProperty(_) => false,
                }) =>
            {
                let texts: Vec<String> = object
                    .properties
                    .iter()
                    .map(|property| self.text(property.span()).to_owned())
                    .collect();
                (true, 0, texts)
            }
            _ => return None,
        };
        let extending = values.len();
        if has_comment(self.source, fresh.statement) {
            return None;
        }
        let symbol = fresh.symbol;
        // The first fill, and the first reference of `$X` that could observe
        // it before that fill. The literal's own assignment is not a read.
        let first = statements[position + 1..]
            .iter()
            .position(|statement| {
                if object {
                    self.key_store(statement, symbol, &[]).is_some()
                } else {
                    self.index_store(statement, symbol, extending).is_some()
                }
            })?
            + position
            + 1;
        if extending > 0 && first != position + 1 {
            return None;
        }
        let first_start = statements[first].span().start;
        if self.scoping.get_resolved_references(symbol).any(|reference| {
            let span = self.nodes.kind(reference.node_id()).span();
            fresh.statement.start <= span.start && span.start < first_start && Some(span) != fresh.assigned_target
        }) {
            return None;
        }
        // A store to a key the literal already has stays a store.
        let mut keys: Vec<String> = match fresh.init {
            Expression::ObjectExpression(literal) => literal
                .properties
                .iter()
                .filter_map(|property| match property {
                    ObjectPropertyKind::ObjectProperty(property) => property_key(&property.key, property.computed),
                    ObjectPropertyKind::SpreadProperty(_) => None,
                })
                .map(|key| format!("\"{key}\""))
                .collect(),
            _ => Vec::new(),
        };
        let mut last = first;
        for (offset, statement) in statements[first..].iter().enumerate() {
            let fill = if object {
                self.key_store(statement, symbol, &keys).map(|(key, value)| {
                    keys.push(key.clone());
                    format!("[{key}]: {value}")
                })
            } else {
                self.index_store(statement, symbol, values.len())
            };
            match fill {
                Some(value) if !has_comment(self.source, statement.span()) => {
                    values.push(value);
                    last = first + offset;
                }
                _ => break,
            }
        }
        if values.len() == extending {
            return None;
        }
        let replacement = if object {
            format!("{} = {{ {} }};", fresh.head, values.join(", "))
        } else {
            // Placeholders past the last store stay.
            while values.len() < placeholders {
                values.push("void 0".to_string());
            }
            format!("{} = [{}];", fresh.head, values.join(", "))
        };
        self.folded += values.len() - extending;
        Some(vec![
            analysis::Edit::delete(whole_line(self.source, fresh.statement)),
            analysis::Edit::replace(
                Span::new(first_start, statements[last].span().end),
                replacement,
            ),
        ])
    }

    /// `$X[index] = value;` where `value` does not read `$X`: its text.
    fn index_store(&self, statement: &Statement<'_>, symbol: SymbolId, index: usize) -> Option<String> {
        let Statement::ExpressionStatement(expression) = statement else {
            return None;
        };
        let Expression::AssignmentExpression(assignment) = &expression.expression else {
            return None;
        };
        if assignment.operator != oxc_syntax::operator::AssignmentOperator::Assign {
            return None;
        }
        let AssignmentTarget::ComputedMemberExpression(member) = &assignment.left else {
            return None;
        };
        let (Expression::Identifier(object), Expression::NumericLiteral(key)) = (&member.object, &member.expression)
        else {
            return None;
        };
        if member.optional
            || analysis::identifier_symbol(object, self.scoping) != Some(symbol)
            || key.value != index as f64
            || self.reads_symbol(&assignment.right, symbol)
        {
            return None;
        }
        let text = self.text(assignment.right.span());
        Some(if matches!(assignment.right, Expression::SequenceExpression(_)) {
            format!("({text})")
        } else {
            text.to_owned()
        })
    }

    /// `$X["key"] = value;` with a new string key that is not `__proto__`
    /// and a value that does not read `$X`: the key's source text and the
    /// value's text.
    fn key_store(&self, statement: &Statement<'_>, symbol: SymbolId, seen: &[String]) -> Option<(String, String)> {
        let Statement::ExpressionStatement(expression) = statement else {
            return None;
        };
        let Expression::AssignmentExpression(assignment) = &expression.expression else {
            return None;
        };
        if assignment.operator != oxc_syntax::operator::AssignmentOperator::Assign {
            return None;
        }
        let AssignmentTarget::ComputedMemberExpression(member) = &assignment.left else {
            return None;
        };
        let (Expression::Identifier(object), Expression::StringLiteral(key)) = (&member.object, &member.expression)
        else {
            return None;
        };
        let key_text = self.text(key.span).to_owned();
        if member.optional
            || analysis::identifier_symbol(object, self.scoping) != Some(symbol)
            || key.value == "__proto__"
            || seen.contains(&key_text)
            || self.reads_symbol(&assignment.right, symbol)
        {
            return None;
        }
        let text = self.text(assignment.right.span());
        let value = if matches!(assignment.right, Expression::SequenceExpression(_)) {
            format!("({text})")
        } else {
            text.to_owned()
        };
        Some((key_text, value))
    }

    /// `let $X = …;` or `$X = …;` with a plain identifier target.
    fn literal_start<'a>(&self, statement: &'a Statement<'a>) -> Option<LiteralStart<'a>> {
        match statement {
            Statement::VariableDeclaration(declaration) => {
                if declaration.declarations.len() != 1 || declaration.kind == VariableDeclarationKind::Var {
                    return None;
                }
                let declarator = &declaration.declarations[0];
                let BindingPattern::BindingIdentifier(binding) = &declarator.id else {
                    return None;
                };
                Some(LiteralStart {
                    symbol: binding.symbol_id.get()?,
                    init: declarator.init.as_ref()?,
                    head: format!("{} {}", declaration.kind.as_str(), binding.name),
                    statement: declaration.span,
                    assigned_target: None,
                })
            }
            Statement::ExpressionStatement(expression) => {
                let Expression::AssignmentExpression(assignment) = &expression.expression else {
                    return None;
                };
                if assignment.operator != oxc_syntax::operator::AssignmentOperator::Assign {
                    return None;
                }
                let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
                    return None;
                };
                let symbol = self.scoping.get_reference(target.reference_id.get()?).symbol_id()?;
                Some(LiteralStart {
                    symbol,
                    init: &assignment.right,
                    head: target.name.to_string(),
                    statement: expression.span,
                    assigned_target: Some(target.span),
                })
            }
            _ => None,
        }
    }

    /// `let $X = { … };` or `$X = { … };` with a plain object literal.
    fn fresh_literal<'a>(&self, statement: &'a Statement<'a>) -> Option<(SymbolId, &'a ObjectExpression<'a>)> {
        let start = self.literal_start(statement)?;
        let Expression::ObjectExpression(literal) = start.init else {
            return None;
        };
        Some((start.symbol, literal))
    }

    /// Every property is `key: void 0`; returns the keys in order.
    fn literal_properties(&self, literal: &ObjectExpression<'_>) -> Option<Vec<String>> {
        let mut keys = Vec::new();
        for property in &literal.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                return None;
            };
            if property.shorthand || property.method || !is_void_zero(&property.value) {
                return None;
            }
            keys.push(property_key(&property.key, property.computed)?);
        }
        Some(keys)
    }

    /// The run of `$X["k"] = value;` statements directly after the literal.
    fn stores(&self, rest: &[Statement<'_>], symbol: SymbolId) -> Vec<Store> {
        let mut stores = Vec::new();
        for statement in rest {
            let Statement::ExpressionStatement(expression) = statement else {
                break;
            };
            let Expression::AssignmentExpression(assignment) = &expression.expression else {
                break;
            };
            if assignment.operator != oxc_syntax::operator::AssignmentOperator::Assign {
                break;
            }
            let AssignmentTarget::ComputedMemberExpression(member) = &assignment.left else {
                break;
            };
            if member.optional {
                break;
            }
            let Expression::Identifier(object) = &member.object else {
                break;
            };
            if analysis::identifier_symbol(object, self.scoping) != Some(symbol) {
                break;
            }
            let Expression::StringLiteral(key_literal) = &member.expression else {
                break;
            };
            let key = key_literal.value.as_str();
            if key == "__proto__" || self.reads_symbol(&assignment.right, symbol) {
                break;
            }
            if has_comment(self.source, statement.span()) {
                break;
            }
            stores.push(Store {
                statement: statement.span(),
                key: key.to_owned(),
                key_text: key_literal.span,
                value: assignment.right.span(),
                sequence: matches!(assignment.right, Expression::SequenceExpression(_)),
            });
        }
        stores
    }

    fn reads_symbol(&self, expression: &Expression<'_>, symbol: SymbolId) -> bool {
        let mut reads = SymbolReads {
            scoping: self.scoping,
            symbol,
            found: false,
        };
        reads.visit_expression(expression);
        reads.found
    }

    /// Replaced keys keep their literal order and every appended key comes
    /// after them, so the store values keep their evaluation order.
    fn order_preserved(&self, properties: &[String], stores: &[Store]) -> bool {
        let mut appended = false;
        let mut previous = None;
        for store in stores {
            match properties.iter().position(|key| *key == store.key) {
                Some(position) => {
                    if appended || previous.is_some_and(|previous| previous >= position) {
                        return false;
                    }
                    previous = Some(position);
                }
                None => appended = true,
            }
        }
        true
    }

    fn rewrite(
        &self,
        literal: &ObjectExpression<'_>,
        properties: &[String],
        stores: &[Store],
        indent: &str,
    ) -> Option<Vec<analysis::Edit>> {
        let close = literal.span.end.checked_sub(1)? as usize;
        if self.source.as_bytes().get(close) != Some(&b'}') {
            return None;
        }
        let property_indent = format!("{indent}  ");
        let mut edits = Vec::new();
        let mut appended = String::new();
        for store in stores {
            match literal
                .properties
                .iter()
                .filter_map(|property| match property {
                    ObjectPropertyKind::ObjectProperty(property) => Some(property),
                    ObjectPropertyKind::SpreadProperty(_) => None,
                })
                .filter(|property| {
                    property_key(&property.key, property.computed)
                        .is_some_and(|key| key == store.key)
                })
                .last()
                .map(|property| property.value.span())
            {
                Some(value) => edits.push(analysis::Edit::replace(value, self.value_text(store))),
                None => {
                    if !appended.is_empty() || !properties.is_empty() {
                        appended.push(',');
                    }
                    appended.push('\n');
                    appended.push_str(&property_indent);
                    appended.push_str(&format!(
                        "[{}]: {}",
                        self.text(store.key_text),
                        self.value_text(store)
                    ));
                }
            }
        }
        if !appended.is_empty() {
            edits.push(analysis::Edit::replace(
                Span::new(close as u32, close as u32),
                appended,
            ));
        }
        for store in stores {
            edits.push(analysis::Edit::delete(whole_line(
                self.source,
                store.statement,
            )));
        }
        Some(edits)
    }

    fn text(&self, span: Span) -> &str {
        &self.source[span.start as usize..span.end as usize]
    }

    /// A property value cannot be a bare comma expression.
    fn value_text(&self, store: &Store) -> String {
        let text = self.text(store.value);
        if store.sequence {
            format!("({text})")
        } else {
            text.to_owned()
        }
    }

    fn indent_at(&self, span: Span) -> String {
        let start = span.start as usize;
        let line = self.source[..start]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        let indent = &self.source[line..start];
        if indent.trim().is_empty() {
            indent.to_owned()
        } else {
            String::new()
        }
    }
}

impl<'a> Visit<'a> for Scan<'_, '_> {
    fn visit_function_body(&mut self, body: &FunctionBody<'a>) {
        let indent = self.indent_at(body.span) + "  ";
        self.body(&body.statements, &indent);
        walk::walk_function_body(self, body);
    }

    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        let indent = self.indent_at(block.span) + "  ";
        self.body(&block.body, &indent);
        walk::walk_block_statement(self, block);
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if identifier.name == "eval" {
            self.dynamic = true;
        }
    }

    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.dynamic = true;
        walk::walk_with_statement(self, statement);
    }
}

struct SymbolReads<'s> {
    scoping: &'s Scoping,
    symbol: SymbolId,
    found: bool,
}

impl<'a> Visit<'a> for SymbolReads<'_> {
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if analysis::identifier_symbol(identifier, self.scoping) == Some(self.symbol) {
            self.found = true;
        }
    }
}

fn property_key(key: &PropertyKey<'_>, computed: bool) -> Option<String> {
    if computed {
        return match key {
            PropertyKey::StringLiteral(literal) => Some(literal.value.as_str().to_owned()),
            _ => None,
        };
    }
    match key {
        PropertyKey::StringLiteral(literal) => Some(literal.value.as_str().to_owned()),
        PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.to_string()),
        _ => None,
    }
}

fn is_void_zero(expression: &Expression<'_>) -> bool {
    let mut expression = expression;
    while let Expression::ParenthesizedExpression(inner) = expression {
        expression = &inner.expression;
    }
    matches!(expression, Expression::UnaryExpression(unary)
        if unary.operator == oxc_syntax::operator::UnaryOperator::Void
            && matches!(&unary.argument, Expression::NumericLiteral(number) if number.value == 0.0))
}

fn has_comment(source: &str, span: Span) -> bool {
    let text = &source[span.start as usize..span.end as usize];
    text.contains("//") || text.contains("/*")
}

#[cfg(test)]
mod tests {
    use super::fold_literal_fills;

    #[test]
    fn folds_stores_into_a_void_zero_literal() {
        let source = "function f() {\n  let $OBJECT_1 = {\n    [\"a\"]: void 0,\n    [\"b\"]: void 0\n  };\n  $OBJECT_1[\"a\"] = $KEY_1;\n  $OBJECT_1[\"b\"] = {};\n  return $OBJECT_1;\n}";
        let result = fold_literal_fills(source).unwrap();
        assert_eq!(result.folded_stores, 2);
        assert!(
            result.code.contains("[\"a\"]: $KEY_1,") && result.code.contains("[\"b\"]: {}"),
            "{}",
            result.code
        );
        assert!(result.code.contains("[\"b\"]: {}"));
        assert!(!result.code.contains("$OBJECT_1[\"a\"]"));
    }

    #[test]
    fn folds_into_an_empty_literal_and_appends_new_keys() {
        let source = "function f() {\n  let $CARRY_1 = {};\n  $CARRY_1[\"args\"] = arguments;\n  $CARRY_1[\"thisObj\"] = this;\n  $CARRY_1[\"result\"] = null;\n  return $CARRY_1;\n}";
        let result = fold_literal_fills(source).unwrap();
        assert_eq!(result.folded_stores, 3);
        assert!(result.code.contains("[\"args\"]: arguments"));
        assert!(result.code.contains("[\"result\"]: null"));
        assert!(!result.code.contains("$CARRY_1[\"args\"]"));
    }

    #[test]
    fn keeps_comma_values_and_escaped_keys() {
        let source = "function f() {\n  let $O_1 = {\n    [\"a\"]: void 0\n  };\n  $O_1[\"a\"] = ((0, x[\"a\"]));\n  $O_1[\"q\\\"\\\\\"] = (0, y);\n  return $O_1;\n}";
        let result = fold_literal_fills(source).unwrap();
        assert_eq!(result.folded_stores, 2);
        assert!(result.code.contains("[\"a\"]: (0, x[\"a\"])"), "{}", result.code);
        assert!(result.code.contains("[\"q\\\"\\\\\"]: (0, y)"), "{}", result.code);
    }

    #[test]
    fn folds_index_fills_into_a_fresh_array() {
        let source = "function f(a, b) {\n  let $SSA_1 = [];\n  var $KEY_4 = a;\n  $SSA_1[0] = a;\n  $SSA_1[1] = (0, b);\n  return $SSA_1;\n}";
        let result = fold_literal_fills(source).unwrap();
        assert_eq!(result.folded_stores, 2, "{}", result.code);
        assert!(result.code.contains("var $KEY_4 = a;\n  let $SSA_1 = [a, (0, b)];\n  return $SSA_1;"), "{}", result.code);
        for source in [
            // read before the fills
            "function f(a) {\n  let $SSA_1 = [];\n  g($SSA_1);\n  $SSA_1[0] = a;\n}",
            // not dense from 0
            "function f(a) {\n  let $SSA_1 = [];\n  $SSA_1[1] = a;\n}",
            // the value reads the array
            "function f(a) {\n  let $SSA_1 = [];\n  $SSA_1[0] = $SSA_1;\n}",
        ] {
            let result = fold_literal_fills(source).unwrap();
            assert_eq!(result.folded_stores, 0, "{source} -> {}", result.code);
        }
    }

    #[test]
    fn extends_an_array_literal_with_the_next_stores() {
        let folded = fold_literal_fills("function f(v, g) {\n  let a = [[]];\n  a[1] = v;\n  a[2] = 3;\n  g(a);\n}").unwrap();
        assert!(folded.code.contains("let a = [[], v, 3];"), "{}", folded.code);
        assert_eq!(folded.folded_stores, 2);
        // a statement in between: the literal's own values must not move
        let kept = fold_literal_fills("function f(v, g) {\n  let a = [g()];\n  g();\n  a[1] = v;\n  g(a);\n}").unwrap();
        assert_eq!(kept.folded_stores, 0, "{}", kept.code);
    }

    #[test]
    fn extends_an_object_literal_with_the_next_stores() {
        let folded = fold_literal_fills("function f(v, g) {\n  let o = { [\"a\"]: g() };\n  o[\"b\"] = v;\n  g(o);\n}").unwrap();
        assert!(folded.code.contains("let o = { [\"a\"]: g(), [\"b\"]: v };"), "{}", folded.code);
        // a prototype from the literal could hold a setter for the store
        let kept = fold_literal_fills("function f(v, g) {\n  let o = { __proto__: g() };\n  o[\"b\"] = v;\n  g(o);\n}").unwrap();
        assert_eq!(kept.folded_stores, 0, "{}", kept.code);
    }

    #[test]
    fn fills_a_presized_array() {
        let source = "function f(a) {\n  let $SSA_1 = [void 0, void 0];\n  $SSA_1[0] = [\"POST\"];\n  return g($SSA_1);\n}";
        let result = fold_literal_fills(source).unwrap();
        assert!(result.code.contains("let $SSA_1 = [[\"POST\"], void 0];\n  return g($SSA_1);"), "{}", result.code);
    }

    #[test]
    fn moves_an_empty_object_down_to_its_stores() {
        let source = "function f(a) {\n  let $SSA_1 = {};\n  var $KEY_0 = a;\n  $SSA_1[\"args\"] = $KEY_0;\n  $SSA_1[\"thisObj\"] = this;\n  $SSA_1[\"args\"] = 2;\n  return $SSA_1;\n}";
        let result = fold_literal_fills(source).unwrap();
        assert!(result.code.contains("let $SSA_1 = { [\"args\"]: $KEY_0, [\"thisObj\"]: this };\n  $SSA_1[\"args\"] = 2;"), "{}", result.code);
    }

    #[test]
    fn folds_stores_into_an_assigned_void_zero_literal() {
        let source = "function f() {\n  let $O_1;\n  $O_1 = {\n    [\"emoji\"]: void 0\n  };\n  $O_1[\"emoji\"] = \"x\";\n  $O_1[\"x\"] = 94;\n  return $O_1;\n}";
        let result = fold_literal_fills(source).unwrap();
        assert_eq!(result.folded_stores, 2, "{}", result.code);
        assert!(result.code.contains("[\"emoji\"]: \"x\""), "{}", result.code);
        assert!(result.code.contains("[\"x\"]: 94"), "{}", result.code);
        assert!(!result.code.contains("$O_1[\"emoji\"]"), "{}", result.code);
    }

    #[test]
    fn extends_an_assigned_object_literal_with_the_next_stores() {
        let folded =
            fold_literal_fills("function f(v, g) {\n  let o;\n  o = { [\"a\"]: g() };\n  o[\"b\"] = v;\n  g(o);\n}").unwrap();
        assert!(folded.code.contains("o = { [\"a\"]: g(), [\"b\"]: v };"), "{}", folded.code);
        assert_eq!(folded.folded_stores, 1);
    }

    #[test]
    fn extends_an_assigned_array_literal_with_the_next_stores() {
        let folded =
            fold_literal_fills("function f(v, g) {\n  let a;\n  a = [g()];\n  a[1] = v;\n  g(a);\n}").unwrap();
        assert!(folded.code.contains("a = [g(), v];"), "{}", folded.code);
        assert_eq!(folded.folded_stores, 1);
    }

    #[test]
    fn moves_an_assigned_empty_object_down_to_its_stores() {
        let source = "function f(a) {\n  let $S_1;\n  $S_1 = {};\n  var $K_0 = a;\n  $S_1[\"args\"] = $K_0;\n  return $S_1;\n}";
        let result = fold_literal_fills(source).unwrap();
        assert!(result.code.contains("var $K_0 = a;\n  $S_1 = { [\"args\"]: $K_0 };"), "{}", result.code);
    }

    #[test]
    fn keeps_an_assigned_literal_seen_before_its_stores() {
        for source in [
            // read between the assignment and the fill
            "function f() {\n  let $O_1;\n  $O_1 = {};\n  observe($O_1);\n  $O_1[\"a\"] = 1;\n}",
            // aliased before the fill
            "function f(v) {\n  let $O_1;\n  $O_1 = { [\"a\"]: v };\n  let alias = $O_1;\n  $O_1[\"b\"] = 1;\n}",
        ] {
            let result = fold_literal_fills(source).unwrap();
            assert_eq!(result.folded_stores, 0, "{source} -> {}", result.code);
            assert_eq!(result.code, source);
        }
    }

    #[test]
    fn declines_a_reordering_or_escaping_store() {
        for source in [
            // appended key before a replaced key
            "function f() {\n  let $O_1 = {\n    [\"a\"]: void 0\n  };\n  $O_1[\"b\"] = x();\n  $O_1[\"a\"] = y();\n}",
            // replaced keys out of literal order
            "function f() {\n  let $O_1 = {\n    [\"a\"]: void 0,\n    [\"b\"]: void 0\n  };\n  $O_1[\"b\"] = x();\n  $O_1[\"a\"] = y();\n}",
            // the value reads the object being built
            "function f() {\n  let $O_1 = {};\n  $O_1[\"a\"] = $O_1;\n}",
            // __proto__ runs the inherited setter
            "function f() {\n  let $O_1 = {};\n  $O_1[\"__proto__\"] = x;\n}",
            // the object is seen before its stores
            "function f() {\n  let $O_1 = {};\n  observe($O_1);\n  $O_1[\"a\"] = 1;\n}",
            // a key that is not a string literal
            "function f() {\n  let $O_1 = {};\n  $O_1[key] = 1;\n}",
            // a literal value with an effect
            "function f() {\n  let $O_1 = {\n    [\"a\"]: g()\n  };\n  $O_1[\"a\"] = 1;\n}",
            // dynamic scope
            "function f() {\n  let $O_1 = {};\n  $O_1[\"a\"] = 1;\n  eval('x');\n}",
        ] {
            let result = fold_literal_fills(source).unwrap();
            assert_eq!(result.folded_stores, 0, "{source} -> {}", result.code);
            assert_eq!(result.code, source);
        }
    }
}
