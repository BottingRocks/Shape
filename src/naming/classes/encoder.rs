//! Independent returned-native-class identity for the serializer's ValueEncoder.
//!
//! The proof links the returned class to its own serializer state, wire
//! operations and callbacks. Only the array-comparison helper is borrowed
//! from p06, at an exact stable callable span.
use crate::signals::extract::bindings::{call_arguments, property_key_name, static_member_name, FunctionSite, Model};
use crate::signals::analysis::roles::{FinderClaim, FinderCtx};
use crate::signals::schema::{ProgramPropertiesLimits, SourceSpan};
use oxc_ast::{ast::*, AstKind};
use oxc_semantic::SymbolId;
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, LogicalOperator};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub(crate) struct EncoderClassProof {
    pub class_span: Span,
    pub canonical_name: &'static str,
    pub evidence: Vec<String>,
}

/// A class identity witness, before generated-name filtering and rename hygiene.
#[derive(Clone, Debug)]
pub struct ValueEncoderClass {
    pub class_span: SourceSpan,
    pub canonical_name: &'static str,
    pub evidence: Vec<String>,
}

/// Parser-only entry point, also useful for testing independently of routing.
pub fn identify_value_encoder_classes(source: &str) -> Result<Vec<ValueEncoderClass>, String> {
    let allocator = oxc_allocator::Allocator::default();
    let program = crate::analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = crate::analysis::analyze(&program);
    let model = Model::new(
        source,
        &program,
        &scoping,
        &nodes,
        ProgramPropertiesLimits::default(),
    )
    .map_err(|error| error.to_string())?;
    let ctx = FinderCtx {
        source,
        program: &program,
        scoping: &scoping,
        nodes: &nodes,
    };
    let claims = crate::signals::analysis::roles::serializer::find(&ctx);
    Ok(prove(&model, &claims)
        .into_iter()
        .map(|proof| ValueEncoderClass {
            class_span: proof.class_span.into(),
            canonical_name: proof.canonical_name,
            evidence: proof.evidence,
        })
        .collect())
}

fn symbol(model: &Model<'_, '_>, value: &Expression<'_>) -> Option<SymbolId> {
    crate::analysis::expression_symbol(value.without_parentheses(), model.scoping)
}
fn fixed(model: &Model<'_, '_>, symbol: SymbolId) -> bool {
    model.binding_writes(symbol) == 0 && model.scoping.symbol_redeclarations(symbol).is_empty()
}
fn body<'a>(function: &'a Function<'a>) -> Option<&'a FunctionBody<'a>> {
    if function.r#async || function.generator {
        return None;
    }
    function
        .body
        .as_deref()
        .filter(|body| body.directives.is_empty())
}
fn expr<'a>(statement: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    match statement {
        Statement::ExpressionStatement(s) => Some(&s.expression),
        _ => None,
    }
}
fn member<'a>(value: &'a Expression<'a>) -> Option<(&'a Expression<'a>, &'a str)> {
    match value.without_parentheses() {
        Expression::StaticMemberExpression(m) => Some((&m.object, m.property.name.as_str())),
        Expression::ComputedMemberExpression(m) => match m.expression.without_parentheses() {
            Expression::StringLiteral(key) => Some((&m.object, key.value.as_str())),
            _ => None,
        },
        _ => None,
    }
}
fn this_field<'a>(value: &'a Expression<'a>) -> Option<&'a str> {
    let (receiver, key) = member(value)?;
    matches!(
        receiver.without_parentheses(),
        Expression::ThisExpression(_)
    )
    .then_some(key)
}
fn assignment<'a>(statement: &'a Statement<'a>) -> Option<&'a AssignmentExpression<'a>> {
    match expr(statement)?.without_parentheses() {
        Expression::AssignmentExpression(a) if a.operator == AssignmentOperator::Assign => Some(a),
        _ => None,
    }
}
fn target_symbol(model: &Model<'_, '_>, target: &AssignmentTarget<'_>) -> Option<SymbolId> {
    match target {
        AssignmentTarget::AssignmentTargetIdentifier(id) => {
            crate::analysis::identifier_symbol(id, model.scoping)
        }
        _ => None,
    }
}
fn target_this<'a>(target: &'a AssignmentTarget<'a>) -> Option<&'a str> {
    match target {
        AssignmentTarget::StaticMemberExpression(m)
            if matches!(
                m.object.without_parentheses(),
                Expression::ThisExpression(_)
            ) =>
        {
            Some(m.property.name.as_str())
        }
        AssignmentTarget::ComputedMemberExpression(m)
            if matches!(
                m.object.without_parentheses(),
                Expression::ThisExpression(_)
            ) =>
        {
            match m.expression.without_parentheses() {
                Expression::StringLiteral(s) => Some(s.value.as_str()),
                _ => None,
            }
        }
        _ => None,
    }
}
fn params(model: &Model<'_, '_>, function: &Function<'_>) -> Option<Vec<SymbolId>> {
    if function.params.rest.is_some() {
        return None;
    }
    function
        .params
        .items
        .iter()
        .map(|param| {
            if param.initializer.is_some() {
                return None;
            }
            let BindingPattern::BindingIdentifier(id) = &param.pattern else {
                return None;
            };
            let symbol = id.symbol_id.get()?;
            fixed(model, symbol).then_some(symbol)
        })
        .collect()
}

/// Only direct, ordered declarations/assignments are origins. A nested,
/// conditional, duplicate, compound or later write invalidates this result.
fn local_origin<'a>(
    model: &Model<'_, 'a>,
    function: &'a Function<'a>,
    wanted: SymbolId,
    before: u32,
) -> Option<&'a Expression<'a>> {
    if !function
        .span
        .contains_inclusive(model.scoping.symbol_span(wanted))
        || !model.scoping.symbol_redeclarations(wanted).is_empty()
    {
        return None;
    }
    let mut origin = None;
    let mut writes = 0;
    for statement in &body(function)?.statements {
        if let Statement::VariableDeclaration(declaration) = statement {
            for d in &declaration.declarations {
                if matches!(&d.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(wanted))
                {
                    if let Some(value) = &d.init {
                        if origin.replace(value).is_some() || value.span().end > before {
                            return None;
                        }
                    }
                }
            }
        } else if let Some(a) = assignment(statement) {
            if target_symbol(model, &a.left) == Some(wanted) {
                writes += 1;
                if origin.replace(&a.right).is_some() || a.span.end > before {
                    return None;
                }
            }
        }
    }
    (model.binding_writes(wanted) == writes).then_some(origin)?
}

/// Factory grammar is intentionally closed: declarations, one class, stable
/// identity aliases, then one return. Nothing executable can change the class.
fn returned_class<'a>(
    model: &Model<'_, 'a>,
    function: &'a Function<'a>,
) -> Option<(&'a Class<'a>, BTreeSet<SymbolId>)> {
    let statements = &body(function)?.statements;
    let Statement::ReturnStatement(returned) = statements.last()? else {
        return None;
    };
    let mut class = None;
    let mut declared = BTreeSet::new();
    let mut ready = BTreeSet::new();
    for statement in &statements[..statements.len() - 1] {
        match statement {
            Statement::ClassDeclaration(c) => {
                if class.replace(c.as_ref()).is_some() {
                    return None;
                }
                let id = c.id.as_ref()?.symbol_id.get()?;
                if !fixed(model, id) {
                    return None;
                }
                ready.insert(id);
            }
            Statement::VariableDeclaration(d) => {
                for variable in &d.declarations {
                    let BindingPattern::BindingIdentifier(id) = &variable.id else {
                        return None;
                    };
                    let id = id.symbol_id.get()?;
                    if !declared.insert(id) || !model.scoping.symbol_redeclarations(id).is_empty() {
                        return None;
                    }
                    if let Some(value) = &variable.init {
                        if !fixed(model, id) || !ready.contains(&symbol(model, value)?) {
                            return None;
                        }
                        ready.insert(id);
                    }
                }
            }
            _ => {
                let a = assignment(statement)?;
                let id = target_symbol(model, &a.left)?;
                if !declared.contains(&id)
                    || ready.contains(&id)
                    || model.binding_writes(id) != 1
                    || !ready.contains(&symbol(model, &a.right)?)
                {
                    return None;
                }
                ready.insert(id);
            }
        }
    }
    if ready.len() > 17
        || !declared.is_subset(&ready)
        || !ready.contains(&symbol(model, returned.argument.as_ref()?)?)
    {
        return None;
    }
    Some((class?, ready))
}

fn null(value: &Expression<'_>) -> bool {
    matches!(value.without_parentheses(), Expression::NullLiteral(_))
}
fn null_test(
    model: &Model<'_, '_>,
    value: &Expression<'_>,
    param: SymbolId,
    op: BinaryOperator,
) -> bool {
    matches!(value.without_parentheses(), Expression::BinaryExpression(b) if b.operator == op && symbol(model, &b.left) == Some(param) && null(&b.right))
}
fn null_pair(
    model: &Model<'_, '_>,
    value: &Expression<'_>,
    a: SymbolId,
    b: SymbolId,
    first: BinaryOperator,
    second: BinaryOperator,
) -> bool {
    matches!(value.without_parentheses(), Expression::LogicalExpression(e) if e.operator == LogicalOperator::And && null_test(model, &e.left, a, first) && null_test(model, &e.right, b, second))
}
fn constructor(model: &Model<'_, '_>, function: &Function<'_>) -> Option<()> {
    let ps = params(model, function)?;
    let [depth, extension] = ps.as_slice() else {
        return None;
    };
    let [Statement::IfStatement(guard), super_call, keys, hist, placeholders, remaining, point] =
        body(function)?.statements.as_slice()
    else {
        return None;
    };
    if guard.alternate.is_some() {
        return None;
    }
    let Expression::LogicalExpression(test) = guard.test.without_parentheses() else {
        return None;
    };
    if test.operator != LogicalOperator::Or
        || !null_pair(
            model,
            &test.left,
            *depth,
            *extension,
            BinaryOperator::Inequality,
            BinaryOperator::Equality,
        )
        || !null_pair(
            model,
            &test.right,
            *depth,
            *extension,
            BinaryOperator::Equality,
            BinaryOperator::Inequality,
        )
    {
        return None;
    }
    let Statement::BlockStatement(block) = &guard.consequent else {
        return None;
    };
    let [Statement::ThrowStatement(thrown)] = block.body.as_slice() else {
        return None;
    };
    let Expression::NewExpression(error) = thrown.argument.without_parentheses() else {
        return None;
    };
    if !matches!(&error.callee, Expression::Identifier(id) if id.name == "TypeError" && crate::analysis::identifier_symbol(id, model.scoping).is_none())
    {
        return None;
    }
    if !matches!(expr(super_call)?, Expression::CallExpression(call) if matches!(call.callee, Expression::Super(_)) && call.arguments.is_empty())
    {
        return None;
    }
    let values: Vec<_> = [keys, hist, placeholders, remaining, point]
        .into_iter()
        .map(assignment)
        .collect::<Option<_>>()?;
    for (a, key) in values.iter().zip([
        "keysets",
        "stringHist",
        "stringPlaceholders",
        "remainingDepth",
        "depthBoundExtensionPoint",
    ]) {
        if target_this(&a.left) != Some(key) {
            return None;
        }
    }
    if !matches!(values[0].right.without_parentheses(), Expression::ArrayExpression(a) if a.elements.is_empty())
        || !matches!(values[1].right.without_parentheses(), Expression::ObjectExpression(o) if o.properties.is_empty())
        || !matches!(values[2].right.without_parentheses(), Expression::BooleanLiteral(b) if b.value)
    {
        return None;
    }
    let Expression::ConditionalExpression(depth_value) = values[3].right.without_parentheses()
    else {
        return None;
    };
    if !null_test(model, &depth_value.test, *depth, BinaryOperator::Equality)
        || !null(&depth_value.consequent)
        || symbol(model, &depth_value.alternate) != Some(*depth)
    {
        return None;
    }
    let Expression::LogicalExpression(point_value) = values[4].right.without_parentheses() else {
        return None;
    };
    (point_value.operator == LogicalOperator::Coalesce
        && symbol(model, &point_value.left) == Some(*extension)
        && null(&point_value.right))
    .then_some(())
}

fn own_nodes<'m, 'a>(
    model: &'m Model<'_, 'a>,
    function: &Function<'_>,
) -> impl Iterator<Item = AstKind<'a>> + 'm {
    let start = function.span.start;
    model
        .nodes
        .iter()
        .filter(move |n| {
            model
                .owning_site(n.id())
                .is_some_and(|s| s.span.start == start)
        })
        .map(|n| n.kind())
}
fn calls_this(model: &Model<'_, '_>, function: &Function<'_>, key: &str) -> bool {
    own_nodes(model, function).any(
        |kind| matches!(kind, AstKind::CallExpression(c) if this_field(&c.callee) == Some(key)),
    )
}

/// Bind the callback argument slots and its receiver to
/// this method's own `this`, and its exact forEach argument (not a donor arrow).
fn push_receiver<'a>(model: &Model<'_, 'a>, function: &'a Function<'a>) -> Option<()> {
    for kind in own_nodes(model, function) {
        let AstKind::CallExpression(call) = kind else {
            continue;
        };
        let Some((receiver, "call")) = member(&call.callee) else {
            continue;
        };
        let Some((prototype, "forEach")) = member(receiver) else {
            continue;
        };
        let Some((array, "prototype")) = member(prototype) else {
            continue;
        };
        if !matches!(array, Expression::Identifier(id) if id.name == "Array" && crate::analysis::identifier_symbol(id, model.scoping).is_none())
        {
            continue;
        }
        let args = call_arguments(call);
        let [input, callback] = args.as_slice() else {
            continue;
        };
        if model.site_parameter_slot(input, model.site_by_span(function.span)?) != Some(0) {
            continue;
        }
        let callback_site = model.function_from_expression(callback)?;
        let AstKind::ArrowFunctionExpression(arrow) = model.nodes.kind(callback_site.node_id)
        else {
            continue;
        };
        if arrow.r#async || arrow.params.items.len() != 1 {
            continue;
        }
        let [statement] = arrow.body.statements.as_slice() else {
            continue;
        };
        let value = if arrow.expression {
            expr(statement)?
        } else {
            match statement {
                Statement::ExpressionStatement(s) => &s.expression,
                _ => continue,
            }
        };
        let Expression::CallExpression(inner) = value.without_parentheses() else {
            continue;
        };
        let Some((owner, "encodeValue")) = member(&inner.callee) else {
            continue;
        };
        let owner = local_origin(model, function, symbol(model, owner)?, call.span.start)?;
        if !matches!(owner.without_parentheses(), Expression::ThisExpression(_)) {
            continue;
        }
        let args = call_arguments(inner);
        let [element, output] = args.as_slice() else {
            continue;
        };
        if model.site_parameter_slot(element, callback_site) != Some(0) {
            continue;
        }
        let output_origin = local_origin(model, function, symbol(model, output)?, call.span.start)?;
        if model.site_parameter_slot(output_origin, model.site_by_span(function.span)?) == Some(1) {
            return Some(());
        }
    }
    None
}

/// Every static return must encode an instance constructed on that return's
/// own straight-line branch. Recovered seeds duplicate this tail across if
/// arms; a new in a sibling arm cannot prove the returned instance.
fn static_identity<'a>(
    model: &Model<'_, 'a>,
    function: &'a Function<'a>,
    identities: &BTreeSet<SymbolId>,
) -> Option<()> {
    fn terminal(statement: &Statement<'_>) -> bool {
        match statement {
            Statement::ReturnStatement(_) | Statement::ThrowStatement(_) => true,
            Statement::BlockStatement(block) => block.body.last().is_some_and(terminal),
            Statement::IfStatement(branch) => {
                terminal(&branch.consequent) && branch.alternate.as_ref().is_some_and(terminal)
            }
            _ => false,
        }
    }
    if !terminal(body(function)?.statements.last()?) {
        return None;
    }
    let returns: Vec<_> = own_nodes(model, function)
        .filter_map(|kind| match kind {
            AstKind::ReturnStatement(ret) => Some(ret),
            _ => None,
        })
        .collect();
    if returns.is_empty() {
        return None;
    }
    for ret in returns {
        let Expression::CallExpression(encode) = ret.argument.as_ref()?.without_parentheses()
        else {
            return None;
        };
        let (receiver, "encode") = member(&encode.callee)? else {
            return None;
        };
        let instance = symbol(model, receiver)?;
        let args = call_arguments(encode);
        if args.len() != 2
            || model.site_parameter_slot(args[0], model.site_by_span(function.span)?) != Some(0)
        {
            return None;
        }
        let options = symbol(model, args[1])?;
        let is_instance = |new: &NewExpression<'a>| {
            identities.contains(&symbol(model, &new.callee).unwrap_or(instance))
                && new.arguments.len() == 2
                && new
                    .arguments
                    .first()
                    .and_then(Argument::as_expression)
                    .and_then(member)
                    .is_some_and(|(receiver, key)| {
                        key == "depthBound" && symbol(model, receiver) == Some(options)
                    })
        };
        let mut origin = None;
        // Consider only direct statements from blocks containing this return.
        let mut blocks = vec![body(function)?.statements.as_slice()];
        for kind in own_nodes(model, function) {
            if let AstKind::BlockStatement(block) = kind {
                if block.span.contains_inclusive(ret.span) {
                    blocks.push(block.body.as_slice());
                }
            }
        }
        for statements in blocks {
            for statement in statements {
                if statement.span().end > ret.span.start {
                    continue;
                }
                if let Some(a) = assignment(statement) {
                    if target_symbol(model, &a.left) == Some(instance) {
                        origin = Some(a.right.without_parentheses());
                    }
                }
                if let Statement::VariableDeclaration(declaration) = statement {
                    for d in &declaration.declarations {
                        if matches!(&d.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(instance))
                        {
                            if let Some(value) = &d.init {
                                origin = Some(value.without_parentheses());
                            }
                        }
                    }
                }
            }
        }
        let Expression::NewExpression(new) = origin? else {
            return None;
        };
        if !is_instance(new) {
            return None;
        }
        let mut writes = 0;
        for kind in own_nodes(model, function) {
            if let AstKind::AssignmentExpression(a) = kind {
                if target_symbol(model, &a.left) == Some(instance) {
                    writes += 1;
                    if a.operator != AssignmentOperator::Assign
                        || !matches!(a.right.without_parentheses(), Expression::NewExpression(new) if is_instance(new))
                    {
                        return None;
                    }
                }
            }
        }
        if model.binding_writes(instance) != writes
            || !model.scoping.symbol_redeclarations(instance).is_empty()
        {
            return None;
        }
        let mut installed = false;
        for kind in own_nodes(model, function) {
            let AstKind::CallExpression(call) = kind else {
                continue;
            };
            if static_member_name(&call.callee) != Some("forEach")
                || call.span.start < new.span.end
                || call.span.end > ret.span.start
            {
                continue;
            }
            for arg in call_arguments(call) {
                let Some(callback) = model.function_from_expression(arg) else {
                    continue;
                };
                for n in model.nodes.iter().filter(|n| {
                    model
                        .owning_site(n.id())
                        .is_some_and(|s| s.span == callback.span)
                }) {
                    let AstKind::CallExpression(extend) = n.kind() else {
                        continue;
                    };
                    if !member(&extend.callee).is_some_and(|(owner, key)| {
                        key == "extend" && symbol(model, owner) == Some(instance)
                    }) {
                        continue;
                    }
                    let args = call_arguments(extend);
                    if args.len() != 4 || model.site_parameter_slot(args[0], callback) != Some(0) {
                        continue;
                    }
                    installed |= args[1..]
                        .iter()
                        .zip(["detector", "serialiser", "deserialiser"])
                        .all(|(arg, key)| {
                            let Some((entry, actual)) = member(arg) else {
                                return false;
                            };
                            let Expression::ComputedMemberExpression(entry) =
                                entry.without_parentheses()
                            else {
                                return false;
                            };
                            actual == key
                                && model.site_parameter_slot(&entry.expression, callback) == Some(0)
                                && member(&entry.object).is_some_and(|(receiver, key)| {
                                    key == "extensions" && symbol(model, receiver) == Some(options)
                                })
                        });
                }
            }
        }
        if !installed {
            return None;
        }
    }
    Some(())
}

pub(crate) fn prove<'a>(model: &Model<'_, 'a>, claims: &[FinderClaim]) -> Vec<EncoderClassProof> {
    let mut result = Vec::new();
    for site in model.all_sites() {
        let AstKind::Function(factory) = model.nodes.kind(site.node_id) else {
            continue;
        };
        let Some((class, identities)) = returned_class(model, factory) else {
            continue;
        };
        let Some(base_params) = params(model, factory) else {
            continue;
        };
        let [base] = base_params.as_slice() else {
            continue;
        };
        if class
            .super_class
            .as_ref()
            .and_then(|value| symbol(model, value))
            != Some(*base)
        {
            continue;
        }
        let mut methods = BTreeMap::new();
        let mut valid = true;
        for element in &class.body.body {
            let ClassElement::MethodDefinition(method) = element else {
                valid = false;
                break;
            };
            let Some(key) = property_key_name(&method.key) else {
                valid = false;
                break;
            };
            if method.computed
                || !matches!(
                    method.kind,
                    MethodDefinitionKind::Method | MethodDefinitionKind::Constructor
                )
                || body(&method.value).is_none()
                || methods
                    .insert((method.r#static, key), method.value.as_ref())
                    .is_some()
            {
                valid = false;
                break;
            }
        }
        let wanted = [
            (false, "constructor", "serializer_encoder_constructor"),
            (false, "encode", "serializer_encoder_encode"),
            (
                false,
                "findKeysetIndex",
                "serializer_encoder_find_keyset_index",
            ),
            (
                false,
                "pushArrayElements",
                "serializer_encoder_push_array_elements",
            ),
            (false, "encodeValue", "serializer_encoder_encode_value"),
            (true, "encode", "serializer_encoder_static_encode"),
        ];
        if !valid
            || methods.len() != wanted.len()
            || !wanted
                .iter()
                .all(|(is_static, key, _)| methods.contains_key(&(*is_static, key.to_string())))
        {
            continue;
        }
        let get = |is_static, key: &str| methods[&(is_static, key.to_string())];
        let ctor = get(false, "constructor");
        let value = get(false, "encodeValue");
        if constructor(model, ctor).is_none()
            || !(encode_contract(model, get(false, "encode"))
                || (transported_encode_contract(model, get(false, "encode"), value)
                    && transported_keyset_return(model, get(false, "findKeysetIndex"))
                    && value_output_return(model, value)))
            || !value_contract(model, value)
            || !keyset_contract(model, get(false, "findKeysetIndex"), claims)
            || push_receiver(model, get(false, "pushArrayElements")).is_none()
            || static_identity(model, get(true, "encode"), &identities).is_none()
            || !calls_this(model, get(false, "encode"), "encodeValue")
            || !calls_this(model, value, "findKeysetIndex")
        {
            continue;
        }
        // State and wire operations must belong to encodeValue itself.
        let mut reads = BTreeSet::new();
        let mut depth_writes = 0;
        for kind in own_nodes(model, value) {
            match kind {
                AstKind::StaticMemberExpression(m)
                    if matches!(
                        m.object.without_parentheses(),
                        Expression::ThisExpression(_)
                    ) =>
                {
                    reads.insert(m.property.name.as_str());
                }
                AstKind::AssignmentExpression(a)
                    if target_this(&a.left) == Some("remainingDepth") =>
                {
                    depth_writes += 1;
                }
                _ => {}
            }
        }
        if depth_writes != 2
            || ![
                "stringPlaceholders",
                "stringHist",
                "remainingDepth",
                "depthBoundExtensionPoint",
                "extensions",
            ]
            .iter()
            .all(|key| reads.contains(key))
        {
            continue;
        }
        result.push(EncoderClassProof { class_span: class.span, canonical_name: "ValueEncoder", evidence: vec![
            "returns this exact native class through ordered, single-origin scoped aliases".into(),
            "extends immutable base formal; own super constructor initializes the encoder state".into(),
            "owned serializer methods link state, tags, keyset search, recursive output and the exact constructed class".into(),
        ] });
    }
    result
}

fn slot<'a>(
    model: &Model<'_, 'a>,
    function: &'a Function<'a>,
    value: &'a Expression<'a>,
    before: u32,
) -> Option<u32> {
    let site = model.site_by_span(function.span)?;
    model.site_parameter_slot(value, site).or_else(|| {
        let origin = local_origin(model, function, symbol(model, value)?, before)?;
        model.site_parameter_slot(origin, site)
    })
}

/// A wire tag is accepted only from a stable scoped encoding-table object.
/// Tags on an unrelated receiver, or tags hidden in a callback, cannot supply
/// evidence to the caller. This is the same table vocabulary as p06.
fn wire_tag<'a>(model: &Model<'_, 'a>, value: &'a Expression<'a>) -> Option<(SymbolId, &'a str)> {
    let (table, tag) = member(value)?;
    let table = symbol(model, table)?;
    let Expression::ObjectExpression(object) =
        scoped_origin(model, table, value.span().start)?.without_parentheses()
    else {
        return None;
    };
    let keys: BTreeSet<_> = object
        .properties
        .iter()
        .filter_map(|p| match p {
            ObjectPropertyKind::ObjectProperty(p) if !p.computed && !p.method => {
                property_key_name(&p.key)
            }
            _ => None,
        })
        .collect();
    [
        "UINT14_BASE",
        "UINT16",
        "UINT24",
        "UINT32",
        "UINT64",
        "NINT4_BASE",
        "FLOAT32",
        "DOUBLE64",
        "STR5_BASE",
        "STRREF",
        "STRLUT",
        "ARRAY8",
        "BARRAY8",
        "MAP_",
        "BMAP_",
        "EXTENSION",
    ]
    .iter()
    .all(|key| keys.contains(*key))
    .then_some((table, tag))
}

fn encode_contract<'a>(model: &Model<'_, 'a>, function: &'a Function<'a>) -> bool {
    let Some(b) = body(function) else {
        return false;
    };
    let Some(Statement::ReturnStatement(ret)) = b.statements.last() else {
        return false;
    };
    let Some(output) = ret.argument.as_ref().and_then(|e| symbol(model, e)) else {
        return false;
    };
    let Some(Expression::ArrayExpression(array)) =
        local_origin(model, function, output, ret.span.start).map(Expression::without_parentheses)
    else {
        return false;
    };
    if !array.elements.is_empty() {
        return false;
    }
    let mut value = false;
    let mut keysets = false;
    let mut header = false;
    let mut hist = false;
    let mut placeholders = false;
    for kind in own_nodes(model, function) {
        match kind {
            AstKind::CallExpression(call) => {
                let args = call_arguments(call);
                if this_field(&call.callee) == Some("encodeValue") && args.len() == 2 {
                    value |= slot(model, function, args[0], call.span.start) == Some(0);
                    keysets |= this_field(args[0]) == Some("keysets");
                }
                if let Some((receiver, "push")) = member(&call.callee) {
                    header |= symbol(model, receiver) == Some(output)
                        && args.first().is_some_and(|a| {
                            wire_tag(model, a).is_some_and(|(_, tag)| tag == "STRLUT")
                        });
                }
                hist |= args.iter().any(|arg| this_field(arg) == Some("stringHist"));
            }
            AstKind::AssignmentExpression(a)
                if target_this(&a.left) == Some("stringPlaceholders") =>
            {
                placeholders |= a.operator == AssignmentOperator::Assign
                    && matches!(a.right.without_parentheses(), Expression::BooleanLiteral(b) if !b.value);
            }
            _ => {}
        }
    }
    value
        && keysets
        && header
        && hist
        && placeholders
        && calls_this(model, function, "pushArrayElements")
}

fn value_contract<'a>(model: &Model<'_, 'a>, function: &'a Function<'a>) -> bool {
    let mut tags: BTreeMap<SymbolId, BTreeSet<&str>> = BTreeMap::new();
    let mut depth = BTreeSet::new();
    for kind in own_nodes(model, function) {
        match kind {
            AstKind::CallExpression(call) => {
                let Some((receiver, "push")) = member(&call.callee) else {
                    continue;
                };
                if slot(model, function, receiver, call.span.start) != Some(1) {
                    continue;
                }
                for arg in call_arguments(call) {
                    if let Some((table, tag)) = wire_tag(model, arg) {
                        tags.entry(table).or_default().insert(tag);
                    }
                }
            }
            AstKind::AssignmentExpression(a) if target_this(&a.left) == Some("remainingDepth") => {
                let Expression::BinaryExpression(binary) = a.right.without_parentheses() else {
                    return false;
                };
                if a.operator != AssignmentOperator::Assign
                    || !matches!(
                        binary.operator,
                        BinaryOperator::Addition | BinaryOperator::Subtraction
                    )
                    || !matches!(binary.right.without_parentheses(), Expression::NumericLiteral(n) if n.value == 1.0)
                {
                    return false;
                }
                let Expression::CallExpression(number) = binary.left.without_parentheses() else {
                    return false;
                };
                if !matches!(number.callee.without_parentheses(), Expression::Identifier(id) if id.name == "Number" && crate::analysis::identifier_symbol(id, model.scoping).is_none())
                    || !matches!(call_arguments(number).as_slice(), [arg] if this_field(arg) == Some("remainingDepth"))
                {
                    return false;
                }
                depth.insert(binary.operator == BinaryOperator::Addition);
            }
            _ => {}
        }
    }
    depth.len() == 2
        && tags.values().any(|tags| {
            [
                "FALSE",
                "TRUE",
                "NULL",
                "UNDEFINED",
                "EXTENSION",
                "BINARY_",
                "MAP_",
                "BMAP_",
            ]
            .iter()
            .all(|tag| tags.contains(tag))
        })
        && calls_this(model, function, "encodeValue")
        && calls_this(model, function, "findKeysetIndex")
        && calls_this(model, function, "pushArrayElements")
}

fn keyset_contract<'a>(
    model: &Model<'_, 'a>,
    function: &'a Function<'a>,
    claims: &[FinderClaim],
) -> bool {
    let mut append = false;
    let mut search = false;
    let mut loop_found = false;
    for kind in own_nodes(model, function) {
        match kind {
            AstKind::ForStatement(_) => loop_found = true,
            AstKind::ReturnStatement(ret) => {
                let Some(Expression::BinaryExpression(binary)) =
                    ret.argument.as_ref().map(Expression::without_parentheses)
                else {
                    continue;
                };
                if binary.operator != BinaryOperator::Subtraction
                    || !matches!(binary.right.without_parentheses(), Expression::NumericLiteral(n) if n.value == 1.0)
                {
                    continue;
                }
                let Expression::CallExpression(call) = binary.left.without_parentheses() else {
                    continue;
                };
                let Some((receiver, "push")) = member(&call.callee) else {
                    continue;
                };
                append |= this_field(receiver) == Some("keysets")
                    && matches!(call_arguments(call).as_slice(), [arg] if slot(model, function, arg, call.span.start) == Some(0));
            }
            AstKind::CallExpression(call) => {
                let Some(matcher) = model.function_from_expression(&call.callee) else {
                    continue;
                };
                if !matcher.binding_symbol.is_some_and(|s| fixed(model, s)) {
                    continue;
                }
                let AstKind::Function(matcher_fn) = model.nodes.kind(matcher.node_id) else {
                    continue;
                };
                if !function.span.contains_inclusive(matcher.span) {
                    continue;
                }
                let Some([Statement::ReturnStatement(ret)]) =
                    body(matcher_fn).map(|b| b.statements.as_slice())
                else {
                    continue;
                };
                let Some(Expression::CallExpression(compare)) =
                    ret.argument.as_ref().map(Expression::without_parentheses)
                else {
                    continue;
                };
                let Some(comparer) = stable_function(model, &compare.callee, compare.span.start)
                else {
                    continue;
                };
                if !comparer.binding_symbol.is_some_and(|s| fixed(model, s)) || !claims.iter().any(|c| matches!(c, FinderClaim::Function { role: "serializer_arrays_equal", function_span, .. } if *function_span == comparer.span)) { continue; }
                let compare_args = call_arguments(compare);
                if !matches!(compare_args.as_slice(), [element, wanted] if model.site_parameter_slot(element, matcher) == Some(0) && slot(model, function, wanted, matcher.span.start) == Some(0))
                {
                    continue;
                }
                let args = call_arguments(call);
                let [element] = args.as_slice() else {
                    continue;
                };
                let Expression::ComputedMemberExpression(element) = element.without_parentheses()
                else {
                    continue;
                };
                let Some(origin) = symbol(model, &element.object)
                    .and_then(|s| local_origin(model, function, s, call.span.start))
                else {
                    continue;
                };
                search |= this_field(origin) == Some("keysets");
            }
            _ => {}
        }
    }
    append && search && loop_found
}

fn scoped_origin<'a>(
    model: &Model<'_, 'a>,
    symbol: SymbolId,
    before: u32,
) -> Option<&'a Expression<'a>> {
    let owner = model.owning_site(model.scoping.symbol_declaration(symbol))?;
    let AstKind::Function(function) = model.nodes.kind(owner.node_id) else {
        return None;
    };
    local_origin(model, function, symbol, before)
}

fn stable_function<'a>(
    model: &Model<'_, 'a>,
    mut value: &'a Expression<'a>,
    mut before: u32,
) -> Option<FunctionSite> {
    let mut seen = BTreeSet::new();
    for _ in 0..16 {
        if let Expression::FunctionExpression(f) = value.without_parentheses() {
            return model.site_by_span(f.span);
        }
        let symbol = symbol(model, value)?;
        if !seen.insert(symbol) {
            return None;
        }
        if let AstKind::Function(function) =
            model.nodes.kind(model.scoping.symbol_declaration(symbol))
        {
            return fixed(model, symbol)
                .then(|| model.site_by_span(function.span))
                .flatten();
        }
        value = scoped_origin(model, symbol, before)?;
        before = value.span().start;
    }
    None
}

/// An additive projection for a fresh array returned through a truthiness guard
/// or an exact [output, output.push] carrier. The old direct proof stays intact.
/// All carrier origins are direct statements in the actual header block; no
/// subtree assignment search or final symbol value supplies an origin.
fn transported_encode_contract<'a>(
    model: &Model<'_, 'a>,
    function: &'a Function<'a>,
    value: &'a Function<'a>,
) -> bool {
    fn single<'s, 'a>(statement: &'s Statement<'a>) -> Option<&'s Statement<'a>> {
        match statement {
            Statement::BlockStatement(block) => match block.body.as_slice() {
                [statement] => Some(statement),
                _ => None,
            },
            _ => None,
        }
    }
    let Some(b) = body(function) else {
        return false;
    };
    let Some(last) = b.statements.last() else {
        return false;
    };
    let returned = match last {
        Statement::ReturnStatement(ret) => ret,
        Statement::IfStatement(branch) => {
            let Some(Statement::ReturnStatement(ret)) = single(&branch.consequent) else {
                return false;
            };
            if !matches!(
                branch.alternate.as_ref().and_then(|s| single(s)),
                Some(Statement::ThrowStatement(_))
            ) || symbol(model, &branch.test)
                != ret.argument.as_ref().and_then(|a| symbol(model, a))
            {
                return false;
            }
            ret
        }
        _ => return false,
    };
    let Some(output) = returned.argument.as_ref().and_then(|a| symbol(model, a)) else {
        return false;
    };
    let Some(Expression::ArrayExpression(array)) =
        local_origin(model, function, output, last.span().start)
            .map(Expression::without_parentheses)
    else {
        return false;
    };
    if !array.elements.is_empty()
        || own_nodes(model, function)
            .filter(|k| matches!(k, AstKind::ReturnStatement(_)))
            .count()
            != 1
    {
        return false;
    }
    // The two encoder calls and histogram origin must be live, direct method
    // statements before the header, not dormant donors in a sibling branch.
    let mut encoded_value = None;
    let mut encoded_keysets = None;
    let mut histogram = None;
    let mut placeholders = false;
    for (statement_index, statement) in b.statements.iter().enumerate() {
        if let Statement::VariableDeclaration(declaration) = statement {
            for d in &declaration.declarations {
                let Some(Expression::CallExpression(call)) =
                    d.init.as_ref().map(Expression::without_parentheses)
                else {
                    continue;
                };
                let args = call_arguments(call);
                if this_field(&call.callee) == Some("encodeValue") {
                    if !matches!(args.as_slice(), [_, array] if matches!(array.without_parentheses(), Expression::ArrayExpression(a) if a.elements.is_empty()))
                    {
                        return false;
                    }
                    let BindingPattern::BindingIdentifier(id) = &d.id else {
                        return false;
                    };
                    let Some(chunk) = id.symbol_id.get() else {
                        return false;
                    };
                    if !plain_encoder_call(call)
                        || !model.scoping.symbol_redeclarations(chunk).is_empty()
                    {
                        return false;
                    }
                    if slot(model, function, args[0], call.span.start) == Some(0) {
                        if encoded_value.replace(chunk).is_some() {
                            return false;
                        }
                    } else if this_field(args[0]) == Some("keysets") {
                        if encoded_value.is_none() || encoded_keysets.replace(chunk).is_some() {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
                if matches!(args.as_slice(), [hist] if this_field(hist) == Some("stringHist")) {
                    let BindingPattern::BindingIdentifier(id) = &d.id else {
                        return false;
                    };
                    let Some(s) = id.symbol_id.get() else {
                        return false;
                    };
                    if !fixed(model, s) || histogram.replace(s).is_some() {
                        return false;
                    }
                }
            }
        }
        if let Some(a) = assignment(statement) {
            if target_this(&a.left) == Some("stringPlaceholders") {
                placeholders = matches!(a.right.without_parentheses(), Expression::BooleanLiteral(v) if !v.value);
            }
        }
        let Statement::IfStatement(branch) = statement else {
            continue;
        };
        let Some(histogram) = histogram else { continue };
        let (Some(payload), Some(keysets)) = (encoded_value, encoded_keysets) else {
            continue;
        };
        if !placeholders || branch.alternate.is_some() {
            continue;
        }
        let Expression::LogicalExpression(condition) = branch.test.without_parentheses() else {
            continue;
        };
        let positive_length = |e: &Expression<'a>, own_keysets: bool| {
            let Expression::BinaryExpression(c) = e.without_parentheses() else {
                return false;
            };
            c.operator == BinaryOperator::GreaterThan
                && matches!(c.right.without_parentheses(), Expression::NumericLiteral(n) if n.value == 0.0)
                && member(&c.left).is_some_and(|(receiver, key)| {
                    key == "length"
                        && if own_keysets {
                            this_field(receiver) == Some("keysets")
                        } else {
                            symbol(model, receiver) == Some(histogram)
                        }
                })
        };
        if condition.operator != LogicalOperator::Or
            || !positive_length(&condition.left, false)
            || !positive_length(&condition.right, true)
        {
            continue;
        }
        let Statement::BlockStatement(header) = &branch.consequent else {
            continue;
        };
        let mut cursor = 0;
        let Some(tag) = header_push(model, &header.body, &mut cursor, output) else {
            continue;
        };
        let Some((table, "STRLUT")) = wire_tag(model, tag) else {
            continue;
        };
        let Some(length) = header_push(model, &header.body, &mut cursor, output) else {
            continue;
        };
        if !member(length).is_some_and(|(receiver, key)| {
            key == "length" && symbol(model, receiver) == Some(histogram)
        }) {
            continue;
        }
        let Some(Expression::CallExpression(writer)) = header
            .body
            .get(cursor)
            .and_then(expr)
            .map(Expression::without_parentheses)
        else {
            continue;
        };
        if !plain_encoder_call(writer)
            || this_field(&writer.callee) != Some("pushArrayElements")
            || !matches!(call_arguments(writer).as_slice(), [hist, out] if symbol(model, hist) == Some(histogram) && symbol(model, out) == Some(output))
        {
            continue;
        }
        let carriers: BTreeSet<_> = header.body[..cursor]
            .iter()
            .filter_map(|s| {
                let Statement::VariableDeclaration(d) = s else {
                    return None;
                };
                let [d] = d.declarations.as_slice() else {
                    return None;
                };
                let Expression::ArrayExpression(a) = d.init.as_ref()?.without_parentheses() else {
                    return None;
                };
                Some((a.span.start, a.span.end))
            })
            .collect();
        cursor += 1;
        // The sole permitted assembly is payload = keysets.concat(payload).
        // Both origins are the exact preceding owned encodeValue results.
        // The only post-header effect is consuming this assembled payload.
        let [assembly] = &header.body[cursor..] else {
            continue;
        };
        let Some(assembly) = assignment(assembly) else {
            continue;
        };
        let Expression::CallExpression(concat) = assembly.right.without_parentheses() else {
            continue;
        };
        let Some((receiver, "concat")) = member(&concat.callee) else {
            continue;
        };
        let concat_args = call_arguments(concat);
        let [tail] = concat_args.as_slice() else {
            continue;
        };
        if !plain_encoder_call(concat)
            || target_symbol(model, &assembly.left) != Some(payload)
            || symbol(model, receiver) != Some(keysets)
            || symbol(model, tail) != Some(payload)
            || matches!(concat.callee.without_parentheses(), Expression::StaticMemberExpression(m) if m.optional)
        {
            continue;
        }
        if !owned_encode_prefix(
            model,
            function,
            statement_index,
            output,
            payload,
            keysets,
            histogram,
        ) {
            return false;
        }
        let [consumer_statement, terminal] = &b.statements[statement_index + 1..] else {
            continue;
        };
        if terminal.span() != last.span() {
            continue;
        }
        let Some(Expression::CallExpression(consumer)) =
            expr(consumer_statement).map(Expression::without_parentheses)
        else {
            continue;
        };
        let Some((consumed, "forEach")) = member(&consumer.callee) else {
            continue;
        };
        let consumer_args = call_arguments(consumer);
        let [callback] = consumer_args.as_slice() else {
            continue;
        };
        let Expression::ArrowFunctionExpression(callback) = callback.without_parentheses() else {
            continue;
        };
        if !plain_encoder_call(consumer)
            || symbol(model, consumed) != Some(payload)
            || matches!(consumer.callee.without_parentheses(), Expression::StaticMemberExpression(m) if m.optional)
            || !chunk_references(
                model,
                payload,
                &[assembly.left.span(), tail.span(), consumed.span()],
                1,
            )
            || !chunk_references(model, keysets, &[receiver.span()], 0)
        {
            continue;
        }
        let Some(proven_consumer) =
            owned_chunk_consumer(model, value, callback, output, histogram, table)
        else {
            continue;
        };
        let Expression::BinaryExpression(hist_condition) = condition.left.without_parentheses()
        else {
            continue;
        };
        let Some((condition_hist, "length")) = member(&hist_condition.left) else {
            continue;
        };
        let Some((length_hist, "length")) = member(length) else {
            continue;
        };
        let writer_args = call_arguments(writer);
        let histogram_operands = [
            condition_hist.span(),
            length_hist.span(),
            writer_args[0].span(),
            callback.span,
        ];
        if !model.scoping.get_resolved_references(histogram).all(|r| {
            histogram_operands
                .iter()
                .any(|span| span.contains_inclusive(model.nodes.kind(r.node_id()).span()))
        }) {
            continue;
        }
        let mut value_table = false;
        let mut foreign_table = false;
        for kind in own_nodes(model, value) {
            let AstKind::CallExpression(call) = kind else {
                continue;
            };
            let Some((receiver, "push")) = member(&call.callee) else {
                continue;
            };
            if slot(model, value, receiver, call.span.start) != Some(1) {
                continue;
            }
            for arg in call_arguments(call) {
                if let Some((actual, _)) = wire_tag(model, arg) {
                    value_table |= actual == table;
                    foreign_table |= actual != table;
                }
            }
        }
        return value_table
            && !foreign_table
            && table_reads_only(model, table)
            && output_uses(
                model,
                function,
                output,
                header.span.start,
                &carriers,
                &proven_consumer,
                &mut BTreeSet::new(),
            );
    }
    false
}

/// Consume exactly one direct push, optionally preceded by its immutable tuple
/// declaration. A carrier has exactly the two reads in this actual call.
fn header_push<'a>(
    model: &Model<'_, 'a>,
    statements: &'a [Statement<'a>],
    cursor: &mut usize,
    output: SymbolId,
) -> Option<&'a Expression<'a>> {
    let statement = statements.get(*cursor)?;
    if let Some(Expression::CallExpression(call)) =
        expr(statement).map(Expression::without_parentheses)
    {
        let (receiver, "push") = member(&call.callee)? else {
            return None;
        };
        let args = call_arguments(call);
        let [tag] = args.as_slice() else { return None };
        if !plain_encoder_call(call)
            || matches!(call.callee.without_parentheses(), Expression::StaticMemberExpression(m) if m.optional)
            || symbol(model, receiver) != Some(output)
        {
            return None;
        }
        *cursor += 1;
        return Some(*tag);
    }
    let Statement::VariableDeclaration(declaration) = statement else {
        return None;
    };
    let [d] = declaration.declarations.as_slice() else {
        return None;
    };
    let BindingPattern::BindingIdentifier(id) = &d.id else {
        return None;
    };
    let carrier = id.symbol_id.get()?;
    if !fixed(model, carrier) {
        return None;
    }
    let Expression::ArrayExpression(tuple) = d.init.as_ref()?.without_parentheses() else {
        return None;
    };
    let [receiver, method] = tuple.elements.as_slice() else {
        return None;
    };
    let receiver = receiver.as_expression()?;
    let method = method.as_expression()?;
    if matches!(method.without_parentheses(), Expression::StaticMemberExpression(m) if m.optional) {
        return None;
    }
    let (owner, "push") = member(method)? else {
        return None;
    };
    if symbol(model, receiver) != Some(output) || symbol(model, owner) != Some(output) {
        return None;
    }
    let Expression::CallExpression(call) =
        expr(statements.get(*cursor + 1)?)?.without_parentheses()
    else {
        return None;
    };
    let (method, "call") = member(&call.callee)? else {
        return None;
    };
    let is_slot = |value: &Expression<'_>, index: f64| {
        matches!(value.without_parentheses(), Expression::ComputedMemberExpression(m)
        if !m.optional && symbol(model, &m.object) == Some(carrier)
        && matches!(m.expression.without_parentheses(), Expression::NumericLiteral(n) if n.value == index))
    };
    let args = call_arguments(call);
    let [receiver, tag] = args.as_slice() else {
        return None;
    };
    if !plain_encoder_call(call)
        || !is_slot(method, 1.0)
        || !is_slot(receiver, 0.0)
        || model.binding_references(carrier) != 2
        || !model.scoping.get_resolved_references(carrier).all(|r| {
            call.span
                .contains_inclusive(model.nodes.kind(r.node_id()).span())
        })
    {
        return None;
    }
    *cursor += 2;
    Some(*tag)
}

/// The table itself may be initialized once by assignment, but it may never
/// escape or have a property written, deleted, or exposed as a call receiver.
fn table_reads_only(model: &Model<'_, '_>, table: SymbolId) -> bool {
    model
        .scoping
        .get_resolved_references(table)
        .all(|reference| {
            if reference.is_write() {
                return true;
            } // scoped_origin already checks the unique origin.
            let parent = model.nodes.parent_id(reference.node_id());
            let AstKind::StaticMemberExpression(_) = model.nodes.kind(parent) else {
                return false;
            };
            match model.nodes.parent_kind(parent) {
                AstKind::AssignmentExpression(_) | AstKind::UpdateExpression(_) => false,
                AstKind::UnaryExpression(u) => {
                    u.operator != oxc_syntax::operator::UnaryOperator::Delete
                }
                AstKind::CallExpression(c) => c.callee.span() != model.nodes.kind(parent).span(),
                _ => true,
            }
        })
}

/// Follow immutable output aliases outside the exact proved consumer callback.
/// Inside it, the complete owned grammar already proves every write and call;
/// outside it, only the existing two array-writing APIs are permitted.
fn output_uses(
    model: &Model<'_, '_>,
    function: &Function<'_>,
    output: SymbolId,
    ready: u32,
    carriers: &BTreeSet<(u32, u32)>,
    consumer: &OwnedChunkConsumer,
    seen: &mut BTreeSet<SymbolId>,
) -> bool {
    if !seen.insert(output) || seen.len() > 16 {
        return false;
    }
    model.scoping.get_resolved_references(output).all(|reference| {
        if reference.is_write() { return true; } // local_origin checked all binding writes.
        let span = model.nodes.kind(reference.node_id()).span();
        if span.start < ready || !function.span.contains_inclusive(span) { return false; }
        // This exact callback has already proved every output/alias use,
        // including STR_ length emission through its joined unsigned writer.
        // Do not reinterpret that writer as an arbitrary external escape.
        if output == consumer.output && consumer.callback_span.contains_inclusive(span)
            && model.owning_site(reference.node_id()).is_some_and(|site| site.span == consumer.callback_span)
        { return true; }
        let parent = model.nodes.parent_id(reference.node_id());
        match model.nodes.kind(parent) {
            AstKind::ReturnStatement(ret) => ret.argument.as_ref().is_some_and(|v| symbol(model, v) == Some(output)),
            AstKind::IfStatement(branch) => symbol(model, &branch.test) == Some(output),
            AstKind::VariableDeclarator(d) => {
                let BindingPattern::BindingIdentifier(id) = &d.id else { return false };
                id.symbol_id.get().is_some_and(|alias| fixed(model, alias) && output_uses(model, function, alias, d.span.end, carriers, consumer, seen))
            }
            AstKind::StaticMemberExpression(m) if m.property.name == "push" => {
                match model.nodes.parent_kind(parent) {
                    AstKind::CallExpression(call) => member(&call.callee).is_some_and(|(receiver, key)| key == "push" && symbol(model, receiver) == Some(output)),
                    AstKind::ArrayExpression(a) => carriers.contains(&(a.span.start, a.span.end)),
                    _ => false,
                }
            }
            AstKind::ArrayExpression(a) => carriers.contains(&(a.span.start, a.span.end)),
            AstKind::CallExpression(call) => {
                let args = call_arguments(call);
                if this_field(&call.callee) == Some("pushArrayElements") {
                    return matches!(args.as_slice(), [_, out] if symbol(model, out) == Some(output));
                }
                let Some((push, "apply")) = member(&call.callee) else { return false };
                let Some((prototype, "push")) = member(push) else { return false };
                let Some((array, "prototype")) = member(prototype) else { return false };
                matches!(array.without_parentheses(), Expression::Identifier(id) if id.name == "Array" && crate::analysis::identifier_symbol(id, model.scoping).is_none())
                    && matches!(args.as_slice(), [out, _] if symbol(model, out) == Some(output))
            }
            _ => false,
        }
    })
}

// These checks apply only to the additive projection. A duplicated found-index
// local needs a separate agreement proof; accepting an arbitrary returned local
// just because a comparer exists elsewhere would be unsound.
fn transported_keyset_return(model: &Model<'_, '_>, function: &Function<'_>) -> bool {
    let Some(b) = body(function) else {
        return false;
    };
    let Some(Statement::IfStatement(branch)) = b.statements.last() else {
        return false;
    };
    let Expression::BinaryExpression(test) = branch.test.without_parentheses() else {
        return false;
    };
    let Some(found) = symbol(model, &test.left) else {
        return false;
    };
    if test.operator != BinaryOperator::LessThan
        || !matches!(test.right.without_parentheses(), Expression::NumericLiteral(n) if n.value == 0.0)
        || own_nodes(model, function)
            .filter(|n| matches!(n, AstKind::ReturnStatement(_)))
            .count()
            != 2
    {
        return false;
    }
    let Some(Statement::BlockStatement(other)) = branch.alternate.as_ref() else {
        return false;
    };
    let [Statement::ReturnStatement(ret)] = other.body.as_slice() else {
        return false;
    };
    ret.argument.as_ref().and_then(|a| symbol(model, a)) == Some(found)
}

fn value_output_return(model: &Model<'_, '_>, function: &Function<'_>) -> bool {
    let Some(Statement::ReturnStatement(_)) = body(function).and_then(|b| b.statements.last())
    else {
        return false;
    };
    own_nodes(model, function).all(|kind| {
        let AstKind::ReturnStatement(ret) = kind else {
            return true;
        };
        ret.argument
            .as_ref()
            .is_some_and(|a| slot(model, function, a, ret.span.start) == Some(1))
    })
}

/// Every chunk reference must be one of the exact assembly/consumer operands.
/// No extra write, alias, property mutation, escape or donor callable is allowed.
fn chunk_references(
    model: &Model<'_, '_>,
    chunk: SymbolId,
    operands: &[Span],
    writes: usize,
) -> bool {
    model.binding_writes(chunk) == writes
        && model.scoping.symbol_redeclarations(chunk).is_empty()
        && model
            .scoping
            .get_resolved_references(chunk)
            .all(|reference| operands.contains(&model.nodes.kind(reference.node_id()).span()))
}

/// Closed AST contract for the current direct string/byte consumer. Scoped
/// bindings are alpha-normalized at semantic identifier spans, then the complete
/// parsed arrow is canonically printed and compared with the grammar below.
/// This retains every branch, operator, operand, effect and statement order.
/// It never matches a source substring or borrows a callback from another call.
fn owned_chunk_consumer<'a>(
    model: &Model<'_, 'a>,
    value: &'a Function<'a>,
    callback: &'a ArrowFunctionExpression<'a>,
    output: SymbolId,
    histogram: SymbolId,
    table: SymbolId,
) -> Option<OwnedChunkConsumer> {
    let actual = normalized_chunk_consumer(model, value, callback, output, histogram, table)?;
    static EXPECTED: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    (EXPECTED
        .get_or_init(|| canonical_consumer(CONSUMER_CONTRACT))
        .as_ref()
        == Some(&actual))
    .then_some(OwnedChunkConsumer {
        callback_span: callback.span,
        output,
    })
}

/// Only the complete canonical consumer proof constructs this witness. Its
/// output binding and actual callback span are joined to output_uses; a span
/// guessed from another callback or an arbitrary named writer is not evidence.
struct OwnedChunkConsumer {
    callback_span: Span,
    output: SymbolId,
}

// Local numbers follow semantic binding declaration order, never source names.
// The external length-writer identity is joined to the owned encodeValue writer.
const CONSUMER_CONTRACT: &str = r#"(__l0 => {
    if (typeof __l0 === "string") {
        let __l1 = __out;
        const __l2 = __hist ? __hist.indexOf(__l0) : -1;
        if (__l2 !== -1) {
            __l1.push(__table.STRREF, __l2);
        } else {
            let __l3 = [];
            let __l4 = unescape(encodeURIComponent(__l0));
            let __l5 = 0;
            let __l6 = false;
            for (; __l5 < __l4.length; __l5++) {
                __l3.push(__l4.charCodeAt(__l5));
                if (__l3[__l5] === 0) { __l6 = true; }
            }
            let __l7 = __l3.length;
            if (__l7 < 32) {
                __l1.push(__table.STR5_BASE | __l7);
            } else if (!__l6) {
                __l1.push(__table.CSTRING);
                __l3.push(0);
            } else if (__l7 <= 255) {
                __l1.push(__table.STR8, __l7);
            } else {
                __l1.push(__table.STR_);
                __length(__l7, __l1);
            }
            if (__l3.length > 65535) {
                for (let __l8 = 0; __l8 < __l3.length; __l8 = __l8 + 65535) {
                    Array.prototype.push.apply(__l1, __l3.slice(__l8, __l8 + 65535));
                }
            } else {
                Array.prototype.push.apply(__l1, __l3);
            }
        }
    } else {
        __out.push(__l0);
    }
});"#;

fn canonical_consumer(source: &str) -> Option<String> {
    let allocator = oxc_allocator::Allocator::default();
    let program = crate::analysis::parse_program(&allocator, source).ok()?;
    Some(
        oxc_codegen::Codegen::new()
            .with_options(oxc_codegen::CodegenOptions {
                comments: oxc_codegen::CommentOptions::disabled(),
                ..oxc_codegen::CodegenOptions::default()
            })
            .build(&program)
            .code,
    )
}

fn normalized_chunk_consumer<'a>(
    model: &Model<'_, 'a>,
    value: &'a Function<'a>,
    callback: &'a ArrowFunctionExpression<'a>,
    output: SymbolId,
    histogram: SymbolId,
    table: SymbolId,
) -> Option<String> {
    if callback.r#async
        || callback.params.rest.is_some()
        || callback.params.items.len() != 1
        || callback.expression
        || !callback.body.directives.is_empty()
        || callback.span.size() > 12000
    {
        return None;
    }
    // Bind the unsigned length writer at its actual encodeValue use: immediately
    // after its own EXTENSION tag, with Number(extension-index) and output slot 1.
    let mut writer = None;
    for kind in own_nodes(model, value) {
        let AstKind::BlockStatement(block) = kind else {
            continue;
        };
        let [first, second, ..] = block.body.as_slice() else {
            continue;
        };
        let Some(Expression::CallExpression(push)) =
            expr(first).map(Expression::without_parentheses)
        else {
            continue;
        };
        let Some((receiver, "push")) = member(&push.callee) else {
            continue;
        };
        if slot(model, value, receiver, push.span.start) != Some(1)
            || !matches!(call_arguments(push).as_slice(), [tag] if wire_tag(model, tag) == Some((table, "EXTENSION")))
        {
            continue;
        }
        let Some(Expression::CallExpression(call)) =
            expr(second).map(Expression::without_parentheses)
        else {
            continue;
        };
        let Expression::Identifier(id) = call.callee.without_parentheses() else {
            continue;
        };
        let args = call_arguments(call);
        let [number, out] = args.as_slice() else {
            continue;
        };
        let Expression::CallExpression(number) = number.without_parentheses() else {
            continue;
        };
        if !matches!(number.callee.without_parentheses(), Expression::Identifier(n) if n.name == "Number" && crate::analysis::identifier_symbol(n, model.scoping).is_none())
            || call_arguments(number).len() != 1
            || !plain_encoder_call(call)
            || !plain_encoder_call(number)
            || !plain_encoder_call(push)
            || slot(model, value, out, call.span.start) != Some(1)
        {
            continue;
        }
        let identity = (
            crate::analysis::identifier_symbol(id, model.scoping),
            id.name.as_str(),
        );
        if identity.0.is_some_and(|s| !fixed(model, s)) || writer.replace(identity).is_some() {
            return None;
        }
    }
    let (writer_symbol, writer_name) = writer?;
    let mut locals: Vec<_> = model
        .scoping
        .symbol_ids()
        .filter(|s| {
            callback
                .span
                .contains_inclusive(model.scoping.symbol_span(*s))
        })
        .collect();
    locals.sort_by_key(|s| model.scoping.symbol_span(*s).start);
    if locals.len() != 9
        || locals
            .iter()
            .any(|s| !model.scoping.symbol_redeclarations(*s).is_empty())
    {
        return None;
    }
    let mut names = BTreeMap::from([
        (output, "__out".to_string()),
        (histogram, "__hist".to_string()),
        (table, "__table".to_string()),
    ]);
    if names.len() != 3 {
        return None;
    }
    if let Some(writer) = writer_symbol {
        if names.insert(writer, "__length".into()).is_some() {
            return None;
        }
    }
    for (index, local) in locals.iter().enumerate() {
        if names.insert(*local, format!("__l{index}")).is_some() {
            return None;
        }
    }
    let mut edits = BTreeMap::new();
    for node in model
        .nodes
        .iter()
        .filter(|n| callback.span.contains_inclusive(n.kind().span()))
    {
        match node.kind() {
            AstKind::BindingIdentifier(id) => {
                edits.insert(
                    (id.span.start, id.span.end),
                    names.get(&id.symbol_id.get()?)?.clone(),
                );
            }
            AstKind::IdentifierReference(id) => {
                let name = if let Some(s) = crate::analysis::identifier_symbol(id, model.scoping) {
                    names.get(&s)?.clone()
                } else if writer_symbol.is_none() && id.name == writer_name {
                    "__length".into()
                } else if matches!(
                    id.name.as_str(),
                    "Array" | "unescape" | "encodeURIComponent"
                ) {
                    id.name.to_string()
                } else {
                    return None;
                };
                edits.insert((id.span.start, id.span.end), name);
            }
            _ => {}
        }
    }
    // Output/table/histogram are joined to the caller, while every local belongs
    // to this callback. Codegen compares syntax trees after these semantic edits.
    let mut source = model
        .source
        .get(callback.span.start as usize..callback.span.end as usize)?
        .to_string();
    for ((start, end), name) in edits.into_iter().rev() {
        source.replace_range(
            (start - callback.span.start) as usize..(end - callback.span.start) as usize,
            &name,
        );
    }
    canonical_consumer(&format!("({source});"))
}

fn plain_encoder_call(call: &CallExpression<'_>) -> bool {
    !call.optional
        && call.type_arguments.is_none()
        && call.arguments.iter().all(|a| a.as_expression().is_some())
        && match call.callee.without_parentheses() {
            Expression::StaticMemberExpression(m) => !m.optional,
            Expression::ComputedMemberExpression(m) => !m.optional,
            _ => true,
        }
}

/// The entire current prefix is a closed ordered grammar: default options,
/// fresh output, omission push, payload encode, omission slice, keyset encode,
/// histogram selection, placeholder transition. No statement can be ignored.
/// In particular direct/conditional throws, breaks, donors and extra effects
/// cannot precede the header while still donating evidence to its proof.
fn owned_encode_prefix<'a>(
    model: &Model<'_, 'a>,
    function: &'a Function<'a>,
    header_index: usize,
    output: SymbolId,
    payload: SymbolId,
    keysets: SymbolId,
    histogram: SymbolId,
) -> bool {
    let Some(b) = body(function) else {
        return false;
    };
    let Some(statements) = b.statements.get(..header_index) else {
        return false;
    };
    let (Some(first), Some(last)) = (statements.first(), statements.last()) else {
        return false;
    };
    let region = Span::new(first.span().start, last.span().end);
    let Some(origin) = local_origin(model, function, histogram, last.span().end) else {
        return false;
    };
    let Expression::CallExpression(select) = origin.without_parentheses() else {
        return false;
    };
    let Expression::Identifier(selector) = select.callee.without_parentheses() else {
        return false;
    };
    let selector_symbol = crate::analysis::identifier_symbol(selector, model.scoping);
    let mut locals: Vec<_> = model
        .scoping
        .symbol_ids()
        .filter(|s| region.contains_inclusive(model.scoping.symbol_span(*s)))
        .collect();
    locals.sort_by_key(|s| model.scoping.symbol_span(*s).start);
    if locals.len() != 6
        || locals[2..] != [output, payload, keysets, histogram]
        || locals
            .iter()
            .any(|s| !model.scoping.symbol_redeclarations(*s).is_empty())
    {
        return false;
    }
    let mut names: BTreeMap<_, _> = locals
        .iter()
        .enumerate()
        .map(|(i, s)| (*s, format!("__p{i}")))
        .collect();
    if let Some(s) = selector_symbol {
        if names.insert(s, "__select".into()).is_some() {
            return false;
        }
    }
    let mut edits = BTreeMap::new();
    for node in model
        .nodes
        .iter()
        .filter(|n| region.contains_inclusive(n.kind().span()))
    {
        match node.kind() {
            AstKind::BindingIdentifier(id) => {
                edits.insert(
                    (id.span.start, id.span.end),
                    id.symbol_id.get().and_then(|s| names.get(&s).cloned()),
                );
            }
            AstKind::IdentifierReference(id) => {
                let replacement =
                    if let Some(s) = crate::analysis::identifier_symbol(id, model.scoping) {
                        names.get(&s).cloned()
                    } else if selector_symbol.is_none() && id.name == selector.name {
                        Some("__select".into())
                    } else if matches!(id.name.as_str(), "arguments" | "undefined" | "Array") {
                        Some(id.name.to_string())
                    } else {
                        None
                    };
                edits.insert((id.span.start, id.span.end), replacement);
            }
            _ => {}
        }
    }
    let Some(mut source) = model
        .source
        .get(region.start as usize..region.end as usize)
        .map(str::to_string)
    else {
        return false;
    };
    for ((start, end), replacement) in edits.into_iter().rev() {
        let Some(replacement) = replacement else {
            return false;
        };
        source.replace_range(
            (start - region.start) as usize..(end - region.start) as usize,
            &replacement,
        );
    }
    static PREFIX: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    let actual = canonical_consumer(&format!("{{{source}}}"));
    actual.is_some()
        && actual.as_ref()
            == PREFIX
                .get_or_init(|| canonical_consumer(ENCODE_PREFIX_CONTRACT))
                .as_ref()
}

const ENCODE_PREFIX_CONTRACT: &str = r#"{
    let __p0 = arguments;
    let __p1;
    if (__p0.length <= 1) { __p1 = true; }
    else { __p0 = arguments; __p1 = __p0[1] === undefined; }
    if (__p1) { __p0 = {}; } else { __p0 = arguments[1]; }
    let __p2 = [];
    if (__p0.keysetsToOmit != null) { Array.prototype.push.apply(this.keysets, __p0.keysetsToOmit); }
    let __p3 = this.encodeValue(arguments[0], []);
    if (__p0.keysetsToOmit != null) { this.keysets = this.keysets.slice(__p0.keysetsToOmit.length); }
    let __p4 = this.encodeValue(this.keysets, []);
    let __p5 = __select(this.stringHist);
    this.stringPlaceholders = false;
}"#;
