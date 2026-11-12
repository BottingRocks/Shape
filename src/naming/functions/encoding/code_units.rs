//! Fast-loop shared rule: null-safe character-code array to string.
//!
//! The whole owned body is proved: a fresh local array, a `formal != null`
//! guard, exactly one `formal.forEach(callback)`, a callback that pushes one
//! provider conversion of its exact element into that same array, and a sole
//! `return array.join("")`. The role name `codeUnitsToString` is the existing
//! canonical catalogue name for exactly this algorithm.
//!
//! This module is the late fallback over the PASS6 shared rules. It accepts two
//! provider forms: `<opaque-root>.String.fromCharCode(...)`, where the root
//! resolves through immutable single-origin plain aliases to an unbound host
//! namespace, and the canonical `String.fromCharCode(...)` global only when that
//! exact identifier is the unshadowed global `String` (a local, parameter or
//! alias binding is refused). The `String` and `fromCharCode` member names are
//! proven structurally from the source; the opaque root is never claimed to be
//! the native global `String`, a sentinel, or a statically bound callee. The sole
//! return is `output.join("")`, or the bounded bound call
//! `<recv>.join.call(output, "")` whose receiver is that same output array or its
//! exact truthy guard `output ? output : void 0` and whose explicit `this` is the
//! output array.
use super::*;
use oxc_semantic::SymbolId;

fn symbol(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<SymbolId> {
    crate::analysis::expression_symbol(e, model.scoping)
}

fn member<'a>(e: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
    match e {
        Expression::StaticMemberExpression(m) if !m.optional && m.property.name == key => {
            Some(&m.object)
        }
        _ => None,
    }
}

fn call<'a>(e: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    match e {
        Expression::CallExpression(c) if !c.optional => Some(c),
        _ => None,
    }
}

fn expression<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    match s {
        Statement::ExpressionStatement(s) => Some(&s.expression),
        _ => None,
    }
}

fn returned<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    match s {
        Statement::ReturnStatement(s) => s.argument.as_ref(),
        _ => None,
    }
}

fn one_variable<'a>(s: &'a Statement<'a>) -> Option<&'a VariableDeclarator<'a>> {
    let Statement::VariableDeclaration(d) = s else {
        return None;
    };
    let [v] = d.declarations.as_slice() else {
        return None;
    };
    Some(v)
}

fn immutable(model: &Model<'_, '_>, s: SymbolId) -> bool {
    model.binding_writes(s) == 0 && model.scoping.symbol_redeclarations(s).is_empty()
}

fn plain_local(model: &Model<'_, '_>, v: &VariableDeclarator<'_>) -> Option<SymbolId> {
    let BindingPattern::BindingIdentifier(id) = &v.id else {
        return None;
    };
    let s = id.symbol_id.get()?;
    model
        .scoping
        .symbol_redeclarations(s)
        .is_empty()
        .then_some(s)
}

/// Resolve a provider namespace root through immutable, single-origin plain
/// `var`/`let`/`const` aliases. The terminal must be an *unbound* identifier:
/// an opaque host root whose meaning this rule never upgrades to a native
/// builtin identity. Destructuring, parameters, loop bindings, writes,
/// redeclarations and non-variable initializers are refused.
fn opaque_root(model: &Model<'_, '_>, e: &Expression<'_>, depth: usize) -> bool {
    if depth > 8 {
        return false;
    }
    let Expression::Identifier(i) = e else {
        return false;
    };
    let Some(s) = crate::analysis::identifier_symbol(i, model.scoping) else {
        // Unbound host namespace root: structural protocol evidence only.
        return true;
    };
    if !immutable(model, s) {
        return false;
    }
    let declaration = model.scoping.symbol_declaration(s);
    let AstKind::VariableDeclarator(v) = model.nodes.kind(declaration) else {
        return false;
    };
    if !matches!(&v.id, BindingPattern::BindingIdentifier(id)
        if id.symbol_id.get() == Some(s))
        || !matches!(
            model.nodes.parent_kind(declaration),
            AstKind::VariableDeclaration(d)
                if matches!(
                    d.kind,
                    VariableDeclarationKind::Var
                        | VariableDeclarationKind::Let
                        | VariableDeclarationKind::Const
                )
        )
        || matches!(
            model.nodes.parent_kind(model.nodes.parent_id(declaration)),
            AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
        )
    {
        return false;
    }
    let Some(init) = v.init.as_ref() else {
        return false;
    };
    opaque_root(model, init, depth + 1)
}

/// The provider must be exactly `<opaque-root>.String.fromCharCode`: both
/// member reads are non-optional, the property names are literal source
/// operations, and the root resolves to an unbound host namespace.
fn provider(model: &Model<'_, '_>, callee: &Expression<'_>) -> Option<()> {
    let string = member(callee, "fromCharCode")?;
    let root = member(string, "String")?;
    opaque_root(model, root, 0).then_some(())
}

/// The canonical `String.fromCharCode(...)` global provider, accepted only as the
/// late fallback when [`provider`] and every earlier PASS6 stage declined the
/// whole body. The `fromCharCode` member read is the same non-optional, literal
/// operation, but the object must be exactly the *unshadowed* global identifier
/// `String`: the scoped path proof yields a path only for an unresolved root, so
/// an arbitrary unbound root and any local, parameter or alias binding fail. This
/// is deliberately narrower than [`provider`], which proves only the opaque
/// `<root>.String` namespace shape and never upgrades a root to native `String`.
fn canonical_provider(model: &Model<'_, '_>, callee: &Expression<'_>) -> bool {
    let Some(string) = member(callee, "fromCharCode") else {
        return false;
    };
    matches!(expression_path(model, string).as_slice(), [root] if root == "String")
}

/// The sole return must produce the joined output. The original arm is the
/// direct `output.join("")`. The added bounded arm is `<recv>.join.call`: the
/// method receiver is either the same output local or its exact truthy guard
/// `output ? output : void 0` (sound because the output local is a proven
/// literal empty array), the explicit `this` is that same output local, and the
/// separator is exactly the empty string literal. Any other receiver, `this`,
/// arity or separator is refused, and the original arm is tested first.
fn joined_output(model: &Model<'_, '_>, e: &Expression<'_>, output: SymbolId) -> bool {
    let Some(join) = call(e) else {
        return false;
    };
    if let Some(target) = member(&join.callee, "join") {
        return symbol(model, target) == Some(output)
            && matches!(
                join.arguments.as_slice(),
                [Argument::StringLiteral(s)] if s.value.is_empty()
            );
    }
    let Some(method) = member(&join.callee, "call") else {
        return false;
    };
    let Some(receiver) = member(method, "join") else {
        return false;
    };
    let guarded = matches!(
        receiver.without_parentheses(),
        Expression::ConditionalExpression(c)
            if symbol(model, &c.test) == Some(output)
                && symbol(model, &c.consequent) == Some(output)
                && matches!(&c.alternate, Expression::UnaryExpression(u)
                    if u.operator.as_str() == "void"
                        && matches!(&u.argument, Expression::NumericLiteral(n) if n.value == 0.0))
    );
    if symbol(model, receiver) != Some(output) && !guarded {
        return false;
    }
    matches!(
        join.arguments.as_slice(),
        [this, Argument::StringLiteral(separator)]
            if separator.value.is_empty()
                && this.as_expression().and_then(|e| symbol(model, e)) == Some(output)
    )
}

fn code_units(model: &Model<'_, '_>, site: FunctionSite, body: &FunctionBody<'_>) -> Option<()> {
    let [initial, Statement::IfStatement(branch), end] = body.statements.as_slice() else {
        return None;
    };
    let v = one_variable(initial)?;
    let output = plain_local(model, v)?;
    if !immutable(model, output)
        || !matches!(v.init.as_ref()?, Expression::ArrayExpression(a) if a.elements.is_empty())
        || branch.alternate.is_some()
    {
        return None;
    }
    let Expression::BinaryExpression(test) = &branch.test else {
        return None;
    };
    if test.operator.as_str() != "!="
        || model.site_parameter_slot(&test.left, site) != Some(0)
        || !matches!(&test.right, Expression::NullLiteral(_))
    {
        return None;
    }
    let Statement::BlockStatement(block) = &branch.consequent else {
        return None;
    };
    let [each] = block.body.as_slice() else {
        return None;
    };
    let each = call(expression(each)?)?;
    if model.site_parameter_slot(member(&each.callee, "forEach")?, site) != Some(0) {
        return None;
    }
    let [Argument::ArrowFunctionExpression(a)] = each.arguments.as_slice() else {
        return None;
    };
    let [p] = a.params.items.as_slice() else {
        return None;
    };
    let BindingPattern::BindingIdentifier(id) = &p.pattern else {
        return None;
    };
    let input = id.symbol_id.get()?;
    if a.r#async
        || a.params.rest.is_some()
        || p.initializer.is_some()
        || !immutable(model, input)
        || !a.body.directives.is_empty()
    {
        return None;
    }
    let [s] = a.body.statements.as_slice() else {
        return None;
    };
    let push = call(expression(s)?)?;
    if symbol(model, member(&push.callee, "push")?) != Some(output) || push.arguments.len() != 1 {
        return None;
    }
    let args = call_arguments(push);
    let convert = call(args.first()?)?;
    if (provider(model, &convert.callee).is_none() && !canonical_provider(model, &convert.callee))
        || convert.arguments.len() != 1
        || call_arguments(convert)
            .first()
            .and_then(|e| symbol(model, e))
            != Some(input)
    {
        return None;
    }
    joined_output(model, returned(end)?, output).then_some(())
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
        || !body.directives.is_empty()
        || function.params.rest.is_some()
    {
        return Vec::new();
    }
    let [parameter] = function.params.items.as_slice() else {
        return Vec::new();
    };
    if parameter.initializer.is_some()
        || !matches!(&parameter.pattern, BindingPattern::BindingIdentifier(_))
    {
        return Vec::new();
    }
    if code_units(model, site, body).is_none() {
        return Vec::new();
    }
    vec![proposal(
        "codeUnitsToString",
        "fast_code_units_opaque_provider",
        "whole owned body: fresh immutable local empty array; exact formal `!= null` guard with no else; exactly one `formal.forEach` with a single plain non-async arrow formal; callback performs one `output.push(root.String.fromCharCode(element))` where the root resolves through immutable single-origin plain aliases to an unbound host namespace, or one `output.push(String.fromCharCode(element))` on the unshadowed global `String`; the member names are literal; sole return of `output.join(\"\")` or the bound `<recv>.join.call(output, \"\")` whose receiver is the same output array or its exact truthy guard `output ? output : void 0`; no other statement, write or redeclaration",
        f,
    )]
}

// codeUnitsToString: the last fallback stage in `stages.rs`.

const CODE_UNITS_TRUTHY: &str = r#"function convert(input){
let output=[];
if(input!=null){input.forEach(unit=>{output.push(String.fromCharCode(unit));});}
return (output?output:output-1).join("");
}"#;
const CODE_UNITS_COMPUTED: &str = r#"function convert(input){
let output=[];
if(input!=null){input.forEach(unit=>{output.push(String.fromCharCode(unit));});}
let absent=!output;
return output[absent?"isArray":"join"]("");
}"#;

pub(in crate::naming) fn evaluate_code_units_to_string(model: &Model<'_, '_>, site: FunctionSite, f: &Facts<'_>) -> Vec<Proposal> {
    if !f.members.contains("forEach") { return vec![]; }
    if [CODE_UNITS_TRUTHY, CODE_UNITS_COMPUTED].iter().any(|s|
        super::aes_gcm_packing::complete_uncaptured(model, site, s)) {
        let mut claim = proposal("codeUnitsToString", "encoding_code_units_truthy", "Complete own body: fresh local array, null-guarded formal.forEach, single owned callback pushes String.fromCharCode of its sole formal; exact truthy-array or boolean-selected join receiver and empty separator; every binding, callback, effect and returned value matched, no captures or extra writers; String must be unshadowed", f);
        claim.evidence = vec![site.span.into()];
        return vec![claim];
    }
    vec![]
}
