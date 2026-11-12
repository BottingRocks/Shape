//! Reconstruct contiguous, unobserved dense array writes. Appends use the
//! thread cleanup's ordinary-prototype policy; existing own indices need no
//! prototype assumption. Effectful values preserve their evaluation order
//! and require a receiver that cannot be observed during construction.
use super::super::index::SpanIndex;
use crate::analysis;
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::AssignmentOperator;
use std::collections::BTreeSet;
use crate::transforms::emitted_js::support::unwrap;

pub(crate) fn fold_dense_arrays(
    source: &str,
    protected: &BTreeSet<String>,
    checked_captures: bool,
) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let ordinary = ordinary_prototypes(&program, &scoping);
    #[derive(Default)]
    struct Owners {
        functions: SpanIndex,
        pending: bool,
    }
    impl<'a> Visit<'a> for Owners {
        fn visit_function(&mut self, f: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
            self.functions.push(f.span);
            self.pending |=
                f.id.as_ref()
                    .is_some_and(|id| id.name.starts_with("$THREAD_"))
                    && f.body
                        .as_ref()
                        .is_some_and(|b| b.statements.is_empty() && b.directives.is_empty());
            walk::walk_function(self, f, flags);
        }
        fn visit_arrow_function_expression(&mut self, f: &ArrowFunctionExpression<'a>) {
            self.functions.push(f.span);
            walk::walk_arrow_function_expression(self, f);
        }
        fn visit_class(&mut self, c: &Class<'a>) {
            self.functions.push(c.span);
            walk::walk_class(self, c);
        }
    }
    let mut owners = Owners::default();
    owners.visit_program(&program);
    owners.functions.build();
    let mut scan = Scan {
        source,
        protected,
        scoping: &scoping,
        nodes: &nodes,
        comments: &program.comments,
        ordinary,
        owners: &owners.functions,
        pending: owners.pending,
        checked_captures,
        edits: vec![],
        count: 0,
        dynamic: false,
    };
    scan.visit_program(&program);
    if scan.dynamic || scan.edits.is_empty() {
        return Ok((source.into(), 0));
    }
    let applied = analysis::apply_edits(source, scan.edits);
    if !applied.rejected.is_empty() {
        return Err("dense array edits overlap".into());
    }
    Ok((applied.output, scan.count))
}
fn index(e: &Expression<'_>) -> Option<usize> {
    let Expression::NumericLiteral(n) = unwrap(e) else {
        return None;
    };
    (n.value >= 0.0 && n.value < 65536.0 && n.value.fract() == 0.0).then_some(n.value as usize)
}
struct Scan<'s, 'a> {
    source: &'s str,
    protected: &'s BTreeSet<String>,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    comments: &'s [Comment],
    ordinary: bool,
    owners: &'s SpanIndex,
    pending: bool,
    checked_captures: bool,
    edits: Vec<analysis::Edit>,
    count: usize,
    dynamic: bool,
}
impl Scan<'_, '_> {
    // Sequence spans can omit their parentheses; one array element must
    // remain one value rather than becoming several comma-separated elements.
    fn element_text(&self, expression: &Expression<'_>) -> String {
        let text = self.text(expression.span());
        if matches!(unwrap(expression), Expression::SequenceExpression(_)) {
            format!("({text})")
        } else {
            text.to_owned()
        }
    }
    fn text(&self, span: Span) -> &str {
        &self.source[span.start as usize..span.end as usize]
    }
    fn write<'s, 'a>(
        &self,
        s: &'s Statement<'a>,
        symbol: SymbolId,
    ) -> Option<(usize, Span, &'s Expression<'a>)> {
        let Statement::ExpressionStatement(s) = s else {
            return None;
        };
        let Expression::AssignmentExpression(a) = unwrap(&s.expression) else {
            return None;
        };
        let AssignmentTarget::ComputedMemberExpression(m) = &a.left else {
            return None;
        };
        let Expression::Identifier(receiver) = unwrap(&m.object) else {
            return None;
        };
        if a.operator != AssignmentOperator::Assign
            || m.optional
            || analysis::identifier_symbol(receiver, self.scoping) != Some(symbol)
        {
            return None;
        }
        Some((index(&m.expression)?, receiver.span, &a.right))
    }
    fn body(&mut self, body: &[Statement<'_>], hoisted_functions: bool) {
        for (at, statement) in body.iter().enumerate() {
            let Statement::VariableDeclaration(d) = statement else {
                continue;
            };
            if !matches!(
                d.kind,
                VariableDeclarationKind::Let | VariableDeclarationKind::Const
            ) || d.declarations.len() != 1
            {
                continue;
            }
            let item = &d.declarations[0];
            let (BindingPattern::BindingIdentifier(id), Some(init)) = (&item.id, &item.init) else {
                continue;
            };
            if self.protected.contains(id.name.as_str()) {
                continue;
            }
            let Expression::ArrayExpression(array) = unwrap(init) else {
                continue;
            };
            let Some(symbol) = id.symbol_id.get() else {
                continue;
            };
            if !self.scoping.symbol_redeclarations(symbol).is_empty()
                || self
                    .scoping
                    .get_resolved_references(symbol)
                    .any(|r| r.is_write())
            {
                continue;
            }
            let Some(mut values) = array
                .elements
                .iter()
                .map(|e| e.as_expression().map(|e| self.element_text(e)))
                .collect::<Option<Vec<_>>>()
            else {
                continue;
            };
            let original_len = values.len();
            let inert_initial = array.elements.iter().all(|e| {
                e.as_expression()
                    .is_some_and(super::super::purity::expression_is_pure)
            });
            let private_effects = (!self.pending
                || self.checked_captures
                    && ["$SSA_", "$CARRY_", "$ARRAY_", "$OBJECT_"]
                        .iter()
                        .any(|p| id.name.starts_with(p)))
                && self.scoping.get_resolved_references(symbol).all(|r| {
                    self.owners
                        .enclosing(self.nodes.get_node(r.node_id()).kind().span())
                        == self.owners.enclosing(d.span)
                });
            if !inert_initial && !private_effects {
                continue;
            }
            let mut writes = vec![];
            let mut next = None;
            for s in &body[at + 1..] {
                if hoisted_functions && matches!(s, Statement::FunctionDeclaration(_)) {
                    continue;
                }
                let Some((index, receiver, rhs)) = self.write(s, symbol) else {
                    break;
                };
                if next.is_some_and(|n| index != n)
                    || index > values.len()
                    || (!self.ordinary && index >= original_len)
                    || !inert_initial && index < original_len
                    || !private_effects && !super::super::purity::expression_is_pure(rhs)
                {
                    break;
                }
                let value = self.element_text(rhs);
                if index == values.len() {
                    values.push(value);
                } else {
                    values[index] = value;
                }
                writes.push((s.span(), receiver));
                next = Some(index + 1);
            }
            let Some((end, _)) = writes.last() else {
                continue;
            };
            if self.scoping.get_resolved_references(symbol).any(|r| {
                let span = self.nodes.get_node(r.node_id()).kind().span();
                span.start < end.end && !writes.iter().any(|(_, receiver)| *receiver == span)
            }) {
                continue;
            }
            let region = Span::new(init.span().start, end.end);
            if self
                .comments
                .iter()
                .any(|comment| region.start < comment.span.end && comment.span.start < region.end)
            {
                continue;
            }
            if self
                .edits
                .iter()
                .any(|edit| region.start < edit.end && edit.start < region.end)
            {
                continue;
            }
            self.edits.push(analysis::Edit::replace(
                init.span(),
                format!("[{}]", values.join(", ")),
            ));
            for (span, _) in writes {
                self.edits.push(analysis::Edit::delete(span));
            }
            self.count += 1;
        }
    }
}
impl<'a> Visit<'a> for Scan<'_, 'a> {
    fn visit_function_body(&mut self, b: &FunctionBody<'a>) {
        self.body(&b.statements, true);
        walk::walk_function_body(self, b);
    }
    fn visit_block_statement(&mut self, b: &BlockStatement<'a>) {
        self.body(&b.body, false);
        walk::walk_block_statement(self, b);
    }
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        self.dynamic |= id.name == "eval";
    }
    fn visit_with_statement(&mut self, s: &WithStatement<'a>) {
        self.dynamic = true;
        walk::walk_with_statement(self, s);
    }
}

// Track a union of native aliases rather than treating reassignment of an
// ordinary VM local as mutation of the global host/prototypes.
type Path = Vec<String>;
type Aliases = std::collections::BTreeMap<SymbolId, BTreeSet<Path>>;
/// The most paths one local may alias. A local assigned from its own
/// properties multiplies them each round, up to `keys ^ depth`: past this
/// the aliases count as not settling.
const MAX_ALIAS_PATHS: usize = 256;
fn native_paths(e: &Expression<'_>, scoping: &Scoping, aliases: &Aliases) -> BTreeSet<Path> {
    let member = match unwrap(e) {
        Expression::Identifier(id) => {
            return if let Some(symbol) = analysis::identifier_symbol(id, scoping) {
                aliases.get(&symbol).cloned().unwrap_or_default()
            } else {
                match id.name.as_str() {
                    "host" => BTreeSet::from([vec!["host".into()]]),
                    "Array" | "Object" => {
                        BTreeSet::from([vec!["host".into(), id.name.to_string()]])
                    }
                    _ => BTreeSet::new(),
                }
            };
        }
        Expression::StaticMemberExpression(m) => (&m.object, Some(m.property.name.to_string())),
        Expression::ComputedMemberExpression(m) => (
            &m.object,
            match unwrap(&m.expression) {
                Expression::StringLiteral(key) => Some(key.value.to_string()),
                Expression::NumericLiteral(key) => Some(key.value.to_string()),
                _ => None,
            },
        ),
        _ => return BTreeSet::new(),
    };
    native_paths(member.0, scoping, aliases)
        .into_iter()
        .filter_map(|mut p| {
            if p.len() >= 10 {
                return None;
            }
            if let Some(key) = &member.1 {
                p.push(key.clone());
            }
            Some(p)
        })
        .collect()
}
pub(crate) fn ordinary_prototypes(program: &Program<'_>, scoping: &Scoping) -> bool {
    let Some(aliases) = native_aliases(program, scoping) else {
        return false;
    };
    let mut guard = PrototypeGuard {
        scoping,
        aliases: &aliases,
        unsafe_native: false,
    };
    guard.visit_program(program);
    !guard.unsafe_native
}

/// The native paths each local may alias, iterated to a fixpoint; `None`
/// when the aliases do not settle within the round or path limit.
fn native_aliases(program: &Program<'_>, scoping: &Scoping) -> Option<Aliases> {
    let mut collect = AliasCollect {
        scoping,
        aliases: Aliases::new(),
        changed: false,
        overflowed: false,
    };
    for _ in 0..16 {
        collect.changed = false;
        collect.visit_program(program);
        if !collect.changed || collect.overflowed {
            break;
        }
    }
    if collect.changed || collect.overflowed {
        return None;
    }
    Some(collect.aliases)
}

/// One round of alias collection from declarations and assignments.
struct AliasCollect<'s> {
    scoping: &'s Scoping,
    aliases: Aliases,
    changed: bool,
    overflowed: bool,
}
impl AliasCollect<'_> {
    fn add(&mut self, symbol: Option<SymbolId>, value: &Expression<'_>) {
        if self.overflowed {
            return;
        }
        if let Some(symbol) = symbol {
            let paths = native_paths(value, self.scoping, &self.aliases);
            let entry = self.aliases.entry(symbol).or_default();
            let before = entry.len();
            entry.extend(paths);
            self.changed |= before != entry.len();
            self.overflowed = entry.len() > MAX_ALIAS_PATHS;
        }
    }
}
impl<'a> Visit<'a> for AliasCollect<'_> {
    fn visit_variable_declarator(&mut self, d: &VariableDeclarator<'a>) {
        if let (BindingPattern::BindingIdentifier(id), Some(value)) = (&d.id, &d.init) {
            self.add(id.symbol_id.get(), value);
        }
        walk::walk_variable_declarator(self, d);
    }
    fn visit_assignment_expression(&mut self, a: &AssignmentExpression<'a>) {
        if let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left {
            self.add(analysis::identifier_symbol(id, self.scoping), &a.right);
        }
        walk::walk_assignment_expression(self, a);
    }
}

/// Flags any write, delete, escape, or dynamic scope that could reach the
/// native `Array`/`Object` prototypes.
struct PrototypeGuard<'s> {
    scoping: &'s Scoping,
    aliases: &'s Aliases,
    unsafe_native: bool,
}
impl PrototypeGuard<'_> {
    fn affects(&self, paths: BTreeSet<Path>, escape: bool) -> bool {
        paths.iter().any(|p| {
            ["Array", "Object"].iter().any(|name| {
                let target = vec![
                    "host".to_string(),
                    name.to_string(),
                    "prototype".to_string(),
                ];
                if escape {
                    target.starts_with(p)
                } else {
                    p.starts_with(&target) && p.len() == target.len() + 1
                }
            })
        })
    }
    fn mutates(&self, paths: BTreeSet<Path>) -> bool {
        paths.iter().any(|p| {
            ["Array", "Object"].iter().any(|name| {
                let target = vec![
                    "host".to_string(),
                    name.to_string(),
                    "prototype".to_string(),
                ];
                p.starts_with(&target)
                    || target.starts_with(p)
                    || (p.len() >= 2 && p[0] == "host" && p[1] == *name)
            })
        })
    }
    /// Flag a write to `object[key]` that reaches a native constructor.
    fn member_write(&mut self, object: &Expression<'_>, key: Option<String>) {
        let paths = native_paths(object, self.scoping, self.aliases)
            .into_iter()
            .map(|mut p| {
                if let Some(key) = &key {
                    p.push(key.clone());
                }
                p
            })
            .collect();
        self.unsafe_native |= self.mutates(paths);
    }
}
impl<'a> Visit<'a> for PrototypeGuard<'_> {
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        self.unsafe_native |= id.name == "eval"
            || analysis::identifier_symbol(id, self.scoping).is_none()
                && matches!(id.name.as_str(), "host" | "Array" | "Object")
                && id
                    .reference_id
                    .get()
                    .is_some_and(|r| self.scoping.get_reference(r).is_write());
    }
    fn visit_assignment_target(&mut self, target: &AssignmentTarget<'a>) {
        let member = match target {
            AssignmentTarget::StaticMemberExpression(m) => {
                Some((&m.object, Some(m.property.name.to_string())))
            }
            AssignmentTarget::ComputedMemberExpression(m) => Some((
                &m.object,
                match unwrap(&m.expression) {
                    Expression::StringLiteral(k) => Some(k.value.to_string()),
                    Expression::NumericLiteral(k) => Some(k.value.to_string()),
                    _ => None,
                },
            )),
            _ => None,
        };
        if let Some((object, key)) = member {
            self.member_write(object, key);
        }
        walk::walk_assignment_target(self, target);
    }
    fn visit_update_expression(&mut self, update: &UpdateExpression<'a>) {
        let member = match &update.argument {
            SimpleAssignmentTarget::StaticMemberExpression(m) => {
                Some((&m.object, Some(m.property.name.to_string())))
            }
            SimpleAssignmentTarget::ComputedMemberExpression(m) => Some((
                &m.object,
                match unwrap(&m.expression) {
                    Expression::StringLiteral(key) => Some(key.value.to_string()),
                    Expression::NumericLiteral(key) => Some(key.value.to_string()),
                    _ => None,
                },
            )),
            _ => None,
        };
        if let Some((object, key)) = member {
            self.member_write(object, key);
        }
        walk::walk_update_expression(self, update);
    }
    fn visit_unary_expression(&mut self, u: &UnaryExpression<'a>) {
        if u.operator == oxc_syntax::operator::UnaryOperator::Delete {
            self.unsafe_native |=
                self.affects(native_paths(&u.argument, self.scoping, self.aliases), true);
        }
        walk::walk_unary_expression(self, u);
    }
    fn visit_call_expression(&mut self, c: &CallExpression<'a>) {
        let callees = native_paths(&c.callee, self.scoping, self.aliases);
        let inspection = object_inspection(&callees);
        let native_this = !callees.is_empty()
            && callees.iter().all(|p| {
                p.len() == 4
                    && p[0] == "host"
                    && matches!(p[1].as_str(), "Array" | "Object")
                    && p.last().is_some_and(|s| s == "call" || s == "bind")
                    && p.get(2).is_some_and(|s| s != "prototype")
            });
        self.unsafe_native |= self.affects(callees, false)
            || !inspection
                && c.arguments
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| !native_this || *i != 0)
                    .filter_map(|(_, a)| a.as_expression())
                    .any(|e| self.affects(native_paths(e, self.scoping, self.aliases), true));
        walk::walk_call_expression(self, c);
    }
    fn visit_with_statement(&mut self, s: &WithStatement<'a>) {
        self.unsafe_native = true;
        walk::walk_with_statement(self, s);
    }
}

/// Every callee is a standard `Object` inspection method.
fn object_inspection(callees: &BTreeSet<Path>) -> bool {
    // Standard Object inspection methods do not mutate or export their
    // receiver/prototype inputs. Function.call's `thisArg` is separate
    // from the argument being inspected or defined.
    !callees.is_empty()
        && callees.iter().all(|p| {
            (p.len() == 4 || p.len() == 5 && matches!(p[4].as_str(), "call" | "apply" | "bind"))
                && p[0] == "host"
                && p[1] == "Object"
                && p[2] == "prototype"
                && matches!(
                    p[3].as_str(),
                    "hasOwnProperty" | "propertyIsEnumerable" | "isPrototypeOf" | "toString"
                )
                || (p.len() == 3
                    || p.len() == 4 && matches!(p[3].as_str(), "call" | "apply" | "bind"))
                    && p[0] == "host"
                    && p[1] == "Object"
                    && matches!(
                        p[2].as_str(),
                        "getPrototypeOf"
                            | "getOwnPropertyDescriptor"
                            | "getOwnPropertyDescriptors"
                            | "getOwnPropertyNames"
                            | "getOwnPropertySymbols"
                            | "keys"
                            | "values"
                            | "entries"
                            | "is"
                            | "isExtensible"
                            | "isSealed"
                            | "isFrozen"
                    )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ordinary(source: &str) -> bool {
        let allocator = Allocator::default();
        let program = analysis::parse_program(&allocator, source).unwrap();
        let (scoping, _) = analysis::analyze(&program);
        ordinary_prototypes(&program, &scoping)
    }

    #[test]
    fn a_local_assigned_from_its_own_properties_stops_at_the_path_limit() {
        let stores: String = (0..12).map(|key| format!("a = a.k{key};")).collect();
        assert!(!ordinary(&format!("let a = host; for (;;) {{ {stores} }}")));
    }

    #[test]
    fn a_few_aliases_stay_ordinary() {
        assert!(ordinary("let a = host; let b = a.k0; let c = b.k1;"));
    }
}
