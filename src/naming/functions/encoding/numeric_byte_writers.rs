//! Whole-body proofs for indexed array equality and tagged numeric writers.
//! Captured tag values require complete writers, nonescape and exposure epochs.
use crate::naming::support::interaction::*;
use crate::naming::*;

fn returned<'a>(statement: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    let Statement::ReturnStatement(ret) = statement else {
        return None;
    };
    ret.argument.as_ref()
}

fn sole_block<'a>(statement: &'a Statement<'a>) -> Option<&'a Statement<'a>> {
    let Statement::BlockStatement(block) = statement else {
        return None;
    };
    let [only] = block.body.as_slice() else {
        return None;
    };
    Some(only)
}

fn equality(model: &Model<'_, '_>, function: &Function<'_>) -> Option<()> {
    if function.r#async || function.generator || function.params.rest.is_some() {
        return None;
    }
    let [left, right] = function.params.items.as_slice() else {
        return None;
    };
    let left = plain_param(model, left)?;
    let right = plain_param(model, right)?;
    let body = function.body.as_ref()?;
    if !body.directives.is_empty() {
        return None;
    }
    let [alias, Statement::IfStatement(branch)] = body.statements.as_slice() else {
        return None;
    };
    let alias = variable(alias)?;
    let alias_symbol = plain_var(model, alias)?;
    if model.binding_writes(alias_symbol) != 0 || symbol(model, alias.init.as_ref()?) != Some(right)
    {
        return None;
    }
    let (a, b) = binary(&branch.test, "===")?;
    if symbol(model, member(a, "length")?) != Some(left)
        || symbol(model, member(b, "length")?) != Some(right)
    {
        return None;
    }
    let every = call(returned(sole_block(&branch.consequent)?)?)?;
    if method_receiver(model, every, "every") != Some(left) {
        return None;
    }
    let [Argument::ArrowFunctionExpression(callback)] = every.arguments.as_slice() else {
        return None;
    };
    if callback.r#async || callback.params.rest.is_some() || !callback.body.directives.is_empty() {
        return None;
    }
    let [value, index] = callback.params.items.as_slice() else {
        return None;
    };
    let value = plain_param(model, value)?;
    let index = plain_param(model, index)?;
    let [comparison] = callback.body.statements.as_slice() else {
        return None;
    };
    let comparison = if callback.expression {
        expression_statement(comparison)?
    } else {
        returned(comparison)?
    };
    let (entry, expected) = binary(comparison, "===")?;
    let Expression::ComputedMemberExpression(entry) = entry else {
        return None;
    };
    if entry.optional
        || symbol(model, &entry.object) != Some(alias_symbol)
        || symbol(model, &entry.expression) != Some(index)
        || symbol(model, expected) != Some(value)
        || !boolean(returned(sole_block(branch.alternate.as_ref()?)?)?, false)
    {
        return None;
    }
    Some(())
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return Vec::new();
    };
    if equality(model, function).is_some() {
        return vec![proposal("arraysEqualByIndex", "shared_arrays_equal_by_index",
            "complete own body: immutable alias of second formal; exact length equality; return first.every of synchronous two-formal callback comparing second[index] === element; alternate returns false; all locals/formals resolve to their own symbols, with no extra statements, effects, captures or parameter writes", f)];
    }
    if unsigned(model, site, function).is_some() {
        return vec![proposal("writeUnsignedIntBytes", "shared_tagged_unsigned_integer_writer",
            "complete own unsigned ladder: exact 63/16383/65535/16777215/4294967295 bounds, big-endian high/low 32-bit stores and ordered push groups on formal slot1; all tag-table writers are executed pure numeric-object stores in the owning function with exact tag values; no table escape or property mutation; every callable exposure follows a proved store, including immutable aliases and hoisted enclosing declarations", f)];
    }
    if typed_float(model, site, function).is_some() {
        return vec![proposal("writeFloatBytesWithTypedArrays", "shared_typed_array_float_writer",
            "complete own float protocol: free Float32Array(1), slot0 store, exact roundtrip inequality, DOUBLE64 tag and free Float64Array fallback through the same scratch holder, free Uint8Array of its buffer, tag push then descending byte pushes; exact receiver/index/cursor identities and writes; all tag-table writers, no escapes/mutations and callable exposure readiness independently proved", f)];
    }
    if equality_wrapper(model, site, function).is_some() {
        return vec![proposal("matchesArrayContents","shared_arrays_equal_wrapper",
            "sole return calls its independently whole-body proved arraysEqualByIndex callee with formal0 and one immutable captured alias of an initialized enclosing formal; exact callee origin/readiness and every wrapper exposure after the executed alias store are proved; no native array or external helper identity inferred",f)];
    }
    if registry_factory(model, site, function, proposals).is_some() {
        return vec![proposal("createExtensionRegistryClass","shared_extension_registry_factory",
            "complete own factory: exact uninitialized alias then owned strict self-guarded ExtensionRegistry constructor, alias assignment, extend descriptor and owned registerExtension body, descriptor value installation, independently proved descriptor-installer call on same constructor/descriptor, sole constructor return; every installer writer is an executed zero-argument call to a whole proved installer factory, and every registry factory exposure follows an executed installer-result store",f)];
    }
    Vec::new()
}

use oxc_semantic::{NodeId, SymbolId};
use oxc_span::GetSpan;

#[derive(Clone, Copy)]
struct TagStore {
    body: NodeId,
    end: u32,
}

/// Every table write is an executed, direct numeric-object assignment in the
/// declaration's owning function. Deferred writers, escaping table identities,
/// property writes, nonnumeric/getter/spread properties and alternate tag values
/// are all refused. Repeated stores may change object identity but not any tag
/// consumed by this protocol; no consumer is allowed to observe that identity.
fn tag_stores(
    model: &Model<'_, '_>,
    site: FunctionSite,
    table: SymbolId,
    required: &[(&str, f64)],
) -> Option<Vec<TagStore>> {
    captured_symbol(model, site, table)?;
    let declaration = model.scoping.symbol_declaration(table);
    let AstKind::VariableDeclarator(variable) = model.nodes.kind(declaration) else {
        return None;
    };
    if variable.init.is_some() {
        return None;
    }
    let owner = model.owning_site(declaration)?.node_id;
    let mut stores = Vec::new();
    for reference in model.scoping.get_resolved_references(table) {
        let node = reference.node_id();
        let parent = model.nodes.parent_id(node);
        if reference.is_write() {
            let AstKind::AssignmentExpression(a) = model.nodes.kind(parent) else {
                return None;
            };
            if a.operator.as_str() != "=" || target_symbol(model, &a.left) != Some(table) {
                return None;
            }
            let statement = model.nodes.parent_id(parent);
            if !matches!(model.nodes.kind(statement), AstKind::ExpressionStatement(_)) {
                return None;
            }
            let body = model.nodes.parent_id(statement);
            if !matches!(
                model.nodes.kind(body),
                AstKind::FunctionBody(_) | AstKind::BlockStatement(_)
            ) || model.owning_site(parent)?.node_id != owner
            {
                return None;
            }
            let Expression::ObjectExpression(object) = &a.right else {
                return None;
            };
            let mut values = BTreeMap::new();
            for property in &object.properties {
                let ObjectPropertyKind::ObjectProperty(p) = property else {
                    return None;
                };
                let key = property_key_name(&p.key)?;
                let Expression::NumericLiteral(n) = &p.value else {
                    return None;
                };
                if p.computed
                    || p.method
                    || p.shorthand
                    || p.kind != PropertyKind::Init
                    || key == "__proto__"
                    || values.insert(key, n.value).is_some()
                {
                    return None;
                }
            }
            if required.iter().any(|(key, n)| values.get(*key) != Some(n)) {
                return None;
            }
            stores.push(TagStore {
                body,
                end: a.span.end,
            });
        } else {
            // The table itself must never be exposed. Even property writes and
            // delete/update use an identifier read, so inspect the member parent.
            let AstKind::StaticMemberExpression(member) = model.nodes.kind(parent) else {
                return None;
            };
            if member.optional || member.object.span() != model.nodes.kind(node).span() {
                return None;
            }
            // Assignment targets have intermediate semantic nodes (including
            // destructuring target properties). Check all enclosing target
            // spans instead of assuming an immediate assignment parent.
            let mut ancestor = parent;
            let mut reached_root = false;
            for _ in 0..128 {
                match model.nodes.kind(ancestor) {
                    AstKind::AssignmentExpression(a)
                        if a.left.span().contains_inclusive(member.span) =>
                    {
                        return None
                    }
                    AstKind::ForInStatement(f) if f.left.span().contains_inclusive(member.span) => {
                        return None
                    }
                    AstKind::ForOfStatement(f) if f.left.span().contains_inclusive(member.span) => {
                        return None
                    }
                    AstKind::UpdateExpression(u) if u.span.contains_inclusive(member.span) => {
                        return None
                    }
                    AstKind::UnaryExpression(u)
                        if u.operator.as_str() == "delete"
                            && u.span.contains_inclusive(member.span) =>
                    {
                        return None
                    }
                    _ => {}
                }
                let next = model.nodes.parent_id(ancestor);
                if next == ancestor {
                    reached_root = true;
                    break;
                }
                ancestor = next;
            }
            if !reached_root {
                return None;
            }
        }
    }
    (!stores.is_empty() && stores.len() == model.binding_writes(table)).then_some(stores)
}

/// Follow immutable callable value aliases without treating a copy as a call.
/// A copied alias must itself be initialized at every later read. For ordinary
/// enclosing declarations, recursively inspect every exposure instead of using
/// declaration text as an invocation. Unknown expressions/callbacks stay under
/// the existing conservative lexical readiness proof, never an async assumption.
fn exposures_ready(
    model: &Model<'_, '_>,
    callable: SymbolId,
    stores: &[TagStore],
    visiting: &mut BTreeSet<SymbolId>,
    depth: usize,
) -> bool {
    if depth > 12
        || model.binding_writes(callable) != 0
        || !model.scoping.symbol_redeclarations(callable).is_empty()
        || !visiting.insert(callable)
    {
        return false;
    }
    let references: Vec<_> = model.scoping.get_resolved_references(callable).collect();
    let valid = !references.is_empty()
        && references.iter().all(|reference| {
            let node = reference.node_id();
            if stores.iter().any(|s| {
                shared::callable_origin_precedes(model, s.body, s.end, node, None, 0, true)
            }) {
                return true;
            }
            let parent = model.nodes.parent_id(node);
            if let AstKind::VariableDeclarator(v) = model.nodes.kind(parent) {
                if v.init
                    .as_ref()
                    .is_some_and(|e| e.span() == model.nodes.kind(node).span())
                {
                    if let Some(alias) = plain_var(model, v) {
                        let statement = model.nodes.parent_id(parent);
                        let body = model.nodes.parent_id(statement);
                        if matches!(model.nodes.kind(statement), AstKind::VariableDeclaration(_))
                            && matches!(
                                model.nodes.kind(body),
                                AstKind::FunctionBody(_)
                                    | AstKind::BlockStatement(_)
                                    | AstKind::Program(_)
                            )
                            && model.scoping.get_resolved_references(alias).all(|r| {
                                shared::callable_origin_precedes(
                                    model,
                                    body,
                                    v.span.end,
                                    r.node_id(),
                                    None,
                                    0,
                                    false,
                                )
                            })
                        {
                            return exposures_ready(model, alias, stores, visiting, depth + 1);
                        }
                    }
                }
            }
            let mut current = parent;
            for _ in 0..128 {
                if let AstKind::Function(f) = model.nodes.kind(current) {
                    // Never justify an early read inside an initialization
                    // owner by following calls of that owner. Its store only
                    // runs after this read on each invocation; an outer
                    // exposure cannot make that future store dominate it.
                    if stores
                        .iter()
                        .any(|s| f.span.contains_inclusive(model.nodes.kind(s.body).span()))
                    {
                        return false;
                    }
                    if matches!(
                        model.nodes.parent_kind(current),
                        AstKind::FunctionBody(_) | AstKind::BlockStatement(_)
                    ) {
                        if let Some(symbol) = f.id.as_ref().and_then(|id| id.symbol_id.get()) {
                            return exposures_ready(model, symbol, stores, visiting, depth + 1);
                        }
                    }
                    // Do not escape an untracked immediately invoked or externally
                    // registered function expression by borrowing its outer scope.
                    return false;
                }
                let next = model.nodes.parent_id(current);
                if next == current {
                    break;
                }
                current = next;
            }
            false
        });
    visiting.remove(&callable);
    valid
}

#[derive(Clone)]
enum ByteExpr {
    Value,
    Number(f64),
    Tag(&'static str),
    Binary(&'static str, Box<ByteExpr>, Box<ByteExpr>),
}
impl ByteExpr {
    fn op(self, op: &'static str, right: Self) -> Self {
        Self::Binary(op, Box::new(self), Box::new(right))
    }
    fn n(self, op: &'static str, right: f64) -> Self {
        self.op(op, Self::Number(right))
    }
    fn matches(
        &self,
        model: &Model<'_, '_>,
        e: &Expression<'_>,
        value: SymbolId,
        table: SymbolId,
    ) -> bool {
        let e = e.without_parentheses();
        match self {
            Self::Value => symbol(model, e) == Some(value),
            Self::Number(n) => number(e, *n),
            Self::Tag(key) => member(e, key).and_then(|e| symbol(model, e)) == Some(table),
            Self::Binary(op, left, right) => binary(e, op).is_some_and(|(a, b)| {
                left.matches(model, a, value, table) && right.matches(model, b, value, table)
            }),
        }
    }
}

fn push_group(
    model: &Model<'_, '_>,
    s: &Statement<'_>,
    output: SymbolId,
    expected: &[ByteExpr],
    value: SymbolId,
    table: SymbolId,
) -> Option<()> {
    let c = call(expression_statement(s)?)?;
    if method_receiver(model, c, "push") != Some(output) {
        return None;
    }
    let args = arguments(c)?;
    (args.len() == expected.len()
        && args
            .iter()
            .zip(expected)
            .all(|(e, p)| p.matches(model, e, value, table)))
    .then_some(())
}
fn plain_pair(model: &Model<'_, '_>, function: &Function<'_>) -> Option<(SymbolId, SymbolId)> {
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || !function.body.as_ref()?.directives.is_empty()
    {
        return None;
    }
    let [value, output] = function.params.items.as_slice() else {
        return None;
    };
    Some((plain_param(model, value)?, plain_param(model, output)?))
}
fn statements<'a>(s: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(b) = s else {
        return None;
    };
    Some(b.body.as_slice())
}
fn unsigned(model: &Model<'_, '_>, site: FunctionSite, function: &Function<'_>) -> Option<()> {
    let (value, output) = plain_pair(model, function)?;
    let [Statement::IfStatement(first)] = function.body.as_ref()?.statements.as_slice() else {
        return None;
    };
    let Statement::IfStatement(second) = first.alternate.as_ref()? else {
        return None;
    };
    let [s] = statements(&second.consequent)? else {
        return None;
    };
    let c = call(expression_statement(s)?)?;
    let first_arg = arguments(c)?.first().copied()?;
    let (tag, _) = binary(first_arg, "|")?;
    let table = captured_symbol(model, site, symbol(model, member(tag, "UINT14_BASE")?)?)?;
    use ByteExpr::{Tag as T, Value as V};
    let bytes = |v: ByteExpr| {
        vec![
            v.clone().n(">>>", 24.0),
            v.clone().n(">>", 16.0).n("&", 255.0),
            v.clone().n(">>", 8.0).n("&", 255.0),
            v.n("&", 255.0),
        ]
    };
    let tiers = vec![
        vec![vec![V]],
        vec![vec![
            T("UINT14_BASE").op("|", V.n(">>", 8.0)),
            V.n("&", 255.0),
        ]],
        vec![vec![T("UINT16"), V.n(">>", 8.0), V.n("&", 255.0)]],
        vec![vec![
            T("UINT24"),
            V.n(">>", 16.0),
            V.n(">>", 8.0).n("&", 255.0),
            V.n("&", 255.0),
        ]],
        vec![vec![T("UINT32")], bytes(V)],
        vec![
            vec![T("UINT64")],
            bytes(V.n("/", 4294967296.0).n("&", 4294967295.0)),
            bytes(V.n("&", 4294967295.0)),
        ],
    ];
    let thresholds = [63.0, 16383.0, 65535.0, 16777215.0, 4294967295.0];
    let mut branch = &**first;
    for (index, groups) in tiers.iter().enumerate() {
        let body = if index < 5 {
            let (input, limit) = binary(&branch.test, "<=")?;
            if symbol(model, input) != Some(value) || !number(limit, thresholds[index]) {
                return None;
            }
            statements(&branch.consequent)?
        } else {
            statements(branch.alternate.as_ref()?)?
        };
        if body.len() != groups.len() {
            return None;
        }
        for (s, expected) in body.iter().zip(groups) {
            push_group(model, s, output, expected, value, table)?;
        }
        if index < 4 {
            let Statement::IfStatement(next) = branch.alternate.as_ref()? else {
                return None;
            };
            branch = next;
        }
    }
    let stores = tag_stores(
        model,
        site,
        table,
        &[
            ("UINT14_BASE", 64.0),
            ("UINT16", 228.0),
            ("UINT24", 229.0),
            ("UINT32", 230.0),
            ("UINT64", 231.0),
        ],
    )?;
    exposures_ready(
        model,
        site.binding_symbol?,
        &stores,
        &mut BTreeSet::new(),
        0,
    )
    .then_some(())
}

fn global(model: &Model<'_, '_>, e: &Expression<'_>, name: &str) -> bool {
    crate::naming::support::installer_events::global_path(model, e).is_some_and(|p| p.as_slice() == [name])
}
fn local_decl(model: &Model<'_, '_>, s: &Statement<'_>) -> Option<SymbolId> {
    plain_var(model, variable(s)?)
}
fn initial<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    variable(s)?.init.as_ref()
}
fn indexed(model: &Model<'_, '_>, e: &Expression<'_>, receiver: SymbolId, index: f64) -> bool {
    matches!(e,Expression::ComputedMemberExpression(m) if !m.optional && symbol(model,&m.object)==Some(receiver) && number(&m.expression,index))
}
fn store_zero(
    model: &Model<'_, '_>,
    s: &Statement<'_>,
    receiver: SymbolId,
    value: SymbolId,
) -> Option<()> {
    let a = assignment(expression_statement(s)?)?;
    let AssignmentTarget::ComputedMemberExpression(m) = &a.left else {
        return None;
    };
    (!m.optional
        && symbol(model, &m.object) == Some(receiver)
        && number(&m.expression, 0.0)
        && symbol(model, &a.right) == Some(value))
    .then_some(())
}
fn plain_assign<'a>(
    model: &Model<'_, '_>,
    s: &'a Statement<'a>,
    target: SymbolId,
) -> Option<&'a Expression<'a>> {
    let a = assignment(expression_statement(s)?)?;
    (target_symbol(model, &a.left) == Some(target)).then_some(&a.right)
}
fn new_one<'a>(e: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
    let Expression::NewExpression(n) = e else {
        return None;
    };
    matches!(n.arguments.as_slice(),[Argument::NumericLiteral(v)] if v.value==1.0)
        .then_some(&n.callee)
}
fn typed_float(model: &Model<'_, '_>, site: FunctionSite, function: &Function<'_>) -> Option<()> {
    let (value, output) = plain_pair(model, function)?;
    let [tag, first, write, alias, Statement::IfStatement(branch), view, push, count, Statement::WhileStatement(w)] =
        function.body.as_ref()?.statements.as_slice()
    else {
        return None;
    };
    let tag_symbol = local_decl(model, tag)?;
    let table = captured_symbol(
        model,
        site,
        symbol(model, member(initial(tag)?, "FLOAT32")?)?,
    )?;
    let first_symbol = local_decl(model, first)?;
    if !global(model, new_one(initial(first)?)?, "Float32Array")
        || model.binding_writes(first_symbol) != 0
    {
        return None;
    }
    store_zero(model, write, first_symbol, value)?;
    let storage = local_decl(model, alias)?;
    if symbol(model, initial(alias)?) != Some(first_symbol)
        || model.binding_writes(storage) != 2
        || model.binding_writes(tag_symbol) != 1
    {
        return None;
    }
    let (roundtrip, input) = binary(&branch.test, "!==")?;
    if !indexed(model, roundtrip, storage, 0.0)
        || symbol(model, input) != Some(value)
        || branch.alternate.is_some()
    {
        return None;
    }
    let [retag, constructor, reallocate, rewrite] = statements(&branch.consequent)? else {
        return None;
    };
    if symbol(
        model,
        member(plain_assign(model, retag, tag_symbol)?, "DOUBLE64")?,
    ) != Some(table)
        || !global(
            model,
            plain_assign(model, constructor, storage)?,
            "Float64Array",
        )
        || symbol(model, new_one(plain_assign(model, reallocate, storage)?)?) != Some(storage)
    {
        return None;
    }
    store_zero(model, rewrite, storage, value)?;
    let view_symbol = local_decl(model, view)?;
    let Expression::NewExpression(n) = initial(view)? else {
        return None;
    };
    let [buffer] = n.arguments.as_slice() else {
        return None;
    };
    if !global(model, &n.callee, "Uint8Array")
        || symbol(model, member(buffer.as_expression()?, "buffer")?) != Some(storage)
        || model.binding_writes(view_symbol) != 0
    {
        return None;
    }
    let c = call(expression_statement(push)?)?;
    if method_receiver(model, c, "push") != Some(output)
        || !matches!(c.arguments.as_slice(),[arg] if arg.as_expression().and_then(|e|symbol(model,e))==Some(tag_symbol))
    {
        return None;
    }
    let cursor = local_decl(model, count)?;
    if symbol(model, member(initial(count)?, "length")?) != Some(view_symbol)
        || model.binding_writes(cursor) != 1
    {
        return None;
    }
    let (minus, zero) = binary(&w.test, ">=")?;
    let (index, one) = binary(minus, "-")?;
    if !number(zero, 0.0) || symbol(model, index) != Some(cursor) || !number(one, 1.0) {
        return None;
    }
    let [append, decrement] = statements(&w.body)? else {
        return None;
    };
    let append = call(expression_statement(append)?)?;
    if method_receiver(model, append, "push") != Some(output) {
        return None;
    }
    let [arg] = append.arguments.as_slice() else {
        return None;
    };
    let Expression::ComputedMemberExpression(byte) = arg.as_expression()? else {
        return None;
    };
    let (index, one) = binary(&byte.expression, "-")?;
    if byte.optional
        || symbol(model, &byte.object) != Some(view_symbol)
        || symbol(model, index) != Some(cursor)
        || !number(one, 1.0)
    {
        return None;
    }
    let Expression::UpdateExpression(decrement) = expression_statement(decrement)? else {
        return None;
    };
    let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &decrement.argument else {
        return None;
    };
    if !decrement.prefix
        || decrement.operator.as_str() != "--"
        || crate::analysis::identifier_symbol(id, model.scoping) != Some(cursor)
    {
        return None;
    }
    for statement in [tag, alias, count] {
        let Statement::VariableDeclaration(d) = statement else {
            return None;
        };
        if d.kind == VariableDeclarationKind::Const {
            return None;
        }
    }
    let locals = [tag_symbol, first_symbol, storage, view_symbol, cursor];

    if locals.iter().copied().collect::<BTreeSet<_>>().len() != locals.len() {
        return None;
    }
    let stores = tag_stores(
        model,
        site,
        table,
        &[("FLOAT32", 236.0), ("DOUBLE64", 237.0)],
    )?;
    exposures_ready(
        model,
        site.binding_symbol?,
        &stores,
        &mut BTreeSet::new(),
        0,
    )
    .then_some(())
}

fn equality_wrapper(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
) -> Option<()> {
    if function.r#async || function.generator || function.params.rest.is_some() {
        return None;
    }
    let [parameter] = function.params.items.as_slice() else {
        return None;
    };
    let parameter = plain_param(model, parameter)?;
    let body = function.body.as_ref()?;
    if !body.directives.is_empty() {
        return None;
    }
    let [ret] = body.statements.as_slice() else {
        return None;
    };
    let c = call(returned(ret)?)?;
    let args = arguments(c)?;
    let [value, comparison] = args.as_slice() else {
        return None;
    };
    if symbol(model, value) != Some(parameter) {
        return None;
    }
    // Prove this exact callee independently; proposals collected in the same
    // shared pass deliberately cannot supply each other's identity.
    let callee = shared::stable_function(model, &c.callee, 0)?;
    let AstKind::Function(callee_function) = model.nodes.kind(callee.node_id) else {
        return None;
    };
    equality(model, callee_function)?;
    let captured = captured_symbol(model, site, symbol(model, comparison)?)?;
    if model.binding_writes(captured) != 0 {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(captured);
    let AstKind::VariableDeclarator(v) = model.nodes.kind(declaration) else {
        return None;
    };
    let input = symbol(model, v.init.as_ref()?)?;
    let owner = model.owning_site(declaration)?;
    let AstKind::Function(owner) = model.nodes.kind(owner.node_id) else {
        return None;
    };
    if owner.params.rest.is_some()
        || !owner
            .params
            .items
            .iter()
            .any(|p| plain_param(model, p) == Some(input))
    {
        return None;
    }
    let parent = model.nodes.parent_id(model.nodes.parent_id(declaration));
    if !matches!(model.nodes.kind(parent), AstKind::FunctionBody(_)) {
        return None;
    }
    exposures_ready(
        model,
        site.binding_symbol?,
        &[TagStore {
            body: parent,
            end: v.span.end,
        }],
        &mut BTreeSet::new(),
        0,
    )
    .then_some(())
}

fn independent_role(
    model: &Model<'_, '_>,
    function: &Function<'_>,
    name: &str,
    proposals: &BTreeMap<u32, Vec<Proposal>>,
    installer: bool,
) -> bool {
    let Some(site) = model.site_by_span(function.span) else {
        return false;
    };
    let facts = Facts::default();
    let claims = if installer {
        runtime::class_member_installer::evaluate(model, site, &facts, proposals)
    } else {
        runtime::extension_registry::evaluate(model, site, &facts, proposals)
    };
    claims.len() == 1 && claims[0].name == name
}

fn installer_stores(
    model: &Model<'_, '_>,
    site: FunctionSite,
    installer: SymbolId,
    proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Option<Vec<TagStore>> {
    captured_symbol(model, site, installer)?;
    let declaration = model.scoping.symbol_declaration(installer);
    let AstKind::VariableDeclarator(v) = model.nodes.kind(declaration) else {
        return None;
    };
    if v.init.is_some() {
        return None;
    }
    let owner = model.owning_site(declaration)?.node_id;
    let mut stores = Vec::new();
    for r in model
        .scoping
        .get_resolved_references(installer)
        .filter(|r| r.is_write())
    {
        let node = model.nodes.parent_id(r.node_id());
        let AstKind::AssignmentExpression(a) = model.nodes.kind(node) else {
            return None;
        };
        if a.operator.as_str() != "="
            || target_symbol(model, &a.left) != Some(installer)
            || model.owning_site(node)?.node_id != owner
        {
            return None;
        }
        let statement = model.nodes.parent_id(node);
        let body = model.nodes.parent_id(statement);
        if !matches!(model.nodes.kind(statement), AstKind::ExpressionStatement(_))
            || !matches!(
                model.nodes.kind(body),
                AstKind::FunctionBody(_) | AstKind::BlockStatement(_)
            )
        {
            return None;
        }
        let c = call(&a.right)?;
        if !c.arguments.is_empty() {
            return None;
        }
        let provider = shared::stable_function(model, &c.callee, 0)?;
        let AstKind::Function(provider) = model.nodes.kind(provider.node_id) else {
            return None;
        };
        if !independent_role(
            model,
            provider,
            "createClassMembersInstaller",
            proposals,
            true,
        ) {
            return None;
        }
        stores.push(TagStore {
            body,
            end: a.span.end,
        });
    }
    (!stores.is_empty() && stores.len() == model.binding_writes(installer)).then_some(stores)
}
fn registry_factory(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Option<()> {
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || !function.params.items.is_empty()
    {
        return None;
    }
    let body = function.body.as_ref()?;
    if !body.directives.is_empty() {
        return None;
    }
    let [alias, Statement::FunctionDeclaration(constructor), assign, descriptor, Statement::FunctionDeclaration(register), attach, install, ret] =
        body.statements.as_slice()
    else {
        return None;
    };
    let alias_var = variable(alias)?;
    let alias = plain_var(model, alias_var)?;
    let constructor_symbol = constructor.id.as_ref()?.symbol_id.get()?;
    let register_symbol = register.id.as_ref()?.symbol_id.get()?;
    if alias_var.init.is_some()
        || model.binding_writes(alias) != 1
        || symbol(model, plain_assign(model, assign, alias)?) != Some(constructor_symbol)
        || model.binding_writes(constructor_symbol) != 0
        || model.binding_writes(register_symbol) != 0
        || !independent_role(model, constructor, "ExtensionRegistry", proposals, false)
        || !independent_role(model, register, "registerExtension", proposals, false)
    {
        return None;
    }
    let descriptor_symbol = local_decl(model, descriptor)?;
    if model.binding_writes(descriptor_symbol) != 0 {
        return None;
    }
    let Expression::ObjectExpression(record) = initial(descriptor)? else {
        return None;
    };
    let [ObjectPropertyKind::ObjectProperty(property)] = record.properties.as_slice() else {
        return None;
    };
    if property.computed
        || property.method
        || property.shorthand
        || property.kind != PropertyKind::Init
        || property_key_name(&property.key)?.as_str() != "key"
        || !matches!(&property.value,Expression::StringLiteral(s)if s.value=="extend")
    {
        return None;
    }
    let attach = assignment(expression_statement(attach)?)?;
    let AssignmentTarget::StaticMemberExpression(target) = &attach.left else {
        return None;
    };
    if target.optional
        || target.property.name != "value"
        || symbol(model, &target.object) != Some(descriptor_symbol)
        || symbol(model, &attach.right) != Some(register_symbol)
    {
        return None;
    }
    let install = call(expression_statement(install)?)?;
    let installer = symbol(model, &install.callee)?;
    let args = arguments(install)?;
    let [receiver, members] = args.as_slice() else {
        return None;
    };
    let Expression::ArrayExpression(members) = members else {
        return None;
    };
    let [descriptor] = members.elements.as_slice() else {
        return None;
    };
    if symbol(model, receiver) != Some(alias)
        || descriptor.as_expression().and_then(|e| symbol(model, e)) != Some(descriptor_symbol)
        || symbol(model, returned(ret)?) != Some(alias)
    {
        return None;
    }
    let stores = installer_stores(model, site, installer, proposals)?;
    exposures_ready(
        model,
        site.binding_symbol?,
        &stores,
        &mut BTreeSet::new(),
        0,
    )
    .then_some(())
}

// FAMILY01 glue: retain the existing private capture implementation and expose
// only its composed boolean proof, not TagStore or a broader value resolver.
pub(in crate::naming) fn software_float_capture_ready(
    model: &Model<'_, '_>,
    site: FunctionSite,
    table: SymbolId,
) -> Option<()> {
    let stores = tag_stores(model, site, table, &[("FLOAT32", 236.0), ("DOUBLE64", 237.0)])?;
    exposures_ready(model, site.binding_symbol?, &stores, &mut BTreeSet::new(), 0).then_some(())
}
