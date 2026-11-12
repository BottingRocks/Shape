//! Substitute exact primitive values at proven private reads, not by assuming
//! that a generated binding remains immutable for its whole lifetime.
use super::super::{
    adjacent_ssa::LocalReads,
    cell_facts::{self, Facts, Token},
};
use crate::analysis;
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_ast_visit::Visit;
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::GetSpan;
use std::collections::BTreeSet;

pub(crate) fn inline_point_literals(
    source: &str,
    protected: &BTreeSet<String>,
    checked: bool,
) -> Result<(String, usize), String> {
    inline(source, protected, checked, false)
}

pub(crate) fn inline_property_keys(
    source: &str,
    protected: &BTreeSet<String>,
    checked: bool,
) -> Result<(String, usize), String> {
    inline(source, protected, checked, true)
}

fn inline(
    source: &str,
    protected: &BTreeSet<String>,
    checked: bool,
    keys_only: bool,
) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    if nodes.iter().any(|n|matches!(n.kind(),AstKind::WithStatement(_))
        ||matches!(n.kind(),AstKind::IdentifierReference(id)if id.name=="eval")
        ||!checked&&matches!(n.kind(),AstKind::Function(f)if f.id.as_ref().is_some_and(|id|id.name.starts_with("$THREAD_"))&&f.body.as_ref().is_some_and(|b|b.statements.is_empty()&&b.directives.is_empty()))) {
        return Ok((source.into(),0));
    }
    let mut locals = LocalReads::default();
    locals.visit_program(&program);
    locals.build(&scoping, &nodes, protected);
    // No argument/prototype assumptions are needed for literal tokens.
    let facts = cell_facts::collect(
        source, &program, &scoping, &nodes, &locals, protected, false,
    );
    let mut edits = vec![];
    for node in nodes.iter() {
        let AstKind::IdentifierReference(id) = node.kind() else {
            continue;
        };
        if !(id.name.starts_with("$SSA_") || id.name.starts_with("$CARRY_"))
            || protected.contains(id.name.as_str())
        {
            continue;
        }
        let Some(reference) = id.reference_id.get().map(|r| scoping.get_reference(r)) else {
            continue;
        };
        let Some(symbol) = analysis::identifier_symbol(id, &scoping) else {
            continue;
        };
        if !reference.is_read()
            || reference.is_write()
            || !locals.private_cell(symbol)
            || !locals.initialized(symbol, id.span)
        {
            continue;
        }
        let mut parent = node.id();
        while matches!(
            nodes.parent_kind(parent),
            AstKind::ParenthesizedExpression(_) | AstKind::ChainExpression(_)
        ) {
            parent = nodes.parent_id(parent);
        }
        let span = nodes.kind(parent).span();
        if keys_only && !property_key(nodes.parent_kind(parent), span) {
            continue;
        }
        // Delete observes a Reference. Keep standalone strings from becoming
        // directives, and preserve the spelling of non-callable callee errors.
        if matches!(nodes.parent_kind(parent),AstKind::UnaryExpression(u)if u.operator==UnaryOperator::Delete)
            || matches!(nodes.parent_kind(parent), AstKind::ExpressionStatement(_))
            || matches!(nodes.parent_kind(parent),AstKind::CallExpression(c)if c.callee.span()==span)
            || matches!(nodes.parent_kind(parent),AstKind::NewExpression(c)if c.callee.span()==span)
            || matches!(nodes.parent_kind(parent),AstKind::TaggedTemplateExpression(t)if t.tag.span()==span)
        {
            continue;
        }
        let Some(token) = facts.values.get(&id.span).and_then(|v| v.token.as_ref()) else {
            continue;
        };
        let Some(value) = literal(token, &facts, &scoping, &nodes, &mut BTreeSet::new()) else {
            continue;
        };
        let text = if matches!(nodes.parent_kind(node.id()),AstKind::ObjectProperty(p)if p.shorthand)
        {
            format!("{}: ({value})", id.name)
        } else {
            format!("({value})")
        };
        edits.push(analysis::Edit::replace(id.span, text));
    }
    let count = edits.len();
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err("point literal edits overlap".into());
    }
    Ok((applied.output, count))
}

fn property_key(parent: AstKind<'_>, span: oxc_span::Span) -> bool {
    match parent {
        AstKind::ComputedMemberExpression(m) => m.expression.span() == span,
        AstKind::ObjectProperty(p) => p.computed && p.key.span() == span,
        AstKind::CallExpression(c) if c.arguments.get(1).is_some_and(|a| a.span() == span) => {
            match &c.callee {
                Expression::StaticMemberExpression(m) => m.property.name == "defineProperty",
                Expression::ComputedMemberExpression(m) => {
                    matches!(&m.expression,Expression::StringLiteral(k)if k.value=="defineProperty")
                }
                _ => false,
            }
        }
        _ => false,
    }
}

fn literal<'s>(
    mut token: &'s Token,
    facts: &'s Facts,
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    seen: &mut BTreeSet<SymbolId>,
) -> Option<String> {
    loop {
        match token {
            Token::Literal(value) => return Some(value.clone()),
            Token::Undefined => return Some("void 0".into()),
            Token::Immutable(s) => {
                if !seen.insert(*s) {
                    return None;
                }
                let AstKind::VariableDeclarator(d) = nodes.kind(scoping.symbol_declaration(*s))
                else {
                    return None;
                };
                token = facts.values.get(&d.init.as_ref()?.span())?.token.as_ref()?;
            }
            Token::Argument(_, _) => return None,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn point_values_follow_stores_in_ten_read_contexts() {
        for read in [
            "use($SSA_value)",
            "use($SSA_value+other())",
            "use(typeof $SSA_value)",
            "use($SSA_value?yes():no())",
            "use($SSA_value&&other())",
            "use(`${$SSA_value}`)",
            "use({$SSA_value})",
            "use(object[$SSA_value])",
            "use(new Wrapper($SSA_value))",
            "use($SSA_value===other())",
        ] {
            let source = format!(
                "function f(){{let $SSA_value='first';{read};$SSA_value='second';{read};}}"
            );
            let (code, count) =
                super::inline_point_literals(&source, &Default::default(), false).unwrap();
            assert_eq!(count, 2, "{code}");
            assert!(
                code.contains("('first')") && code.contains("('second')"),
                "{code}"
            );
        }
    }
    #[test]
    fn references_errors_captures_and_unseen_children_keep_their_reads() {
        for source in [
            "function f(){let $SSA_value='x';return delete ($SSA_value);}",
            "function f(){let $SSA_value='x';return $SSA_value();}",
            "function f(){let $SSA_value='x';return new $SSA_value();}",
            "function f(){let $SSA_value='x';return $SSA_value``;}",
            "function f(){let $SSA_value='x';return ()=>$SSA_value;}",
            "function f(){let $SSA_value='x';function change(){$SSA_value='y';}change();return $SSA_value;}",
            "function f(){return $SSA_value;let $SSA_value='x';}",
            "function f(){let $SSA_value='x';function $THREAD_0_1(){}return $SSA_value;}",
            "function f(){let $SSA_value='x';eval('anything');return $SSA_value;}",
            "function f(){var $KEY_value=1;{function $KEY_value(){}}let $SSA_value=$KEY_value;return $SSA_value;}",
        ]{
            let(code,count)=super::inline_point_literals(source,&Default::default(),false).unwrap();
            assert_eq!(count,0,"{code}");
        }
    }
    #[test]
    fn undefined_lexicals_are_not_iteration_values_or_zero_sign_proofs() {
        let source = "function f(){let $SSA_value;return [$SSA_value,$SSA_value];}";
        assert_eq!(
            super::inline_point_literals(source, &Default::default(), false)
                .unwrap()
                .1,
            2
        );
        for source in [
            "function f(){for(let $SSA_value of values){use($SSA_value);}}",
            "function f(){for(let $SSA_value in object){use($SSA_value);}}",
            "function f(){let $SSA_value=read();if($SSA_value===0)return 1/$SSA_value;}",
            "function f(){let $SSA_value=read();if($SSA_value!==0){}else return 1/$SSA_value;}",
        ] {
            let (code, count) =
                super::inline_point_literals(source, &Default::default(), false).unwrap();
            assert_eq!(count, 0, "{code}");
        }
        for source in [
            "function f(input){let $SSA_value=input;if($SSA_value==='yes')return $SSA_value;}",
            "function f(input){let $SSA_value=input;if($SSA_value!==7){}else return $SSA_value;}",
        ] {
            let (code, count) =
                super::inline_point_literals(source, &Default::default(), false).unwrap();
            assert_eq!(count, 1, "{code}");
        }
    }
}
