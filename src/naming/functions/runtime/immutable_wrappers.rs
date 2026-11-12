//! Bounded source proofs for immutable alias preludes and exact forwarding.
//! No proposal metadata is used to prove a callee's behavior.
use super::*;
use oxc_semantic::SymbolId;
use oxc_span::GetSpan;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Value {
    Slot(u32),
    Capture(SymbolId),
    Global(Vec<String>),
    Bool(bool),
    EmptyString,
}

struct Normalizer<'m, 's, 'a> {
    model: &'m Model<'s, 'a>,
    site: FunctionSite,
    locals: BTreeMap<SymbolId, Value>,
}

impl Normalizer<'_, '_, '_> {
    fn value(&self, e: &Expression<'_>, depth: usize) -> Option<Value> {
        if depth > 12 {
            return None;
        }
        match e {
            Expression::Identifier(id) => {
                let Some(s) = crate::analysis::identifier_symbol(id, self.model.scoping) else {
                    return Some(Value::Global(vec![id.name.to_string()]));
                };
                if let Some(value) = self.locals.get(&s) {
                    return Some(value.clone());
                }
                if let Some(slot) = self.model.site_parameter_slot(e, self.site) {
                    return Some(Value::Slot(slot));
                }
                // An owned local must have been established by an earlier
                // prelude statement. In particular, do not follow forward or
                // cyclic local initializers through the global Model index.
                if self
                    .site
                    .span
                    .contains_inclusive(self.model.scoping.symbol_span(s))
                {
                    return None;
                }
                // API aliases may originate only in unredeclared, immutable,
                // plain variable initializers. Parameter/catch/loop origins
                // are never interpreted as global APIs or callable origins.
                if self.model.binding_writes(s) == 0
                    && self.model.scoping.symbol_redeclarations(s).is_empty()
                {
                    let declaration = self.model.scoping.symbol_declaration(s);
                    if let AstKind::VariableDeclarator(v) = self.model.nodes.kind(declaration) {
                        if matches!(&v.id, BindingPattern::BindingIdentifier(i) if i.symbol_id.get() == Some(s))
                            && matches!(
                                self.model.nodes.parent_kind(declaration),
                                AstKind::VariableDeclaration(d) if matches!(d.kind, VariableDeclarationKind::Var | VariableDeclarationKind::Let | VariableDeclarationKind::Const)
                            )
                            && !matches!(
                                self.model
                                    .nodes
                                    .parent_kind(self.model.nodes.parent_id(declaration)),
                                AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
                            )
                        {
                            if let Some(value @ Value::Global(_)) = v
                                .init
                                .as_ref()
                                .filter(|v| v.span().end <= id.span.start)
                                .and_then(|v| self.value(v, depth + 1))
                            {
                                return Some(value);
                            }
                        }
                    }
                }
                (self.model.binding_writes(s) == 0
                    && self.model.scoping.symbol_redeclarations(s).is_empty())
                .then_some(Value::Capture(s))
            }
            Expression::StaticMemberExpression(m) if !m.optional => {
                let Value::Global(mut path) = self.value(&m.object, depth + 1)? else {
                    return None;
                };
                path.push(m.property.name.to_string());
                Some(Value::Global(path))
            }
            Expression::BooleanLiteral(b) => Some(Value::Bool(b.value)),
            Expression::StringLiteral(s) if s.value.is_empty() => Some(Value::EmptyString),
            _ => None,
        }
    }

    /// A sentinel is proved by its single plain initialization, never its name.
    /// Keep this separate from callable resolution: parameters, catches and
    /// opaque captures can be values but cannot establish this source origin.
    fn null_prototype_sentinel(&self, e: &Expression<'_>) -> bool {
        let Some(Value::Capture(symbol)) = self.value(e, 0) else {
            return false;
        };
        let declaration = self.model.scoping.symbol_declaration(symbol);
        let AstKind::VariableDeclarator(v) = self.model.nodes.kind(declaration) else {
            return false;
        };
        if !matches!(&v.id, BindingPattern::BindingIdentifier(i) if i.symbol_id.get() == Some(symbol))
            || !matches!(
                self.model.nodes.parent_kind(declaration),
                AstKind::VariableDeclaration(d) if matches!(d.kind, VariableDeclarationKind::Var | VariableDeclarationKind::Let | VariableDeclarationKind::Const)
            )
            || matches!(
                self.model
                    .nodes
                    .parent_kind(self.model.nodes.parent_id(declaration)),
                AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
            )
        {
            return false;
        }
        let Some(Expression::CallExpression(call)) = &v.init else {
            return false;
        };
        let container = self
            .model
            .nodes
            .parent_id(self.model.nodes.parent_id(declaration));
        if !matches!(
            self.model.nodes.kind(container),
            AstKind::FunctionBody(_) | AstKind::Program(_)
        ) {
            return false;
        }
        // The normalizer already refused writes/redeclarations. Reject future
        // initializers as well, including a copied capture in the alias prelude.
        if call.span.end > self.site.span.start || call.optional || call.arguments.len() != 1 {
            return false;
        }
        let args = call_arguments(call);
        args.len() == 1
            && matches!(args[0], Expression::NullLiteral(_))
            && self.is_path(&call.callee, &["Object", "create"])
    }

    fn is_slot(&self, e: &Expression<'_>, slot: u32) -> bool {
        self.value(e, 0) == Some(Value::Slot(slot))
    }

    fn is_path(&self, e: &Expression<'_>, path: &[&str]) -> bool {
        self.value(e, 0) == Some(Value::Global(path.iter().map(|s| s.to_string()).collect()))
    }
}

// Every variable alias must already hold its origin at the point it is copied.
// Function declarations are hoisted; the existing stable resolver still enforces
// plain origins, writes, redeclarations and loop exclusions.
fn ordered_callee(model: &Model<'_, '_>, e: &Expression<'_>, depth: usize) -> Option<FunctionSite> {
    if depth > 16 {
        return None;
    }
    let target = stable_function(model, e, 0)?;
    if let Expression::Identifier(i) = e {
        let s = crate::analysis::identifier_symbol(i, model.scoping)?;
        if matches!(
            model.nodes.kind(model.scoping.symbol_declaration(s)),
            AstKind::VariableDeclarator(_)
        ) {
            let value = model.symbol_value(s)?;
            if value.span().end > i.span.start {
                return None;
            }
            // Reuse the strict statement-list proof for aliases; a setup
            // callback or loop assignment is not an initialized callable.
            crate::naming::resolve::ordered_callable(model, e, 0)?;
            ordered_callee(model, value, depth + 1)?;
        }
    }
    Some(target)
}

/// Prove a complete owned body, not a set of operations found in its subtree.
/// The recursion bound limits direct-call composition; each callee is re-proved.
fn prove(model: &Model<'_, '_>, site: FunctionSite, depth: usize) -> Vec<&'static str> {
    if depth > 2 {
        return vec![];
    }
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return vec![];
    };
    let Some(body) = &function.body else {
        return vec![];
    };
    if function.r#async
        || function.generator
        || !body.directives.is_empty()
        || function.params.rest.is_some()
        || function.params.items.len() > 8
        || body.statements.len() > 17
        || !function.params.items.iter().all(|p| {
            p.initializer.is_none()
                && matches!(&p.pattern, BindingPattern::BindingIdentifier(id)
                if id.symbol_id.get().is_some_and(|s| model.binding_writes(s) == 0
                    && model.scoping.symbol_redeclarations(s).is_empty()))
        })
    {
        return vec![];
    }
    let Some((last, prelude)) = body.statements.split_last() else {
        return vec![];
    };
    let mut n = Normalizer {
        model,
        site,
        locals: BTreeMap::new(),
    };
    for statement in prelude {
        let Statement::VariableDeclaration(d) = statement else {
            return vec![];
        };
        if !matches!(
            d.kind,
            VariableDeclarationKind::Var
                | VariableDeclarationKind::Let
                | VariableDeclarationKind::Const
        ) {
            return vec![];
        }
        for v in &d.declarations {
            if n.locals.len() >= 16 {
                return vec![];
            }
            let Some(s) = plain_local(model, v) else {
                return vec![];
            };
            if model.binding_writes(s) != 0 {
                return vec![];
            }
            let Some(init) = &v.init else { return vec![] };
            // Copying a binding or primitive literal has no getter/call effects.
            // Member reads, calls, assignments and destructuring are refused.
            if !matches!(
                init,
                Expression::Identifier(_)
                    | Expression::BooleanLiteral(_)
                    | Expression::StringLiteral(_)
            ) {
                return vec![];
            }
            // Unresolved identifiers could throw; they are not pure alias copies.
            if matches!(init, Expression::Identifier(i) if crate::analysis::identifier_symbol(i, model.scoping).is_none())
            {
                return vec![];
            }
            let Some(value) = n.value(init, 0) else {
                return vec![];
            };
            if n.locals.insert(s, value).is_some() {
                return vec![];
            }
        }
    }
    let (expression, returns) = match last {
        Statement::ReturnStatement(r) => match &r.argument {
            Some(e) => (e, true),
            None => return vec![],
        },
        Statement::ExpressionStatement(s) => (&s.expression, false),
        _ => return vec![],
    };
    let mut claims = Vec::new();
    if returns {
        match n.value(expression, 0) {
            Some(Value::Slot(_)) => claims.push("identity"),
            Some(Value::Bool(true)) => claims.push("returnTrue"),
            Some(Value::Bool(false)) => claims.push("returnFalse"),
            Some(Value::EmptyString) => claims.push("returnEmptyString"),
            _ => (),
        }
    }
    // Exact identity with a single captured null-prototype sentinel; these
    // roles preserve the captured instance, not merely its object shape.
    if returns && function.params.items.is_empty() && n.null_prototype_sentinel(expression) {
        claims.push("getNullPrototypeSentinel");
    }
    if returns && function.params.items.len() == 1 {
        if let Expression::BinaryExpression(binary) = expression {
            if binary.operator.as_str() == "==="
                && n.is_slot(&binary.left, 0)
                && n.null_prototype_sentinel(&binary.right)
            {
                claims.push("isNullPrototypeSentinel");
            }
        }
    }
    let Expression::CallExpression(call) = expression else {
        return claims;
    };
    if call.optional || call.arguments.iter().any(|a| a.as_expression().is_none()) {
        return claims;
    }
    let args = call_arguments(call);
    let arity = function.params.items.len();
    if returns && arity == 2 && args.len() == 2 && n.is_slot(args[0], 0) && n.is_slot(args[1], 1) {
        if matches!(&call.callee, Expression::StaticMemberExpression(m)
            if !m.optional && m.property.name == "call"
                && n.is_path(&m.object, &["Object", "prototype", "hasOwnProperty"]))
        {
            claims.push("hasOwnProperty");
        }
        if n.is_path(&call.callee, &["Object", "setPrototypeOf"]) {
            claims.push("setObjectPrototype");
        }
    }
    if !returns && arity == 1 {
        if let Expression::StaticMemberExpression(m) = &call.callee {
            if !m.optional
                && m.property.name == "error"
                && args.len() == 1
                && n.is_slot(args[0], 0)
                && matches!(n.value(&m.object, 0), Some(Value::Capture(_)))
            {
                claims.push("forwardError");
            }
            if !m.optional
                && m.property.name == "preventDefault"
                && args.is_empty()
                && n.is_slot(&m.object, 0)
            {
                claims.push("preventDefaultHandler");
            }
        }
    }
    // Identical formal slots, exact arity, direct identifier call and an
    // independently proved callee body. No method receiver can be discarded.
    if matches!(&call.callee, Expression::Identifier(_))
        && args.len() == arity
        && args.iter().enumerate().all(|(i, a)| n.is_slot(a, i as u32))
    {
        if let Some(target) = ordered_callee(model, &call.callee, 0) {
            if target.span != site.span {
                if let AstKind::Function(callee) = model.nodes.kind(target.node_id) {
                    if callee.params.items.len() == arity {
                        for name in prove(model, target, depth + 1) {
                            // These roles encode return behavior; require it in
                            // the caller as well. Discarded results never inherit.
                            let returns_value =
                                !matches!(name, "forwardError" | "preventDefaultHandler");
                            if returns == returns_value {
                                claims.push(name);
                            }
                        }
                    }
                }
            }
        }
    }
    claims.sort_unstable();
    claims.dedup();
    claims
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    prove(model, site, 0).into_iter().map(|name| proposal(name,
        "shared_immutable_wrapper",
        "complete bounded owned-body proof: immutable plain formal slots and ordered pure alias prelude, exact terminal operation/arguments/return; direct callees are independently re-proved through stable scoped origins",
        f)).collect()
}
