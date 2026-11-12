//! Remove unused values under the emitted VM's standard-intrinsics policy.
//! Unknown member reads still evaluate: getters, proxies, and throws matter.

use std::collections::{BTreeMap, BTreeSet};

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{Scoping, SymbolId};
use oxc_span::GetSpan;

use crate::analysis;

type Path = Vec<String>;

fn unwrap<'s, 'a>(mut expression: &'s Expression<'a>) -> &'s Expression<'a> {
    while let Expression::ParenthesizedExpression(p) = expression {
        expression = &p.expression;
    }
    expression
}

fn member<'s, 'a>(expression: &'s Expression<'a>) -> Option<(&'s Expression<'a>, &'s str)> {
    match unwrap(expression) {
        Expression::StaticMemberExpression(m) if !m.optional => {
            Some((&m.object, m.property.name.as_str()))
        }
        Expression::ComputedMemberExpression(m) if !m.optional => match unwrap(&m.expression) {
            Expression::StringLiteral(key) => Some((&m.object, key.value.as_str())),
            _ => None,
        },
        _ => None,
    }
}

fn path(
    expression: &Expression<'_>,
    scoping: &Scoping,
    aliases: &BTreeMap<SymbolId, Path>,
) -> Option<Path> {
    if let Some((object, key)) = member(expression) {
        let mut path = path(object, scoping, aliases)?;
        path.push(key.into());
        return Some(path);
    }
    let Expression::Identifier(id) = unwrap(expression) else {
        return None;
    };
    if let Some(symbol) = analysis::identifier_symbol(id, scoping) {
        return aliases.get(&symbol).cloned();
    }
    match id.name.as_str() {
        "host" => Some(vec!["host".into()]),
        "Object" | "Array" | "String" => Some(vec!["host".into(), id.name.to_string()]),
        _ => None,
    }
}

struct Facts<'s> {
    scoping: &'s Scoping,
    aliases: BTreeMap<SymbolId, Path>,
    invalid: BTreeSet<Path>,
    dynamic: bool,
}

impl<'a> Visit<'a> for Facts<'_> {
    fn visit_variable_declarator(&mut self, binding: &VariableDeclarator<'a>) {
        if let (BindingPattern::BindingIdentifier(id), Some(init)) = (&binding.id, &binding.init) {
            if let (Some(symbol), Some(value)) =
                (id.symbol_id.get(), path(init, self.scoping, &self.aliases))
            {
                if self.aliases.get(&symbol).is_some_and(|old| old != &value) {
                    self.invalid.insert(vec!["host".into()]);
                }
                self.aliases.insert(symbol, value);
            }
        }
        walk::walk_variable_declarator(self, binding);
    }
    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        match &assignment.left {
            AssignmentTarget::AssignmentTargetIdentifier(id) => {
                if let Some(symbol) = analysis::identifier_symbol(id, self.scoping) {
                    if let Some(value) = path(&assignment.right, self.scoping, &self.aliases) {
                        if self.aliases.get(&symbol).is_some_and(|old| old != &value) {
                            self.invalid.insert(vec!["host".into()]);
                        }
                        self.aliases.insert(symbol, value);
                    }
                }
            }
            AssignmentTarget::StaticMemberExpression(m) => {
                if let Some(mut target) = path(&m.object, self.scoping, &self.aliases) {
                    target.push(m.property.name.to_string());
                    self.invalid.insert(target);
                }
            }
            AssignmentTarget::ComputedMemberExpression(m) => {
                if let Some(mut target) = path(&m.object, self.scoping, &self.aliases) {
                    if let Expression::StringLiteral(key) = unwrap(&m.expression) {
                        target.push(key.value.to_string());
                    }
                    self.invalid.insert(target);
                }
            }
            _ => {
                self.invalid.insert(vec!["host".into()]);
            }
        }
        walk::walk_assignment_expression(self, assignment);
    }
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        self.dynamic |= id.name == "eval";
        if matches!(id.name.as_str(), "host" | "Object" | "Array" | "String")
            && analysis::identifier_symbol(id, self.scoping).is_none()
            && id
                .reference_id
                .get()
                .is_some_and(|id| self.scoping.get_reference(id).is_write())
        {
            self.invalid.insert(vec!["host".into()]);
        }
    }
    fn visit_unary_expression(&mut self, expression: &UnaryExpression<'a>) {
        if expression.operator == oxc_syntax::operator::UnaryOperator::Delete {
            self.invalid.insert(vec!["host".into()]);
        }
        walk::walk_unary_expression(self, expression);
    }
    fn visit_update_expression(&mut self, expression: &UpdateExpression<'a>) {
        // Rare in generated code; decline rather than miss a native alias or
        // computed-member update in a customization-heavy input.
        self.invalid.insert(vec!["host".into()]);
        walk::walk_update_expression(self, expression);
    }
    fn visit_assignment_target(&mut self, target: &AssignmentTarget<'a>) {
        match target {
            AssignmentTarget::StaticMemberExpression(m) => {
                if let Some(mut target) = path(&m.object, self.scoping, &self.aliases) {
                    target.push(m.property.name.to_string());
                    self.invalid.insert(target);
                }
            }
            AssignmentTarget::ComputedMemberExpression(m) => {
                if let Some(mut target) = path(&m.object, self.scoping, &self.aliases) {
                    if let Expression::StringLiteral(key) = unwrap(&m.expression) {
                        target.push(key.value.to_string());
                    }
                    self.invalid.insert(target);
                }
            }
            AssignmentTarget::AssignmentTargetIdentifier(_) => {}
            _ => {
                self.invalid.insert(vec!["host".into()]);
            }
        }
        walk::walk_assignment_target(self, target);
    }
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if member(&call.callee).is_some_and(|(_, name)| {
            matches!(
                name,
                "defineProperty" | "defineProperties" | "setPrototypeOf"
            )
        }) || path(&call.callee, self.scoping, &self.aliases).is_some_and(|p| {
            p.last().is_some_and(|name| {
                matches!(
                    name.as_str(),
                    "defineProperty" | "defineProperties" | "setPrototypeOf"
                )
            })
        }) {
            if let Some(target) = call
                .arguments
                .first()
                .filter(|a| !a.is_spread())
                .and_then(|a| path(a.to_expression(), self.scoping, &self.aliases))
            {
                // Any descriptor/prototype mutation of a known intrinsic
                // invalidates that receiver, including dynamic property keys.
                self.invalid.insert(target);
            }
        }
        // `defineProperty.call(this, target, …)` / `.apply(this, [target, …])`.
        if let Some((function, method)) = member(&call.callee) {
            let mutator = |expression: &Expression<'_>| {
                member(expression).is_some_and(|(_, name)| matches!(name, "defineProperty" | "defineProperties" | "setPrototypeOf"))
                    || path(expression, self.scoping, &self.aliases).is_some_and(|p| {
                        p.last().is_some_and(|name| matches!(name.as_str(), "defineProperty" | "defineProperties" | "setPrototypeOf"))
                    })
            };
            if matches!(method, "call" | "apply") && mutator(function) {
                let target = match method {
                    "call" => call.arguments.get(1).filter(|a| !a.is_spread()).map(|a| a.to_expression()),
                    _ => match call.arguments.get(1) {
                        Some(Argument::ArrayExpression(array)) => array.elements.first().and_then(|e| e.as_expression()),
                        _ => None,
                    },
                };
                match target.and_then(|t| path(t, self.scoping, &self.aliases)) {
                    Some(target) => {
                        self.invalid.insert(target);
                    }
                    // Unknown receiver: any intrinsic may have changed.
                    None => {
                        self.invalid.insert(vec!["host".into()]);
                    }
                }
            }
        }
        walk::walk_call_expression(self, call);
    }
    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.dynamic = true;
        walk::walk_with_statement(self, statement);
    }
}

fn native_read(expression: &Expression<'_>, facts: &Facts<'_>) -> bool {
    let target = if let Some((object, key)) = member(expression) {
        match unwrap(object) {
            Expression::ObjectExpression(object)
                if object.properties.is_empty()
                    && matches!(
                        key,
                        "hasOwnProperty"
                            | "toString"
                            | "valueOf"
                            | "propertyIsEnumerable"
                            | "isPrototypeOf"
                    ) =>
            {
                Some(vec![
                    "host".into(),
                    "Object".into(),
                    "prototype".into(),
                    key.into(),
                ])
            }
            Expression::StringLiteral(_)
                if matches!(
                    key,
                    "charCodeAt" | "charAt" | "slice" | "substring" | "toString" | "valueOf"
                ) =>
            {
                Some(vec![
                    "host".into(),
                    "String".into(),
                    "prototype".into(),
                    key.into(),
                ])
            }
            _ => path(expression, facts.scoping, &BTreeMap::new()),
        }
    } else {
        path(expression, facts.scoping, &BTreeMap::new())
    };
    let Some(target) = target else {
        return false;
    };
    let known = match target
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["host", "Object" | "Array" | "String"] => true,
        ["host", "Object", method] => matches!(
            *method,
            "keys"
                | "values"
                | "entries"
                | "create"
                | "assign"
                | "defineProperty"
                | "getOwnPropertyDescriptor"
                | "getOwnPropertyNames"
                | "getOwnPropertySymbols"
                | "getPrototypeOf"
                | "freeze"
                | "seal"
        ),
        ["host", "Array", method] => matches!(*method, "isArray" | "from" | "of"),
        ["host", "Object", "prototype", name] => matches!(
            *name,
            "hasOwnProperty" | "toString" | "valueOf" | "propertyIsEnumerable" | "isPrototypeOf"
        ),
        ["host", "String", "prototype", name] => matches!(
            *name,
            "charCodeAt" | "charAt" | "slice" | "substring" | "toString" | "valueOf"
        ),
        _ => false,
    };
    known
        && !facts
            .invalid
            .iter()
            .any(|invalid| target.starts_with(invalid))
}

pub(crate) fn remove_discarded_values(source: &str) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, _) = analysis::analyze(&program);
    let mut facts = Facts {
        scoping: &scoping,
        aliases: BTreeMap::new(),
        invalid: BTreeSet::new(),
        dynamic: false,
    };
    // A second walk resolves aliases declared later than a nested function.
    facts.visit_program(&program);
    facts.visit_program(&program);
    if facts.dynamic {
        return Ok((source.into(), 0));
    }
    struct Scan<'s, 't> {
        facts: &'s Facts<'t>,
        edits: Vec<analysis::Edit>,
        list_statements: BTreeSet<(u32, u32)>,
    }
    impl<'a> Visit<'a> for Scan<'_, '_> {
        fn visit_statements(&mut self, statements: &oxc_allocator::Vec<'a, Statement<'a>>) {
            for s in statements {
                self.list_statements.insert((s.span().start, s.span().end));
            }
            walk::walk_statements(self, statements);
        }
        fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
            if arrow.expression {
                if let Some(Statement::ExpressionStatement(s)) = arrow.body.statements.first() {
                    self.visit_expression(&s.expression);
                }
            } else {
                walk::walk_arrow_function_expression(self, arrow);
            }
        }
        fn visit_expression_statement(&mut self, statement: &ExpressionStatement<'a>) {
            if super::super::purity::expression_is_pure(&statement.expression)
                || native_read(&statement.expression, self.facts)
            {
                self.edits.push(analysis::Edit::replace(
                    statement.span,
                    if self
                        .list_statements
                        .contains(&(statement.span.start, statement.span.end))
                    {
                        ""
                    } else {
                        ";"
                    },
                ));
            } else {
                walk::walk_expression_statement(self, statement);
            }
        }
    }
    let mut scan = Scan {
        facts: &facts,
        edits: Vec::new(),
        list_statements: BTreeSet::new(),
    };
    scan.visit_program(&program);
    let removed = scan.edits.len();
    let applied = analysis::apply_edits(source, scan.edits);
    if !applied.rejected.is_empty() {
        return Err("discarded-value edits overlap".into());
    }
    Ok((applied.output, removed))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn run(source: &str) -> String {
        let output = std::process::Command::new("node")
            .args([
                "-e",
                &format!("globalThis.host=globalThis;{source};console.log(JSON.stringify(f()));"),
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }
    #[test]
    fn implicit_returns_and_required_statement_bodies_survive() {
        for source in [
            "function f(){const get=()=>1;return get();}",
            "function f(){if(true) 1;else 2;return 3;}",
            "function f(){let i=0;while(i++<2) 1;return i;}",
        ] {
            let (code, _) = remove_discarded_values(source).unwrap();
            assert_eq!(run(source), run(&code), "{code}");
        }
    }
    #[test]
    fn drops_discarded_native_loads_but_keeps_live_calls() {
        let source="function f(){host['Object']['keys'];host['Array']['isArray'];({})['hasOwnProperty'];return [host.Array.isArray([]),host.Object.keys({a:1})];}";
        let (code, count) = remove_discarded_values(source).unwrap();
        assert_eq!(count, 3, "{code}");
        assert_eq!(run(source), run(&code));
        assert!(code.contains("host.Object.keys({a:1})"));
    }
    #[test]
    fn preserves_unknown_getters_throws_and_shadowed_hosts() {
        for source in [
            "function f(){const log=[];const obj={get value(){log.push('get');return 1}};obj.value;return log;}",
            "function f(){try{null.value;return 'lost throw';}catch(e){return e.name;}}",
            "function f(){const log=[];const host={get Object(){log.push('Object');return {get keys(){log.push('keys');return 1}}}};host.Object.keys;return log;}",
            "function f(){const log=[];({get hasOwnProperty(){log.push('own');return 1}}).hasOwnProperty;return log;}",
        ] {let (code,count)=remove_discarded_values(source).unwrap();assert_eq!(count,0,"{code}");assert_eq!(run(source),run(&code));}
    }
    #[test]
    fn native_descriptor_customization_and_aliases_keep_getter_reads() {
        for source in [
            "function f(){const log=[],old=Object.getOwnPropertyDescriptor(Object,'keys');Object.defineProperty(host.Object,'keys',{configurable:true,get(){log.push('keys');return 1;}});host.Object.keys;Object.defineProperty(Object,'keys',old);return log;}",
            "function f(){const log=[],old=Object.getOwnPropertyDescriptor(Object.prototype,'hasOwnProperty');Object.defineProperty(Object.prototype,'hasOwnProperty',{configurable:true,get(){log.push('own');return 1;}});({}).hasOwnProperty;Object.defineProperty(Object.prototype,'hasOwnProperty',old);return log;}",
            "function f(){const log=[],old=Object.getOwnPropertyDescriptor(Object,'keys');const O=host.Object,D=Object.defineProperty;D(O,'keys',{configurable:true,get(){log.push('alias');return 1;}});host.Object.keys;D(O,'keys',old);return log;}",
            "function f(){const log=[],old=Object.getOwnPropertyDescriptor(Object,'keys');function change(){Object.defineProperty(O,'keys',{configurable:true,get(){log.push('late');return 1;}});}const O=host.Object;change();host.Object.keys;Object.defineProperty(Object,'keys',old);return log;}",
        ] {let (code,count)=remove_discarded_values(source).unwrap();assert_eq!(count,0,"{code}");assert_eq!(run(source),run(&code));}
    }
    #[test]
    fn declines_dynamic_scope_and_dynamic_or_loop_target_mutations() {
        for source in [
            "function f(k){host.Object[k]=1;host.Object.keys;}",
            "function f(k){delete host.Object[k];host.Object.keys;}",
            "function f(xs){for(host.Object.keys of xs){} host.Object.keys;}",
            "function f(xs){[host.Object.keys]=xs;host.Object.keys;}",
            "function f(){host.Object.keys;eval('0');}",
            // Through `.call` / `.apply` the mutator's target is an argument.
            "function f(){Object.defineProperty.call(Object,host.Object,'keys',{value:1});host.Object.keys;}",
            "function f(){Object.defineProperty.apply(Object,[host.Object,'keys',{value:1}]);host.Object.keys;}",
            "function f(t){Object.defineProperty.call(Object,t(),'keys',{value:1});host.Object.keys;}",
        ] {
            assert_eq!(remove_discarded_values(source).unwrap().1, 0, "{source}");
        }
    }
}
