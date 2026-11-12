//! Fold literal-only primitive computations before propagating their values.
//! Oxc supplies ECMAScript arithmetic; our stricter purity check supplies the
//! no-getter/no-throw proof rather than the optimizer's broader assumptions.
use crate::analysis;
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstBuilder};
use oxc_ast_visit::{walk, Visit};
use oxc_ecmascript::{
    constant_evaluation::{ConstantEvaluation, ConstantEvaluationCtx, ConstantValue},
    side_effects::{MayHaveSideEffectsContext, PropertyReadSideEffects},
    GlobalContext,
};
use oxc_span::GetSpan;
use oxc_syntax::number::ToJsString;

struct Context<'a>(AstBuilder<'a>);
impl<'a> GlobalContext<'a> for Context<'a> {
    fn is_global_reference(&self, _: &IdentifierReference<'a>) -> bool {
        false
    }
}
impl<'a> MayHaveSideEffectsContext<'a> for Context<'a> {
    fn annotations(&self) -> bool {
        false
    }
    fn manual_pure_functions(&self, _: &Expression<'_>) -> bool {
        false
    }
    fn property_read_side_effects(&self) -> PropertyReadSideEffects {
        PropertyReadSideEffects::All
    }
    fn unknown_global_side_effects(&self) -> bool {
        true
    }
}
impl<'a> ConstantEvaluationCtx<'a> for Context<'a> {
    fn ast(&self) -> AstBuilder<'a> {
        self.0
    }
}
pub(crate) fn fold_constant_values(source: &str) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let mut scan = Scan {
        source,
        context: Context(AstBuilder::new(&allocator)),
        edits: vec![],
    };
    scan.visit_program(&program);
    let count = scan.edits.len();
    let result = analysis::apply_edits(source, scan.edits);
    if !result.rejected.is_empty() {
        return Err("constant value edits overlap".into());
    }
    Ok((result.output, count))
}
struct Scan<'s, 'a> {
    source: &'s str,
    context: Context<'a>,
    edits: Vec<analysis::Edit>,
}
#[derive(Default)]
struct OpaqueLiterals(bool);
impl<'a> Visit<'a> for OpaqueLiterals {
    fn visit_big_int_literal(&mut self, _: &BigIntLiteral<'a>) {
        self.0 = true;
    }
    fn visit_string_literal(&mut self, s: &StringLiteral<'a>) {
        self.0 |= s.lone_surrogates;
    }
    fn visit_template_literal(&mut self, _: &TemplateLiteral<'a>) {
        self.0 = true;
    }
}
impl<'a> Visit<'a> for Scan<'_, 'a> {
    fn visit_expression(&mut self, e: &Expression<'a>) {
        if matches!(
            e,
            Expression::UnaryExpression(_)
                | Expression::BinaryExpression(_)
                | Expression::LogicalExpression(_)
                | Expression::ConditionalExpression(_)
        ) && super::super::purity::expression_is_pure(e)
        {
            let mut big = OpaqueLiterals::default();
            big.visit_expression(e);
            if !big.0 {
                let text = match e.evaluate_value(&self.context) {
                    Some(ConstantValue::Number(n)) => Some(if n.is_nan() {
                        "(0/0)".into()
                    } else if n == f64::INFINITY {
                        "(1/0)".into()
                    } else if n == f64::NEG_INFINITY {
                        "(-1/0)".into()
                    } else if n == 0.0 && n.is_sign_negative() {
                        "(-0)".into()
                    } else {
                        format!("({})", n.to_js_string())
                    }),
                    Some(ConstantValue::Boolean(b)) => Some(format!("({b})")),
                    Some(ConstantValue::Null) => Some("(null)".into()),
                    Some(ConstantValue::Undefined) => Some("(void 0)".into()),
                    // JS strings can contain lone UTF-16 surrogates, unlike
                    // Rust strings. Do not normalize string-valued results.
                    _ => None,
                };
                if let Some(text) = text {
                    let original = &self.source[e.span().start as usize..e.span().end as usize];
                    let compact =
                        |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
                    if compact(&text[1..text.len() - 1]) != compact(original)
                        && !original.contains("/*")
                        && !original.contains("//")
                    {
                        self.edits.push(analysis::Edit::replace(e.span(), text));
                        return;
                    }
                }
            }
        }
        walk::walk_expression(self, e);
    }
}
