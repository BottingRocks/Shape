//! Conservative proof of the lowered snapshot partition loop. Unsupported
//! shapes retain registry identity but never claim final payload positions.
use crate::signals::extract::bindings::{call_arguments, is_integer_array, object_members, Model};
use crate::signals::schema::{BindingRef, DescriptorRegistry, LiteralValue, SourceSpan};
use oxc_ast::{ast::*, AstKind};
use oxc_semantic::SymbolId;
use oxc_span::GetSpan;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PayloadSlot {
    pub descriptor_index: usize,
    pub default_filter_slot: usize,
    pub route: String,
    pub positional_index: Option<usize>,
    pub key: Option<LiteralValue>,
    pub reason: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PayloadRouting {
    pub snapshot_span: Option<SourceSpan>,
    pub partition_span: Option<SourceSpan>,
    pub default_filter: Option<BindingRef>,
    pub default_filter_indices: Vec<usize>,
    pub slots: Vec<PayloadSlot>,
    pub unresolved: Vec<String>,
    pub condition: String,
}

pub(crate) fn extract(
    model: &Model<'_, '_>,
    registry: Option<&DescriptorRegistry>,
) -> PayloadRouting {
    let mut result = PayloadRouting {
        snapshot_span: None, partition_span: None, default_filter: None,
        default_filter_indices: Vec::new(), slots: Vec::new(), unresolved: Vec::new(),
        condition: "Positions apply to the proven default signalFilter only. A caller-supplied filter can reorder, repeat or omit descriptors; indices must then be recomputed in filter order.".into(),
    };
    let Some(registry) = registry else {
        result.unresolved.push("descriptor registry absent".into());
        return result;
    };
    let mut proofs = Vec::new();
    for node in model.nodes.iter() {
        let AstKind::ReturnStatement(ret) = node.kind() else {
            continue;
        };
        let Some(site) = model.owning_site(node.id()) else {
            continue;
        };
        let Some(members) = ret.argument.as_ref().and_then(object_members) else {
            continue;
        };
        let sym = |key: &str| {
            members
                .iter()
                .find(|m| m.name == key)
                .and_then(|m| symbol(model, m.value))
        };
        let (Some(pos), Some(keyed), Some(named)) = (
            sym("positionalSignals"),
            sym("keyedSignals"),
            sym("signals"),
        ) else {
            continue;
        };
        for loop_node in model.nodes.iter() {
            let AstKind::ForStatement(loop_) = loop_node.kind() else {
                continue;
            };
            if model
                .owning_site(loop_node.id())
                .is_none_or(|s| s.span != site.span)
            {
                continue;
            }
            if let Some(filter) =
                prove_partition(model, loop_, registry.binding.symbol, pos, keyed, named)
            {
                proofs.push((site, loop_.span, filter));
            }
        }
    }
    if proofs.len() != 1 {
        result.unresolved.push(format!("expected one fully proven snapshot partition loop; found {} (unsupported or ambiguous shape)", proofs.len()));
        return result;
    }
    let (site, loop_span, filter) = proofs[0];
    result.snapshot_span = Some(site.span.into());
    result.partition_span = Some(loop_span.into());
    let Some(Expression::ConditionalExpression(choice)) = model.initializer(filter) else {
        result
            .unresolved
            .push("snapshot filter is not a proven default/custom conditional".into());
        return result;
    };
    let Some(default) = symbol(model, &choice.consequent) else {
        result
            .unresolved
            .push("default filter branch is not a binding".into());
        return result;
    };
    let default_guard = if let Expression::BinaryExpression(test) = &choice.test {
        test.operator.as_str() == "==="
            && is_void_zero(&test.right)
            && same_member(model, &test.left, &choice.alternate, "signalFilter")
    } else {
        false
    };
    if !default_guard {
        result
            .unresolved
            .push("default filter guard not proven".into());
        return result;
    }
    let Some(values) = model.symbol_value(default).and_then(is_integer_array) else {
        result
            .unresolved
            .push("default filter is not an exact static integer array".into());
        return result;
    };
    let expected_writes = usize::from(model.initializer(default).is_none());
    if model.binding_writes(default) != expected_writes {
        result
            .unresolved
            .push("default filter has multiple possible assignments".into());
        return result;
    }
    // The only read of the array must be the default branch. Any method call,
    // alias or indexed mutation would invalidate the static order proof.
    if model.scoping.get_resolved_references(default).filter(|r| r.is_read()).any(|r| {
        let node = model.nodes.get_node(r.node_id());
        if node.span() == choice.consequent.span() { return false; }
        let AstKind::ConditionalExpression(c) = model.nodes.parent_node(r.node_id()).kind() else { return true };
        if c.consequent.span() != node.span() { return true; }
        !matches!(&c.test, Expression::BinaryExpression(t) if t.operator.as_str()=="===" && is_void_zero(&t.right) && same_member(model,&t.left,&c.alternate,"signalFilter"))
    }) {
        result.unresolved.push("default filter has other read/alias/mutation sites".into()); return result;
    }
    result.default_filter = Some(model.binding_ref(default));
    result.default_filter_indices = values.clone();
    let mut position = Some(0usize);
    for (slot, index) in values.into_iter().enumerate() {
        let mut record = PayloadSlot {
            descriptor_index: index,
            default_filter_slot: slot,
            route: "unresolved".into(),
            positional_index: None,
            key: None,
            reason: None,
        };
        if let Some(entry) = registry.elements.iter().find(|e| e.index == index) {
            match descriptor_name(model, entry.span) {
                DescriptorName::Literal(name) => {
                    record.key = Some(name.clone());
                    match &name {
                        LiteralValue::Number(_) => record.route = "keyed".into(),
                        _ => record.route = "named".into(),
                    }
                }
                DescriptorName::Absent => {
                    record.route = "positional".into();
                    record.positional_index = position;
                    position = position.map(|p| p + 1);
                }
                DescriptorName::Dynamic => {
                    record.reason=Some("own name exists but its type/key is dynamic; named versus keyed unresolved".into());
                }
                DescriptorName::Unsupported => {
                    record.reason = Some(
                    "descriptor name presence/value cannot be proven from a plain static object"
                        .into(),
                );
                    position = None;
                }
            }
        } else {
            position = None;
            record.reason =
                Some("filter descriptor absent; subsequent positional offsets unproven".into());
        }
        result.slots.push(record);
    }
    result
}

enum DescriptorName {
    Absent,
    Literal(LiteralValue),
    Dynamic,
    Unsupported,
}
fn descriptor_name(model: &Model<'_, '_>, span: SourceSpan) -> DescriptorName {
    for node in model.nodes.iter() {
        let AstKind::ObjectExpression(object) = node.kind() else {
            continue;
        };
        if SourceSpan::from(object.span) != span {
            continue;
        }
        let mut seen = std::collections::BTreeSet::new();
        let mut name = DescriptorName::Absent;
        for property in &object.properties {
            let ObjectPropertyKind::ObjectProperty(p) = property else {
                return DescriptorName::Unsupported;
            };
            let Some(key) = crate::signals::extract::bindings::property_key_name(&p.key) else {
                return DescriptorName::Unsupported;
            };
            if p.computed || key == "__proto__" || !seen.insert(key.clone()) {
                return DescriptorName::Unsupported;
            }
            if key == "name" {
                name = crate::signals::extract::bindings::literal_value(&p.value)
                    .map_or(DescriptorName::Dynamic, DescriptorName::Literal);
            }
        }
        return name;
    }
    DescriptorName::Unsupported
}

fn symbol(model: &Model<'_, '_>, expr: &Expression<'_>) -> Option<SymbolId> {
    crate::analysis::expression_symbol(expr, model.scoping)
}
fn id(model: &Model<'_, '_>, expr: &Expression<'_>, expected: SymbolId) -> bool {
    symbol(model, expr) == Some(expected)
}
fn member<'e, 'a>(expr: &'e Expression<'a>, name: &str) -> Option<&'e Expression<'a>> {
    match expr {
        Expression::StaticMemberExpression(m) if m.property.name == name => Some(&m.object),
        _ => None,
    }
}
fn same_member(model: &Model<'_, '_>, a: &Expression<'_>, b: &Expression<'_>, name: &str) -> bool {
    member(a, name)
        .and_then(|a| symbol(model, a))
        .is_some_and(|s| member(b, name).is_some_and(|b| id(model, b, s)))
}
fn is_void_zero(expr: &Expression<'_>) -> bool {
    matches!(expr, Expression::UnaryExpression(u) if u.operator.as_str() == "void" && matches!(&u.argument, Expression::NumericLiteral(n) if n.value == 0.0))
}
fn binding(decl: &VariableDeclaration<'_>) -> Option<SymbolId> {
    (decl.declarations.len() == 1)
        .then(|| crate::signals::extract::bindings::binding_pattern_symbol(&decl.declarations[0].id))
        .flatten()
}
fn assignment<'e, 'a>(
    model: &Model<'_, '_>,
    statement: &'e Statement<'a>,
) -> Option<(SymbolId, &'e Expression<'a>)> {
    let Statement::ExpressionStatement(s) = statement else {
        return None;
    };
    let Expression::AssignmentExpression(a) = &s.expression else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(left) = &a.left else {
        return None;
    };
    if a.operator.as_str() != "=" {
        return None;
    }
    Some((
        crate::analysis::identifier_symbol(left, model.scoping)?,
        &a.right,
    ))
}
fn block<'e, 'a>(s: &'e Statement<'a>) -> Option<&'e [Statement<'a>]> {
    if let Statement::BlockStatement(b) = s {
        Some(&b.body)
    } else {
        None
    }
}
fn indexed(model: &Model<'_, '_>, expr: &Expression<'_>, base: SymbolId, index: SymbolId) -> bool {
    matches!(expr, Expression::ComputedMemberExpression(m) if id(model, &m.object, base) && id(model, &m.expression, index))
}
fn prove_partition(
    model: &Model<'_, '_>,
    loop_: &ForStatement<'_>,
    registry: u32,
    pos: SymbolId,
    keyed: SymbolId,
    named: SymbolId,
) -> Option<SymbolId> {
    let statements = block(&loop_.body)?;
    if statements.len() != 4 {
        return None;
    }
    let Statement::VariableDeclaration(descriptor) = &statements[0] else {
        return None;
    };
    let descriptor_symbol = binding(descriptor)?;
    let Expression::ComputedMemberExpression(entry) = descriptor.declarations[0].init.as_ref()?
    else {
        return None;
    };
    if symbol(model, &entry.object)?.index() as u32 != registry {
        return None;
    }
    let Expression::ComputedMemberExpression(selection) = &entry.expression else {
        return None;
    };
    let filter = symbol(model, &selection.object)?;
    let index = symbol(model, &selection.expression)?;
    let Expression::BinaryExpression(test) = loop_.test.as_ref()? else {
        return None;
    };
    if test.operator.as_str() != "<" || !id(model, &test.left, index) {
        return None;
    }
    let mapped = symbol(model, member(&test.right, "length")?)?;
    if model
        .scoping
        .get_resolved_references(filter)
        .any(|r| r.is_write())
    {
        return None;
    }
    // map preserves the filter length. Do not infer length/order from an
    // arbitrary same-spelled array, a slice/filter call, or a later overwrite.
    let writes: Vec<_> = model
        .scoping
        .get_resolved_references(mapped)
        .filter(|r| r.is_write())
        .collect();
    if writes.is_empty() {
        return None;
    }
    for write in writes {
        let parent = model.nodes.parent_node(write.node_id());
        let AstKind::AssignmentExpression(a) = parent.kind() else {
            return None;
        };
        let Expression::CallExpression(call) = &a.right else {
            return None;
        };
        if !member(&call.callee, "map").is_some_and(|e| id(model, e, filter))
            || call.arguments.len() != 1
        {
            return None;
        }
    }
    let Expression::UpdateExpression(update) = loop_.update.as_ref()? else {
        return None;
    };
    let SimpleAssignmentTarget::AssignmentTargetIdentifier(update_id) = &update.argument else {
        return None;
    };
    if update.operator.as_str() != "++"
        || crate::analysis::identifier_symbol(update_id, model.scoping)? != index
    {
        return None;
    }
    // The index has exactly one initialization to zero; no other writes beyond ++.
    let Expression::NumericLiteral(zero) = model.initializer(index)? else {
        return None;
    };
    if zero.value != 0.0
        || model
            .scoping
            .get_resolved_references(index)
            .filter(|r| r.is_write())
            .count()
            != 1
    {
        return None;
    }
    if loop_.init.is_some() {
        return None;
    }
    let Statement::VariableDeclaration(temporaries) = &statements[1] else {
        return None;
    };
    if temporaries.declarations.len() != 3
        || temporaries.declarations.iter().any(|d| d.init.is_some())
    {
        return None;
    }
    let Statement::IfStatement(branch) = &statements[2] else {
        return None;
    };
    let Expression::CallExpression(guard) = &branch.test else {
        return None;
    };
    let own = member(&guard.callee, "call")?;
    let prototype = member(own, "hasOwnProperty")?;
    let object = member(prototype, "prototype")?;
    if !matches!(object, Expression::Identifier(i) if i.name == "Object" && crate::analysis::identifier_symbol(i, model.scoping).is_none())
    {
        return None;
    }
    let args = call_arguments(guard);
    if args.len() != 2
        || !id(model, args[0], descriptor_symbol)
        || !matches!(args[1], Expression::StringLiteral(s) if s.value == "name")
    {
        return None;
    }
    let yes = block(&branch.consequent)?;
    let no = block(branch.alternate.as_ref()?)?;
    if yes.len() != 3 || no.len() != 3 {
        return None;
    }
    let (destination, choice) = assignment(model, &yes[0])?;
    let Expression::ConditionalExpression(choice) = choice else {
        return None;
    };
    let Expression::BinaryExpression(kind_test) = &choice.test else {
        return None;
    };
    let Expression::UnaryExpression(typeof_) = &kind_test.left else {
        return None;
    };
    if kind_test.operator.as_str() != "==="
        || typeof_.operator.as_str() != "typeof"
        || !member(&typeof_.argument, "name").is_some_and(|e| id(model, e, descriptor_symbol))
        || !matches!(&kind_test.right, Expression::StringLiteral(s) if s.value == "number")
        || !id(model, &choice.consequent, keyed)
        || !id(model, &choice.alternate, named)
    {
        return None;
    }
    let (push, push_expr) = assignment(model, &yes[1])?;
    if !member(push_expr, "push").is_some_and(|e| id(model, e, destination)) {
        return None;
    }
    let (item, pair) = assignment(model, &yes[2])?;
    let Expression::ArrayExpression(pair) = pair else {
        return None;
    };
    let pair: Vec<_> = pair
        .elements
        .iter()
        .filter_map(|e| e.as_expression())
        .collect();
    if pair.len() != 2
        || !member(pair[0], "name").is_some_and(|e| id(model, e, descriptor_symbol))
        || !indexed(model, pair[1], mapped, index)
    {
        return None;
    }
    let (npush, npush_expr) = assignment(model, &no[0])?;
    let (ndest, ndest_expr) = assignment(model, &no[1])?;
    let (nitem, nitem_expr) = assignment(model, &no[2])?;
    if npush != push
        || ndest != destination
        || nitem != item
        || !member(npush_expr, "push").is_some_and(|e| id(model, e, pos))
        || !id(model, ndest_expr, pos)
        || !indexed(model, nitem_expr, mapped, index)
    {
        return None;
    }
    let Statement::ExpressionStatement(end) = &statements[3] else {
        return None;
    };
    let Expression::CallExpression(call) = &end.expression else {
        return None;
    };
    let args = call_arguments(call);
    if !member(&call.callee, "call").is_some_and(|e| id(model, e, push))
        || args.len() != 2
        || !id(model, args[0], destination)
        || !id(model, args[1], item)
    {
        return None;
    }
    // Only these branch assignments may write the temporaries. Output arrays
    // must be distinct, initialized empty, and have no other read/use sites
    // beyond the partition and returned snapshot object.
    if [pos, keyed, named]
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        != 3
    {
        return None;
    }
    for output in [pos, keyed, named] {
        if !matches!(model.initializer(output), Some(Expression::ArrayExpression(a)) if a.elements.is_empty())
        {
            return None;
        }
        for r in model.scoping.get_resolved_references(output) {
            let node = model.nodes.get_node(r.node_id());
            if loop_.span.contains_inclusive(node.span()) {
                continue;
            }
            if !model
                .nodes
                .ancestors(r.node_id())
                .any(|n| matches!(n.kind(), AstKind::ReturnStatement(_)))
            {
                return None;
            }
        }
    }
    Some(filter)
}
