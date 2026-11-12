//! Small rotation rule for one complete, source-backed signal snapshot
//! protocol.
//!
//! The debug record may be returned as one `{ value, signal }` object literal or
//! built from a `{ void 0, void 0 }` placeholder plus the two field writes; the
//! partition lists may be declared in any order and the named/unnamed selector
//! assignments may appear in any order. Roles still come only from the returned
//! `signals` / `positionalSignals` / `keyedSignals` keys and the same
//! `push.call(sink, item)` dataflow. The dispatch may be written with the three
//! selector locals plus a final `push.call(target, item)`, or compressed inline
//! as `<descriptor-name routing>.push.call(<the same routing>, item)` with a
//! direct `positional.push(item)` on the unnamed branch. The active filter may be
//! initialized by the compact `signalFilter` conditional or declared bare and
//! filled by the identical two-arm `if`/`else`. The four-key report may be
//! returned directly, built as `let report = {...}` plus field writes, or
//! assigned back onto the already-declared `signals` list.
//!
//! The rule owns every statement, argument, callback, loop cursor, branch
//! target and return key of the function it names. Captured identifiers are
//! proven only as distinct plain-variable identities; their value, data domain
//! or callable origin is never inferred. Built-in lowering (`forEach`, `map`,
//! `push`, `call`, `Object.prototype.hasOwnProperty`) is matched structurally,
//! not as a vendor or runtime API claim.
//! Additive lowerings also cover a default options object with explicit
//! filter/false/true fields (nullish notification and full-table debug mapping),
//! a split undefined-only options initializer, and a split debug temporary.
//! They retain a separate single-capture-origin gate; the historical proof
//! remains available with its original acceptance and naming behavior.
use super::*;
#[path="snapshot_partitioned.rs"]
pub(super) mod partitioned;
use oxc_semantic::SymbolId;

type Symbol = SymbolId;

fn symbol(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<Symbol> {
    crate::analysis::expression_symbol(e, model.scoping)
}

fn member<'a>(e: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(m) = e else {
        return None;
    };
    (!m.optional && m.property.name == key).then_some(&m.object)
}

fn index<'a>(e: &'a Expression<'a>) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::ComputedMemberExpression(m) = e else {
        return None;
    };
    (!m.optional).then_some((&m.object, &m.expression))
}

fn call<'a>(e: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    let Expression::CallExpression(c) = e else {
        return None;
    };
    (!c.optional).then_some(c)
}

fn statement_expression<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    let Statement::ExpressionStatement(s) = s else {
        return None;
    };
    Some(&s.expression)
}

fn block<'a>(s: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(b) = s else {
        return None;
    };
    Some(&b.body)
}

fn returned<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    let Statement::ReturnStatement(r) = s else {
        return None;
    };
    r.argument.as_ref()
}

fn binary<'a>(e: &'a Expression<'a>, op: &str) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::BinaryExpression(b) = e else {
        return None;
    };
    (b.operator.as_str() == op).then_some((&b.left, &b.right))
}

fn logical<'a>(
    e: &'a Expression<'a>,
    op: &str,
) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::LogicalExpression(b) = e else {
        return None;
    };
    (b.operator.as_str() == op).then_some((&b.left, &b.right))
}

fn void_zero(e: &Expression<'_>) -> bool {
    matches!(e, Expression::UnaryExpression(u) if u.operator.as_str() == "void"
        && matches!(&u.argument, Expression::NumericLiteral(n) if n.value == 0.0))
}

fn plain(model: &Model<'_, '_>, p: &BindingPattern<'_>, writes: usize) -> Option<Symbol> {
    let BindingPattern::BindingIdentifier(id) = p else {
        return None;
    };
    let s = id.symbol_id.get()?;
    (model.binding_writes(s) == writes && model.scoping.symbol_redeclarations(s).is_empty())
        .then_some(s)
}

// The same identity check for an already-resolved variable declarator, used
// when a report object is written back onto a previously declared symbol.
fn plain_variable(model: &Model<'_, '_>, s: Symbol, writes: usize) -> bool {
    let AstKind::VariableDeclarator(v) = model.nodes.kind(model.scoping.symbol_declaration(s))
    else {
        return false;
    };
    matches!(&v.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(s))
        && plain(model, &v.id, writes).is_some()
}

fn array_is_empty(e: &Expression<'_>) -> bool {
    matches!(e, Expression::ArrayExpression(a) if a.elements.is_empty())
}

fn member_of(model: &Model<'_, '_>, e: &Expression<'_>, owner: Symbol, key: &str) -> bool {
    member(e, key).and_then(|o| symbol(model, o)) == Some(owner)
}

// Captures must be plain variable declarators outside the owned function, with
// no redeclaration and no destructuring origin. Their writes are irrelevant to
// the protocol: only the identity of each reference matters.
fn capture(model: &Model<'_, '_>, site: FunctionSite, e: &Expression<'_>) -> Option<Symbol> {
    let s = symbol(model, e)?;
    if site.span.contains_inclusive(model.scoping.symbol_span(s))
        || !model.scoping.symbol_redeclarations(s).is_empty()
    {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(s);
    let AstKind::VariableDeclarator(v) = model.nodes.kind(declaration) else {
        return None;
    };
    if !matches!(&v.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(s))
        || !matches!(
            model.nodes.parent_kind(declaration),
            AstKind::VariableDeclaration(_)
        )
        || matches!(
            model.nodes.parent_kind(model.nodes.parent_id(declaration)),
            AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
        )
    {
        return None;
    }
    Some(s)
}

fn assignment<'a>(
    model: &Model<'_, '_>,
    e: &'a Expression<'a>,
    target: Symbol,
) -> Option<&'a Expression<'a>> {
    let Expression::AssignmentExpression(a) = e else {
        return None;
    };
    if a.operator.as_str() != "=" {
        return None;
    }
    let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else {
        return None;
    };
    (crate::analysis::identifier_symbol(id, model.scoping) == Some(target)).then_some(&a.right)
}

fn member_assignment<'a>(
    model: &Model<'_, '_>,
    e: &'a Expression<'a>,
    owner: Symbol,
    key: &str,
) -> Option<&'a Expression<'a>> {
    let Expression::AssignmentExpression(a) = e else {
        return None;
    };
    if a.operator.as_str() != "=" {
        return None;
    }
    let AssignmentTarget::StaticMemberExpression(m) = &a.left else {
        return None;
    };
    (!m.optional && m.property.name == key && symbol(model, &m.object) == Some(owner))
        .then_some(&a.right)
}

fn unoptional_chain(e: &Expression<'_>) -> bool {
    match e {
        Expression::Identifier(_) => true,
        Expression::StaticMemberExpression(m) => !m.optional && unoptional_chain(&m.object),
        _ => false,
    }
}

fn global_has_own_property(model: &Model<'_, '_>, e: &Expression<'_>) -> bool {
    expression_path(model, e) == ["Object", "prototype", "hasOwnProperty"] && unoptional_chain(e)
}

fn map_callback<'a>(
    model: &Model<'_, '_>,
    e: &'a Expression<'a>,
    source: Symbol,
) -> Option<&'a ArrowFunctionExpression<'a>> {
    let c = call(e)?;
    if c.arguments.len() != 1 {
        return None;
    }
    let Expression::StaticMemberExpression(callee) = &c.callee else {
        return None;
    };
    if callee.optional
        || callee.property.name != "map"
        || symbol(model, &callee.object) != Some(source)
    {
        return None;
    }
    match &c.arguments[0] {
        Argument::ArrowFunctionExpression(callback) => Some(callback),
        _ => None,
    }
}

fn arrow_single_param(
    model: &Model<'_, '_>,
    callback: &ArrowFunctionExpression<'_>,
    expression: bool,
) -> Option<Symbol> {
    if callback.r#async
        || callback.expression != expression
        || callback.params.rest.is_some()
        || !callback.body.directives.is_empty()
    {
        return None;
    }
    let [parameter] = callback.params.items.as_slice() else {
        return None;
    };
    if parameter.initializer.is_some() {
        return None;
    }
    plain(model, &parameter.pattern, 0)
}

fn unique(symbols: &[Symbol]) -> bool {
    for (i, left) in symbols.iter().enumerate() {
        if symbols[i + 1..].contains(left) {
            return false;
        }
    }
    true
}

fn is_plain_property(property: &ObjectProperty<'_>) -> bool {
    !property.computed
        && !property.method
        && !property.shorthand
        && property.kind == PropertyKind::Init
}

// A freshly allocated partition list. The declaration order is irrelevant: the
// role is fixed afterwards from the returned report keys. A list may carry a
// single later write only when the report object is assigned back onto it; that
// is proved after the report keys are known.
fn empty_list(model: &Model<'_, '_>, statement: &Statement<'_>) -> Option<Symbol> {
    let declaration = one_variable(statement)?;
    if !array_is_empty(declaration.init.as_ref()?) {
        return None;
    }
    let BindingPattern::BindingIdentifier(id) = &declaration.id else {
        return None;
    };
    let s = id.symbol_id.get()?;
    (model.binding_writes(s) <= 1 && model.scoping.symbol_redeclarations(s).is_empty()).then_some(s)
}

// `typeof descriptor.name === "number" ? keyed : named` in either branch order.
fn numeric_routing(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    descriptor: Symbol,
    keyed: Symbol,
    named: Symbol,
) -> bool {
    let Expression::ConditionalExpression(choose) = e else {
        return false;
    };
    let Some((test, number)) = binary(&choose.test, "===") else {
        return false;
    };
    let Expression::UnaryExpression(typeof_name) = test else {
        return false;
    };
    typeof_name.operator.as_str() == "typeof"
        && member_of(model, &typeof_name.argument, descriptor, "name")
        && matches!(number, Expression::StringLiteral(s) if s.value == "number")
        && symbol(model, &choose.consequent) == Some(keyed)
        && symbol(model, &choose.alternate) == Some(named)
}

// A debug-record field: either the raw `{value: values[i], signal: signals[i]}`
// object-literal property or the right-hand side of the placeholder assignment
// `record.value = values[i]`.
fn debug_pair(
    model: &Model<'_, '_>,
    site: FunctionSite,
    name: Option<String>,
    value: &Expression<'_>,
    key: Symbol,
) -> Option<(usize, Symbol)> {
    let slot = match name.as_deref()? {
        "value" => 0,
        "signal" => 1,
        _ => return None,
    };
    let (table, index_key) = index(value)?;
    if symbol(model, index_key) != Some(key) {
        return None;
    }
    Some((slot, capture(model, site, table)?))
}

fn report_slot(key: Option<String>) -> Option<usize> {
    match key.as_deref()? {
        "signals" => Some(0),
        "positionalSignals" => Some(1),
        "keyedSignals" => Some(2),
        "errors" => Some(3),
        _ => None,
    }
}

// One report field. `errors` is proved from its captured `.toArray()` sink; the
// three list fields are only plain identities here and get their roles from the
// `signals` / `positionalSignals` / `keyedSignals` keys, never from positions.
fn report_field(
    model: &Model<'_, '_>,
    site: FunctionSite,
    key: Option<String>,
    value: &Expression<'_>,
) -> Option<(usize, Symbol)> {
    let slot = report_slot(key)?;
    if slot != 3 {
        return Some((slot, symbol(model, value)?));
    }
    let errors = call(value)?;
    let Expression::StaticMemberExpression(callee) = &errors.callee else {
        return None;
    };
    if callee.optional || callee.property.name != "toArray" || !errors.arguments.is_empty() {
        return None;
    }
    Some((slot, capture(model, site, &callee.object)?))
}

fn assign_slot(slots: &mut [Option<Symbol>; 4], slot: usize, value: Symbol) -> Option<()> {
    if slots[slot].replace(value).is_some() {
        return None;
    }
    Some(())
}

fn finish_slots(slots: [Option<Symbol>; 4]) -> Option<[Symbol; 4]> {
    Some([slots[0]?, slots[1]?, slots[2]?, slots[3]?])
}

// The four-key report object. Keys are matched by name; extra, duplicate or
// missing fields are refused. The declared/assigned form may use `void 0`
// placeholders for fields written afterwards; the direct-return form may not.
fn report_shape(
    model: &Model<'_, '_>,
    site: FunctionSite,
    shape: &ObjectExpression<'_>,
    allow_placeholders: bool,
) -> Option<[Option<Symbol>; 4]> {
    if shape.properties.len() != 4 {
        return None;
    }
    let mut slots = [None; 4];
    let mut seen = [false; 4];
    for property in &shape.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return None;
        };
        if !is_plain_property(property) {
            return None;
        }
        let key = property_key_name(&property.key);
        let slot = report_slot(key.clone())?;
        if seen[slot] {
            return None;
        }
        seen[slot] = true;
        if allow_placeholders && void_zero(&property.value) {
            continue;
        }
        let (slot, value) = report_field(model, site, key, &property.value)?;
        assign_slot(&mut slots, slot, value)?;
    }
    Some(slots)
}

// The exact four-key report: returned directly as one object literal, built as
// `let report = {...}` plus field writes, or written back onto an already
// declared list symbol (`report = {...}`) and then filled. The back-written
// symbol is returned so the caller can prove it is the `signals` list itself.
fn report_result(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statements: &[Statement<'_>],
) -> Option<(Option<Symbol>, [Symbol; 4])> {
    if let [Statement::ReturnStatement(ret)] = statements {
        let Expression::ObjectExpression(object) = ret.argument.as_ref()? else {
            return None;
        };
        return Some((
            None,
            finish_slots(report_shape(model, site, object, false)?)?,
        ));
    }
    if statements.len() < 2 {
        return None;
    }
    let first = statements.first()?;
    let (record, shape, reported) = match first {
        Statement::VariableDeclaration(_) => {
            let declaration = one_variable(first)?;
            let record = plain(model, &declaration.id, 0)?;
            let Expression::ObjectExpression(shape) = declaration.init.as_ref()? else {
                return None;
            };
            (record, shape, None)
        }
        Statement::ExpressionStatement(_) => {
            let Expression::AssignmentExpression(assignment) = statement_expression(first)? else {
                return None;
            };
            if assignment.operator.as_str() != "=" {
                return None;
            }
            let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
                return None;
            };
            let record = crate::analysis::identifier_symbol(identifier, model.scoping)?;
            if !plain_variable(model, record, 1) {
                return None;
            }
            let Expression::ObjectExpression(shape) = &assignment.right else {
                return None;
            };
            (record, shape, Some(record))
        }
        _ => return None,
    };
    let mut slots = report_shape(model, site, shape, true)?;
    for statement in &statements[1..statements.len() - 1] {
        let expression = statement_expression(statement)?;
        let Expression::AssignmentExpression(assignment) = expression else {
            return None;
        };
        if assignment.operator.as_str() != "=" {
            return None;
        }
        let AssignmentTarget::StaticMemberExpression(member) = &assignment.left else {
            return None;
        };
        if member.optional || symbol(model, &member.object) != Some(record) {
            return None;
        }
        let (slot, value) = report_field(
            model,
            site,
            Some(member.property.name.to_string()),
            &assignment.right,
        )?;
        assign_slot(&mut slots, slot, value)?;
    }
    if symbol(model, returned(statements.last()?)?) != Some(record) {
        return None;
    }
    Some((reported, finish_slots(slots)?))
}

// The three selector assignments in any statement order. Each role is assigned
// exactly once, no other local may be written in the branch, and the returned
// position lets callers keep a `target` definition ahead of `target.push`.
fn selector_values<'a>(
    model: &Model<'_, '_>,
    statements: &'a [Statement<'a>],
    target: Symbol,
    push: Symbol,
    payload: Symbol,
) -> Option<[(&'a Expression<'a>, usize); 3]> {
    if statements.len() != 3 {
        return None;
    }
    let mut target_value = None;
    let mut push_value = None;
    let mut payload_value = None;
    for (position, statement) in statements.iter().enumerate() {
        let expression = statement_expression(statement)?;
        let Expression::AssignmentExpression(assignment) = expression else {
            return None;
        };
        if assignment.operator.as_str() != "=" {
            return None;
        }
        let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
            return None;
        };
        let assigned = crate::analysis::identifier_symbol(identifier, model.scoping)?;
        let slot = if assigned == target {
            &mut target_value
        } else if assigned == push {
            &mut push_value
        } else if assigned == payload {
            &mut payload_value
        } else {
            return None;
        };
        if slot.replace((&assignment.right, position)).is_some() {
            return None;
        }
    }
    Some([target_value?, push_value?, payload_value?])
}

// The exact `if (Object.prototype.hasOwnProperty.call(descriptor, "name"))`
// partition shared by the selector and compressed loop-body forms.
fn name_partition<'a>(
    model: &Model<'_, '_>,
    statement: &'a Statement<'a>,
    descriptor: Symbol,
) -> Option<&'a IfStatement<'a>> {
    let Statement::IfStatement(partition) = statement else {
        return None;
    };
    let has_name = call(&partition.test)?;
    let Expression::StaticMemberExpression(has_name_callee) = &has_name.callee else {
        return None;
    };
    let has_name_arguments = call_arguments(has_name);
    if has_name_callee.optional
        || has_name_callee.property.name != "call"
        || !global_has_own_property(model, &has_name_callee.object)
        || has_name.arguments.len() != 2
        || !matches!(has_name_arguments.as_slice(), [subject, name]
            if symbol(model, subject) == Some(descriptor)
                && matches!(name, Expression::StringLiteral(s) if s.value == "name"))
    {
        return None;
    }
    Some(partition)
}

// The filter binding and its captured default. The compact form initializes the
// filter with the `options.signalFilter === void 0 ? capturedDefault :
// options.signalFilter` conditional. The split form declares the same local
// bare and then proves the identical conditional as a two-arm if/else that
// writes only that local, so both spellings keep one captured default filter
// and exactly the same later role.
fn filter_binding<'a>(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statements: &'a [Statement<'a>],
    options: Symbol,
) -> Option<(Symbol, Symbol, &'a [Statement<'a>])> {
    let declaration = one_variable(statements.first()?)?;
    if let Some(init) = declaration.init.as_ref() {
        // 2a. let filter = options.signalFilter === void 0 ? capturedDefault : options.signalFilter;
        let filter = plain(model, &declaration.id, 0)?;
        let Expression::ConditionalExpression(filter_default) = init else {
            return None;
        };
        let (test, absent) = binary(&filter_default.test, "===")?;
        if !member_of(model, test, options, "signalFilter")
            || !void_zero(absent)
            || !member_of(model, &filter_default.alternate, options, "signalFilter")
        {
            return None;
        }
        let default_filter = capture(model, site, &filter_default.consequent)?;
        return Some((filter, default_filter, &statements[1..]));
    }

    // 2b. let filter; if (options.signalFilter === void 0) { filter = capturedDefault; }
    //     else { filter = options.signalFilter; }
    let filter = plain(model, &declaration.id, 2)?;
    let Statement::IfStatement(split) = statements.get(1)? else {
        return None;
    };
    let (test, absent) = binary(&split.test, "===")?;
    if !member_of(model, test, options, "signalFilter") || !void_zero(absent) {
        return None;
    }
    let [default] = block(&split.consequent)? else {
        return None;
    };
    let default_filter = capture(
        model,
        site,
        assignment(model, statement_expression(default)?, filter)?,
    )?;
    let [alternate] = block(split.alternate.as_ref()?)? else {
        return None;
    };
    if !member_of(
        model,
        assignment(model, statement_expression(alternate)?, filter)?,
        options,
        "signalFilter",
    ) {
        return None;
    }
    Some((filter, default_filter, &statements[2..]))
}

// New lowerings retain their own option semantics. In the object-default form
// the selected filter is a property read, not a separately defaulted local.
#[derive(Clone, Copy)]
enum SnapshotFilter {
    Local(Symbol),
    OptionProperty(Symbol),
}

impl SnapshotFilter {
    fn matches(self, model: &Model<'_, '_>, e: &Expression<'_>) -> bool {
        match self {
            Self::Local(s) => symbol(model, e) == Some(s),
            Self::OptionProperty(s) => member_of(model, e, s, "signalFilter"),
        }
    }
}

fn snapshot_map<'a>(
    model: &Model<'_, '_>,
    e: &'a Expression<'a>,
    source: SnapshotFilter,
) -> Option<&'a ArrowFunctionExpression<'a>> {
    if let SnapshotFilter::Local(source) = source {
        return map_callback(model, e, source);
    }
    let c = call(e)?;
    if c.arguments.len() != 1 || !source.matches(model, member(&c.callee, "map")?) {
        return None;
    }
    let [Argument::ArrowFunctionExpression(callback)] = c.arguments.as_slice() else {
        return None;
    };
    Some(callback)
}

// Accept only a single value origin for the additive path. Captures may be
// initialized by the owning runtime after the function declaration, but a
// second assignment (including an assignment from a donor) is ambiguous.
fn single_capture_origin(model: &Model<'_, '_>, s: Symbol) -> bool {
    let AstKind::VariableDeclarator(v) = model.nodes.kind(model.scoping.symbol_declaration(s))
    else {
        return false;
    };
    usize::from(v.init.is_some()) + model.binding_writes(s) == 1
}

fn options_binding<'a>(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statements: &'a [Statement<'a>],
    input: Symbol,
    extended: bool,
) -> Option<(Symbol, Option<Symbol>, &'a [Statement<'a>])> {
    let declaration = one_variable(statements.first()?)?;
    let (options, default, rest) = if let Some(init) = declaration.init.as_ref() {
        let options = plain(model, &declaration.id, 0)?;
        let Expression::ConditionalExpression(choose) = init else {
            return None;
        };
        let (test, absent) = binary(&choose.test, "===")?;
        if symbol(model, test) != Some(input)
            || !void_zero(absent)
            || symbol(model, &choose.alternate) != Some(input)
        {
            return None;
        }
        (options, &choose.consequent, &statements[1..])
    } else {
        if !extended {
            return None;
        }
        let options = plain(model, &declaration.id, 2)?;
        let Statement::IfStatement(split) = statements.get(1)? else {
            return None;
        };
        let (test, absent) = binary(&split.test, "===")?;
        if symbol(model, test) != Some(input) || !void_zero(absent) {
            return None;
        }
        let [yes] = block(&split.consequent)? else {
            return None;
        };
        let [no] = block(split.alternate.as_ref()?)? else {
            return None;
        };
        if symbol(
            model,
            assignment(model, statement_expression(no)?, options)?,
        ) != Some(input)
        {
            return None;
        }
        (
            options,
            assignment(model, statement_expression(yes)?, options)?,
            &statements[2..],
        )
    };
    let Expression::ObjectExpression(default) = default else {
        return None;
    };
    if default.properties.is_empty() {
        return Some((options, None, rest));
    }
    if !extended || default.properties.len() != 3 {
        return None;
    }
    let mut filter = None;
    let mut debug = false;
    let mut notify = false;
    for property in &default.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return None;
        };
        if !is_plain_property(property) {
            return None;
        }
        match property_key_name(&property.key).as_deref()? {
            "signalFilter" if filter.is_none() => {
                filter = Some(capture(model, site, &property.value)?)
            }
            "includeDebugNames"
                if !debug
                    && matches!(&property.value, Expression::BooleanLiteral(b) if !b.value) =>
            {
                debug = true
            }
            "notifyLastChance"
                if !notify
                    && matches!(&property.value, Expression::BooleanLiteral(b) if b.value) =>
            {
                notify = true
            }
            _ => return None,
        }
    }
    (debug && notify).then_some((options, Some(filter?), rest))
}

// An optional split debug temporary has exactly two writes, false on undefined
// and the very same option otherwise. Neither nullish defaulting nor extra
// writes are interchangeable with this undefined-only lowering.
fn debug_binding<'a>(
    model: &Model<'_, '_>,
    statements: &'a [Statement<'a>],
    options: Symbol,
) -> Option<(Symbol, &'a [Statement<'a>])> {
    let declaration = one_variable(statements.first()?)?;
    if declaration.init.is_some() {
        return None;
    }
    let flag = plain(model, &declaration.id, 2)?;
    let Statement::IfStatement(split) = statements.get(1)? else {
        return None;
    };
    let (test, absent) = binary(&split.test, "===")?;
    if !member_of(model, test, options, "includeDebugNames") || !void_zero(absent) {
        return None;
    }
    let [yes] = block(&split.consequent)? else {
        return None;
    };
    let [no] = block(split.alternate.as_ref()?)? else {
        return None;
    };
    if !matches!(assignment(model, statement_expression(yes)?, flag)?, Expression::BooleanLiteral(b) if !b.value)
        || !member_of(
            model,
            assignment(model, statement_expression(no)?, flag)?,
            options,
            "includeDebugNames",
        )
    {
        return None;
    }
    Some((flag, &statements[2..]))
}

// The complete body, statement by statement. Each returned symbol is later
// required to be pairwise distinct so no index, list, callback or capture can
// silently alias another role.
fn signal_snapshot(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
    extended: bool,
) -> Option<()> {
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || !body.directives.is_empty()
    {
        return None;
    }
    let [parameter] = function.params.items.as_slice() else {
        return None;
    };
    if parameter.initializer.is_some() {
        return None;
    }
    let input = plain(model, &parameter.pattern, 0)?;

    let (options, object_filter, rest) =
        options_binding(model, site, &body.statements, input, extended)?;
    let (filter, default_filter, rest) = if let Some(default) = object_filter {
        (SnapshotFilter::OptionProperty(options), default, rest)
    } else {
        let (filter, default, rest) = filter_binding(model, site, rest, options)?;
        (SnapshotFilter::Local(filter), default, rest)
    };
    let (debug_flag, rest) = if extended
        && object_filter.is_none()
        && matches!(rest.first(), Some(Statement::VariableDeclaration(_)))
    {
        let (flag, rest) = debug_binding(model, rest, options)?;
        (Some(flag), rest)
    } else {
        (None, rest)
    };

    let [notify, bundles_declaration, debug, scheduled @ ..] = rest else {
        return None;
    };

    // 3. notifyLastChance: filter.forEach(callback => { if (table[key] != null) table[key].call(void 0); });
    let Statement::IfStatement(notify) = notify else {
        return None;
    };
    if notify.alternate.is_some() {
        return None;
    }
    let (absent, requested) = logical(&notify.test, "||")?;
    let (field, absent) = binary(absent, if object_filter.is_some() { "==" } else { "===" })?;
    if !member_of(model, field, options, "notifyLastChance")
        || !(if object_filter.is_some() {
            matches!(absent, Expression::NullLiteral(_))
        } else {
            void_zero(absent)
        })
        || !member_of(model, requested, options, "notifyLastChance")
    {
        return None;
    }
    let [each] = block(&notify.consequent)? else {
        return None;
    };
    let each = call(statement_expression(each)?)?;
    let Expression::StaticMemberExpression(each_callee) = &each.callee else {
        return None;
    };
    if each_callee.optional
        || each_callee.property.name != "forEach"
        || !filter.matches(model, &each_callee.object)
        || each.arguments.len() != 1
    {
        return None;
    }
    let [Argument::ArrowFunctionExpression(last_chance)] = each.arguments.as_slice() else {
        return None;
    };
    let last_chance_key = arrow_single_param(model, last_chance, false)?;
    let [guard] = last_chance.body.statements.as_slice() else {
        return None;
    };
    let Statement::IfStatement(guard) = guard else {
        return None;
    };
    if guard.alternate.is_some() {
        return None;
    }
    let (present, null) = binary(&guard.test, "!=")?;
    let (last_chance_table, present_key) = index(present)?;
    let last_chance_table = capture(model, site, last_chance_table)?;
    if symbol(model, present_key) != Some(last_chance_key)
        || !matches!(null, Expression::NullLiteral(_))
    {
        return None;
    }
    let [invoke] = block(&guard.consequent)? else {
        return None;
    };
    let invoke = call(statement_expression(invoke)?)?;
    let Expression::StaticMemberExpression(invoke_callee) = &invoke.callee else {
        return None;
    };
    let invoke_arguments = call_arguments(invoke);
    let (invoke_table, invoke_key) = index(&invoke_callee.object)?;
    if invoke_callee.optional
        || invoke_callee.property.name != "call"
        || symbol(model, invoke_table) != Some(last_chance_table)
        || symbol(model, invoke_key) != Some(last_chance_key)
        || invoke.arguments.len() != 1
        || !matches!(invoke_arguments.as_slice(), [receiver] if void_zero(receiver))
    {
        return None;
    }

    // 4. let bundles; (exactly two direct assignments follow)
    let bundles_declaration = one_variable(bundles_declaration)?;
    let bundles = plain(model, &bundles_declaration.id, 2)?;
    if bundles_declaration.init.is_some() {
        return None;
    }

    // 5. includeDebugNames: bundles = filter.map(debug-tagged or raw callback).
    let Statement::IfStatement(debug) = debug else {
        return None;
    };
    if let Some(flag) = debug_flag {
        if symbol(model, &debug.test) != Some(flag) {
            return None;
        }
    } else if object_filter.is_some() {
        if !member_of(model, &debug.test, options, "includeDebugNames") {
            return None;
        }
    } else {
        let (present, requested) = logical(&debug.test, "&&")?;
        let (field, absent) = binary(present, "!==")?;
        if !member_of(model, field, options, "includeDebugNames")
            || !void_zero(absent)
            || !member_of(model, requested, options, "includeDebugNames")
        {
            return None;
        }
    }
    let [tagged] = block(&debug.consequent)? else {
        return None;
    };
    let tagged = assignment(model, statement_expression(tagged)?, bundles)?;
    let (tagged, full_values) = if object_filter.is_some() {
        let c = call(tagged)?;
        if c.arguments.len() != 1 {
            return None;
        }
        let values = capture(model, site, member(&c.callee, "map")?)?;
        let [Argument::ArrowFunctionExpression(callback)] = c.arguments.as_slice() else {
            return None;
        };
        (callback.as_ref(), Some(values))
    } else {
        (snapshot_map(model, tagged, filter)?, None)
    };
    let (tagged_key, full_index) = if full_values.is_some() {
        if tagged.r#async
            || tagged.expression
            || tagged.params.rest.is_some()
            || !tagged.body.directives.is_empty()
        {
            return None;
        }
        let [value, index] = tagged.params.items.as_slice() else {
            return None;
        };
        if value.initializer.is_some() || index.initializer.is_some() {
            return None;
        }
        (
            plain(model, &value.pattern, 0)?,
            Some(plain(model, &index.pattern, 0)?),
        )
    } else {
        (arrow_single_param(model, tagged, false)?, None)
    };
    // The debug callback either returns one `{value,signal}` object literal or
    // builds the same record from a `{void 0,void 0}` placeholder plus the two
    // field writes. Both forms must read the captured value/signal tables by
    // the same callback key; the two tables stay distinct roles.
    let (values, signals) = if let Some(values) = full_values {
        let [result] = tagged.body.statements.as_slice() else {
            return None;
        };
        let Expression::ObjectExpression(shape) = returned(result)? else {
            return None;
        };
        if shape.properties.len() != 2 {
            return None;
        }
        let mut value_seen = false;
        let mut signals = None;
        for property in &shape.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                return None;
            };
            if !is_plain_property(property) {
                return None;
            }
            match property_key_name(&property.key).as_deref()? {
                "value" if !value_seen && symbol(model, &property.value) == Some(tagged_key) => {
                    value_seen = true
                }
                "signal" if signals.is_none() => {
                    let (table, key) = index(&property.value)?;
                    if symbol(model, key) != full_index {
                        return None;
                    }
                    signals = Some(capture(model, site, table)?);
                }
                _ => return None,
            }
        }
        if !value_seen {
            return None;
        }
        (values, signals?)
    } else {
        match tagged.body.statements.as_slice() {
            [tagged_result] => {
                let Expression::ObjectExpression(record_shape) = returned(tagged_result)? else {
                    return None;
                };
                if record_shape.properties.len() != 2 {
                    return None;
                }
                let mut tables: [Option<Symbol>; 2] = [None; 2];
                for property in &record_shape.properties {
                    let ObjectPropertyKind::ObjectProperty(property) = property else {
                        return None;
                    };
                    if !is_plain_property(property) {
                        return None;
                    }
                    let (slot, table) = debug_pair(
                        model,
                        site,
                        property_key_name(&property.key),
                        &property.value,
                        tagged_key,
                    )?;
                    if tables[slot].replace(table).is_some() {
                        return None;
                    }
                }
                (tables[0]?, tables[1]?)
            }
            [record_declaration, set_value, set_signal, tagged_result] => {
                let record_declaration = one_variable(record_declaration)?;
                let record = plain(model, &record_declaration.id, 0)?;
                let Expression::ObjectExpression(record_shape) =
                    record_declaration.init.as_ref()?
                else {
                    return None;
                };
                let [
                    ObjectPropertyKind::ObjectProperty(value_property),
                    ObjectPropertyKind::ObjectProperty(signal_property),
                ] = record_shape.properties.as_slice()
                else {
                    return None;
                };
                if !is_plain_property(value_property)
                    || property_key_name(&value_property.key).as_deref() != Some("value")
                    || !void_zero(&value_property.value)
                    || !is_plain_property(signal_property)
                    || property_key_name(&signal_property.key).as_deref() != Some("signal")
                    || !void_zero(&signal_property.value)
                {
                    return None;
                }
                let value =
                    member_assignment(model, statement_expression(set_value)?, record, "value")?;
                let (value_table, value_key) = index(value)?;
                let values = capture(model, site, value_table)?;
                let signal =
                    member_assignment(model, statement_expression(set_signal)?, record, "signal")?;
                let (signal_table, signal_key) = index(signal)?;
                let signals = capture(model, site, signal_table)?;
                if symbol(model, value_key) != Some(tagged_key)
                    || symbol(model, signal_key) != Some(tagged_key)
                    || symbol(model, returned(tagged_result)?) != Some(record)
                {
                    return None;
                }
                (values, signals)
            }
            _ => return None,
        }
    };

    let [raw] = block(debug.alternate.as_ref()?)? else {
        return None;
    };
    let raw = assignment(model, statement_expression(raw)?, bundles)?;
    let raw = snapshot_map(model, raw, filter)?;
    let raw_key = arrow_single_param(model, raw, true)?;
    let [raw_value] = raw.body.statements.as_slice() else {
        return None;
    };
    let (raw_table, raw_key_expression) = index(statement_expression(raw_value)?)?;
    if capture(model, site, raw_table)? != values
        || symbol(model, raw_key_expression) != Some(raw_key)
    {
        return None;
    }

    // 6..8. Three freshly allocated partition lists, in any declaration order.
    // Their roles come from the returned report keys below, never from where
    // they are declared.
    let [first_list, second_list, third_list, scheduled @ ..] = scheduled else {
        return None;
    };
    let lists = [
        empty_list(model, first_list)?,
        empty_list(model, second_list)?,
        empty_list(model, third_list)?,
    ];
    if !unique(&lists) {
        return None;
    }
    // Keep the historical pre-loop cursor path intact. The additive lexical
    // loop spelling is checked in the report-builder helper, then feeds this
    // same whole-body proof without transforming the source program.
    let (cursor, scan, result) = match scheduled {
        [Statement::ForStatement(scan), result @ ..] if extended => (
            super::signal_report_builder::inline_snapshot_cursor(model, scan)?,
            scan.as_ref(),
            result,
        ),
        [
            cursor_declaration,
            Statement::ForStatement(scan),
            result @ ..,
        ] => {
            let cursor_declaration = one_variable(cursor_declaration)?;
            let cursor = plain(model, &cursor_declaration.id, 1)?;
            if !matches!(cursor_declaration.init.as_ref()?, Expression::NumericLiteral(n) if n.value == 0.0)
                || scan.init.is_some()
            {
                return None;
            }
            (cursor, scan.as_ref(), result)
        }
        _ => return None,
    };
    // 11. The exact four-key report, matched by key name. `signals` is the
    // string-name list, `positionalSignals` the unnamed list and `keyedSignals`
    // the numeric-name list regardless of declaration order. The object may be
    // a fresh declaration or written back onto the already-declared `signals`
    // list, which is then its single extra write.
    let (reported, slots) = report_result(model, site, result)?;
    let [named, positional, keyed, errors] = slots;
    if !unique(&[named, positional, keyed])
        || !lists
            .iter()
            .all(|list| [named, positional, keyed].contains(list))
    {
        return None;
    }
    for list in lists {
        if model.binding_writes(list) != usize::from(reported == Some(list)) {
            return None;
        }
    }
    if let Some(record) = reported {
        if record != named {
            return None;
        }
    }

    // The sole cursor update and all loop expressions are shared by both
    // declaration spellings; neither permits an additional initializer effect.
    let (left, length) = binary(scan.test.as_ref()?, "<")?;
    if symbol(model, left) != Some(cursor) || !member_of(model, length, bundles, "length") {
        return None;
    }
    let Expression::UpdateExpression(update) = scan.update.as_ref()? else {
        return None;
    };
    if update.operator.as_str() != "++"
        || !matches!(&update.argument, SimpleAssignmentTarget::AssignmentTargetIdentifier(id)
            if crate::analysis::identifier_symbol(id, model.scoping) == Some(cursor))
    {
        return None;
    }
    let scan_body = block(&scan.body)?;
    let [descriptor_declaration, tail @ ..] = scan_body else {
        return None;
    };
    let descriptor_declaration = one_variable(descriptor_declaration)?;
    let descriptor = plain(model, &descriptor_declaration.id, 0)?;
    let (descriptor_table, filter_index) = index(descriptor_declaration.init.as_ref()?)?;
    let descriptor_table = capture(model, site, descriptor_table)?;
    let (filter_object, filter_cursor) = index(filter_index)?;
    if !filter.matches(model, filter_object) || symbol(model, filter_cursor) != Some(cursor) {
        return None;
    }

    // The dispatch locals belong to the selector form only; the compressed form
    // inlines the same `push.call` routing and owns just the item expression.
    let mut dispatch_locals: Vec<Symbol> = Vec::new();
    match tail {
        [selectors_declaration, partition, dispatch] => {
            let Statement::VariableDeclaration(selectors) = selectors_declaration else {
                return None;
            };
            let [target, push, payload] = selectors.declarations.as_slice() else {
                return None;
            };
            if target.init.is_some() || push.init.is_some() || payload.init.is_some() {
                return None;
            }
            let target = plain(model, &target.id, 2)?;
            let push = plain(model, &push.id, 2)?;
            let payload = plain(model, &payload.id, 2)?;

            let partition = name_partition(model, partition, descriptor)?;

            // Named branch: the sink routing is always the numeric-name
            // conditional; the push selector is either `target.push` or the
            // same conditional's `.push` computed directly. The three
            // assignments may appear in any order, but a `target.push` read
            // must follow the `target` definition.
            let [
                (named_sink, named_sink_at),
                (named_push, named_push_at),
                (named_payload, _),
            ] = selector_values(model, block(&partition.consequent)?, target, push, payload)?;
            if !numeric_routing(model, named_sink, descriptor, keyed, named) {
                return None;
            }
            let named_push_object = member(named_push, "push")?;
            if symbol(model, named_push_object) == Some(target) {
                if named_push_at <= named_sink_at {
                    return None;
                }
            } else if !numeric_routing(model, named_push_object, descriptor, keyed, named) {
                return None;
            }
            let Expression::ArrayExpression(named_payload) = named_payload else {
                return None;
            };
            let [name_element, bundle_element] = named_payload.elements.as_slice() else {
                return None;
            };
            if !member_of(model, name_element.as_expression()?, descriptor, "name") {
                return None;
            }
            let (bundle_object, bundle_index) = index(bundle_element.as_expression()?)?;
            if symbol(model, bundle_object) != Some(bundles)
                || symbol(model, bundle_index) != Some(cursor)
            {
                return None;
            }

            // Unnamed branch: the sink is the positional list and the push
            // selector is that same list's `.push`, in any assignment order.
            let [
                (positional_sink, _),
                (positional_push, _),
                (positional_payload, _),
            ] = selector_values(
                model,
                block(partition.alternate.as_ref()?)?,
                target,
                push,
                payload,
            )?;
            if symbol(model, positional_sink) != Some(positional)
                || symbol(model, member(positional_push, "push")?) != Some(positional)
            {
                return None;
            }
            let (bundle_object, bundle_index) = index(positional_payload)?;
            if symbol(model, bundle_object) != Some(bundles)
                || symbol(model, bundle_index) != Some(cursor)
            {
                return None;
            }

            let dispatch = call(statement_expression(dispatch)?)?;
            let Expression::StaticMemberExpression(dispatch_callee) = &dispatch.callee else {
                return None;
            };
            let dispatch_arguments = call_arguments(dispatch);
            if dispatch_callee.optional
                || dispatch_callee.property.name != "call"
                || symbol(model, &dispatch_callee.object) != Some(push)
                || dispatch.arguments.len() != 2
                || !matches!(dispatch_arguments.as_slice(), [receiver, argument]
                    if symbol(model, receiver) == Some(target)
                        && symbol(model, argument) == Some(payload))
            {
                return None;
            }
            dispatch_locals = vec![target, push, payload];
        }
        [partition] => {
            let partition = name_partition(model, partition, descriptor)?;

            // Compressed named branch: `let item = [...]; <routing>.push.call
            // (<routing>, item);`. The callee's receiver and the explicit `this`
            // argument must both be the same descriptor-name routing.
            let [payload_declaration, inline_push] = block(&partition.consequent)? else {
                return None;
            };
            let payload_declaration = one_variable(payload_declaration)?;
            let payload = plain(model, &payload_declaration.id, 0)?;
            let Expression::ArrayExpression(payload_array) = payload_declaration.init.as_ref()?
            else {
                return None;
            };
            let [name_element, bundle_element] = payload_array.elements.as_slice() else {
                return None;
            };
            if !member_of(model, name_element.as_expression()?, descriptor, "name") {
                return None;
            }
            let (bundle_object, bundle_index) = index(bundle_element.as_expression()?)?;
            if symbol(model, bundle_object) != Some(bundles)
                || symbol(model, bundle_index) != Some(cursor)
            {
                return None;
            }

            let inline_push = call(statement_expression(inline_push)?)?;
            let Expression::StaticMemberExpression(inline_callee) = &inline_push.callee else {
                return None;
            };
            if inline_callee.optional || inline_callee.property.name != "call" {
                return None;
            }
            let Some(inline_routing) = member(&inline_callee.object, "push") else {
                return None;
            };
            if !numeric_routing(model, inline_routing, descriptor, keyed, named) {
                return None;
            }
            let inline_arguments = call_arguments(inline_push);
            let [receiver, argument] = inline_arguments.as_slice() else {
                return None;
            };
            if inline_push.arguments.len() != 2
                || !numeric_routing(model, *receiver, descriptor, keyed, named)
                || symbol(model, *argument) != Some(payload)
            {
                return None;
            }

            // Compressed unnamed branch: the positional list pushes the raw
            // bundle itself, in the direct method-call form.
            let [positional_push] = block(partition.alternate.as_ref()?)? else {
                return None;
            };
            let positional_push = call(statement_expression(positional_push)?)?;
            let Expression::StaticMemberExpression(positional_callee) = &positional_push.callee
            else {
                return None;
            };
            let positional_arguments = call_arguments(positional_push);
            let [positional_bundle] = positional_arguments.as_slice() else {
                return None;
            };
            if positional_callee.optional
                || positional_callee.property.name != "push"
                || symbol(model, &positional_callee.object) != Some(positional)
                || positional_push.arguments.len() != 1
            {
                return None;
            }
            let (bundle_object, bundle_index) = index(*positional_bundle)?;
            if symbol(model, bundle_object) != Some(bundles)
                || symbol(model, bundle_index) != Some(cursor)
            {
                return None;
            }
            dispatch_locals = vec![payload];
        }
        _ => return None,
    }

    let captures = [
        default_filter,
        last_chance_table,
        values,
        signals,
        descriptor_table,
        errors,
    ];
    let mut owners = vec![
        input, options, bundles, keyed, positional, named, cursor, descriptor,
    ];
    if let SnapshotFilter::Local(filter) = filter {
        owners.push(filter);
    }
    if let Some(flag) = debug_flag {
        owners.push(flag);
    }
    owners.extend(dispatch_locals);
    if extended && !captures.iter().all(|s| single_capture_origin(model, *s)) {
        return None;
    }
    let mut callbacks = vec![last_chance_key, tagged_key, raw_key];
    if let Some(index) = full_index {
        callbacks.push(index);
    }
    (unique(&captures) && unique(&owners) && unique(&callbacks)).then_some(())
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return vec![];
    };
    let Some(body) = &function.body else {
        return vec![];
    };
    let historical = signal_snapshot(model, site, function, body, false).is_some();
    if !historical && signal_snapshot(model, site, function, body, true).is_none() {
        return partitioned::evaluate(model, site, f);
    }
    (historical || signal_snapshot(model, site, function, body, true).is_some())
        .then(|| {
            proposal(
                "getSignalSnapshot",
                "signals_signal_snapshot",
                if historical { "complete snapshot protocol: one options formal; captured filter (either the compact signalFilter conditional or a bare declaration filled by the identical two-arm if/else), last-chance callback table, values/signal tables, descriptor table and errors object kept distinct; last-chance callbacks invoked with void receiver; bundles built as direct or placeholder debug {value,signal} records or raw values; cursor-indexed descriptors partitioned by optional numeric name into keyed/named versus positional lists whose declarations and selector assignments may be reordered but are bound by return key; each push dispatched through an exact chosen receiver, either the reordered selector locals with push.call(target,item) or the compressed descriptor-name routing .push.call(routing,item) with direct positional.push(item); returns four-key signals/positionalSignals/keyedSignals plus errors.toArray() from a direct literal, a fresh placeholder report, or a report written back onto the signals list" } else { "owned signal snapshot variant: exact undefined-only compact/split options initialization or complete filter/false/true default object; preserves undefined-only versus nullish notification semantics and selected-filter versus full-value-table debug domain; own ordered last-chance invocation, distinct single-origin captures, raw/debug value correspondence, descriptor lookup by the selected filter, numeric/named/positional routing with matching selected method/receiver/payload, exact partition return identities and errors.toArray(); optional split false-default debug temporary" },
                f,
            )
        })
        .into_iter()
        .collect()
}
