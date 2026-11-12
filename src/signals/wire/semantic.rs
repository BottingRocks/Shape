//! Source-derived, reversible meanings of byte tags. Numeric bytes stay numeric
//! unless a boolean field card or an explicit nullable source branch proves a meaning.
use super::{
    named::{Field, Naming, Node, Step},
    WireError,
};
use crate::signals::{
    extract::contracts::{GrammarNode, SignalContract},
    schema::SourceSpan,
};
use oxc_ast::ast::Expression;
use oxc_span::{GetSpan, SourceType};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LogicalValue {
    Boolean(bool),
    Null,
}
impl LogicalValue {
    fn json(&self) -> Value {
        match self {
            Self::Boolean(b) => json!(b),
            Self::Null => Value::Null,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ByteCase {
    pub byte: u8,
    pub value: LogicalValue,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", try_from = "ByteMappingEvidence")]
pub struct ByteMapping {
    pub cases: Vec<ByteCase>,
    pub condition: String,
    pub source: SourceSpan,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ByteMappingEvidence {
    cases: Vec<ByteCase>,
    condition: String,
    source: SourceSpan,
}
impl TryFrom<ByteMappingEvidence> for ByteMapping {
    type Error = WireError;
    fn try_from(value: ByteMappingEvidence) -> Result<Self, Self::Error> {
        let mapping = Self {
            cases: value.cases,
            condition: value.condition,
            source: value.source,
        };
        mapping.validate()?;
        Ok(mapping)
    }
}
impl ByteMapping {
    pub fn validate(&self) -> Result<(), WireError> {
        if self.cases.is_empty()
            || self.condition.trim().is_empty()
            || self.source.start > self.source.end
            || self.cases.iter().enumerate().any(|(i, c)| {
                self.cases[..i]
                    .iter()
                    .any(|p| p.byte == c.byte || p.value == c.value)
            })
        {
            return Err(WireError::Evidence);
        }
        Ok(())
    }
    /// Decode a logical, already-unmasked byte. Masks/XOR remain codec concerns.
    pub fn decode(&self, byte: u8) -> Result<Value, WireError> {
        self.validate()?;
        self.cases
            .iter()
            .find(|c| c.byte == byte)
            .map(|c| c.value.json())
            .ok_or(WireError::Discriminant)
    }
    /// Recover the seed-specific concrete byte for the existing signal writer.
    pub fn encode(&self, value: &Value) -> Result<u8, WireError> {
        self.validate()?;
        self.cases
            .iter()
            .find(|c| c.value.json() == *value)
            .map(|c| c.byte)
            .ok_or(WireError::Type)
    }
}
fn number(e: &Expression<'_>) -> Option<u8> {
    match e.without_parentheses() {
        Expression::NumericLiteral(n)
            if n.value.fract() == 0. && (0. ..=255.).contains(&n.value) =>
        {
            Some(n.value as u8)
        }
        _ => None,
    }
}
fn polarity(e: &Expression<'_>) -> bool {
    match e.without_parentheses() {
        Expression::UnaryExpression(u) if u.operator.as_str() == "!" => !polarity(&u.argument),
        Expression::BinaryExpression(b) => {
            let (value, flag) = match (b.left.without_parentheses(), b.right.without_parentheses())
            {
                (Expression::BooleanLiteral(v), other) => (other, v.value),
                (other, Expression::BooleanLiteral(v)) => (other, v.value),
                _ => return true,
            };
            match b.operator.as_str() {
                "==" | "===" => polarity(value) == flag,
                "!=" | "!==" => polarity(value) != flag,
                _ => true,
            }
        }
        _ => true,
    }
}
fn boolean_mapping(g: &GrammarNode) -> Option<ByteMapping> {
    let GrammarNode::Byte(w) = g else { return None };
    let arena = oxc_allocator::Allocator::default();
    let e = oxc_parser::Parser::new(&arena, &w.value, SourceType::mjs())
        .parse_expression()
        .ok()?;
    let Expression::ConditionalExpression(c) = e.without_parentheses() else {
        return None;
    };
    let a = number(&c.consequent)?;
    let b = number(&c.alternate)?;
    if a == b {
        return None;
    }
    let positive = polarity(&c.test);
    Some(ByteMapping {
        cases: vec![
            ByteCase {
                byte: a,
                value: LogicalValue::Boolean(positive),
            },
            ByteCase {
                byte: b,
                value: LogicalValue::Boolean(!positive),
            },
        ],
        condition: w
            .value
            .get(c.test.span().start as usize..c.test.span().end as usize)?
            .into(),
        source: w.value_span.into(),
    })
}
// Which source arm denotes a null/missing value? No numeric sentinel is assumed.
fn null_arm(condition: &str) -> Option<usize> {
    let arena = oxc_allocator::Allocator::default();
    let e = oxc_parser::Parser::new(&arena, condition, SourceType::mjs())
        .parse_expression()
        .ok()?;
    fn missing(e: &Expression<'_>) -> bool {
        match e.without_parentheses() {
            Expression::NullLiteral(_) => true,
            // A local binding can shadow `undefined`; only literal null / void 0
            // are established from this source expression alone.
            Expression::UnaryExpression(u) => {
                u.operator.as_str() == "void"
                    && matches!(u.argument.without_parentheses(),Expression::NumericLiteral(n) if n.value==0.)
            }
            _ => false,
        }
    }
    fn arm(e: &Expression<'_>) -> Option<usize> {
        match e.without_parentheses() {
            Expression::UnaryExpression(u) if u.operator.as_str() == "!" => {
                arm(&u.argument).map(|i| 1 - i)
            }
            Expression::BinaryExpression(b) if missing(&b.left) || missing(&b.right) => {
                match b.operator.as_str() {
                    "==" | "===" => Some(0),
                    "!=" | "!==" => Some(1),
                    _ => None,
                }
            }
            _ => None,
        }
    }
    arm(&e)
}
fn at<'a>(root: &'a GrammarNode, path: &[Step]) -> Option<&'a GrammarNode> {
    let mut g = root;
    for step in path {
        g = match (step, g) {
            (Step::Child(i), GrammarNode::Sequence { items }) => items.get(*i)?,
            (Step::Arm(i), GrammarNode::Choice { arms, .. }) => &arms.get(*i)?.body,
            (Step::Body, GrammarNode::Repeat { body, .. }) => body,
            _ => return None,
        };
    }
    Some(g)
}
fn literal_byte(g: &GrammarNode) -> Option<u8> {
    match g {
        GrammarNode::Byte(w) => {
            let arena = oxc_allocator::Allocator::default();
            let e = oxc_parser::Parser::new(&arena, &w.value, SourceType::mjs())
                .parse_expression()
                .ok()?;
            number(&e)
        }
        GrammarNode::Sequence { items } if items.len() == 1 => literal_byte(&items[0]),
        _ => None,
    }
}
fn boolean_branch_mapping(root: &GrammarNode, path: &[Step]) -> Option<ByteMapping> {
    let byte = literal_byte(at(root, path)?)?;
    for i in (0..path.len()).rev() {
        let Step::Arm(selected) = path[i] else {
            continue;
        };
        let GrammarNode::Choice {
            condition: Some(condition),
            arms,
            span,
            ..
        } = at(root, &path[..i])?
        else {
            continue;
        };
        if arms.len() != 2 {
            continue;
        }
        let (Some(a), Some(b)) = (literal_byte(&arms[0].body), literal_byte(&arms[1].body)) else {
            continue;
        };
        if a == b {
            continue;
        }
        let arena = oxc_allocator::Allocator::default();
        let test = oxc_parser::Parser::new(&arena, condition, SourceType::mjs())
            .parse_expression()
            .ok()?;
        let value = if selected == 0 {
            polarity(&test)
        } else {
            !polarity(&test)
        };
        return Some(ByteMapping {
            cases: vec![ByteCase {
                byte,
                value: LogicalValue::Boolean(value),
            }],
            condition: condition.clone(),
            source: (*span).into(),
        });
    }
    None
}
fn nullable_mapping(root: &GrammarNode, path: &[Step]) -> Option<ByteMapping> {
    let GrammarNode::Byte(w) = at(root, path)? else {
        return None;
    };
    let arena = oxc_allocator::Allocator::default();
    let e = oxc_parser::Parser::new(&arena, &w.value, SourceType::mjs())
        .parse_expression()
        .ok()?;
    let byte = number(&e)?;
    // A nullable value has exactly one byte-marker arm and one value arm.
    for i in (0..path.len()).rev() {
        let Step::Arm(selected) = path[i] else {
            continue;
        };
        let GrammarNode::Choice {
            condition: Some(condition),
            arms,
            span,
            ..
        } = at(root, &path[..i])?
        else {
            continue;
        };
        if arms.len() != 2 || null_arm(condition) != Some(selected) {
            continue;
        }
        fn value(g: &GrammarNode) -> bool {
            match g {
                GrammarNode::Number(_) | GrammarNode::String(_) => true,
                GrammarNode::Sequence { items } if items.len() == 1 => value(&items[0]),
                _ => false,
            }
        }
        // Never apply an ancestor's null guard to a byte inside a record or
        // another conditional: the whole missing arm must be this marker.
        if literal_byte(&arms[selected].body) != Some(byte) || !value(&arms[1 - selected].body) {
            continue;
        }
        return Some(ByteMapping {
            cases: vec![ByteCase {
                byte,
                value: LogicalValue::Null,
            }],
            condition: condition.clone(),
            source: (*span).into(),
        });
    }
    None
}

pub(super) fn bind(naming: &mut Naming, contract: &SignalContract) {
    let card = crate::signals::catalog::card(&naming.canonical_name);
    use crate::signals::catalog::card::{CardBody, CardField};
    fn fields(list: &mut [Field], g: &GrammarNode, cards: &[CardField]) {
        for field in list {
            if let Node::FixedArray { items } = &mut field.node {
                fields(items, g, cards);
                continue;
            }
            node(
                &mut field.node,
                g,
                cards.iter().find(|c| c.name == field.name),
            );
        }
    }
    fn node(n: &mut Node, g: &GrammarNode, card: Option<&CardField>) {
        match n {
            Node::Leaf {
                grammar_path,
                byte_mapping,
                ..
            } => {
                *byte_mapping = if card.is_some_and(|c| c.boolean) {
                    at(g, grammar_path)
                        .and_then(boolean_mapping)
                        .or_else(|| boolean_branch_mapping(g, grammar_path))
                } else {
                    None
                }
                .or_else(|| nullable_mapping(g, grammar_path))
                .or_else(|| {
                    if !card.is_some_and(|c| c.null_marker) {
                        return None;
                    }
                    let GrammarNode::Byte(w) = at(g, grammar_path)? else {
                        return None;
                    };
                    let arena = oxc_allocator::Allocator::default();
                    let expression = oxc_parser::Parser::new(&arena, &w.value, SourceType::mjs())
                        .parse_expression()
                        .ok()?;
                    Some(ByteMapping {
                        cases: vec![ByteCase {
                            byte: number(&expression)?,
                            value: LogicalValue::Null,
                        }],
                        condition: "documented null marker".into(),
                        source: w.value_span.into(),
                    })
                });
            }
            Node::Record { fields: f } | Node::ScalarNull { fields: f } => {
                // A scalar's two-byte null alternative is represented as a
                // record. Only explicitly documented nested component fields
                // may supply evidence; never propagate the
                // parent's null meaning to arbitrary child bytes.
                fields(f, g, card.map(CardField::fields).unwrap_or(&[]));
                let certified = card.is_some_and(|c| {
                    !c.null_components.is_empty()
                        && f.len() == c.null_components.len()
                        && f.iter().zip(c.null_components).all(|(f, c)| {
                            f.name == c.name
                                && matches!(&f.node, Node::Leaf { byte_mapping: Some(m), .. }
                            if m.cases.iter().all(|c| matches!(c.value, LogicalValue::Null)))
                        })
                });
                let f = f.clone();
                *n = if certified {
                    Node::ScalarNull { fields: f }
                } else {
                    Node::Record { fields: f }
                };
            }
            Node::Array { item, .. } => node(item, g, card),
            Node::Choice { arms, .. } => {
                for n in arms {
                    node(n, g, card)
                }
            }
            _ => {}
        }
    }
    for part in &mut naming.parts {
        if let Some(e) = contract.emissions.get(part.emission) {
            let cards = match card.map(|c| &c.body) {
                Some(CardBody::Fields(fields)) => *fields,
                Some(CardBody::Messages(messages)) => messages
                    .get(part.message)
                    .map(|message| message.fields)
                    .unwrap_or(&[]),
                None => &[],
            };
            fields(&mut part.fields, &e.grammar, cards);
        }
    }
}

/// Source-free artifacts retain their source expressions. Recheck every saved
/// mapping against those expressions and the scoped catalog field. Older
/// artifacts without logical mappings keep their numeric projection.
pub(crate) fn validate(naming: &Naming, contract: &SignalContract) -> Result<(), WireError> {
    super::readable::validate(naming)?;
    super::named::validate_leaf_paths(naming, contract)?;
    let mut expected = naming.clone();
    bind(&mut expected, contract);
    fn nodes(saved: &Node, expected: &Node) -> Result<(), WireError> {
        match (saved, expected) {
            (
                Node::Leaf {
                    byte_mapping: Some(saved),
                    ..
                },
                Node::Leaf {
                    byte_mapping: expected,
                    ..
                },
            ) => {
                saved.validate()?;
                if Some(saved) != expected.as_ref() {
                    return Err(WireError::Evidence);
                }
            }
            (Node::ScalarNull { fields: saved }, Node::ScalarNull { fields: expected })
            | (Node::Record { fields: saved }, Node::ScalarNull { fields: expected })
            | (Node::FixedArray { items: saved }, Node::FixedArray { items: expected })
            | (Node::Record { fields: saved }, Node::Record { fields: expected }) => {
                for (s, e) in saved.iter().zip(expected) {
                    nodes(&s.node, &e.node)?;
                }
            }
            (Node::ScalarNull { .. }, _) => return Err(WireError::Evidence),
            (Node::Array { item: saved, .. }, Node::Array { item: expected, .. }) => {
                nodes(saved, expected)?
            }
            (Node::Choice { arms: saved, .. }, Node::Choice { arms: expected, .. }) => {
                for (s, e) in saved.iter().zip(expected) {
                    nodes(s, e)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    for (saved, expected) in naming.parts.iter().zip(&expected.parts) {
        for (s, e) in saved.fields.iter().zip(&expected.fields) {
            nodes(&s.node, &e.node)?;
        }
    }
    Ok(())
}

/// Add logical mappings to the descriptive schema too; wire nodes remain the codec input.
pub(crate) fn describe(signals: &mut serde_json::Map<String, Value>, wire: &super::WireSchema) {
    fn node(n: &Node, v: &mut Value, emission: usize) {
        match n {
            Node::Leaf {
                byte_mapping: Some(mapping),
                ..
            } => {
                v["logicalType"] = json!(if mapping
                    .cases
                    .iter()
                    .all(|c| matches!(c.value, LogicalValue::Null))
                {
                    "null"
                } else {
                    "boolean"
                });
                let entry = json!({"emission":emission,"mapping":mapping});
                if v.get("valueMappings").is_none() {
                    v["valueMappings"] = json!([]);
                }
                if let Some(a) = v["valueMappings"].as_array_mut() {
                    if !a.contains(&entry) {
                        a.push(entry);
                    }
                }
            }
            Node::Record { fields: f } => fields(f, v, emission),
            Node::ScalarNull { fields: f } => {
                fields(f, v, emission);
                v["logicalType"] = json!("null");
                v["description"] =
                    json!("Scalar null; all physical marker components are retained.");
            }
            Node::FixedArray { items } => fields(items, v, emission),
            Node::Array { item, .. } => {
                if let Some(v) = v.get_mut("item") {
                    node(item, v, emission)
                }
            }
            Node::Choice { arms, .. } => {
                let mut nullable = false;
                if let Some(values) = v.get_mut("arms").and_then(Value::as_array_mut) {
                    for (n, v) in arms.iter().zip(values) {
                        node(n, v, emission);
                        nullable |= v.get("logicalType").and_then(Value::as_str) == Some("null");
                    }
                }
                if nullable {
                    v["nullable"] = json!(true);
                }
                if arms.iter().any(optional) {
                    v["optional"] = json!(true);
                }
            }
            _ => {}
        }
    }
    fn fields(f: &[Field], v: &mut Value, emission: usize) {
        for f in f {
            if let Node::FixedArray { items } = &f.node {
                fields(items, v, emission);
                continue;
            }
            if let Some(field) = v.get_mut("fields").and_then(|v| v.get_mut(&f.name)) {
                node(&f.node, field, emission);
                if optional(&f.node) {
                    field["optional"] = json!(true);
                }
            }
        }
    }
    fn optional(n: &Node) -> bool {
        match n {
            Node::Empty => true,
            Node::Choice { arms, .. } => arms.iter().any(optional),
            _ => false,
        }
    }
    for slot in &wire.slots {
        if let Some(n) = &slot.naming {
            if let Some(v) = signals.get_mut(&n.schema_key) {
                for part in &n.parts {
                    if v.get("messages").is_some() {
                        if let Some(message) = v["messages"].get_mut(part.message) {
                            fields(&part.fields, message, part.emission)
                        }
                    } else {
                        fields(&part.fields, v, part.emission)
                    }
                }
                super::readable::describe(n, v);
            }
        }
    }
}
