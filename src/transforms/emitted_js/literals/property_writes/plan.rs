//! Planning a receiver's writes and building the literal that replaces them.

use super::*;

pub(super) fn receiver_references_are_safe(
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    symbol: SymbolId,
    last_write: usize,
    statement_spans: &[Span],
    safe_receivers: &BTreeSet<u32>,
) -> Result<(), (PropertyWriteSkipReason, String)> {
    let symbol_scope = scoping.symbol_scope_id(symbol);
    for reference in scoping.get_resolved_references(symbol) {
        let span = nodes.get_node(reference.node_id()).kind().span();
        if reference.is_write() {
            return Err((
                PropertyWriteSkipReason::ReceiverReassigned,
                "receiver binding is reassigned or updated".into(),
            ));
        }
        if safe_receivers.contains(&span.start) {
            continue;
        }
        if reference_is_nested_closure(scoping, reference.scope_id(), symbol_scope) {
            return Err((
                PropertyWriteSkipReason::ReceiverEscaped,
                "receiver is captured by a nested closure".into(),
            ));
        }
        let Some(statement_index) = statement_spans
            .iter()
            .position(|statement| span_contains(*statement, span))
        else {
            return Err((
                PropertyWriteSkipReason::ReceiverObservedBeforeWrites,
                "receiver reference is outside the allocation block".into(),
            ));
        };
        if statement_index <= last_write {
            return Err((
                PropertyWriteSkipReason::ReceiverObservedBeforeWrites,
                format!(
                    "receiver is observed at bytes {}..{} before the last rewritten write",
                    span.start, span.end
                ),
            ));
        }
    }
    Ok(())
}

pub(super) fn reference_is_nested_closure(
    scoping: &Scoping,
    reference_scope: oxc_syntax::scope::ScopeId,
    symbol_scope: oxc_syntax::scope::ScopeId,
) -> bool {
    for scope in scoping.scope_ancestors(reference_scope) {
        if scope == symbol_scope {
            return false;
        }
        let flags = scoping.scope_flags(scope);
        if flags.intersects(ScopeFlags::Function | ScopeFlags::Arrow) {
            return true;
        }
    }
    true
}

#[derive(Debug, Default)]
pub(super) struct GroupPlan {
    pub(super) rewrites: Vec<(usize, String)>,
    pub(super) preseed_edit: Option<analysis::Edit>,
    pub(super) preseeded_keys: usize,
    pub(super) skips: Vec<(usize, PropertyWriteSkipReason, String)>,
}

pub(super) fn plan_symbol_writes(
    source: &str,
    calls: &[CandidateCall],
    indices: &[usize],
    plan: &AllocationPlan,
) -> GroupPlan {
    let mut group = GroupPlan::default();
    let mut facts = match &plan.literal {
        AllocationLiteral::Array { existing, .. } | AllocationLiteral::Object { existing, .. } => {
            existing.clone()
        }
    };

    let literal_preseedable = match &plan.literal {
        AllocationLiteral::Array { preseedable, .. }
        | AllocationLiteral::Object { preseedable, .. } => *preseedable,
    };
    let (array_preseed, array_preseed_ok, object_preseed, object_preseed_ok) = match &plan.literal {
        AllocationLiteral::Array {
            elements,
            preseedable,
            ..
        } => {
            let (keys, ok) = array_preseed(calls, indices, &facts, elements.len(), *preseedable);
            (keys, ok, Vec::new(), false)
        }
        AllocationLiteral::Object {
            preseedable,
            property_count,
            ..
        } => {
            let (keys, ok) = object_preseed(calls, indices, &facts, *preseedable, *property_count);
            (BTreeSet::new(), false, keys, ok)
        }
    };

    for index in indices {
        let call = &calls[*index];
        let key = &call.key;
        if key.canonical == "__proto__" {
            group.skips.push((
                *index,
                PropertyWriteSkipReason::ProtoKey,
                "__proto__ is special for ordinary assignment and is never rewritten".into(),
            ));
            facts.insert(key.canonical.clone());
            continue;
        }
        let rewrite = if facts.contains(&key.canonical) {
            true
        } else {
            match &plan.literal {
                AllocationLiteral::Array { .. } => key.array_index.is_some_and(|array_index| {
                    array_preseed_ok && array_preseed.contains(&array_index)
                }),
                AllocationLiteral::Object { .. } => {
                    object_preseed_ok && object_preseed.contains(&key.canonical)
                }
            }
        };
        if rewrite {
            group.rewrites.push((*index, write_text(source, call)));
            facts.insert(key.canonical.clone());
            continue;
        }
        let (reason, detail) = skip_reason(plan, key, literal_preseedable);
        group.skips.push((*index, reason, detail));
        facts.insert(key.canonical.clone());
    }

    preseed_edit(
        source,
        plan,
        &mut group,
        &array_preseed,
        array_preseed_ok,
        &object_preseed,
        object_preseed_ok,
    );
    group
}

/// The array indices the literal can be pre-seeded with, and whether it can
/// be: no write beyond the pre-seed limit, `__proto__` and known keys aside.
fn array_preseed(
    calls: &[CandidateCall],
    indices: &[usize],
    facts: &BTreeSet<String>,
    elements: usize,
    preseedable: bool,
) -> (BTreeSet<u32>, bool) {
    let mut array_preseed = BTreeSet::<u32>::new();
    let mut array_preseed_ok = preseedable && elements <= MAX_PRESEED_ARRAY_ELEMENTS;
    if array_preseed_ok {
        for index in indices {
            let key = &calls[*index].key;
            if key.canonical == "__proto__" || facts.contains(&key.canonical) {
                continue;
            }
            let Some(array_index) = key.array_index else {
                continue;
            };
            if array_index as usize >= MAX_PRESEED_ARRAY_ELEMENTS {
                array_preseed_ok = false;
                array_preseed.clear();
                break;
            }
            array_preseed.insert(array_index);
        }
    }
    (array_preseed, array_preseed_ok)
}

/// The keys the object literal can be pre-seeded with, in order, and whether
/// it can be: no `__proto__` write, and within the key limit.
fn object_preseed(
    calls: &[CandidateCall],
    indices: &[usize],
    facts: &BTreeSet<String>,
    preseedable: bool,
    property_count: usize,
) -> (Vec<String>, bool) {
    let mut object_preseed = Vec::<String>::new();
    let mut object_preseed_ok = preseedable;
    if object_preseed_ok {
        for index in indices {
            let key = &calls[*index].key;
            if facts.contains(&key.canonical) {
                continue;
            }
            if key.canonical == "__proto__" {
                object_preseed_ok = false;
                object_preseed.clear();
                break;
            }
            if !object_preseed.contains(&key.canonical) {
                object_preseed.push(key.canonical.clone());
            }
        }
    }
    if object_preseed_ok
        && property_count.saturating_add(object_preseed.len()) > MAX_PRESEED_OBJECT_KEYS
    {
        object_preseed_ok = false;
        object_preseed.clear();
    }

    (object_preseed, object_preseed_ok)
}

/// `receiver[key] = value` for a `defineProperty` call the literal makes
/// ordinary.
fn write_text(source: &str, call: &CandidateCall) -> String {
    let key = &call.key;
    let receiver = &source
        [call.receiver_expr_span.start as usize..call.receiver_expr_span.end as usize];
    let key_source = &source[key.span.start as usize..key.span.end as usize];
    let value = &source[call.descriptor.value_span.start as usize
        ..call.descriptor.value_span.end as usize];
    let value = if call.descriptor.value_needs_parentheses {
        format!("({value})")
    } else {
        value.to_owned()
    };
    format!("{receiver}[{key_source}] = {value}")
}

/// Why a write the literal cannot take stays a `defineProperty` call.
fn skip_reason(
    plan: &AllocationPlan,
    key: &KeyInfo,
    literal_preseedable: bool,
) -> (PropertyWriteSkipReason, String) {
    if !literal_preseedable {
        (
            PropertyWriteSkipReason::LiteralNotEligible,
            "allocation literal contains a spread, accessor, or dynamic key and cannot be pre-seeded"
                .into(),
        )
    } else {
        match &plan.literal {
            AllocationLiteral::Array { .. } => {
                if key.array_index.is_some_and(|array_index| {
                    array_index as usize >= MAX_PRESEED_ARRAY_ELEMENTS
                }) {
                    (
                        PropertyWriteSkipReason::PreseedLimit,
                        format!(
                            "array index {} exceeds the pre-seed limit {}",
                            key.canonical, MAX_PRESEED_ARRAY_ELEMENTS
                        ),
                    )
                } else {
                    (
                        PropertyWriteSkipReason::KeyNotPreseedable,
                        "missing array property cannot be created safely in the literal".into(),
                    )
                }
            }
            AllocationLiteral::Object { .. } => (
                PropertyWriteSkipReason::KeyNotPreseedable,
                "missing object property cannot be appended to this literal".into(),
            ),
        }
    }
}

/// The edit adding the pre-seeded keys to the literal, if any.
fn preseed_edit(
    source: &str,
    plan: &AllocationPlan,
    group: &mut GroupPlan,
    array_preseed: &BTreeSet<u32>,
    array_preseed_ok: bool,
    object_preseed: &[String],
    object_preseed_ok: bool,
) {
    if array_preseed_ok && !array_preseed.is_empty() {
        if let AllocationLiteral::Array { span, elements, .. } = &plan.literal {
            group.preseed_edit = Some(analysis::Edit::replace(
                *span,
                build_array_literal(source, elements, array_preseed),
            ));
            group.preseeded_keys = array_preseed.len();
        }
    } else if object_preseed_ok && !object_preseed.is_empty() {
        if let AllocationLiteral::Object {
            span,
            property_count,
            last_property_end,
            ..
        } = &plan.literal
        {
            let (start, text) = build_object_preseed(
                source,
                *span,
                *property_count,
                *last_property_end,
                object_preseed,
            );
            group.preseed_edit = Some(analysis::Edit {
                start,
                end: start,
                replacement: text,
            });
            group.preseeded_keys = object_preseed.len();
        }
    }

}

pub(super) fn build_array_literal(
    source: &str,
    elements: &[Option<ArrayElementSource>],
    preseed: &BTreeSet<u32>,
) -> String {
    let existing_len = elements.len();
    let max_preseed = preseed.iter().next_back().copied().unwrap_or(0) as usize;
    let end = existing_len.saturating_sub(1).max(max_preseed);
    let mut text = String::from("[");
    for index in 0..=end {
        if preseed.contains(&(index as u32)) {
            text.push_str("void 0");
        } else if let Some(Some(element)) = elements.get(index) {
            let value = &source[element.span.start as usize..element.span.end as usize];
            if element.needs_parentheses {
                text.push('(');
                text.push_str(value);
                text.push(')');
            } else {
                text.push_str(value);
            }
        }
        text.push(',');
    }
    text.push(']');
    text
}

pub(super) fn build_object_preseed(
    _source: &str,
    span: Span,
    property_count: usize,
    last_property_end: Option<u32>,
    keys: &[String],
) -> (u32, String) {
    let entries = keys
        .iter()
        .map(|key| format!("[{}]: void 0", quote_js_string(key)))
        .collect::<Vec<_>>()
        .join(",");
    if property_count == 0 {
        (span.end - 1, entries)
    } else {
        let start = last_property_end.unwrap_or(span.end - 1);
        (start, format!(",{entries}"))
    }
}

pub(super) fn quote_js_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000C}' => out.push_str("\\f"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            character if (character as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => out.push(character),
        }
    }
    out.push('"');
    out
}

pub(super) fn unwrap_parens<'e, 'a>(mut expression: &'e Expression<'a>) -> &'e Expression<'a> {
    while let Expression::ParenthesizedExpression(parenthesized) = expression {
        expression = &parenthesized.expression;
    }
    expression
}
