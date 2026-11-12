//! Property keys and data descriptors a rewrite can state exactly.

use super::*;

#[derive(Debug, Clone)]
pub(super) struct KeyInfo {
    /// ECMAScript property key after `ToPropertyKey`.
    pub(super) canonical: String,
    /// `Some(index)` when the key is an array-index property key.
    pub(super) array_index: Option<u32>,
    pub(super) span: Span,
}

#[derive(Debug, Clone)]
pub(super) struct DescriptorInfo {
    pub(super) value_span: Span,
    pub(super) value_needs_parentheses: bool,
}

pub(super) fn static_property_key(expression: &Expression<'_>) -> Option<KeyInfo> {
    let canonical = match expression {
        Expression::StringLiteral(literal) if !literal.lone_surrogates => literal.value.to_string(),
        Expression::NumericLiteral(literal) => number_to_js_string(literal.value),
        Expression::UnaryExpression(unary) => match unary.operator {
            UnaryOperator::UnaryNegation | UnaryOperator::UnaryPlus => {
                let Expression::NumericLiteral(literal) = unwrap_parens(&unary.argument) else {
                    return None;
                };
                let value = if unary.operator == UnaryOperator::UnaryNegation {
                    -literal.value
                } else {
                    literal.value
                };
                number_to_js_string(value)
            }
            _ => return None,
        },
        _ => return None,
    };
    Some(KeyInfo {
        array_index: canonical_array_index(&canonical),
        canonical,
        span: expression.span(),
    })
}

pub(super) fn canonical_array_index(key: &str) -> Option<u32> {
    if key == "0" {
        return Some(0);
    }
    if key.is_empty() || key.starts_with('0') || !key.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let value = key.parse::<u64>().ok()?;
    if value < u32::MAX as u64 {
        Some(value as u32)
    } else {
        None
    }
}

pub(super) fn exact_data_descriptor(expression: &Expression<'_>) -> Result<DescriptorInfo, String> {
    let Expression::ObjectExpression(object) = expression else {
        return Err("descriptor argument is not an object literal".into());
    };
    let mut seen = BTreeSet::new();
    let mut value_span = None;
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return Err("descriptor contains a spread property".into());
        };
        if property.method {
            return Err("descriptor contains a method definition".into());
        }
        if property.kind != PropertyKind::Init {
            return Err("descriptor contains an accessor".into());
        }
        if property.computed {
            return Err("descriptor field is computed".into());
        }
        let Some(name) = object_literal_property_key(&property.key, false) else {
            return Err("descriptor field key is not a static data field".into());
        };
        if !seen.insert(name.clone()) {
            return Err(format!("descriptor repeats field `{name}`"));
        }
        match name.as_str() {
            "writable" | "configurable" | "enumerable" => {
                if !is_true_literal(&property.value) {
                    return Err(format!("descriptor field `{name}` is not the literal true"));
                }
            }
            "value" => {
                if value_span.is_some() {
                    return Err("descriptor repeats field `value`".into());
                }
                value_span = Some((
                    property.value.span(),
                    matches!(&property.value, Expression::SequenceExpression(_)),
                ));
            }
            _ => {
                return Err(format!("descriptor has unexpected field `{name}`"));
            }
        }
    }
    if seen.len() != 4
        || !seen.contains("writable")
        || !seen.contains("configurable")
        || !seen.contains("enumerable")
    {
        return Err("descriptor is not exactly writable/configurable/enumerable/value".into());
    }
    let Some((value_span, value_needs_parentheses)) = value_span else {
        return Err("descriptor has no value field".into());
    };
    Ok(DescriptorInfo {
        value_span,
        value_needs_parentheses,
    })
}

pub(super) fn is_true_literal(expression: &Expression<'_>) -> bool {
    matches!(unwrap_parens(expression), Expression::BooleanLiteral(literal) if literal.value)
}

pub(super) fn static_string_expression(expression: &Expression<'_>) -> Option<String> {
    match unwrap_parens(expression) {
        Expression::StringLiteral(literal) if !literal.lone_surrogates => {
            Some(literal.value.to_string())
        }
        Expression::TemplateLiteral(template)
            if template.expressions.is_empty() && template.quasis.len() == 1 =>
        {
            template
                .quasis
                .first()?
                .value
                .cooked
                .map(|value| value.to_string())
        }
        Expression::BinaryExpression(binary) if binary.operator == BinaryOperator::Addition => {
            Some(format!(
                "{}{}",
                static_string_expression(&binary.left)?,
                static_string_expression(&binary.right)?
            ))
        }
        _ => None,
    }
}

pub(super) fn computed_object_is_function_like(expression: &Expression<'_>) -> bool {
    matches!(
        unwrap_parens(expression),
        Expression::FunctionExpression(_)
            | Expression::ArrowFunctionExpression(_)
            | Expression::ClassExpression(_)
    )
}
