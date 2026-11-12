//! Shared VM dispatch-shape recognition.
//!
//! The nested dispatch read `TABLE[this.STATE][BYTES[this.PC++]]` is recognized
//! in one place so every caller observes the same structural facts. Its current
//! users are [`crate::kernel::config`], [`crate::dispatch::proof`], and
//! [`crate::transforms::deob::pipeline`]. Recognition is purely structural and never
//! grants execution authority; callers still re-prove what they consume against
//! binding identity.

use oxc_ast::ast::{ComputedMemberExpression, Expression, SimpleAssignmentTarget};
use oxc_semantic::Scoping;
use oxc_syntax::operator::UpdateOperator;

/// The acceptance contract for [`recognize_nested_dispatch`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NestedDispatchPolicy {
    /// Require the proof-backed `TABLE[this.STATE][BYTES[this.PC++]]` shape.
    StrictThisPc,
    /// Preserve the pipeline's historical renaming heuristic.  Only the
    /// table binding and postfix increment are constrained; the state and
    /// bytecode expressions remain intentionally loose.
    PipelineHeuristic,
}

/// The common structural facts of a nested dispatch read.
///
/// The optional fields are intentional: the pipeline policy historically
/// accepted computed/non-member state and bytecode expressions, and it only
/// needs the table binding.  Strict callers reject those cases before using
/// the optional metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NestedDispatch {
    pub table: oxc_semantic::SymbolId,
    pub table_name: String,
    pub bytecode: Option<oxc_semantic::SymbolId>,
    pub bytecode_name: Option<String>,
    pub pc_property: Option<String>,
    pub state_property: Option<String>,
    pub pc_receiver_is_this: bool,
}

/// Recognize the shared nested-dispatch read shape and resolve its bindings.
///
/// Both strict callers use the same proof contract.  The pipeline policy is
/// deliberately separate because its existing use is a renaming heuristic,
/// not a runtime proof, and therefore accepts unresolved/non-identifier
/// bytecode objects as it did before this helper was introduced.
///
/// This is the expression-position wrapper around
/// [`recognize_nested_dispatch_member`].  Callers that walk Oxc
/// `ComputedMemberExpression` nodes (including assignment, update, and
/// destructuring targets) must use the member form so they observe the same
/// nodes as an unconditional `visit_computed_member_expression`.
pub fn recognize_nested_dispatch(
    expression: &Expression<'_>,
    scoping: &Scoping,
    policy: NestedDispatchPolicy,
) -> Option<NestedDispatch> {
    let Expression::ComputedMemberExpression(member) = expression else {
        return None;
    };
    recognize_nested_dispatch_member(member, scoping, policy)
}

/// Recognize the shared nested-dispatch read shape from a computed member
/// node, regardless of whether that node is an expression or an assignment,
/// update, or destructuring target.
///
/// This is the one structural recognizer; [`recognize_nested_dispatch`] is a
/// thin expression adapter.  Keeping the target-capable form here prevents a
/// visitor from silently losing traversal parity when it changes from
/// `visit_computed_member_expression` to `visit_expression`.
pub fn recognize_nested_dispatch_member(
    member: &ComputedMemberExpression<'_>,
    scoping: &Scoping,
    policy: NestedDispatchPolicy,
) -> Option<NestedDispatch> {
    let Expression::ComputedMemberExpression(row) = &member.object else {
        return None;
    };
    let Expression::Identifier(table) = &row.object else {
        return None;
    };
    let Expression::ComputedMemberExpression(bytecode) = &member.expression else {
        return None;
    };
    let Expression::UpdateExpression(update) = &bytecode.expression else {
        return None;
    };
    if update.operator != UpdateOperator::Increment || update.prefix {
        return None;
    }

    let table_name = table.name.to_string();
    let table = table
        .reference_id
        .get()
        .and_then(|reference| scoping.get_reference(reference).symbol_id())?;
    let bytecode_identifier = match &bytecode.object {
        Expression::Identifier(identifier) => Some(identifier),
        _ => None,
    };
    let bytecode = bytecode_identifier.and_then(|identifier| {
        identifier
            .reference_id
            .get()
            .and_then(|reference| scoping.get_reference(reference).symbol_id())
    });
    let bytecode_name = bytecode_identifier.map(|identifier| identifier.name.to_string());

    let state_property = match &row.expression {
        Expression::StaticMemberExpression(state) => Some(state.property.name.to_string()),
        _ => None,
    };
    let pc_property = match &update.argument {
        SimpleAssignmentTarget::StaticMemberExpression(pc) => Some(pc.property.name.to_string()),
        _ => None,
    };
    let pc_receiver_is_this = matches!(
        &update.argument,
        SimpleAssignmentTarget::StaticMemberExpression(pc)
            if matches!(&pc.object, Expression::ThisExpression(_))
    );

    if matches!(policy, NestedDispatchPolicy::StrictThisPc)
        && (!matches!(&row.expression, Expression::StaticMemberExpression(state) if matches!(&state.object, Expression::ThisExpression(_)))
            || bytecode.is_none()
            || !pc_receiver_is_this
            || pc_property.is_none())
    {
        return None;
    }

    Some(NestedDispatch {
        table,
        table_name,
        bytecode,
        bytecode_name,
        pc_property,
        state_property,
        pc_receiver_is_this,
    })
}

#[cfg(test)]
mod nested_dispatch_tests {
    // These pin the shared recognizer itself.  Caller-adapter coverage lives
    // with the production call sites: the dispatch proof finder, the kernel
    // extraction path, and the pipeline `Detector`.
    use super::*;
    use crate::oxc::{analyze, parse};
    use oxc_ast::ast::{Expression, Statement};

    const CASES: &[(&str, bool, bool)] = &[
        ("TABLE[this.STATE][BYTES[this.P++]]", true, true),
        ("TABLE[this.STATE][BYTES[cursor++]]", false, true),
        ("TABLE[this.STATE][BYTES[receiver.P++]]", false, true),
        ("TABLE[this.STATE][BYTES[this[\"P\"]++]]", false, true),
        ("TABLE[this.STATE][BYTES[++this.P]]", false, false),
        ("TABLE[this.STATE][BYTES[this.P--]]", false, false),
        ("TABLE[this.STATE][BYTES[this.P += 1]]", false, false),
        ("TABLE[receiver.STATE][BYTES[this.P++]]", false, true),
        ("TABLE[this[\"STATE\"]][BYTES[this.P++]]", false, true),
        ("TABLE[this.STATE][makeBytes()[this.P++]]", false, true),
        ("holder.TABLE[this.STATE][BYTES[this.P++]]", false, false),
    ];

    fn recognize(source_expression: &str, policy: NestedDispatchPolicy) -> Option<NestedDispatch> {
        let source = format!(
            "var TABLE = []; var BYTES = []; var cursor = 0; var receiver = {{}}; var holder = {{ TABLE: TABLE }}; var value = {source_expression};"
        );
        let allocator = oxc_allocator::Allocator::default();
        let parsed = parse(&allocator, &source);
        assert!(parsed.errors.is_empty(), "{source:?}: {:?}", parsed.errors);
        let (scoping, _) = analyze(&parsed.program);
        let Statement::VariableDeclaration(declaration) = &parsed.program.body[5] else {
            panic!("test source did not end in a variable declaration")
        };
        let expression = declaration.declarations[0]
            .init
            .as_ref()
            .expect("test value has an initializer");
        assert!(matches!(
            expression,
            Expression::ComputedMemberExpression(_)
        ));
        recognize_nested_dispatch(expression, &scoping, policy)
    }

    fn assert_characterization(policy: NestedDispatchPolicy) {
        for (source, strict, loose) in CASES {
            let expected = match policy {
                NestedDispatchPolicy::StrictThisPc => *strict,
                NestedDispatchPolicy::PipelineHeuristic => *loose,
            };
            assert_eq!(recognize(source, policy).is_some(), expected, "{source}");
        }
    }

    #[test]
    fn dispatch_proof_characterization_is_pinned() {
        assert_characterization(NestedDispatchPolicy::StrictThisPc);
        let result = recognize(CASES[0].0, NestedDispatchPolicy::StrictThisPc)
            .expect("exact dispatch shape");
        assert_eq!(result.table_name, "TABLE");
        assert_eq!(result.bytecode_name.as_deref(), Some("BYTES"));
        assert_eq!(result.pc_property.as_deref(), Some("P"));
        assert_eq!(result.state_property.as_deref(), Some("STATE"));
        assert!(result.pc_receiver_is_this);
    }

    #[test]
    fn pipeline_characterization_is_pinned() {
        assert_characterization(NestedDispatchPolicy::PipelineHeuristic);
        assert!(recognize(
            "TABLE[this.STATE][missingBytes[this.P++]]",
            NestedDispatchPolicy::PipelineHeuristic,
        )
        .is_some());
    }
}
