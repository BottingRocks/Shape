//! Fold the program's fingerprint of the VM's own source into constants.
//!
//! The program reads `vmEntry`, the VM's interpreter function, only to
//! measure its source:
//!
//! ```js
//! let k = vmEntry;
//! let t = Function["prototype"]["call"]["bind"](Function["prototype"]["toString"]);
//! …
//! let s = t(k);
//! let body = s["substring"](s["indexOf"]("{"), s["lastIndexOf"]("}"));
//! x = body["length"];
//! let m = body["match"](r);          // r = RegExp("\\s", "g")
//! y = m == null ? 0 : m["length"];
//! ```
//!
//! That source is the raw script's entry function, which `trace program`
//! has: the chain is evaluated on it, as JavaScript would (UTF-16 units),
//! and each store keeps only its number (`x = 345595;`). The temporaries,
//! the slot and the helper go. Only this chain folds: string values through
//! `substring`/`indexOf`/`lastIndexOf`/`length`, and `match` with a global
//! `RegExp` whose every match is one character from a set this pass knows
//! exactly (a class of literals and `\s`/`\d`/`\w`). Anything else leaves
//! the program as it was.

use std::collections::HashMap;
use std::rc::Rc;

use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_semantic::{AstNodes, NodeId, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};

use crate::analysis;
use super::super::support::whole_line;

/// The edits folding every fingerprint of `vmEntry`, or `None` when some
/// use is not one.
pub(crate) fn fold_entry_fingerprint(source: &str, scoping: &Scoping, nodes: &AstNodes<'_>, raw: &str) -> Option<Vec<analysis::Edit>> {
    let entry: Rc<Vec<u16>> = Rc::new(entry_function_source(raw)?.encode_utf16().collect());
    let reads: Vec<NodeId> = scoping
        .root_unresolved_references()
        .iter()
        .filter(|(name, _)| name.as_str() == "vmEntry")
        .flat_map(|(_, references)| references.iter().map(|&reference| scoping.get_reference(reference).node_id()))
        .collect();
    let [read] = reads.as_slice() else { return None };
    let slot_node = nodes.parent_id(*read);
    let AstKind::VariableDeclarator(declarator) = nodes.kind(slot_node) else { return None };
    let BindingPattern::BindingIdentifier(slot) = &declarator.id else { return None };
    let slot = slot.symbol_id();
    let fold = Fold { source, scoping, nodes, entry, slot };
    let mut edits = Vec::new();
    let mut helpers: HashMap<SymbolId, usize> = HashMap::new();
    for reference in scoping.get_resolved_references(slot) {
        if reference.is_write() {
            return None;
        }
        let (helper, chain) = fold.chain(reference.node_id())?;
        *helpers.entry(helper).or_default() += 1;
        edits.extend(chain);
    }
    if helpers.is_empty() {
        return None;
    }
    // The slot and the helpers only these chains used go.
    edits.push(analysis::Edit::delete(whole_line(source, declaration_of(nodes, slot_node)?)));
    for (helper, uses) in helpers {
        if scoping.get_resolved_references(helper).count() == uses {
            edits.push(analysis::Edit::delete(whole_line(source, declaration_of(nodes, scoping.symbol_declaration(helper))?)));
        }
    }
    Some(edits)
}

/// The raw script's entry function, `function L(…){…}` in
/// `(function L(…){…}(…))`, byte for byte: what
/// `Function.prototype.toString` returns for it.
fn entry_function_source(raw: &str) -> Option<String> {
    let allocator = oxc_allocator::Allocator::default();
    let program = analysis::parse_program(&allocator, raw).ok()?;
    let [Statement::ExpressionStatement(statement)] = program.body.as_slice() else { return None };
    let Expression::CallExpression(call) = statement.expression.get_inner_expression() else { return None };
    let Expression::FunctionExpression(function) = call.callee.get_inner_expression() else { return None };
    Some(raw[function.span.start as usize..function.span.end as usize].to_string())
}

/// A single-declarator `let`/`const`/`var` statement's span, from its
/// declarator's node.
fn declaration_of(nodes: &AstNodes<'_>, declarator: NodeId) -> Option<Span> {
    match nodes.parent_kind(declarator) {
        AstKind::VariableDeclaration(declaration) if declaration.declarations.len() == 1 => Some(declaration.span),
        _ => None,
    }
}

#[derive(Clone)]
enum Value {
    String(Rc<Vec<u16>>),
    Number(f64),
    Null,
    /// A successful global `match`: its array's length.
    Matches(usize),
}

struct Fold<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    entry: Rc<Vec<u16>>,
    slot: SymbolId,
}

impl<'s, 'a> Fold<'s, 'a> {
    fn text(&self, span: Span) -> &'s str {
        &self.source[span.start as usize..span.end as usize]
    }

    fn symbol(&self, expression: &Expression<'_>) -> Option<SymbolId> {
        match expression.get_inner_expression() {
            Expression::Identifier(identifier) => self.scoping.get_reference(identifier.reference_id()).symbol_id(),
            _ => None,
        }
    }

    /// A never-written `Function["prototype"]["call"]["bind"](Function["prototype"]["toString"])`.
    fn to_string_helper(&self, callee: &Expression<'_>) -> Option<SymbolId> {
        let helper = self.symbol(callee)?;
        if self.scoping.get_resolved_references(helper).any(|reference| reference.is_write()) {
            return None;
        }
        let AstKind::VariableDeclarator(definition) = self.nodes.kind(self.scoping.symbol_declaration(helper)) else { return None };
        let init = self.text(definition.init.as_ref()?.span());
        (init == "Function[\"prototype\"][\"call\"][\"bind\"](Function[\"prototype\"][\"toString\"])").then_some(helper)
    }

    /// `let s = t(slot);` and what follows it in its statement list, folded.
    fn chain(&self, read: NodeId) -> Option<(SymbolId, Vec<analysis::Edit>)> {
        let AstKind::CallExpression(call) = self.nodes.parent_kind(read) else { return None };
        let [_] = call.arguments.as_slice() else { return None };
        let helper = self.to_string_helper(&call.callee)?;
        let call_node = self.nodes.parent_id(read);
        let declarator = self.nodes.parent_id(call_node);
        if !matches!(self.nodes.kind(declarator), AstKind::VariableDeclarator(_)) {
            return None;
        }
        let declaration = self.nodes.parent_id(declarator);
        let statements = match self.nodes.parent_kind(declaration) {
            AstKind::BlockStatement(block) => &block.body,
            AstKind::FunctionBody(body) => &body.statements,
            _ => return None,
        };
        let start = statements.iter().position(|statement| statement.span() == self.nodes.get_node(declaration).span())?;
        let mut env: HashMap<SymbolId, Value> = HashMap::new();
        let mut folded: Vec<Span> = Vec::new();
        let mut edits = Vec::new();
        for statement in &statements[start..] {
            let touches = self.touches(&env, statement.span()) || statement.span().start == statements[start].span().start;
            if !touches {
                continue;
            }
            match statement {
                Statement::VariableDeclaration(declaration) => {
                    let [declarator] = declaration.declarations.as_slice() else { return None };
                    let BindingPattern::BindingIdentifier(id) = &declarator.id else { return None };
                    let value = self.eval(declarator.init.as_ref()?, &env)?;
                    env.insert(id.symbol_id(), value);
                    edits.push(analysis::Edit::delete(whole_line(self.source, statement.span())));
                }
                Statement::ExpressionStatement(expression) => {
                    let Expression::AssignmentExpression(assignment) = &expression.expression else { return None };
                    let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else { return None };
                    if assignment.operator != oxc_syntax::operator::AssignmentOperator::Assign
                        || self.scoping.get_reference(target.reference_id()).symbol_id().is_some_and(|symbol| env.contains_key(&symbol))
                    {
                        return None;
                    }
                    let Value::Number(number) = self.eval(&assignment.right, &env)? else { return None };
                    edits.push(analysis::Edit::replace(assignment.right.span(), number_text(number)?));
                }
                _ => return None,
            }
            folded.push(statement.span());
        }
        // Every temporary lives only in the folded statements.
        for &symbol in env.keys() {
            let outside = self.scoping.get_resolved_references(symbol).any(|reference| {
                let span = self.nodes.get_node(reference.node_id()).span();
                !folded.iter().any(|statement| statement.contains_inclusive(span))
            });
            if outside {
                return None;
            }
        }
        Some((helper, edits))
    }

    /// Whether `span` reads any temporary.
    fn touches(&self, env: &HashMap<SymbolId, Value>, span: Span) -> bool {
        env.keys().any(|&symbol| {
            self.scoping.get_resolved_references(symbol).any(|reference| span.contains_inclusive(self.nodes.get_node(reference.node_id()).span()))
        })
    }

    fn eval(&self, expression: &Expression<'_>, env: &HashMap<SymbolId, Value>) -> Option<Value> {
        match expression.get_inner_expression() {
            Expression::StringLiteral(literal) => Some(Value::String(Rc::new(literal.value.encode_utf16().collect()))),
            Expression::NumericLiteral(literal) => Some(Value::Number(literal.value)),
            Expression::Identifier(_) => env.get(&self.symbol(expression)?).cloned(),
            Expression::StaticMemberExpression(member) if member.property.name == "length" => self.length(self.eval(&member.object, env)?),
            Expression::ComputedMemberExpression(member) if matches!(&member.expression, Expression::StringLiteral(key) if key.value == "length") => {
                self.length(self.eval(&member.object, env)?)
            }
            Expression::ConditionalExpression(conditional) => {
                // m == null ? a : b
                let Expression::BinaryExpression(test) = conditional.test.get_inner_expression() else { return None };
                if test.operator != oxc_syntax::operator::BinaryOperator::Equality || !matches!(test.right, Expression::NullLiteral(_)) {
                    return None;
                }
                let null = match self.eval(&test.left, env)? {
                    Value::Null => true,
                    Value::Matches(_) | Value::String(_) | Value::Number(_) => false,
                };
                self.eval(if null { &conditional.consequent } else { &conditional.alternate }, env)
            }
            Expression::CallExpression(call) => self.call(call, env),
            _ => None,
        }
    }

    fn length(&self, value: Value) -> Option<Value> {
        match value {
            Value::String(text) => Some(Value::Number(text.len() as f64)),
            Value::Matches(count) => Some(Value::Number(count as f64)),
            _ => None,
        }
    }

    fn call(&self, call: &CallExpression<'_>, env: &HashMap<SymbolId, Value>) -> Option<Value> {
        if call.optional {
            return None;
        }
        // t(slot): the entry function's source.
        if let ([argument], Expression::Identifier(_)) = (call.arguments.as_slice(), &call.callee) {
            if self.symbol(argument.as_expression()?) == Some(self.slot) && self.to_string_helper(&call.callee).is_some() {
                return Some(Value::String(self.entry.clone()));
            }
            return None;
        }
        let member = call.callee.get_inner_expression().as_member_expression()?;
        let method = match member {
            MemberExpression::StaticMemberExpression(member) => member.property.name.as_str(),
            MemberExpression::ComputedMemberExpression(member) => match &member.expression {
                Expression::StringLiteral(key) => key.value.as_str(),
                _ => return None,
            },
            _ => return None,
        };
        let Value::String(text) = self.eval(member.object(), env)? else { return None };
        let arguments: Vec<&Expression<'_>> = call.arguments.iter().map(Argument::as_expression).collect::<Option<_>>()?;
        match (method, arguments.as_slice()) {
            ("indexOf", [search]) | ("lastIndexOf", [search]) => {
                let Value::String(search) = self.eval(search, env)? else { return None };
                let found = if method == "indexOf" { index_of(&text, &search) } else { last_index_of(&text, &search) };
                Some(Value::Number(found.map_or(-1.0, |at| at as f64)))
            }
            ("substring", [start, end]) => {
                let (Value::Number(start), Value::Number(end)) = (self.eval(start, env)?, self.eval(end, env)?) else { return None };
                let clamp = |value: f64| if value.is_nan() { 0 } else { value.clamp(0.0, text.len() as f64) as usize };
                let (from, to) = (clamp(start), clamp(end));
                let (from, to) = if from > to { (to, from) } else { (from, to) };
                Some(Value::String(Rc::new(text[from..to].to_vec())))
            }
            ("match", [pattern]) => {
                let set = self.regex_set(pattern)?;
                let count = text.iter().filter(|&&unit| set.contains(unit)).count();
                Some(if count == 0 { Value::Null } else { Value::Matches(count) })
            }
            _ => None,
        }
    }

    /// A never-written `RegExp("…", "g")` whose matches are single
    /// characters from a known set.
    fn regex_set(&self, pattern: &Expression<'_>) -> Option<CharSet> {
        let symbol = self.symbol(pattern)?;
        if self.scoping.get_resolved_references(symbol).any(|reference| reference.is_write()) {
            return None;
        }
        let AstKind::VariableDeclarator(definition) = self.nodes.kind(self.scoping.symbol_declaration(symbol)) else { return None };
        let (callee, arguments) = match definition.init.as_ref()?.get_inner_expression() {
            Expression::CallExpression(call) => (&call.callee, &call.arguments),
            Expression::NewExpression(new) => (&new.callee, &new.arguments),
            _ => return None,
        };
        let Expression::Identifier(constructor) = callee else { return None };
        if constructor.name != "RegExp" || self.scoping.get_reference(constructor.reference_id()).symbol_id().is_some() {
            return None;
        }
        let [Argument::StringLiteral(source), Argument::StringLiteral(flags)] = arguments.as_slice() else { return None };
        if flags.value != "g" {
            return None;
        }
        CharSet::parse(&source.value)
    }
}

/// The UTF-16 units one regex character matches.
struct CharSet {
    ranges: Vec<(u16, u16)>,
}

impl CharSet {
    fn contains(&self, unit: u16) -> bool {
        self.ranges.iter().any(|&(low, high)| low <= unit && unit <= high)
    }

    /// `\s`, `\d`, `\w`, one literal character, or a class of those (with
    /// literal ranges); anything else is not known exactly.
    fn parse(pattern: &str) -> Option<Self> {
        let units: Vec<char> = pattern.chars().collect();
        let mut ranges = Vec::new();
        match units.as_slice() {
            ['[', inner @ .., ']'] => {
                if inner.first() == Some(&'^') {
                    return None;
                }
                let mut at = 0;
                while at < inner.len() {
                    let (low, step) = Self::atom(&inner[at..], &mut ranges, true)?;
                    at += step;
                    if let Some(low) = low {
                        // a-b
                        if inner.get(at) == Some(&'-') && at + 1 < inner.len() {
                            let mut ignored = Vec::new();
                            let (Some(high), step) = Self::atom(&inner[at + 1..], &mut ignored, true)? else { return None };
                            if high < low {
                                return None;
                            }
                            ranges.push((low, high));
                            at += 1 + step;
                        } else {
                            ranges.push((low, low));
                        }
                    }
                }
            }
            atom => {
                let (single, step) = Self::atom(atom, &mut ranges, false)?;
                if step != atom.len() {
                    return None;
                }
                if let Some(unit) = single {
                    ranges.push((unit, unit));
                }
            }
        }
        (!ranges.is_empty()).then_some(Self { ranges })
    }

    /// One atom: a literal character (`Some`) or a class escape (pushed to
    /// `ranges`), and how many characters it took.
    fn atom(chars: &[char], ranges: &mut Vec<(u16, u16)>, in_class: bool) -> Option<(Option<u16>, usize)> {
        let first = *chars.first()?;
        if first == '\\' {
            let escaped = *chars.get(1)?;
            match escaped {
                's' => ranges.extend(WHITESPACE),
                'd' => ranges.push((0x30, 0x39)),
                'w' => ranges.extend([(0x30, 0x39), (0x41, 0x5A), (0x5F, 0x5F), (0x61, 0x7A)]),
                't' => return Some((Some(0x09), 2)),
                'n' => return Some((Some(0x0A), 2)),
                'v' => return Some((Some(0x0B), 2)),
                'f' => return Some((Some(0x0C), 2)),
                'r' => return Some((Some(0x0D), 2)),
                // An escaped punctuator is itself.
                c if c.is_ascii_punctuation() => return Some((Some(c as u16), 2)),
                _ => return None,
            }
            return Some((None, 2));
        }
        // Outside a class, a metacharacter is not a literal.
        if !in_class && "^$.*+?()[]{}|/".contains(first) {
            return None;
        }
        if in_class && first == ']' {
            return None;
        }
        let unit = u16::try_from(first as u32).ok()?;
        Some((Some(unit), 1))
    }
}

/// JavaScript's `\s`: white space and line terminators.
const WHITESPACE: [(u16, u16); 10] = [
    (0x09, 0x0D),
    (0x20, 0x20),
    (0xA0, 0xA0),
    (0x1680, 0x1680),
    (0x2000, 0x200A),
    (0x2028, 0x2029),
    (0x202F, 0x202F),
    (0x205F, 0x205F),
    (0x3000, 0x3000),
    (0xFEFF, 0xFEFF),
];

fn index_of(text: &[u16], search: &[u16]) -> Option<usize> {
    if search.is_empty() {
        return Some(0);
    }
    text.windows(search.len()).position(|window| window == search)
}

fn last_index_of(text: &[u16], search: &[u16]) -> Option<usize> {
    if search.is_empty() {
        return Some(text.len());
    }
    text.windows(search.len()).rposition(|window| window == search)
}

/// A whole number's literal; other numbers do not fold.
fn number_text(number: f64) -> Option<String> {
    (number.fract() == 0.0 && number.abs() < 9e15).then(|| format!("{}", number as i64))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fold(source: &str, raw: &str) -> String {
        let allocator = oxc_allocator::Allocator::default();
        let program = analysis::parse_program(&allocator, source).unwrap();
        let (scoping, nodes) = analysis::analyze(&program);
        match fold_entry_fingerprint(source, &scoping, &nodes, raw) {
            Some(edits) => analysis::apply_edits(source, edits).output,
            None => source.to_string(),
        }
    }

    const PROGRAM: &str = "function $THREAD_0_0() {\n\tlet $KEY_320 = RegExp(\"\\\\s\", \"g\");\n\tlet $KEY_406 = RegExp(\"[.{([,;=/]\", \"g\");\n\tlet $KEY_339 = vmEntry;\n\tlet $KEY_266 = Function[\"prototype\"][\"call\"][\"bind\"](Function[\"prototype\"][\"toString\"]);\n\tg(() => {\n\t\tlet $KEY_16 = $KEY_266($KEY_339);\n\t\tlet $KEY_1 = $KEY_16[\"substring\"]($KEY_16[\"indexOf\"](\"{\"), $KEY_16[\"lastIndexOf\"](\"}\"));\n\t\t$KEY_22 = $KEY_1[\"length\"];\n\t\tlet $AR = $KEY_1[\"match\"]($KEY_320);\n\t\t$KEY_10 = $AR == null ? 0 : $AR[\"length\"];\n\t\tlet $AS = $KEY_1[\"match\"]($KEY_406);\n\t\t$KEY_26 = $AS == null ? 0 : $AS[\"length\"];\n\t\th($KEY_22);\n\t});\n}\n";

    #[test]
    fn folds_the_fingerprint_of_the_entry_source() {
        // Body "{var a = 1;\n\treturn a": 21 units, 6 whitespace, 3 of [.{([,;=/].
        let raw = "(function L(s){var a = 1;\n\treturn a}(0))";
        let code = fold(PROGRAM, raw);
        assert_eq!(code, "function $THREAD_0_0() {\n\tlet $KEY_320 = RegExp(\"\\\\s\", \"g\");\n\tlet $KEY_406 = RegExp(\"[.{([,;=/]\", \"g\");\n\tg(() => {\n\t\t$KEY_22 = 21;\n\t\t$KEY_10 = 6;\n\t\t$KEY_26 = 3;\n\t\th($KEY_22);\n\t});\n}\n");
    }

    #[test]
    fn leaves_other_uses_of_the_entry_alone() {
        // The source escapes into a call.
        let escaped = PROGRAM.replace("\t\th($KEY_22);\n", "\t\th($KEY_16);\n");
        assert_eq!(fold(&escaped, "(function L(s){}(0))"), escaped);
        // A pattern the pass cannot count exactly.
        let lookahead = PROGRAM.replace("[.{([,;=/]", "a(?=b)");
        assert_eq!(fold(&lookahead, "(function L(s){}(0))"), lookahead);
    }

    #[test]
    fn character_sets_follow_javascript() {
        let whitespace = CharSet::parse("\\s").unwrap();
        assert!(whitespace.contains(0x20) && whitespace.contains(0xA0) && whitespace.contains(0xFEFF) && !whitespace.contains(0x41));
        let class = CharSet::parse("[.{([,;=/]").unwrap();
        assert!(".{([,;=/".encode_utf16().all(|unit| class.contains(unit)) && !class.contains(u16::from(b'}')));
        assert!(CharSet::parse("[^a]").is_none() && CharSet::parse("a+").is_none());
    }
}
