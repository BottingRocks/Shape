//! Reuse branch truth only for tests over immutable lexical values. Strict
//! equality, null equality, typeof and truth testing cannot invoke user code.
//! Branch deletion retains hoisted declarations; value substitution retains
//! reference stripping and anonymous-function name semantics.
use std::collections::{BTreeMap, BTreeSet};

use crate::analysis;
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{Scoping, SymbolId};
use oxc_span::GetSpan;
use oxc_syntax::scope::ScopeFlags;
use crate::transforms::emitted_js::support::unwrap;

pub(crate) fn simplify_branch_facts(source: &str) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, _) = analysis::analyze(&program);
    let stable = scoping
        .symbol_ids()
        .filter(|&s| {
            scoping
                .symbol_flags(s)
                .contains(oxc_syntax::symbol::SymbolFlags::BlockScopedVariable)
                && scoping.symbol_redeclarations(s).is_empty()
                && scoping.get_resolved_references(s).all(|r| !r.is_write())
        })
        .collect();
    let mut scan = Scan {
        source,
        scoping: &scoping,
        stable,
        facts: BTreeMap::new(),
        edits: vec![],
        dynamic: false,
    };
    scan.visit_program(&program);
    if scan.dynamic {
        return Ok((source.into(), 0));
    }
    scan.edits
        .sort_by_key(|e| (e.start, std::cmp::Reverse(e.end)));
    let mut end = 0;
    scan.edits.retain(|e| {
        if e.start < end {
            return false;
        }
        end = e.end;
        true
    });
    let count = scan.edits.len();
    let applied = analysis::apply_edits(source, scan.edits);
    if !applied.rejected.is_empty() {
        return Err("branch fact edits overlap".into());
    }
    Ok((applied.output, count))
}

struct Scan<'s> {
    source: &'s str,
    scoping: &'s Scoping,
    stable: BTreeSet<SymbolId>,
    facts: BTreeMap<String, bool>,
    edits: Vec<analysis::Edit>,
    dynamic: bool,
}
impl Scan<'_> {
    fn key(&self, e: &Expression<'_>) -> Option<String> {
        let text = match unwrap(e) {
            Expression::Identifier(id) => {
                let symbol = analysis::identifier_symbol(id, self.scoping)?;
                if !self.stable.contains(&symbol) {
                    return None;
                }
                format!("id:{symbol:?}")
            }
            Expression::NullLiteral(_) => "null".into(),
            Expression::BooleanLiteral(b) => format!("bool:{}", b.value),
            Expression::NumericLiteral(n) => format!("number:{}", n.value),
            Expression::StringLiteral(s) if !s.lone_surrogates => format!("string:{:?}", s.value),
            Expression::UnaryExpression(u)
                if matches!(
                    u.operator,
                    UnaryOperator::LogicalNot | UnaryOperator::Typeof | UnaryOperator::Void
                ) =>
            {
                format!("({} {})", u.operator.as_str(), self.key(&u.argument)?)
            }
            Expression::BinaryExpression(b)
                if matches!(
                    b.operator,
                    BinaryOperator::StrictEquality | BinaryOperator::StrictInequality
                ) || (matches!(
                    b.operator,
                    BinaryOperator::Equality | BinaryOperator::Inequality
                ) && (matches!(unwrap(&b.left), Expression::NullLiteral(_))
                    || matches!(unwrap(&b.right), Expression::NullLiteral(_)))) =>
            {
                format!(
                    "({} {} {})",
                    b.operator.as_str(),
                    self.key(&b.left)?,
                    self.key(&b.right)?
                )
            }
            _ => return None,
        };
        Some(text)
    }
    fn condition(&self, e: &Expression<'_>) -> Option<(String, bool)> {
        if let Expression::UnaryExpression(u) = unwrap(e) {
            if u.operator == UnaryOperator::LogicalNot {
                let (key, inverse) = self.condition(&u.argument)?;
                return Some((key, !inverse));
            }
        }
        Some((self.key(e)?, false))
    }
    fn known(&self, e: &Expression<'_>) -> Option<bool> {
        if let Expression::BooleanLiteral(b) = unwrap(e) {
            return Some(b.value);
        }
        let (key, inverse) = self.condition(e)?;
        Some(*self.facts.get(&key)? ^ inverse)
    }
    fn assume(&mut self, e: &Expression<'_>, truth: bool) {
        if let Some((key, inverse)) = self.condition(e) {
            self.facts.insert(key, truth ^ inverse);
        }
    }
    fn replace(&mut self, span: oxc_span::Span, replacement: String) {
        let original = &self.source[span.start as usize..span.end as usize];
        if !original.contains("/*") && !original.contains("//") {
            self.edits.push(analysis::Edit::replace(span, replacement));
        }
    }
}

#[derive(Default)]
struct Hoisted(bool);
impl<'a> Visit<'a> for Hoisted {
    fn visit_variable_declaration(&mut self, d: &VariableDeclaration<'a>) {
        self.0 |= d.kind == VariableDeclarationKind::Var;
        walk::walk_variable_declaration(self, d);
    }
    fn visit_function(&mut self, f: &Function<'a>, _: ScopeFlags) {
        self.0 |= f.r#type == FunctionType::FunctionDeclaration;
    }
    fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}
    fn visit_class(&mut self, _: &Class<'a>) {}
}

impl<'a> Visit<'a> for Scan<'_> {
    fn visit_if_statement(&mut self, s: &IfStatement<'a>) {
        self.visit_expression(&s.test);
        if let Some(truth) = self.known(&s.test) {
            let (chosen, discarded) = if truth {
                (Some(&s.consequent), s.alternate.as_ref())
            } else {
                (s.alternate.as_ref(), Some(&s.consequent))
            };
            let mut hoisted = Hoisted::default();
            if let Some(discarded) = discarded {
                hoisted.visit_statement(discarded);
            }
            if !hoisted.0 && !matches!(chosen, Some(Statement::FunctionDeclaration(_))) {
                let text = chosen
                    .map(|c| &self.source[c.span().start as usize..c.span().end as usize])
                    .unwrap_or("");
                self.replace(s.span, format!("{{{text}}}"));
            }
        }
        let saved = self.facts.clone();
        self.assume(&s.test, true);
        self.visit_statement(&s.consequent);
        self.facts = saved.clone();
        self.assume(&s.test, false);
        if let Some(alt) = &s.alternate {
            self.visit_statement(alt);
        }
        self.facts = saved;
    }
    fn visit_conditional_expression(&mut self, c: &ConditionalExpression<'a>) {
        self.visit_expression(&c.test);
        if let Some(truth) = self.known(&c.test) {
            let chosen = if truth { &c.consequent } else { &c.alternate };
            let text = &self.source[chosen.span().start as usize..chosen.span().end as usize];
            let strip_reference = matches!(
                unwrap(chosen),
                Expression::StaticMemberExpression(_)
                    | Expression::ComputedMemberExpression(_)
                    | Expression::PrivateFieldExpression(_)
                    | Expression::ChainExpression(_)
                    | Expression::FunctionExpression(_)
                    | Expression::ArrowFunctionExpression(_)
                    | Expression::ClassExpression(_)
            );
            self.replace(
                c.span,
                if strip_reference {
                    format!("(0, ({text}))")
                } else {
                    format!("({text})")
                },
            );
        }
        let saved = self.facts.clone();
        self.assume(&c.test, true);
        self.visit_expression(&c.consequent);
        self.facts = saved.clone();
        self.assume(&c.test, false);
        self.visit_expression(&c.alternate);
        self.facts = saved;
    }
    fn visit_function(&mut self, f: &Function<'a>, flags: ScopeFlags) {
        let saved = std::mem::take(&mut self.facts);
        walk::walk_function(self, f, flags);
        self.facts = saved;
    }
    fn visit_arrow_function_expression(&mut self, f: &ArrowFunctionExpression<'a>) {
        let saved = std::mem::take(&mut self.facts);
        walk::walk_arrow_function_expression(self, f);
        self.facts = saved;
    }
    fn visit_class(&mut self, c: &Class<'a>) {
        let saved = std::mem::take(&mut self.facts);
        walk::walk_class(self, c);
        self.facts = saved;
    }
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        self.dynamic |= id.name == "eval";
    }
    fn visit_with_statement(&mut self, s: &WithStatement<'a>) {
        self.dynamic = true;
        walk::walk_with_statement(self, s);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn literal_truth_keeps_hoisted_names_and_getvalue_semantics() {
        let (code, count) =
            super::simplify_branch_facts("function f(){if(true){good();}else{bad();}}").unwrap();
        assert_eq!(count, 1, "{code}");
        assert!(!code.contains("bad()"), "{code}");
        let (code, count) =
            super::simplify_branch_facts("function f(){return (true?object.method:other)();}")
                .unwrap();
        assert_eq!(count, 1, "{code}");
        assert!(code.contains("(0, (object.method))"), "{code}");
        let source = "function f(){if(false){var hoisted=1;}return typeof hoisted;}";
        assert_eq!(super::simplify_branch_facts(source).unwrap().1, 0);
    }
    #[test]
    fn immutable_tests_collapse_in_both_arms_and_keep_reference_semantics() {
        for source in [
            "function f(input){const value=input;if(value){return value?1:2;}else{return value?3:4;}}",
            "function f(input){const value=input;return value==null?(value==null?1:2):(value==null?3:4);}",
            "function f(input){const value=input;if(value){if(!value){bad();}else{good();}}}",
            "function f(input,obj){const value=input;if(value){return (value?obj.method:other)();}}",
            "function f(input){const value=input;if(value){const named=value?function(){}:other;return named.name;}}",
        ] {
            let (code, count) = super::simplify_branch_facts(source).unwrap();
            assert!(count > 0, "{code}");
            if source.contains("obj.method") { assert!(code.contains("(0, (obj.method))"), "{code}"); }
            if source.contains("function()") { assert!(code.contains("(0, (function(){}))"), "{code}"); }
        }
    }

    #[test]
    fn retain_mutations_getters_coercions_environments_and_hoisted_names() {
        for source in [
            "function f(input){let value=input;if(value){change();return value?1:2;}function change(){value=0;}}",
            "function f(obj){if(obj.value){return obj.value?1:2;}}",
            "function f(input){const value=input;if(value==1){return value==1?1:2;}}",
            "function f(input){const value=input;if(value<1){return value<1?1:2;}}",
            "function f(input){const value=input;if(value){return ()=>value?1:2;}}",
            "function f(input){const value=input;if(value){if(!value){var hoisted=1;}return typeof hoisted;}}",
            "function f(input){const value=input;if(value){if(!value){function hoisted(){}}return typeof hoisted;}}",
            "function f(input){const value=input;if(value){return eval('value')?value?1:2:3;}}",
            "function f(value){if(value){arguments[0]=0;return value?1:2;}}",
            "function f(input){const value=input;if(value){let value=other;return value?1:2;}}",
        ] {
            let (code, count) = super::simplify_branch_facts(source).unwrap();
            assert_eq!(count, 0, "{code}");
            assert_eq!(code, source);
        }
    }
}
