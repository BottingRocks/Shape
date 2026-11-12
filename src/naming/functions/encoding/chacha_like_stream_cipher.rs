//! Bounded, source-backed shared stream-cipher rule.
//!
//! A rule fires only when the function's own body, its scoped identities and the
//! independently established role of the callable it returns prove the whole
//! operation. Generated spellings, byte offsets, reference labels and
//! whole-program facts never participate.
use super::*;
use oxc_semantic::SymbolId;

fn binding_symbol(pattern: &BindingPattern<'_>) -> Option<SymbolId> {
    let BindingPattern::BindingIdentifier(id) = pattern else {
        return None;
    };
    id.symbol_id.get()
}

// A plain local binding with a single static origin: never written or
// redeclared.
fn immutable(model: &Model<'_, '_>, symbol: SymbolId) -> bool {
    model.binding_writes(symbol) == 0 && model.scoping.symbol_redeclarations(symbol).is_empty()
}

// A pure numeric expression, returning the number of `Math.pow` calls it
// contains. Allowed: numeric literals, the explicit unary and binary arithmetic
// operators, unshadowed `Math.pow` of two such arguments, and reads of earlier
// proven prelude constants of this same owned body. Every other expression
// (any other call, resource origin, mutation or non-numeric read) is refused.
fn pure_numeric(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    prelude: &[SymbolId],
) -> Option<usize> {
    match expression.without_parentheses() {
        Expression::NumericLiteral(_) => Some(0),
        Expression::UnaryExpression(unary)
            if matches!(unary.operator.as_str(), "-" | "+" | "~") =>
        {
            pure_numeric(model, &unary.argument, prelude)
        }
        Expression::BinaryExpression(binary)
            if matches!(
                binary.operator.as_str(),
                "+" | "-" | "*" | "/" | "%" | "**" | "<<" | ">>" | ">>>" | "&" | "|" | "^"
            ) =>
        {
            pure_numeric(model, &binary.left, prelude)?.checked_add(pure_numeric(
                model,
                &binary.right,
                prelude,
            )?)
        }
        Expression::Identifier(identifier) => {
            let symbol = crate::analysis::identifier_symbol(identifier, model.scoping)?;
            prelude.contains(&symbol).then_some(0)
        }
        Expression::CallExpression(call) => {
            if call.optional
                || call.arguments.len() != 2
                || expression_path(model, &call.callee) != ["Math", "pow"]
            {
                return None;
            }
            let mut calls = 1usize;
            for argument in &call.arguments {
                calls =
                    calls.checked_add(pure_numeric(model, argument.as_expression()?, prelude)?)?;
            }
            Some(calls)
        }
        _ => None,
    }
}

// A zero-input factory whose entire owned body is a supported pure numeric
// prelude, one nested function declaration and that unmutated function's direct
// return. The nested function must be proven by its own independent evaluator as
// `applyStreamCipher`; an inherited reference label or the binding spelling is
// never evidence. No loop, throw, `new`, deletion, write, extra return, control
// flow, resource origin or unexpected call is allowed.
fn stream_cipher_factory(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
    f: &Facts<'_>,
    proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Option<()> {
    // Zero formals: the callable takes no inputs.
    if !function.params.items.is_empty() || function.params.rest.is_some() {
        return None;
    }
    // Whole-body shape facts. `members` may only be the prelude's `pow` and
    // `globals` only the unshadowed `Math` root.
    if f.returns.len() != 1
        || f.loops != 0
        || f.throws != 0
        || f.news != 0
        || f.deletes != 0
        || !f.assignments.is_empty()
        || !f.regexes.is_empty()
        || !f.writes.is_empty()
        || !f.members.iter().all(|member| member == "pow")
        || !f.globals.iter().all(|global| global == "Math")
    {
        return None;
    }
    let Some(Statement::ReturnStatement(returned)) = body.statements.last() else {
        return None;
    };
    let argument = returned.argument.as_ref()?;
    let mut prelude = Vec::new();
    let mut child = None;
    let mut pow_calls = 0usize;
    for statement in &body.statements[..body.statements.len() - 1] {
        match statement {
            Statement::VariableDeclaration(declaration) => {
                // The whole owned prelude proves plain locals; resource
                // declarations (using / await using) never qualify.
                if !matches!(
                    declaration.kind,
                    VariableDeclarationKind::Var
                        | VariableDeclarationKind::Let
                        | VariableDeclarationKind::Const
                ) {
                    return None;
                }
                for variable in &declaration.declarations {
                    let symbol = binding_symbol(&variable.id)?;
                    if !immutable(model, symbol) {
                        return None;
                    }
                    // Every initializer is a supported pure numeric prelude
                    // expression; nothing effectful may hide here.
                    pow_calls = pow_calls.checked_add(pure_numeric(
                        model,
                        variable.init.as_ref()?,
                        &prelude,
                    )?)?;
                    prelude.push(symbol);
                }
            }
            Statement::FunctionDeclaration(found) => {
                if child.is_some() {
                    return None;
                }
                let symbol = found.id.as_ref()?.symbol_id.get()?;
                if !immutable(model, symbol) {
                    return None;
                }
                child = Some(symbol);
            }
            // Any other statement (expression, block, if, loop, class, throw,
            // second function, extra return) is unexpected.
            _ => return None,
        }
    }
    let child = child?;
    // All calls in the factory-owned body are the proven numeric prelude and at
    // least one `Math.pow` establishes the prelude.
    if pow_calls == 0 || pow_calls != f.calls.len() {
        return None;
    }
    // The sole return is exactly that nested function, directly and unmutated.
    let Expression::Identifier(returned_identifier) = argument.without_parentheses() else {
        return None;
    };
    if crate::analysis::identifier_symbol(returned_identifier, model.scoping) != Some(child) {
        return None;
    }
    let child_site = stable_function(model, argument, 0)?;
    if child_site.span == site.span
        || model
            .enclosing_function(child_site.node_id)
            .map(|parent| parent.span)
            != Some(site.span)
    {
        return None;
    }
    // Never a source spelling and never an inherited reference label: only the
    // child's own independent, non-inherited evaluator establishes the role.
    (role(model, argument, proposals) == Some("applyStreamCipher")).then_some(())
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
    let Some(body) = function.body.as_ref() else {
        return Vec::new();
    };
    if function.r#async || function.generator || !body.directives.is_empty() {
        return Vec::new();
    }
    if stream_cipher_factory(model, site, function, body, f, proposals).is_some() {
        return vec![proposal(
            "createChaChaLikeStreamCipher",
            "encoding_chacha_like_stream_cipher",
            "zero-input factory whose whole owned body is a proven pure numeric prelude plus one direct, unmutated nested function independently evaluated as applyStreamCipher",
            f,
        )];
    }
    Vec::new()
}
