//! Inline small, closed VM wrappers whose arguments are already lexical values.
//!
//! The supported return shapes read parameters in call-argument order before
//! any potentially observable operation. Literal tuple returns may repeat a
//! parameter because no effect can occur between those reads. This preserves
//! argument evaluation and the wrapper's bare-call `this` binding.

use std::collections::BTreeMap;

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::scope::ScopeFlags;

use crate::analysis;
use super::super::support::whole_line;

pub struct InlinedVmSubs {
    pub code: String,
    pub removed: usize,
}

pub(crate) fn inline_vm_sub_wrappers(source: &str) -> Result<InlinedVmSubs, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, _) = analysis::analyze(&program);
    let mut scan = Scan {
        scoping: &scoping,
        helpers: BTreeMap::new(),
        fallbacks: BTreeMap::new(),
        calls: BTreeMap::new(),
        scalarizable_calls: BTreeMap::new(),
        number_binding: false,
        dynamic: false,
    };
    scan.visit_program(&program);
    if scan.dynamic {
        return Ok(InlinedVmSubs {
            code: source.into(),
            removed: 0,
        });
    }
    let mut edits = Vec::new();
    let mut selected = Vec::<Span>::new();
    let mut touched = std::collections::BTreeSet::new();
    let mut removed = 0;
    for (symbol, helper) in &scan.helpers {
        if helper.uses_global_number && scan.number_binding {
            continue;
        }
        let refs = scoping.get_resolved_references(*symbol).collect::<Vec<_>>();
        let Some(calls) = scan.calls.get(symbol) else {
            continue;
        };
        if refs.is_empty() || refs.iter().any(|reference| reference.is_write()) {
            continue;
        }
        let scalarizable_call = |call: &Call| {
            helper.array_len.is_some_and(|length| {
                scan.scalarizable_calls.get(&call.span.start) == Some(&length)
            })
        };
        let partial_array = helper.array_len.is_some() && calls.iter().any(scalarizable_call);
        if !partial_array && refs.len() != calls.len() {
            continue;
        }
        let mut replacements = Vec::new();
        for call in calls {
            if partial_array && !scalarizable_call(call) {
                continue;
            }
            if !call_inlinable(helper, call, &scoping) {
                if partial_array {
                    continue;
                }
                replacements.clear();
                break;
            }
            replacements.push(call_replacement(source, helper, call));
        }
        if replacements.is_empty() || (!partial_array && replacements.len() != calls.len()) {
            continue;
        }
        let declaration = whole_line(source, helper.span);
        let extra_bytes: isize = replacements
            .iter()
            .map(|edit| edit.replacement.len() as isize - (edit.end - edit.start) as isize)
            .sum();
        if !partial_array && extra_bytes >= (declaration.end - declaration.start) as isize {
            continue;
        }
        let remove_declaration = replacements.len() == refs.len();
        let candidate_spans = replacements
            .iter()
            .map(|edit| Span::new(edit.start, edit.end))
            .chain(remove_declaration.then_some(declaration))
            .collect::<Vec<_>>();
        if candidate_spans.iter().enumerate().any(|(index, span)| {
            candidate_spans[..index]
                .iter()
                .any(|earlier| overlaps(*span, *earlier))
                || selected.iter().any(|earlier| overlaps(*span, *earlier))
        }) {
            continue;
        }
        touched.insert(*symbol);
        selected.extend(candidate_spans);
        edits.extend(replacements);
        if remove_declaration {
            edits.push(analysis::Edit::delete(declaration));
            removed += 1;
        }
    }
    if edits.is_empty() {
        return Ok(InlinedVmSubs {
            code: source.into(),
            removed: 0,
        });
    }
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err("inline VM wrapper edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|e| format!("inline VM wrappers: {e}"))?;
    Ok(InlinedVmSubs {
        code: applied.output,
        removed,
    })
}


/// Final pass: remove `vm_sub_*` helpers that survived the cleanup stages.
///
/// These are the shapes the substitution inliner declines (multi-statement body,
/// a written parameter, arguments that would have to be reordered). Each call
/// becomes an arrow that runs the original body with the original arguments, so
/// argument evaluation, `this` and the result are unchanged; the declaration is
/// then unreferenced and deleted.
///
/// It runs last on purpose: earlier stages still expect `vm_sub_*` helpers and
/// would trip over arrows injected mid-pipeline.
pub fn inline_remaining_vm_subs(source: &str, force: bool) -> Result<InlinedVmSubs, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, _) = analysis::analyze(&program);
    let mut scan = Scan {
        scoping: &scoping,
        helpers: BTreeMap::new(),
        fallbacks: BTreeMap::new(),
        calls: BTreeMap::new(),
        scalarizable_calls: BTreeMap::new(),
        number_binding: false,
        dynamic: false,
    };
    scan.visit_program(&program);
    if scan.dynamic {
        return Ok(InlinedVmSubs { code: source.into(), removed: 0 });
    }

    let mut edits = Vec::new();
    let mut selected = Vec::<Span>::new();
    let mut removed = 0;
    // Anything the substitution inliner could not take (multi-statement body, a
    // written parameter) still has exactly one meaning: run the original body
    // with the original arguments. Turn the call into an arrow that does that.
    // Argument evaluation, `this` and the returned value are all unchanged; the
    // `vm_sub_*` declaration then has no references and is deleted.
    for (symbol, fallback) in &scan.fallbacks {
        let refs = scoping.get_resolved_references(*symbol).collect::<Vec<_>>();
        if refs.is_empty() || refs.iter().any(|reference| reference.is_write()) {
            continue;
        }
        let Some(calls) = scan.calls.get(symbol) else {
            continue;
        };
        if calls.len() != refs.len() {
            continue;
        }
        // a call inside its own body would recurse through the arrow
        if calls.iter().any(|call| {
            call.span.start >= fallback.body.start && call.span.end <= fallback.body.end
        }) {
            continue;
        }
        let raw = &source[fallback.body.start as usize..fallback.body.end as usize];
        let body = if raw.starts_with('{') {
            raw.to_owned()
        } else {
            format!("{{{raw}}}")
        };
        let replacements = calls
            .iter()
            .map(|call| {
                let args = call
                    .args
                    .iter()
                    .map(|arg| arg.render(source))
                    .collect::<Vec<_>>()
                    .join(", ");
                analysis::Edit::replace(
                    call.span,
                    format!("(({}) => {})({})", fallback.params.join(", "), body, args),
                )
            })
            .collect::<Vec<_>>();
        let declaration = whole_line(source, fallback.span);
        // The pipeline's contract is that cleanup never grows the file: only
        // take the arrow when deleting the declaration pays for the duplicated
        // body at every call site.
        let growth: isize = replacements
            .iter()
            .map(|edit| edit.replacement.len() as isize - (edit.end - edit.start) as isize)
            .sum();
        // `force` is the last sweep: removing the registry name is worth a few
        // bytes, and nothing downstream depends on the size by then.
        if !force && growth >= (declaration.end - declaration.start) as isize {
            continue;
        }
        let candidate_spans = replacements
            .iter()
            .map(|edit| Span::new(edit.start, edit.end))
            .chain(std::iter::once(declaration))
            .collect::<Vec<_>>();
        if candidate_spans.iter().enumerate().any(|(index, span)| {
            candidate_spans[..index]
                .iter()
                .any(|earlier| overlaps(*span, *earlier))
                || selected.iter().any(|earlier| overlaps(*span, *earlier))
        }) {
            continue;
        }
        selected.extend(candidate_spans);
        edits.extend(replacements);
        edits.push(analysis::Edit::delete(declaration));
        removed += 1;
    }

    if edits.is_empty() {
        return Ok(InlinedVmSubs { code: source.into(), removed: 0 });
    }
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err("inline remaining VM helpers: edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|e| format!("inline remaining VM helpers: {e}"))?;
    Ok(InlinedVmSubs { code: applied.output, removed })
}

/// The call passes what the helper's body can take in place of its
/// parameters: one argument each, simple unless the helper allows otherwise,
/// generated bindings only, and stable ones where the helper needs them.
fn call_inlinable(helper: &Helper, call: &Call, scoping: &Scoping) -> bool {
    if call.args.len() != helper.params.len() {
        return false;
    }
    if call
        .args
        .iter()
        .any(|arg| matches!(arg, SimpleArg::Expression(_)))
        && !helper.allow_complex_argument
    {
        return false;
    }
    // Only generated lexical bindings are accepted. Their reads are
    // effect-free, so replacing parameter reads cannot move a getter.
    if call.args.iter().any(|arg| match arg {
        SimpleArg::Identifier(name, _, symbol) => {
            !(name.starts_with("$SSA_")
                || name.starts_with("$KEY_")
                || name.starts_with("$CARRY_")
                || name.starts_with("$PARAM")
                || name.starts_with("$OBJECT_"))
                || symbol.is_none()
        }
        SimpleArg::Literal(_) => false,
        SimpleArg::Expression(_) => false,
    }) {
        return false;
    }
    if helper.requires_stable_args
        && call.args.iter().any(|arg| match arg {
            SimpleArg::Identifier(name, _, Some(symbol)) => {
                !name.starts_with("$SSA_")
                    || scoping
                        .get_resolved_references(*symbol)
                        .any(|reference| reference.is_write())
            }
            _ => false,
        })
    {
        return false;
    }
    true
}

/// The helper's expression with the call's arguments in place of its
/// parameters, as the edit replacing the call.
fn call_replacement(source: &str, helper: &Helper, call: &Call) -> analysis::Edit {
    let mut expression = String::new();
    let mut cursor = helper.expression.start;
    for (index, span) in &helper.references {
        expression.push_str(&source[cursor as usize..span.start as usize]);
        expression.push_str(&call.args[*index].render(source));
        cursor = span.end;
    }
    expression.push_str(&source[cursor as usize..helper.expression.end as usize]);
    // Comma converts a member reference to a value when the result is
    // subsequently called: vm_sub(...)(x) must not acquire `this`.
    let replacement = if helper.member_result {
        format!("(0, {expression})")
    } else {
        format!("({expression})")
    };
    analysis::Edit::replace(call.span, replacement)
}

fn overlaps(left: Span, right: Span) -> bool {
    left.start < right.end && right.start < left.end
}

/// A `vm_sub_*` function the substitution inliner cannot take: a multi-statement
/// body, or a parameter that is written. Those are still removable — by turning
/// the call into an arrow that runs the original body, which keeps argument
/// evaluation and `this` exactly as they were.
struct Fallback {
    span: Span,
    body: Span,
    params: Vec<String>,
}

#[derive(Default)]
struct BodyProbe {
    has_this: bool,
    has_arguments: bool,
}

impl<'a> Visit<'a> for BodyProbe {
    fn visit_this_expression(&mut self, _: &ThisExpression) {
        self.has_this = true;
    }
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        if id.name == "arguments" {
            self.has_arguments = true;
        }
    }
}

struct Helper {
    span: Span,
    expression: Span,
    member_result: bool,
    array_len: Option<usize>,
    params: Vec<SymbolId>,
    references: Vec<(usize, Span)>,
    requires_stable_args: bool,
    uses_global_number: bool,
    allow_complex_argument: bool,
}

enum SimpleArg {
    Identifier(String, Span, Option<SymbolId>),
    Literal(Span),
    Expression(Span),
}

impl SimpleArg {
    fn render(&self, source: &str) -> String {
        let span = match self {
            Self::Identifier(_, span, _) | Self::Literal(span) | Self::Expression(span) => *span,
        };
        let text = &source[span.start as usize..span.end as usize];
        if matches!(self, Self::Expression(_)) {
            format!("({text})")
        } else {
            text.to_owned()
        }
    }
}

struct Call {
    span: Span,
    args: Vec<SimpleArg>,
}

struct Scan<'s> {
    scoping: &'s Scoping,
    helpers: BTreeMap<SymbolId, Helper>,
    fallbacks: BTreeMap<SymbolId, Fallback>,
    calls: BTreeMap<SymbolId, Vec<Call>>,
    scalarizable_calls: BTreeMap<u32, usize>,
    number_binding: bool,
    dynamic: bool,
}

impl Scan<'_> {
    fn body(&mut self, statements: &[Statement<'_>]) {
        for (index, statement) in statements.iter().enumerate() {
            let Statement::VariableDeclaration(declaration) = statement else {
                continue;
            };
            if declaration.kind != VariableDeclarationKind::Let
                || declaration.declarations.len() != 1
            {
                continue;
            }
            let binding = &declaration.declarations[0];
            let BindingPattern::BindingIdentifier(id) = &binding.id else {
                continue;
            };
            if !id.name.starts_with("$SSA_") {
                continue;
            }
            let (Some(symbol), Some(Expression::CallExpression(call))) =
                (id.symbol_id.get(), binding.init.as_ref())
            else {
                continue;
            };
            if !matches!(&call.callee, Expression::Identifier(_)) {
                continue;
            }
            let mut count = 0;
            for next in &statements[index + 1..] {
                let Statement::VariableDeclaration(extraction) = next else {
                    break;
                };
                if extraction.kind != VariableDeclarationKind::Let
                    || extraction.declarations.len() != 1
                {
                    break;
                }
                if !matches!(&extraction.declarations[0].id, BindingPattern::BindingIdentifier(id) if id.name.starts_with("$SSA_"))
                {
                    break;
                }
                let Some(Expression::ComputedMemberExpression(member)) =
                    extraction.declarations[0].init.as_ref()
                else {
                    break;
                };
                let (Expression::Identifier(object), Expression::NumericLiteral(key)) =
                    (&member.object, &member.expression)
                else {
                    break;
                };
                if analysis::identifier_symbol(object, self.scoping) != Some(symbol)
                    || key.value != count as f64
                {
                    break;
                }
                count += 1;
                if count == 4 {
                    break;
                }
            }
            if count < 2 {
                continue;
            }
            let refs = self
                .scoping
                .get_resolved_references(symbol)
                .collect::<Vec<_>>();
            if refs.len() == count && refs.iter().all(|reference| !reference.is_write()) {
                self.scalarizable_calls.insert(call.span.start, count);
            }
        }
    }
}

impl<'a> Visit<'a> for Scan<'_> {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        if let Some(body) = &function.body {
            self.body(&body.statements);
        }
        if let Some(id) = &function.id {
            if (id.name.starts_with("vm_sub_") || id.name.starts_with("$VMHELPER_"))
                && !function.r#async
                && !function.generator
            {
                if let (Some(symbol), Some(body)) = (id.symbol_id.get(), function.body.as_ref()) {
                    let mut probe = BodyProbe::default();
                    probe.visit_function_body(body);
                    if !probe.has_this && !probe.has_arguments {
                        let params = function
                            .params
                            .items
                            .iter()
                            .map(|p| match &p.pattern {
                                BindingPattern::BindingIdentifier(id) => Some(id.name.to_string()),
                                _ => None,
                            })
                            .collect::<Option<Vec<_>>>();
                        if let Some(params) = params {
                            self.fallbacks.insert(
                                symbol,
                                Fallback {
                                    span: function.span,
                                    body: body.span,
                                    params,
                                },
                            );
                        }
                    }
                    if body.statements.len() == 1
                        && body.directives.is_empty()
                        && function.params.rest.is_none()
                    {
                        if let Statement::ReturnStatement(ret) = &body.statements[0] {
                            if let Some(expr) = ret.argument.as_ref() {
                                let params = function
                                    .params
                                    .items
                                    .iter()
                                    .map(|p| match &p.pattern {
                                        BindingPattern::BindingIdentifier(id) => id.symbol_id.get(),
                                        _ => None,
                                    })
                                    .collect::<Option<Vec<_>>>();
                                if let Some(params) = params {
                                    let mut references = Vec::new();
                                    if shape_is_safe(expr, &params, self.scoping, &mut references) {
                                        let requires_stable_args = first_use_order(
                                            &references,
                                            params.len(),
                                        ) != (0..params.len())
                                            .collect::<Vec<_>>()
                                            || matches!(expr, Expression::ArrayExpression(array) if array.elements.iter().any(|element| matches!(element.as_expression(), Some(Expression::StaticMemberExpression(_) | Expression::ComputedMemberExpression(_)))))
                                            || has_effectful_callee_read(expr);
                                        let allow_complex_argument = params.len() == 1
                                            && references.len() == 1
                                            && !requires_stable_args
                                            && !is_call_expression(expr)
                                            && !uses_global_number(expr);
                                        self.helpers.insert(
                                            symbol,
                                            Helper {
                                                span: function.span,
                                                expression: expr.span(),
                                                member_result: is_member_result(expr),
                                                array_len: match expr {
                                                    Expression::ArrayExpression(array) => {
                                                        Some(array.elements.len())
                                                    }
                                                    _ => None,
                                                },
                                                params,
                                                references,
                                                requires_stable_args,
                                                uses_global_number: uses_global_number(expr),
                                                allow_complex_argument,
                                            },
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        walk::walk_function(self, function, flags);
    }

    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        self.body(&block.body);
        walk::walk_block_statement(self, block);
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if let Expression::Identifier(callee) = &call.callee {
            if let Some(symbol) = analysis::identifier_symbol(callee, self.scoping) {
                let args = call
                    .arguments
                    .iter()
                    .map(|arg| {
                        if arg.is_spread() {
                            return None;
                        }
                        let expr = arg.to_expression();
                        match expr {
                            Expression::Identifier(id) => Some(SimpleArg::Identifier(
                                id.name.to_string(),
                                id.span,
                                analysis::identifier_symbol(id, self.scoping),
                            )),
                            Expression::BooleanLiteral(_)
                            | Expression::NullLiteral(_)
                            | Expression::NumericLiteral(_)
                            | Expression::BigIntLiteral(_)
                            | Expression::StringLiteral(_) => Some(SimpleArg::Literal(expr.span())),
                            _ => Some(SimpleArg::Expression(expr.span())),
                        }
                    })
                    .collect::<Option<Vec<_>>>();
                if let Some(args) = args {
                    self.calls.entry(symbol).or_default().push(Call {
                        span: call.span,
                        args,
                    });
                }
            }
        }
        walk::walk_call_expression(self, call);
    }

    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        if id.name == "eval" {
            self.dynamic = true;
        }
    }
    fn visit_binding_identifier(&mut self, id: &BindingIdentifier<'a>) {
        if id.name == "Number" {
            self.number_binding = true;
        }
    }
    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.dynamic = true;
        walk::walk_with_statement(self, statement);
    }
}

fn shape_is_safe(
    expr: &Expression<'_>,
    params: &[SymbolId],
    scoping: &Scoping,
    refs: &mut Vec<(usize, Span)>,
) -> bool {
    fn atom(
        expr: &Expression<'_>,
        params: &[SymbolId],
        scoping: &Scoping,
        refs: &mut Vec<(usize, Span)>,
    ) -> bool {
        match expr {
            Expression::Identifier(id) => {
                let index = analysis::identifier_symbol(id, scoping)
                    .and_then(|symbol| params.iter().position(|param| *param == symbol));
                if let Some(index) = index {
                    refs.push((index, id.span));
                    true
                } else {
                    false
                }
            }
            Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_)
            | Expression::NumericLiteral(_)
            | Expression::BigIntLiteral(_)
            | Expression::StringLiteral(_) => true,
            Expression::ParenthesizedExpression(p) => atom(&p.expression, params, scoping, refs),
            Expression::BinaryExpression(binary) if params.len() == 1 => {
                atom(&binary.left, params, scoping, refs)
                    && atom(&binary.right, params, scoping, refs)
            }
            Expression::UnaryExpression(unary)
                if params.len() == 1 && unary.operator != oxc_ast::ast::UnaryOperator::Delete =>
            {
                atom(&unary.argument, params, scoping, refs)
            }
            _ => false,
        }
    }
    let okay = match expr {
        Expression::ParenthesizedExpression(p) => {
            shape_is_safe(&p.expression, params, scoping, refs)
        }
        Expression::Identifier(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BigIntLiteral(_)
        | Expression::StringLiteral(_) => atom(expr, params, scoping, refs),
        Expression::StaticMemberExpression(m) => atom(&m.object, params, scoping, refs),
        Expression::ComputedMemberExpression(m) => {
            atom(&m.object, params, scoping, refs) && atom(&m.expression, params, scoping, refs)
        }
        Expression::BinaryExpression(b) => {
            atom(&b.left, params, scoping, refs) && atom(&b.right, params, scoping, refs)
        }
        Expression::UnaryExpression(u) if u.operator != oxc_ast::ast::UnaryOperator::Delete => {
            atom(&u.argument, params, scoping, refs)
        }
        Expression::ArrayExpression(array)
            if array
                .elements
                .iter()
                .all(|element| element.as_expression().is_some()) =>
        {
            array.elements.iter().all(|element| {
                let value = element.as_expression().unwrap();
                match value {
                    Expression::StaticMemberExpression(member) => {
                        atom(&member.object, params, scoping, refs)
                    }
                    Expression::ComputedMemberExpression(member) => {
                        atom(&member.object, params, scoping, refs)
                            && atom(&member.expression, params, scoping, refs)
                    }
                    _ => atom(value, params, scoping, refs),
                }
            })
        }
        Expression::CallExpression(c)
            if matches!(&c.callee, Expression::Identifier(id) if id.name == "Number" && analysis::identifier_symbol(id, scoping).is_none())
                && c.arguments.len() == 1
                && !c.arguments[0].is_spread() =>
        {
            atom(c.arguments[0].to_expression(), params, scoping, refs)
        }
        Expression::CallExpression(c) if c.arguments.iter().all(|a| !a.is_spread()) => {
            (atom(&c.callee, params, scoping, refs)
                || matches!(&c.callee, Expression::StaticMemberExpression(member)
                    if atom(&member.object, params, scoping, refs)))
                && c.arguments
                    .iter()
                    .all(|a| atom(a.to_expression(), params, scoping, refs))
        }
        _ => false,
    };
    if !okay {
        return false;
    }
    let mut first_seen = Vec::new();
    for (index, _) in refs.iter() {
        if !first_seen.contains(index) {
            first_seen.push(*index);
        }
    }
    first_seen.len() == params.len()
}

fn first_use_order(refs: &[(usize, Span)], count: usize) -> Vec<usize> {
    let mut order = Vec::with_capacity(count);
    for (index, _) in refs {
        if !order.contains(index) {
            order.push(*index);
        }
    }
    order
}

fn has_effectful_callee_read(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::ParenthesizedExpression(parenthesized) => {
            has_effectful_callee_read(&parenthesized.expression)
        }
        Expression::CallExpression(call) => matches!(
            &call.callee,
            Expression::StaticMemberExpression(_) | Expression::ComputedMemberExpression(_)
        ),
        _ => false,
    }
}

fn uses_global_number(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::ParenthesizedExpression(parenthesized) => {
            uses_global_number(&parenthesized.expression)
        }
        Expression::CallExpression(call) => {
            matches!(&call.callee, Expression::Identifier(id) if id.name == "Number")
        }
        _ => false,
    }
}

fn is_call_expression(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::ParenthesizedExpression(parenthesized) => {
            is_call_expression(&parenthesized.expression)
        }
        Expression::CallExpression(_) => true,
        _ => false,
    }
}

fn is_member_result(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::ParenthesizedExpression(parenthesized) => {
            is_member_result(&parenthesized.expression)
        }
        Expression::StaticMemberExpression(_)
        | Expression::ComputedMemberExpression(_)
        | Expression::PrivateFieldExpression(_) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::{inline_remaining_vm_subs, inline_vm_sub_wrappers};

    #[test]
    fn removes_multi_statement_wrapper_with_an_arrow() {
        let source = "function outer($SSA_1, $SSA_2, $SSA_3) { function vm_sub_1_2($PARAMA, $PARAMB, $PARAMC, $PARAMD) { $PARAMA[$PARAMB] = $PARAMC; return $PARAMD; } return vm_sub_1_2($SSA_1, $SSA_2, $SSA_3, $SSA_1); }";
        let output = inline_remaining_vm_subs(source, false).unwrap();
        assert_eq!(output.removed, 1, "{}", output.code);
        assert!(!output.code.contains("vm_sub_"), "{}", output.code);
        assert!(output.code.contains("=>"), "{}", output.code);
    }

    #[test]
    fn removes_wrapper_that_writes_a_parameter_without_touching_the_caller() {
        let source = "function outer($SSA_1, $SSA_2) { function vm_sub_1_2($PARAMA, $PARAMB) { $PARAMA = new $PARAMA(1); $PARAMA[0] = $PARAMB; return $PARAMA; } return vm_sub_1_2(Float64Array, $SSA_1); }";
        let output = inline_remaining_vm_subs(source, false).unwrap();
        assert_eq!(output.removed, 1, "{}", output.code);
        assert!(!output.code.contains("vm_sub_"), "{}", output.code);
        // the callee name must not be assigned at the call site
        assert!(!output.code.contains("Float64Array = new"), "{}", output.code);
    }

    #[test]
    fn keeps_wrapper_that_uses_this_or_arguments() {
        let with_this = "function outer($SSA_1) { function vm_sub_1_2($PARAMA) { this.x = $PARAMA; return this; } return vm_sub_1_2($SSA_1); }";
        assert_eq!(inline_remaining_vm_subs(with_this, true).unwrap().removed, 0);
        let with_arguments = "function outer($SSA_1) { function vm_sub_1_2($PARAMA) { let a = arguments.length; return $PARAMA + a; } return vm_sub_1_2($SSA_1); }";
        assert_eq!(inline_remaining_vm_subs(with_arguments, true).unwrap().removed, 0);
    }

    #[test]
    fn inlines_closed_member_wrapper() {
        let source = "function outer() { function vm_sub_1_2($PARAMA, $PARAMB) { return $PARAMA[$PARAMB]; } let $SSA_1 = obj; let $SSA_2 = 'x'; return vm_sub_1_2($SSA_1, $SSA_2); }";
        let output = inline_vm_sub_wrappers(source).unwrap();
        assert_eq!(output.removed, 1);
        assert!(
            output.code.contains("return (0, $SSA_1[$SSA_2]);"),
            "{}",
            output.code
        );
    }

    #[test]
    fn retains_effectful_arguments_and_extra_references() {
        let source = "function outer() { function vm_sub_1_2($PARAMA) { return $PARAMA.x; } let $SSA_1 = obj; let a = vm_sub_1_2(make()); let b = vm_sub_1_2; return [a,b]; }";
        assert_eq!(inline_vm_sub_wrappers(source).unwrap().removed, 0);
    }

    #[test]
    fn exposes_fresh_literal_tuple_for_scalarization() {
        let source = "function outer($SSA_arg) { function vm_sub_1_2($PARAMA) { return [$PARAMA, 'name']; } let $SSA_1 = vm_sub_1_2($SSA_arg); let $SSA_2 = $SSA_1[0]; let $SSA_3 = $SSA_1[1]; return $SSA_2; }";
        let output = inline_vm_sub_wrappers(source).unwrap();
        assert_eq!(output.removed, 1, "{}", output.code);
        assert!(
            output.code.contains("let $SSA_1 = ([$SSA_arg, 'name']);"),
            "{}",
            output.code
        );
    }

    #[test]
    fn scalarizes_frequently_called_tuple_helper_even_when_inlining_alone_grows() {
        let mut source = String::from("function outer($SSA_arg) { function vm_sub_1_2($PARAMA) { return [$PARAMA, $PARAMA]; }");
        for n in 0..12 {
            source.push_str(&format!("let $SSA_{}_0 = vm_sub_1_2($SSA_arg); let $SSA_{}_0 = $SSA_{}_0[0]; let $SSA_{}_0 = $SSA_{}_0[1];", n * 3 + 1, n * 3 + 2, n * 3 + 1, n * 3 + 3, n * 3 + 1));
        }
        source.push_str("return 0; }");
        let inlined = inline_vm_sub_wrappers(&source).unwrap();
        assert_eq!(inlined.removed, 1, "{}", inlined.code);
        let scalarized = crate::emitted_cleanup::scalarize_literal_tuples(&inlined.code).unwrap();
        assert_eq!(scalarized.removed, 12, "{}", scalarized.code);
        assert!(scalarized.code.len() < source.len());
    }

    #[test]
    fn scalarizes_simple_tuple_calls_while_retaining_unsupported_calls() {
        let source = "function outer($SSA_arg) { function vm_sub_1_2($PARAMA) { return [$PARAMA, $PARAMA]; } let $SSA_1 = vm_sub_1_2($SSA_arg); let $SSA_2 = $SSA_1[0]; let $SSA_3 = $SSA_1[1]; let $SSA_4 = vm_sub_1_2(arguments[0]); return [$SSA_2, $SSA_3, $SSA_4]; }";
        let inlined = inline_vm_sub_wrappers(source).unwrap();
        assert_eq!(inlined.removed, 0);
        assert!(inlined.code.contains("function vm_sub_1_2"));
        assert!(inlined.code.contains("vm_sub_1_2(arguments[0])"));
        assert!(inlined
            .code
            .contains("let $SSA_1 = ([$SSA_arg, $SSA_arg]);"));
        let scalarized = crate::emitted_cleanup::scalarize_literal_tuples(&inlined.code).unwrap();
        assert_eq!(scalarized.removed, 1);
    }

    #[test]
    fn inlines_length_tuple_after_local_simplification() {
        let source = "function outer(x, y) { function vm_sub_1_2($PARAMA, $PARAMB) { let $SSA_1 = $PARAMA.length; return [$SSA_1, $PARAMB]; } let $SSA_2 = x; let $SSA_3 = y; let $SSA_4 = vm_sub_1_2($SSA_2, $SSA_3); let $SSA_5 = $SSA_4[0]; let $SSA_6 = $SSA_4[1]; return [$SSA_5, $SSA_6]; }";
        let simple = crate::emitted_cleanup::simplify_vm_sub_bodies(source).unwrap();
        let output = inline_vm_sub_wrappers(&simple.code).unwrap();
        assert_eq!(output.removed, 1, "{}", output.code);
        assert!(output.code.contains("$SSA_2.length"), "{}", output.code);
    }

    #[test]
    fn inlines_number_conversion_without_changing_global_resolution() {
        let source = "function outer(x) { function vm_sub_1_2($PARAMA) { return (Number($PARAMA)); } let $SSA_2 = x; return vm_sub_1_2($SSA_2); }";
        let output = inline_vm_sub_wrappers(source).unwrap();
        assert_eq!(output.removed, 1, "{}", output.code);
        assert!(output.code.contains("Number($SSA_2)"), "{}", output.code);
        let shadowed = format!("let Number = custom; {source}");
        assert_eq!(inline_vm_sub_wrappers(&shadowed).unwrap().removed, 0);
        let carry = "function outer(x) { function vm_sub_1_2($PARAMA) { return Number($PARAMA); } let $CARRY_1 = x; $CARRY_1 = x + 1; return vm_sub_1_2($CARRY_1); }";
        assert_eq!(inline_vm_sub_wrappers(carry).unwrap().removed, 1);
    }

    #[test]
    fn inlines_out_of_order_parameter_reads_only_for_stable_ssa_arguments() {
        let stable = "function outer(x, y) { function vm_sub_1_2($PARAMA, $PARAMB) { return $PARAMB - $PARAMA; } let $SSA_1 = x; let $SSA_2 = y; return vm_sub_1_2($SSA_1, $SSA_2); }";
        assert_eq!(inline_vm_sub_wrappers(stable).unwrap().removed, 1);
        let unstable = "function outer(x, y) { function vm_sub_1_2($PARAMA, $PARAMB) { return $PARAMB - $PARAMA; } let $KEY_1 = x; let $SSA_2 = y; $KEY_1 = y; return vm_sub_1_2($KEY_1, $SSA_2); }";
        assert_eq!(inline_vm_sub_wrappers(unstable).unwrap().removed, 0);
    }

    #[test]
    fn length_getter_and_tuple_values_match_after_inlining() {
        let source = "function f() { let hits = 0; function vm_sub_1_2($PARAMA, $PARAMB) { let $SSA_1 = $PARAMA.length; return [$SSA_1, $PARAMB]; } let $SSA_2 = {get length() { hits++; return 7; }}; let $SSA_3 = 5; let $SSA_4 = vm_sub_1_2($SSA_2, $SSA_3); let $SSA_5 = $SSA_4[0]; let $SSA_6 = $SSA_4[1]; return [hits, $SSA_5, $SSA_6]; }";
        let simplified = crate::emitted_cleanup::simplify_vm_sub_bodies(source).unwrap();
        let inlined = inline_vm_sub_wrappers(&simplified.code).unwrap();
        let scalar = crate::emitted_cleanup::scalarize_literal_tuples(&inlined.code).unwrap();
        assert_eq!(scalar.removed, 1, "{}", scalar.code);
        let observe = |code: &str| {
            let script = format!("{code}\nconsole.log(JSON.stringify([f(), f()]));");
            let output = Command::new("node").arg("-e").arg(script).output().unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8(output.stdout).unwrap()
        };
        assert_eq!(observe(source), observe(&scalar.code));
    }

    #[test]
    fn inlines_single_use_parameter_with_computed_argument_once() {
        let source = "function f() { let hits = 0; function vm_sub_1_2($PARAMA) { return $PARAMA >> 12; } let $SSA_1 = 65536; let a = vm_sub_1_2($SSA_1); let b = vm_sub_1_2((hits++, $SSA_1 + 4096)); return [a, b, hits]; }";
        let inlined = inline_vm_sub_wrappers(source).unwrap();
        assert_eq!(inlined.removed, 1, "{}", inlined.code);
        let observe = |code: &str| {
            let script = format!("{code}\nconsole.log(JSON.stringify(f()));");
            let output = Command::new("node").arg("-e").arg(script).output().unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8(output.stdout).unwrap()
        };
        assert_eq!(observe(source), observe(&inlined.code));
    }

    #[test]
    fn nested_wrapper_calls_do_not_produce_overlapping_edits() {
        let source = "function f(x) { function vm_sub_1_2($PARAMA) { return $PARAMA >> 1; } function vm_sub_2_3($PARAMA) { return $PARAMA + 1; } let $SSA_1 = x; return vm_sub_2_3(vm_sub_1_2($SSA_1)); }";
        let first = inline_vm_sub_wrappers(source).unwrap();
        let second = inline_vm_sub_wrappers(&first.code).unwrap();
        assert!(!second.code.is_empty());
    }

    #[test]
    fn inlines_nested_bitwise_wrapper_with_computed_argument() {
        let source = "function f(x) { function vm_sub_1_2($PARAMA) { return (($PARAMA >> 6) & 63); } let $SSA_1 = x; return vm_sub_1_2(65536 + $SSA_1); }";
        let output = inline_vm_sub_wrappers(source).unwrap();
        assert_eq!(output.removed, 1, "{}", output.code);
    }
}
