//! Source declarations, safe later writes and recycled inputs.

use super::*;

#[derive(Default)]
pub(super) struct Declarations {
    pub(super) items: BTreeMap<SymbolId, SourceDeclaration>,
}
#[derive(Clone, Copy)]
pub(super) struct SourceDeclaration {
    pub(super) span: Span,
    pub(super) kind: VariableDeclarationKind,
    pub(super) undefined: bool,
}
impl Declarations {
    pub(super) fn body(&mut self, statements: &[Statement<'_>]) {
        for statement in statements {
            if let Statement::VariableDeclaration(d) = statement {
                if d.kind != VariableDeclarationKind::Let
                    && !(d.kind == VariableDeclarationKind::Const && d.declarations.len() == 1)
                {
                    continue;
                }
                for item in &d.declarations {
                    if let BindingPattern::BindingIdentifier(id) = &item.id {
                        if let Some(symbol) = id.symbol_id.get() {
                            self.items.insert(
                                symbol,
                                SourceDeclaration {
                                    span: d.span,
                                    kind: d.kind,
                                    undefined: d.kind == VariableDeclarationKind::Let
                                        && item.init.is_none(),
                                },
                            );
                        }
                    }
                }
            }
        }
    }
}

pub(super) fn safe_later_write(kind: AstKind<'_>) -> bool {
    match kind {
        AstKind::UpdateExpression(_) => true,
        AstKind::AssignmentExpression(a) => {
            if !matches!(a.left, AssignmentTarget::AssignmentTargetIdentifier(_)) {
                return false;
            }
            let mut value = &a.right;
            while let Expression::ParenthesizedExpression(p) = value {
                value = &p.expression;
            }
            // Assignment also infers function/class names from its target.
            !matches!(
                value,
                Expression::FunctionExpression(_)
                    | Expression::ArrowFunctionExpression(_)
                    | Expression::ClassExpression(_)
            )
        }
        _ => false,
    }
}

pub(super) struct Inputs<'s> {
    pub(super) scoping: &'s Scoping,
    pub(super) symbols: BTreeSet<SymbolId>,
}
impl<'a> Visit<'a> for Inputs<'_> {
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        if id
            .reference_id
            .get()
            .is_some_and(|r| self.scoping.get_reference(r).is_read())
        {
            if let Some(s) = analysis::identifier_symbol(id, self.scoping) {
                self.symbols.insert(s);
            }
        }
    }
}
