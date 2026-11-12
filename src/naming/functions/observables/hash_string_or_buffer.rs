use super::*;
use oxc_span::GetSpan;

use crate::naming::support::entropy_relay::*;

fn new_call_arguments<'a>(n: &'a NewExpression<'a>) -> Vec<&'a Expression<'a>> {
    n.arguments
        .iter()
        .filter_map(Argument::as_expression)
        .collect()
}

fn call_resolves_to(model: &Model<'_, '_>, c: &CallExpression<'_>, target: FunctionSite) -> bool {
    !c.optional
        && ordered_function(model, &c.callee, c.callee.span().start, 0)
            .is_some_and(|site| site.span == target.span)
}

fn char_code_accessor(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<FunctionSite> {
    let target = ordered_function(model, e, e.span().start, 0)?;
    let (params, body, expression) = match model.nodes.kind(target.node_id) {
        AstKind::Function(function) => {
            if function.r#async || function.generator {
                return None;
            }
            (&function.params, function.body.as_ref()?, false)
        }
        AstKind::ArrowFunctionExpression(arrow) => {
            if arrow.r#async {
                return None;
            }
            (&arrow.params, &arrow.body, arrow.expression)
        }
        _ => return None,
    };
    if params.rest.is_some() || params.items.len() != 1 || !body.directives.is_empty() {
        return None;
    }
    let parameter = &params.items[0];
    if parameter.initializer.is_some() {
        return None;
    }
    let BindingPattern::BindingIdentifier(identifier) = &parameter.pattern else {
        return None;
    };
    let formal = identifier.symbol_id.get()?;
    if model.binding_writes(formal) != 0 || !model.scoping.symbol_redeclarations(formal).is_empty()
    {
        return None;
    }
    let call = if expression {
        let [Statement::ExpressionStatement(statement)] = body.statements.as_slice() else {
            return None;
        };
        let Expression::CallExpression(call) = &statement.expression else {
            return None;
        };
        call
    } else {
        let [Statement::ReturnStatement(result)] = body.statements.as_slice() else {
            return None;
        };
        let Expression::CallExpression(call) = result.argument.as_ref()? else {
            return None;
        };
        call
    };
    let object = member(&call.callee, "charCodeAt")?;
    (!call.optional && call.arguments.is_empty() && is_symbol(model, object, formal))
        .then_some(target)
}

fn char_index<'a>(
    model: &Model<'_, '_>,
    c: &'a CallExpression<'a>,
    accessor: FunctionSite,
) -> Option<(Symbol, &'a Expression<'a>)> {
    if !call_resolves_to(model, c, accessor) {
        return None;
    }
    let arguments = call_arguments(c);
    let [argument] = arguments.as_slice() else {
        return None;
    };
    let Expression::ComputedMemberExpression(read) = argument else {
        return None;
    };
    if read.optional {
        return None;
    }
    Some((symbol(model, &read.object)?, &read.expression))
}

fn hash_tail<'a>(
    model: &Model<'_, '_>,
    e: &'a Expression<'a>,
    hash: Symbol,
) -> Option<&'a Expression<'a>> {
    let Expression::BinaryExpression(or) = e else {
        return None;
    };
    if or.operator.as_str() != "|" || !number(&or.right, 0.0) {
        return None;
    }
    let Expression::BinaryExpression(add) = &or.left else {
        return None;
    };
    if add.operator.as_str() != "+" {
        return None;
    }
    let Expression::BinaryExpression(subtract) = &add.left else {
        return None;
    };
    if subtract.operator.as_str() != "-" {
        return None;
    }
    let Expression::BinaryExpression(shift) = &subtract.left else {
        return None;
    };
    if shift.operator.as_str() != "<<"
        || !number(&shift.right, 5.0)
        || !is_symbol(model, &shift.left, hash)
        || !is_symbol(model, &subtract.right, hash)
    {
        return None;
    }
    Some(&add.right)
}

fn string_pair(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    input: Symbol,
    index: Symbol,
) -> Option<FunctionSite> {
    let Expression::BinaryExpression(or) = e else {
        return None;
    };
    if or.operator.as_str() != "|" {
        return None;
    }
    let Expression::BinaryExpression(shift) = &or.left else {
        return None;
    };
    if shift.operator.as_str() != "<<" || !number(&shift.right, 16.0) {
        return None;
    }
    let first = call_expr(&shift.left)?;
    let accessor = char_code_accessor(model, &first.callee)?;
    let (object, at) = char_index(model, first, accessor)?;
    if object != input || !is_symbol(model, at, index) {
        return None;
    }
    let second = call_expr(&or.right)?;
    let (second_object, second_at) = char_index(model, second, accessor)?;
    let Expression::BinaryExpression(next) = second_at else {
        return None;
    };
    (second_object == input
        && next.operator.as_str() == "+"
        && is_symbol(model, &next.left, index)
        && number(&next.right, 1.0))
    .then_some(accessor)
}

fn string_single(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    accessor: FunctionSite,
    input: Symbol,
    index: Symbol,
) -> bool {
    let Expression::BinaryExpression(shift) = e else {
        return false;
    };
    if shift.operator.as_str() != "<<" || !number(&shift.right, 16.0) {
        return false;
    }
    let Some(call) = call_expr(&shift.left) else {
        return false;
    };
    let Some((object, at)) = char_index(model, call, accessor) else {
        return false;
    };
    object == input && is_symbol(model, at, index)
}

// ``typeof input === "string"`` branch: Java-style h * 31 over UTF-16 code
// unit pairs, with a one-unit tail when the length is odd; this is the
// established hashStringOrBuffer algorithm. The char accessor itself
// is independently proved to return exactly ``formal.charCodeAt()``.
fn string_branch(
    model: &Model<'_, '_>,
    site: FunctionSite,
    input: Symbol,
    statement: &Statement<'_>,
) -> Option<FunctionSite> {
    let Statement::IfStatement(branch) = statement else {
        return None;
    };
    if branch.alternate.is_some() {
        return None;
    }
    let Expression::BinaryExpression(kind) = &branch.test else {
        return None;
    };
    let Expression::UnaryExpression(typeof_) = &kind.left else {
        return None;
    };
    if kind.operator.as_str() != "==="
        || typeof_.operator.as_str() != "typeof"
        || slot_of(model, site, &typeof_.argument) != Some(0)
        || !matches!(&kind.right, Expression::StringLiteral(s) if s.value == "string")
    {
        return None;
    }
    let [even_statement, index_statement, hash_statement, Statement::ForStatement(loop_), Statement::IfStatement(check)] =
        block(&branch.consequent)?
    else {
        return None;
    };
    let even_value = one_variable(even_statement)?;
    let even = plain_declarator(model, even_value)?;
    if model.binding_writes(even) != 0 {
        return None;
    }
    let Expression::BinaryExpression(outer) = even_value.init.as_ref()? else {
        return None;
    };
    let Expression::BinaryExpression(inner) = &outer.left else {
        return None;
    };
    let length = member(&inner.left, "length")?;
    if outer.operator.as_str() != "<<"
        || !number(&outer.right, 1.0)
        || inner.operator.as_str() != ">>"
        || !number(&inner.right, 1.0)
        || !is_symbol(model, length, input)
    {
        return None;
    }
    let index_value = one_variable(index_statement)?;
    let index = plain_declarator(model, index_value)?;
    if !matches!(index_value.init.as_ref()?, Expression::NumericLiteral(n) if n.value == 0.0) {
        return None;
    }
    let hash_value = one_variable(hash_statement)?;
    let hash = plain_declarator(model, hash_value)?;
    if !matches!(hash_value.init.as_ref()?, Expression::NumericLiteral(n) if n.value == 0.0) {
        return None;
    }
    if loop_.init.is_some() {
        return None;
    }
    let Expression::BinaryExpression(test) = loop_.test.as_ref()? else {
        return None;
    };
    if test.operator.as_str() != "<"
        || !is_symbol(model, &test.left, index)
        || !is_symbol(model, &test.right, even)
    {
        return None;
    }
    let Expression::AssignmentExpression(update) = loop_.update.as_ref()? else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(update_target) = &update.left else {
        return None;
    };
    let Expression::BinaryExpression(step) = &update.right else {
        return None;
    };
    if update.operator.as_str() != "="
        || crate::analysis::identifier_symbol(update_target, model.scoping) != Some(index)
        || step.operator.as_str() != "+"
        || !is_symbol(model, &step.left, index)
        || !number(&step.right, 2.0)
    {
        return None;
    }
    let [Statement::ExpressionStatement(body)] = block(&loop_.body)? else {
        return None;
    };
    let Expression::AssignmentExpression(body) = &body.expression else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(body_target) = &body.left else {
        return None;
    };
    if body.operator.as_str() != "="
        || crate::analysis::identifier_symbol(body_target, model.scoping) != Some(hash)
    {
        return None;
    }
    let tail = hash_tail(model, &body.right, hash)?;
    let accessor = string_pair(model, tail, input, index)?;
    let Expression::BinaryExpression(check_test) = &check.test else {
        return None;
    };
    let check_length = member(&check_test.right, "length")?;
    let [Statement::ReturnStatement(remainder)] = block(&check.consequent)? else {
        return None;
    };
    let [Statement::ReturnStatement(exact)] = block(check.alternate.as_ref()?)? else {
        return None;
    };
    if check_test.operator.as_str() != "!=="
        || !is_symbol(model, &check_test.left, even)
        || !is_symbol(model, check_length, input)
        || !is_symbol(model, exact.argument.as_ref()?, hash)
    {
        return None;
    }
    let remainder_tail = hash_tail(model, remainder.argument.as_ref()?, hash)?;
    string_single(model, remainder_tail, accessor, input, even).then_some(accessor)
}

// ``ArrayBuffer.isView(input)`` branch: align the view to 4 bytes, hash the
// Int32 words with the same djb2 update, and return the accumulator.
fn buffer_branch(
    model: &Model<'_, '_>,
    _site: FunctionSite,
    input: Symbol,
    statement: &Statement<'_>,
) -> Option<()> {
    let [buffer_statement, Statement::IfStatement(align), Statement::ExpressionStatement(convert), hash_statement, index_statement, Statement::ForStatement(loop_), Statement::ReturnStatement(result)] =
        block(statement)?
    else {
        return None;
    };
    let buffer_value = one_variable(buffer_statement)?;
    let buffer = plain_declarator(model, buffer_value)?;
    if buffer_value.init.is_some() {
        return None;
    }
    let Expression::BinaryExpression(aligned) = &align.test else {
        return None;
    };
    let Expression::BinaryExpression(remainder) = &aligned.left else {
        return None;
    };
    let byte_length = member(&remainder.left, "byteLength")?;
    if aligned.operator.as_str() != "==="
        || remainder.operator.as_str() != "%"
        || !number(&remainder.right, 4.0)
        || !number(&aligned.right, 0.0)
        || !is_symbol(model, byte_length, input)
    {
        return None;
    }
    let [Statement::ExpressionStatement(exact)] = block(&align.consequent)? else {
        return None;
    };
    let Expression::AssignmentExpression(exact) = &exact.expression else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(exact_target) = &exact.left else {
        return None;
    };
    let Expression::StaticMemberExpression(exact_read) = &exact.right else {
        return None;
    };
    if exact.operator.as_str() != "="
        || crate::analysis::identifier_symbol(exact_target, model.scoping) != Some(buffer)
        || exact_read.optional
        || exact_read.property.name != "buffer"
        || !is_symbol(model, &exact_read.object, input)
    {
        return None;
    }
    let [Statement::ExpressionStatement(copy), Statement::ExpressionStatement(fill)] =
        block(align.alternate.as_ref()?)?
    else {
        return None;
    };
    let Expression::AssignmentExpression(copy) = &copy.expression else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(copy_target) = &copy.left else {
        return None;
    };
    let Expression::NewExpression(copy_new) = &copy.right else {
        return None;
    };
    let copy_arguments = new_call_arguments(copy_new);
    let [copy_size] = copy_arguments.as_slice() else {
        return None;
    };
    let Expression::BinaryExpression(copy_size) = copy_size else {
        return None;
    };
    let Expression::BinaryExpression(copy_tail) = &copy_size.right else {
        return None;
    };
    let Expression::BinaryExpression(copy_remainder) = &copy_tail.right else {
        return None;
    };
    if copy.operator.as_str() != "="
        || crate::analysis::identifier_symbol(copy_target, model.scoping) != Some(buffer)
        || expression_path(model, &copy_new.callee) != ["ArrayBuffer"]
        || copy_size.operator.as_str() != "+"
        || !member(&copy_size.left, "byteLength").is_some_and(|e| is_symbol(model, e, input))
        || copy_tail.operator.as_str() != "-"
        || !number(&copy_tail.left, 4.0)
        || copy_remainder.operator.as_str() != "%"
        || !member(&copy_remainder.left, "byteLength").is_some_and(|e| is_symbol(model, e, input))
        || !number(&copy_remainder.right, 4.0)
    {
        return None;
    }
    let Expression::CallExpression(fill) = &fill.expression else {
        return None;
    };
    let Some(set_receiver) = member(&fill.callee, "set") else {
        return None;
    };
    let Expression::NewExpression(set_receiver) = set_receiver else {
        return None;
    };
    let set_arguments = new_call_arguments(set_receiver);
    let [set_buffer] = set_arguments.as_slice() else {
        return None;
    };
    let fill_arguments = call_arguments(fill);
    let [fill_source] = fill_arguments.as_slice() else {
        return None;
    };
    let Expression::NewExpression(fill_source) = fill_source else {
        return None;
    };
    let fill_arguments = new_call_arguments(fill_source);
    let [fill_buffer] = fill_arguments.as_slice() else {
        return None;
    };
    let fill_buffer = member(fill_buffer, "buffer")?;
    if fill.optional
        || expression_path(model, &set_receiver.callee) != ["Int8Array"]
        || !is_symbol(model, set_buffer, buffer)
        || expression_path(model, &fill_source.callee) != ["Int8Array"]
        || !is_symbol(model, fill_buffer, input)
    {
        return None;
    }
    let Expression::AssignmentExpression(convert) = &convert.expression else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(convert_target) = &convert.left else {
        return None;
    };
    let Expression::NewExpression(convert_new) = &convert.right else {
        return None;
    };
    let convert_arguments = new_call_arguments(convert_new);
    let [convert_buffer] = convert_arguments.as_slice() else {
        return None;
    };
    if convert.operator.as_str() != "="
        || crate::analysis::identifier_symbol(convert_target, model.scoping) != Some(buffer)
        || expression_path(model, &convert_new.callee) != ["Int32Array"]
        || !is_symbol(model, convert_buffer, buffer)
    {
        return None;
    }
    let hash_value = one_variable(hash_statement)?;
    let hash = plain_declarator(model, hash_value)?;
    if !matches!(hash_value.init.as_ref()?, Expression::NumericLiteral(n) if n.value == 0.0) {
        return None;
    }
    let index_value = one_variable(index_statement)?;
    let index = plain_declarator(model, index_value)?;
    if !matches!(index_value.init.as_ref()?, Expression::NumericLiteral(n) if n.value == 0.0) {
        return None;
    }
    if loop_.init.is_some() {
        return None;
    }
    let Expression::BinaryExpression(test) = loop_.test.as_ref()? else {
        return None;
    };
    let loop_length = member(&test.right, "length")?;
    if test.operator.as_str() != "<"
        || !is_symbol(model, &test.left, index)
        || !is_symbol(model, loop_length, buffer)
    {
        return None;
    }
    let Some(Expression::UpdateExpression(update)) = loop_.update.as_ref() else {
        return None;
    };
    if update.operator.as_str() != "++"
        || !matches!(&update.argument, SimpleAssignmentTarget::AssignmentTargetIdentifier(i)
            if crate::analysis::identifier_symbol(i, model.scoping) == Some(index))
    {
        return None;
    }
    let [Statement::ExpressionStatement(body)] = block(&loop_.body)? else {
        return None;
    };
    let Expression::AssignmentExpression(body) = &body.expression else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(body_target) = &body.left else {
        return None;
    };
    let tail = hash_tail(model, &body.right, hash)?;
    let Expression::ComputedMemberExpression(read) = tail else {
        return None;
    };
    if body.operator.as_str() != "="
        || crate::analysis::identifier_symbol(body_target, model.scoping) != Some(hash)
        || read.optional
        || !is_symbol(model, &read.object, buffer)
        || !is_symbol(model, &read.expression, index)
        || !is_symbol(model, result.argument.as_ref()?, hash)
    {
        return None;
    }
    Some(())
}

fn hash_string_or_array_buffer_view(
    model: &Model<'_, '_>,
    site: FunctionSite,
    input: Symbol,
    body: &FunctionBody<'_>,
) -> bool {
    let [string_branch_statement, flag_statement, flag_test, dispatch] = body.statements.as_slice()
    else {
        return false;
    };
    let Some(accessor) = string_branch(model, site, input, string_branch_statement) else {
        return false;
    };
    let Some(flag_value) = one_variable(flag_statement) else {
        return false;
    };
    let Some(flag) = plain_declarator(model, flag_value) else {
        return false;
    };
    let Some(flag_init) = flag_value.init.as_ref() else {
        return false;
    };
    if accessor.span == site.span || data_capture(model, site, flag_init).is_none() || flag == input
    {
        return false;
    }
    let Statement::IfStatement(flag_check) = flag_test else {
        return false;
    };
    if flag_check.alternate.is_some() || !is_symbol(model, &flag_check.test, flag) {
        return false;
    }
    let Some([Statement::ExpressionStatement(set)]) = block(&flag_check.consequent) else {
        return false;
    };
    let Expression::AssignmentExpression(set) = &set.expression else {
        return false;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(set_target) = &set.left else {
        return false;
    };
    let Expression::CallExpression(view_test) = &set.right else {
        return false;
    };
    let Some(array_buffer) = member(&view_test.callee, "isView") else {
        return false;
    };
    if set.operator.as_str() != "="
        || crate::analysis::identifier_symbol(set_target, model.scoping) != Some(flag)
        || view_test.optional
        || expression_path(model, array_buffer) != ["ArrayBuffer"]
        || view_test.arguments.len() != 1
        || call_arguments(view_test)
            .first()
            .and_then(|e| slot_of(model, site, e))
            != Some(0)
    {
        return false;
    }
    let Statement::IfStatement(dispatch) = dispatch else {
        return false;
    };
    if !is_symbol(model, &dispatch.test, flag) {
        return false;
    }
    if buffer_branch(model, site, input, &dispatch.consequent).is_none() {
        return false;
    }
    let Some(alternate) = dispatch.alternate.as_ref() else {
        return false;
    };
    let Some([Statement::ThrowStatement(failure)]) = block(alternate) else {
        return false;
    };
    let Expression::NewExpression(error) = &failure.argument else {
        return false;
    };
    expression_path(model, &error.callee) == ["TypeError"] && error.arguments.is_empty()
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return Vec::new();
    };
    let Some(body) = function.body.as_ref() else {
        return Vec::new();
    };
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || !body.directives.is_empty()
    {
        return Vec::new();
    }
    let plain_params = function.params.items.iter().all(|p| {
        p.initializer.is_none()
            && matches!(&p.pattern, BindingPattern::BindingIdentifier(id)
                if id.symbol_id.get().is_some_and(|s| unchanged(model, s)))
    });
    if !plain_params || function.params.items.len() != 1 || f.calls.is_empty() {
        return Vec::new();
    }
    let input = function.params.items.first().and_then(|parameter| {
        let BindingPattern::BindingIdentifier(identifier) = &parameter.pattern else {
            return None;
        };
        identifier.symbol_id.get()
    });
    let _ = input;
    let mut claims = Vec::new();
    if f.members.contains("byteLength")
        && input.is_some_and(|input| hash_string_or_array_buffer_view(model, site, input, body))
    {
        claims.push(proposal(
            "hashStringOrBuffer",
            "signals_hash_string_or_buffer",
            "typeof-string branch computes the exact djb2 pair hash with a one-unit tail; otherwise a captured ArrayBuffer.isView feature flag gates an Int32Array word hash after 4-byte alignment, and every char accessor call is independently proved to return its formal's zero-argument charCodeAt",
            f,
        ));
    }
    claims
}
