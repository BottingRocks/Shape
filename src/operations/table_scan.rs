//! Shared operation-table scanning primitives.
//!
//! The catalogue, handler-template, and operand-separation passes all need to
//! locate the proven operation table and read its numeric handler entries.  The
//! table shape is common; what each pass does with a handler is not.  This
//! module therefore owns only the structural scan and the small value types it
//! exposes.
//!
//! Two scan policies are intentional and documented here rather than hidden:
//!
//! * [`TableScanMode::Strict`] is the catalogue/template policy.  A numeric-key
//!   object must contain only plain properties whose values are functions, the
//!   keys must be canonical unsigned 32-bit indices, and entries are sorted by
//!   key.
//! * [`TableScanMode::Lenient`] is the operand-separation policy.  Unsupported
//!   object members are skipped, source order is retained, and numeric keys use
//!   the operand pass's historical coercion (saturating integer casts and a
//!   plain `u32` parse).  This divergence predates the shared module and is
//!   preserved so output does not change.
//!
//! Object element counts also differ historically: the catalogue computes
//! `max_key + 1` (checking overflow exactly as before) while handler templates
//! use `max_key.saturating_add(1)`.  Callers select the matching method; the
//! scanner stores the raw maximum key instead of choosing one behavior.

use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_span::Span;
use serde::{Deserialize, Serialize};

/// A source span as recorded by the operation catalogue and handler templates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpanRecord {
    pub start: u32,
    pub end: u32,
}

pub(crate) struct TableEntry<'a> {
    pub(crate) key: u32,
    pub(crate) handler: Option<Handler<'a>>,
}

pub(crate) struct TableLiteral<'a> {
    pub(crate) shape: &'static str,
    /// Every array slot, or only the accepted handler properties of an object.
    pub(crate) entries: Vec<TableEntry<'a>>,
    /// `Some` for arrays, where the element count is the array length.
    pub(crate) array_len: Option<u32>,
    /// The greatest accepted key for a numeric-key object.
    pub(crate) max_key: Option<u32>,
    pub(crate) function_count: u32,
}

impl TableLiteral<'_> {
    /// Catalogue element count: `max_key + 1` for objects, preserving the
    /// original addition (debug panic or release wrap on overflow).
    pub(crate) fn element_count(&self) -> u32 {
        self.array_len
            .unwrap_or_else(|| self.max_key.map_or(0, |key| key + 1))
    }

    /// Handler-template element count: `max_key.saturating_add(1)` for objects.
    pub(crate) fn element_count_saturating(&self) -> u32 {
        self.array_len
            .unwrap_or_else(|| self.max_key.map_or(0, |key| key.saturating_add(1)))
    }
}

pub(crate) enum Handler<'a> {
    Function(&'a Function<'a>),
    Arrow(&'a ArrowFunctionExpression<'a>),
}

impl<'a> Handler<'a> {
    pub(crate) fn span(&self) -> Span {
        match self {
            Self::Function(function) => function.span,
            Self::Arrow(function) => function.span,
        }
    }

    pub(crate) fn params(&self) -> &'a FormalParameters<'a> {
        match self {
            Self::Function(function) => &function.params,
            Self::Arrow(function) => &function.params,
        }
    }

    pub(crate) fn body(&self) -> Option<&'a FunctionBody<'a>> {
        match self {
            Self::Function(function) => function.body.as_deref(),
            Self::Arrow(function) => Some(&function.body),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TableScanMode {
    Strict,
    Lenient,
}

/// Find the exactly-targeted variable declarator's operation table.  The
/// scanner counts every declarator in document order and never searches by
/// spelling.  The table is handed to the sink while the AST borrow is live;
/// callers turn entries into their own owned records.
pub(crate) trait TableSink {
    fn table(&mut self, table: TableLiteral<'_>);
}

pub(crate) fn scan_target_table<'a, S: TableSink>(
    program: &Program<'a>,
    target: u32,
    mode: TableScanMode,
    sink: &mut S,
) {
    let mut scanner = TableScanner {
        target,
        ordinal: 0,
        mode,
        sink,
    };
    scanner.visit_program(program);
}

struct TableScanner<'s, S> {
    target: u32,
    ordinal: u32,
    mode: TableScanMode,
    sink: &'s mut S,
}

impl<'a, S: TableSink> Visit<'a> for TableScanner<'_, S> {
    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        if self.ordinal == self.target {
            if let Some(table) = declarator
                .init
                .as_ref()
                .and_then(|init| scan_table(init, self.mode))
            {
                self.sink.table(table);
            }
        }
        self.ordinal += 1;
        walk::walk_variable_declarator(self, declarator);
    }
}

pub(crate) fn scan_table<'a>(
    expression: &'a Expression<'a>,
    mode: TableScanMode,
) -> Option<TableLiteral<'a>> {
    match expression {
        Expression::ArrayExpression(array) => {
            let mut entries = Vec::new();
            let mut functions = 0;
            for (key, element) in array.elements.iter().enumerate() {
                let handler = match element {
                    ArrayExpressionElement::FunctionExpression(f) => {
                        functions += 1;
                        Some(Handler::Function(f))
                    }
                    ArrayExpressionElement::ArrowFunctionExpression(f) => {
                        functions += 1;
                        Some(Handler::Arrow(f))
                    }
                    _ => None,
                };
                entries.push(TableEntry {
                    key: key as u32,
                    handler,
                });
            }
            Some(TableLiteral {
                shape: "array",
                entries,
                array_len: Some(array.elements.len() as u32),
                max_key: None,
                function_count: functions,
            })
        }
        Expression::ObjectExpression(object) => match mode {
            TableScanMode::Strict => {
                let mut entries = Vec::new();
                let mut functions = 0;
                for property in &object.properties {
                    let ObjectPropertyKind::ObjectProperty(property) = property else {
                        return None;
                    };
                    let key = numeric_key(&property.key)?;
                    let handler = match &property.value {
                        Expression::FunctionExpression(f) => {
                            functions += 1;
                            Some(Handler::Function(f))
                        }
                        Expression::ArrowFunctionExpression(f) => {
                            functions += 1;
                            Some(Handler::Arrow(f))
                        }
                        _ => return None,
                    };
                    entries.push(TableEntry { key, handler });
                }
                entries.sort_by_key(|entry| entry.key);
                let max_key = entries.last().map(|entry| entry.key);
                Some(TableLiteral {
                    shape: "numeric-key-object",
                    entries,
                    array_len: None,
                    max_key,
                    function_count: functions,
                })
            }
            TableScanMode::Lenient => {
                let mut entries = Vec::new();
                let mut functions = 0;
                for property in &object.properties {
                    let ObjectPropertyKind::ObjectProperty(property) = property else {
                        continue;
                    };
                    let Some(key) = numeric_key_lenient(&property.key) else {
                        continue;
                    };
                    let handler = match &property.value {
                        Expression::FunctionExpression(f) => {
                            functions += 1;
                            Some(Handler::Function(f))
                        }
                        Expression::ArrowFunctionExpression(f) => {
                            functions += 1;
                            Some(Handler::Arrow(f))
                        }
                        _ => continue,
                    };
                    entries.push(TableEntry { key, handler });
                }
                let max_key = entries.iter().map(|entry| entry.key).max();
                Some(TableLiteral {
                    shape: "numeric-key-object",
                    entries,
                    array_len: None,
                    max_key,
                    function_count: functions,
                })
            }
        },
        _ => None,
    }
}

/// Canonical numeric key used by the catalogue and handler templates.
pub(crate) fn numeric_key(key: &PropertyKey<'_>) -> Option<u32> {
    match key {
        PropertyKey::NumericLiteral(n)
            if n.value.is_finite()
                && n.value >= 0.0
                && n.value.fract() == 0.0
                && n.value < u32::MAX as f64 =>
        {
            Some(n.value as u32)
        }
        PropertyKey::StringLiteral(s) => {
            let n = s.value.parse::<u32>().ok()?;
            (s.value == n.to_string()).then_some(n)
        }
        _ => None,
    }
}

/// Numeric key accepted by the operand-separation pass.  This is deliberately
/// more permissive than [`numeric_key`]: it keeps the pre-refactor behavior for
/// non-canonical string spellings and uses JavaScript-number-to-`u32`
/// saturation instead of rejecting values outside the exact index range.
pub(crate) fn numeric_key_lenient(key: &PropertyKey<'_>) -> Option<u32> {
    match key {
        PropertyKey::NumericLiteral(number)
            if number.value >= 0.0 && number.value.fract() == 0.0 =>
        {
            Some(number.value as u32)
        }
        PropertyKey::StringLiteral(string) => string.value.parse().ok(),
        _ => None,
    }
}
