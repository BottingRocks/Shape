use super::{
    declared_within_body, gc_assign_named, gc_collection_init, gc_declarator_symbol,
    gc_function_symbol, gc_init_pair, gc_restore_pair, gc_return_symbol, gc_symbol,
    gc_target_global, property_key_name, Facts, FunctionSite, Model,
};
use oxc_ast::{ast::*, AstKind};
use std::collections::{BTreeMap, BTreeSet};

/// Equivalent carrier layout, resolved by binding rather than declaration
/// positions. Only six locals (three immutable formal aliases and the three
/// exact saved/target slots) and two directly owned functions are admitted.
/// Every executable lifecycle operation still uses the existing child proofs.
pub(in crate::naming) fn matches(
    model: &Model<'_, '_>,
    site: FunctionSite,
    all: &BTreeMap<u32, Facts<'_>>,
) -> bool {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return false;
    };
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || function.params.items.len() != 4
    {
        return false;
    }
    for parameter in &function.params.items {
        let BindingPattern::BindingIdentifier(identifier) = &parameter.pattern else {
            return false;
        };
        let Some(symbol) = identifier.symbol_id.get() else {
            return false;
        };
        if parameter.initializer.is_some()
            || model.binding_writes(symbol) != 0
            || !model.scoping.symbol_redeclarations(symbol).is_empty()
        {
            return false;
        }
    }
    let Some(body) = function.body.as_deref() else {
        return false;
    };
    if !body.directives.is_empty() {
        return false;
    }
    let statements = body.statements.as_slice();
    let prefix_end = statements
        .iter()
        .take_while(|s| matches!(s, Statement::VariableDeclaration(_)))
        .count();
    let mut aliases = BTreeSet::new();
    let mut slots = BTreeSet::new();
    let mut locals = BTreeSet::new();
    for statement in &statements[..prefix_end] {
        let Statement::VariableDeclaration(declaration) = statement else {
            return false;
        };
        for declarator in &declaration.declarations {
            let Some(symbol) = gc_declarator_symbol(&declarator.id) else {
                return false;
            };
            if !declared_within_body(model, symbol, site)
                || !locals.insert(symbol)
                || !model.scoping.symbol_redeclarations(symbol).is_empty()
            {
                return false;
            }
            if let Some(init) = &declarator.init {
                let Some(slot @ 1..=3) = model.site_parameter_slot(init, site) else {
                    return false;
                };
                if model.binding_writes(symbol) != 0 || !aliases.insert(slot) {
                    return false;
                }
            } else {
                slots.insert(symbol);
            }
        }
    }
    if locals.len() != 6 || aliases.len() != 3 || slots.len() != 3 {
        return false;
    }
    let [first, second, global, tail @ ..] = &statements[prefix_end..] else {
        return false;
    };
    let Some((first_symbol, first_site)) = gc_function_symbol(model, first) else {
        return false;
    };
    let Some((second_symbol, second_site)) = gc_function_symbol(model, second) else {
        return false;
    };
    for (statement, symbol, child_site) in [
        (first, first_symbol, first_site),
        (second, second_symbol, second_site),
    ] {
        let Statement::FunctionDeclaration(child) = statement else {
            return false;
        };
        if child.r#async
            || child.generator
            || child.params.rest.is_some()
            || !child.params.items.is_empty()
            || child.span != child_site.span
            || model.binding_writes(symbol) != 0
            || !model.scoping.symbol_redeclarations(symbol).is_empty()
        {
            return false;
        }
    }
    let (init_symbol, init, teardown_symbol, teardown_site) = match (
        gc_collection_init(model, first_site, all),
        gc_collection_init(model, second_site, all),
    ) {
        (Some(init), None) => (first_symbol, init, second_symbol, second_site),
        (None, Some(init)) => (second_symbol, init, first_symbol, first_site),
        _ => return false,
    };
    let Some(target) = gc_target_global(model, global, site) else {
        return false;
    };
    let expected_slots = BTreeSet::from([target, init.saved_collector, init.saved_mapper]);
    if init.target != target
        || expected_slots.len() != 3
        || slots != expected_slots
        || !gc_restore_pair(
            model,
            teardown_site,
            target,
            init.key,
            init.saved_collector,
            init.saved_mapper,
        )
    {
        return false;
    }
    match tail {
        [pair_declaration, teardown_assignment, returned] => {
            let Some(pair) = gc_init_pair(model, pair_declaration, init_symbol) else {
                return false;
            };
            gc_assign_named(
                model,
                teardown_assignment,
                pair,
                "teardown",
                teardown_symbol,
            ) && gc_return_symbol(model, returned) == Some(pair)
        }
        [Statement::ReturnStatement(returned)] => {
            let Some(Expression::ObjectExpression(object)) =
                returned.argument.as_ref().map(|v| v.without_parentheses())
            else {
                return false;
            };
            let mut fields = BTreeSet::new();
            for property in &object.properties {
                let ObjectPropertyKind::ObjectProperty(property) = property else {
                    return false;
                };
                if property.computed || property.method || property.kind != PropertyKind::Init {
                    return false;
                }
                let Some(key) = property_key_name(&property.key) else {
                    return false;
                };
                let expected = match key.as_str() {
                    "init" => init_symbol,
                    "teardown" => teardown_symbol,
                    _ => return false,
                };
                if !fields.insert(key) || gc_symbol(model, &property.value) != Some(expected) {
                    return false;
                }
            }
            fields.len() == 2
        }
        _ => false,
    }
}
