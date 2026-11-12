//! Spell literal property names plainly: `o["key"]` → `o.key`,
//! `{ ["key"]: v }` → `{ key: v }`.
//!
//! Both spellings are the same operation, so this changes nothing but text:
//! only the brackets are rewritten. A name that is not an ASCII identifier
//! keeps its brackets (`o["data-x"]`), except a canonical index, `o["0"]` →
//! `o[0]`, and an object key, which keeps its quotes (`"data-x": v`). A
//! number literal's member keeps its brackets (`1.toString` does not parse).
//! An object's `["__proto__"]` key stays computed: written plainly it would
//! set the prototype instead of an own property.
//!
//! This is the last stage: earlier passes recognise code by its bracketed
//! spelling.

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_span::{GetSpan, Span};

use crate::analysis;

pub(crate) fn dotted_properties(source: &str) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let mut scan = Scan { source, edits: Vec::new() };
    scan.visit_program(&program);
    let count = scan.edits.len();
    if count == 0 {
        return Ok((source.into(), 0));
    }
    let output = super::super::support::apply_checked(source, scan.edits, "dotted property")?;
    Ok((output, count))
}

/// `[A-Za-z_$][A-Za-z0-9_$]*`: reserved words included, which are valid
/// property names.
fn identifier_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c == '$' || c == '_' || c.is_ascii_alphabetic()) && chars.all(|c| c == '$' || c == '_' || c.is_ascii_alphanumeric())
}

/// `"0"`, `"12"`: the string a small non-negative integer prints as.
fn canonical_index(name: &str) -> bool {
    !name.is_empty() && name.len() <= 15 && name.bytes().all(|b| b.is_ascii_digit()) && (name == "0" || !name.starts_with('0'))
}

struct Scan<'s> {
    source: &'s str,
    edits: Vec<analysis::Edit>,
}

impl<'a> Visit<'a> for Scan<'_> {
    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'a>) {
        walk::walk_computed_member_expression(self, member);
        let Expression::StringLiteral(key) = &member.expression else {
            return;
        };
        let name = key.value.as_str();
        // `["k"]` or `?.["k"]`, from the `[` before the key: an object in
        // parentheses ends before its `)`, which stays.
        let Some(open) = self.source[..key.span.start as usize].rfind('[') else {
            return;
        };
        if !self.source[open + 1..key.span.start as usize].trim().is_empty() {
            return;
        }
        let start = if member.optional {
            let before = self.source[..open].trim_end();
            if !before.ends_with("?.") {
                return;
            }
            before.len() - 2
        } else {
            open
        };
        let brackets = Span::new(start as u32, member.span.end);
        let written = &self.source[brackets.start as usize..brackets.end as usize];
        if !written.ends_with(']') || (start as u32) < member.object.span().end {
            return;
        }
        let link = if member.optional { "?." } else { "" };
        if identifier_name(name) && !matches!(member.object.get_inner_expression(), Expression::NumericLiteral(_) | Expression::BigIntLiteral(_)) {
            let dot = if member.optional { "?." } else { "." };
            self.edits.push(analysis::Edit::replace(brackets, format!("{dot}{name}")));
        } else if canonical_index(name) {
            self.edits.push(analysis::Edit::replace(brackets, format!("{link}[{name}]")));
        }
    }

    fn visit_object_property(&mut self, property: &ObjectProperty<'a>) {
        walk::walk_object_property(self, property);
        if !property.computed || property.kind != PropertyKind::Init || property.method || property.shorthand {
            return;
        }
        let Some(Expression::StringLiteral(key)) = property.key.as_expression() else {
            return;
        };
        let name = key.value.as_str();
        if name == "__proto__" {
            return;
        }
        // `["k"]`: from the property's start to just past the key's `]`.
        let rest = &self.source[key.span.end as usize..property.span.end as usize];
        let Some(close) = rest.find(']') else {
            return;
        };
        if !rest[..close].trim().is_empty() || !self.source[property.span.start as usize..key.span.start as usize].trim().eq("[") {
            return;
        }
        let brackets = Span::new(property.span.start, key.span.end + close as u32 + 1);
        let plain = if identifier_name(name) { name.to_string() } else { self.source[key.span.start as usize..key.span.end as usize].to_string() };
        self.edits.push(analysis::Edit::replace(brackets, plain));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spells_literal_names_plainly() {
        let source = "let o = { [\"a\"]: 1, [\"data-x\"]: 2, [\"0\"]: 3, [\"__proto__\"]: 4, [\"default\"]: 5 };\no[\"b\"][\"c\"] = o?.[\"d\"];\nf(o[\"data-x\"], o[\"0\"], o[\"01\"], 1[\"toString\"](), o[\"default\"], (a ? \"\" : b)[\"length\"], (a)?.[\"x\"]);\n";
        let (code, _) = dotted_properties(source).unwrap();
        assert_eq!(code, "let o = { a: 1, \"data-x\": 2, \"0\": 3, [\"__proto__\"]: 4, default: 5 };\no.b.c = o?.d;\nf(o[\"data-x\"], o[0], o[\"01\"], 1[\"toString\"](), o.default, (a ? \"\" : b).length, (a)?.x);\n");
    }
}
