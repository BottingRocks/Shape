//! Collecting `defineProperty` calls and the candidates among them.

use super::*;

pub(super) struct ScanCollector<'s> {
    pub(super) scoping: &'s Scoping,
    pub(super) raw_calls: Vec<RawCall>,
    /// `(object identifier span, member expression span)` for every
    /// `Object.defineProperty` / `Object["defineProperty"]` member read.
    pub(super) define_property_members: Vec<(Span, Span)>,
    pub(super) write_spans: Vec<Span>,
    pub(super) saw_with: bool,
    pub(super) saw_eval_reference: bool,
    pub(super) saw_this: bool,
    pub(super) saw_constructor_access: bool,
    pub(super) saw_unknown_computed_call: bool,
}

impl<'a> Visit<'a> for ScanCollector<'_> {
    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.saw_with = true;
        walk::walk_with_statement(self, statement);
    }

    fn visit_this_expression(&mut self, expression: &ThisExpression) {
        self.saw_this = true;
        walk::walk_this_expression(self, expression);
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if identifier.name == "eval" {
            self.saw_eval_reference = true;
        }
    }

    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'a>) {
        if member.property.name == "constructor" {
            self.saw_constructor_access = true;
        }
        if member.property.name == "defineProperty" {
            if let Expression::Identifier(object) = unwrap_parens(&member.object) {
                if object.name == "Object"
                    && analysis::identifier_symbol(object, self.scoping).is_none()
                {
                    self.define_property_members
                        .push((object.span, member.span));
                }
            }
        }
        walk::walk_static_member_expression(self, member);
    }

    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'a>) {
        if let Some(key) = static_string_expression(&member.expression) {
            if key == "constructor" {
                self.saw_constructor_access = true;
            }
            if key == "defineProperty" {
                if let Expression::Identifier(object) = unwrap_parens(&member.object) {
                    if object.name == "Object"
                        && analysis::identifier_symbol(object, self.scoping).is_none()
                    {
                        self.define_property_members
                            .push((object.span, member.span));
                    }
                }
            }
        }
        walk::walk_computed_member_expression(self, member);
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if let Expression::ComputedMemberExpression(member) = unwrap_parens(&call.callee) {
            match static_string_expression(&member.expression) {
                Some(key) if key == "constructor" => self.saw_constructor_access = true,
                None if computed_object_is_function_like(&member.object) => {
                    self.saw_unknown_computed_call = true;
                }
                Some(_) => {}
                None => {}
            }
        }
        if let Some(raw) = raw_call_from_call(call, self.scoping) {
            self.raw_calls.push(raw);
        }
        walk::walk_call_expression(self, call);
    }

    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        self.write_spans.push(assignment.left.span());
        walk::walk_assignment_expression(self, assignment);
    }

    fn visit_update_expression(&mut self, update: &UpdateExpression<'a>) {
        self.write_spans.push(update.argument.span());
        walk::walk_update_expression(self, update);
    }

    fn visit_unary_expression(&mut self, unary: &UnaryExpression<'a>) {
        if unary.operator == UnaryOperator::Delete {
            self.write_spans.push(unary.argument.span());
        }
        walk::walk_unary_expression(self, unary);
    }

    fn visit_for_in_statement(&mut self, statement: &ForInStatement<'a>) {
        self.write_spans.push(statement.left.span());
        walk::walk_for_in_statement(self, statement);
    }

    fn visit_for_of_statement(&mut self, statement: &ForOfStatement<'a>) {
        self.write_spans.push(statement.left.span());
        walk::walk_for_of_statement(self, statement);
    }
}

#[derive(Debug, Clone)]
pub(super) struct RawCall {
    pub(super) span: Span,
    pub(super) receiver_symbol: Option<SymbolId>,
    pub(super) receiver_ident_span: Option<Span>,
    pub(super) receiver_expr_span: Option<Span>,
    pub(super) key: Option<KeyInfo>,
    pub(super) descriptor: Option<DescriptorInfo>,
    pub(super) skip: Option<PropertyWriteSkip>,
}

pub(super) fn raw_call_from_call<'a>(call: &CallExpression<'a>, scoping: &Scoping) -> Option<RawCall> {
    let callee = unwrap_parens(&call.callee);
    let (object, member_span, member_optional) = match callee {
        Expression::StaticMemberExpression(member) if member.property.name == "defineProperty" => {
            (unwrap_parens(&member.object), member.span, member.optional)
        }
        Expression::ComputedMemberExpression(member) => {
            let Expression::StringLiteral(key) = unwrap_parens(&member.expression) else {
                return None;
            };
            if key.value != "defineProperty" {
                return None;
            }
            (unwrap_parens(&member.object), member.span, member.optional)
        }
        _ => return None,
    };
    let Expression::Identifier(object_identifier) = object else {
        return None;
    };
    if object_identifier.name != "Object" {
        return None;
    }

    let skip = |reason: PropertyWriteSkipReason, detail: &str| PropertyWriteSkip {
        span: (member_span.start, member_span.end),
        reason,
        detail: detail.to_owned(),
    };

    let shadowed = analysis::identifier_symbol(object_identifier, scoping).is_some();
    let mut raw = RawCall {
        span: call.span,
        receiver_symbol: None,
        receiver_ident_span: None,
        receiver_expr_span: None,
        key: None,
        descriptor: None,
        skip: None,
    };

    if shadowed {
        raw.skip = Some(skip(
            PropertyWriteSkipReason::ShadowedObject,
            "Object at this call resolves to a local binding",
        ));
        return Some(raw);
    }
    if call.optional || member_optional {
        raw.skip = Some(skip(
            PropertyWriteSkipReason::OptionalChain,
            "optional member/call is not rewritten",
        ));
        return Some(raw);
    }
    if call.arguments.len() != 3 || call.arguments.iter().any(Argument::is_spread) {
        raw.skip = Some(skip(
            PropertyWriteSkipReason::WrongArgumentCount,
            "defineProperty requires exactly three non-spread arguments",
        ));
        return Some(raw);
    }

    let receiver = unwrap_parens(call.arguments[0].to_expression());
    let Expression::Identifier(receiver_identifier) = receiver else {
        raw.skip = Some(skip(
            PropertyWriteSkipReason::ReceiverNotIdentifier,
            "receiver is not a plain identifier",
        ));
        return Some(raw);
    };
    raw.receiver_symbol = analysis::identifier_symbol(receiver_identifier, scoping);
    raw.receiver_ident_span = Some(receiver_identifier.span);
    raw.receiver_expr_span = Some(receiver.span());
    if raw.receiver_symbol.is_none() {
        raw.skip = Some(skip(
            PropertyWriteSkipReason::ReceiverUnknown,
            "receiver does not resolve to a local binding",
        ));
        return Some(raw);
    }

    let key_expression = unwrap_parens(call.arguments[1].to_expression());
    match static_property_key(key_expression) {
        Some(key) => raw.key = Some(key),
        None => {
            raw.skip = Some(skip(
                PropertyWriteSkipReason::NonLiteralKey,
                "key is not a side-effect-free static string/number key",
            ));
            return Some(raw);
        }
    }

    let descriptor_expression = unwrap_parens(call.arguments[2].to_expression());
    match exact_data_descriptor(descriptor_expression) {
        Ok(descriptor) => raw.descriptor = Some(descriptor),
        Err(detail) => {
            raw.skip = Some(skip(PropertyWriteSkipReason::DescriptorNotExact, &detail));
            return Some(raw);
        }
    }

    Some(raw)
}

#[derive(Debug, Clone)]
pub(super) struct CandidateCall {
    pub(super) span: Span,
    pub(super) receiver_symbol: Option<SymbolId>,
    pub(super) receiver_ident_span: Option<Span>,
    pub(super) receiver_expr_span: Span,
    pub(super) key: KeyInfo,
    pub(super) descriptor: DescriptorInfo,
    pub(super) context: Option<DirectContext>,
    pub(super) skip: Option<PropertyWriteSkip>,
}

pub(super) fn build_calls(
    raw_calls: &[RawCall],
    contexts: &ContextCollector,
    scoping: &Scoping,
) -> Vec<CandidateCall> {
    let mut calls = raw_calls
        .iter()
        .map(|raw| {
            let context = contexts.call_contexts.get(&raw.span.start).copied();
            let mut skip = raw.skip.clone();
            let outside_function = raw.receiver_symbol.is_some_and(|symbol| {
                !scoping.scope_ancestors(scoping.symbol_scope_id(symbol)).any(|scope| {
                    scoping.scope_flags(scope).intersects(ScopeFlags::Function | ScopeFlags::Arrow)
                })
            });
            if skip.is_none() && (outside_function || context.is_some_and(|context| {
                Some(context.block_id) == contexts.program_block_id
            })) {
                skip = Some(PropertyWriteSkip {
                    span: span_pair(raw.span),
                    reason: PropertyWriteSkipReason::NotDirectExpressionStatement,
                    detail: "program-level call completion value is observable; only function/block calls with discarded results are rewritten".into(),
                });
            } else if skip.is_none() && context.is_none() {
                skip = Some(PropertyWriteSkip {
                    span: span_pair(raw.span),
                    reason: PropertyWriteSkipReason::NotDirectExpressionStatement,
                    detail: "call result is used or the call is not a direct expression statement"
                        .into(),
                });
            }
            CandidateCall {
                span: raw.span,
                receiver_symbol: raw.receiver_symbol,
                receiver_ident_span: raw.receiver_ident_span,
                receiver_expr_span: raw.receiver_expr_span.unwrap_or(raw.span),
                key: raw.key.clone().unwrap_or(KeyInfo {
                    canonical: String::new(),
                    array_index: None,
                    span: raw.span,
                }),
                descriptor: raw.descriptor.clone().unwrap_or(DescriptorInfo {
                    value_span: raw.span,
                    value_needs_parentheses: false,
                }),
                context,
                skip,
            }
        })
        .collect::<Vec<_>>();
    calls.sort_by_key(|call| call.span.start);
    calls
}
