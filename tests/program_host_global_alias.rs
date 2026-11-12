//! Focused regression: the outer entry's proven host parameter is canonicalized
//! to the tracer's `host` binding at the shared handler-IR boundary.
//!
//! The bug: a JavaScript kernel whose global object arrives as the first
//! argument of the outer IIFE (`(function(b, s, F, z) { ... })(window, 0, 0, [1, 0])`)
//! keeps that parameter's obfuscated spelling in the recovered IR. The linked
//! program drops the IIFE but not the references, so the output carries a free
//! `b` and the host cleanup (which only knows `host`) never runs. These cases
//! prove the fix is symbol-based: the exact proven host parameter is renamed,
//! and nothing else is.

use std::collections::BTreeSet;

use serde_json::Value;
use shape::{
    handler_templates_from_report, BindingMapping, PipelineReport, PropertyMapping,
    ProvenOperationTable,
};

/// A report with one proven handler table at the first variable declarator.
fn report(output: &str) -> PipelineReport {
    PipelineReport {
        output: output.into(),
        input_hash: "host-alias-test".into(),
        stages: Vec::new(),
        diagnostics: Vec::new(),
        binding_mappings: Vec::<BindingMapping>::new(),
        property_mappings: Vec::<PropertyMapping>::new(),
        binding_residue: Vec::new(),
        extraction: Vec::new(),
        proven_operation_tables: vec![ProvenOperationTable {
            declaration_ordinal: 0,
            original_name: "ops".into(),
            canonical_name: "vmOperations".into(),
            initializer_elements: 1,
            initializer_functions: 1,
        }],
    }
}

/// Every `Binding` name that appears in the extracted handler recipes.
fn binding_names(output: &str) -> BTreeSet<String> {
    let templates = handler_templates_from_report(&report(output)).expect("extract templates");
    let mut names = BTreeSet::new();
    collect(&serde_json::to_value(&templates).expect("serialize templates"), &mut names);
    names
}

fn collect(value: &Value, names: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::Object(binding)) = map.get("Binding") {
                if let Some(Value::String(name)) = binding.get("name") {
                    names.insert(name.clone());
                }
            }
            for value in map.values() {
                collect(value, names);
            }
        }
        Value::Array(values) => {
            for value in values {
                collect(value, names);
            }
        }
        _ => {}
    }
}

/// `Q` is the outer entry's first parameter and the entry receives a host
/// expression, so `Q` means the global object. It must be canonicalized even
/// though its spelling is not `b`.
#[test]
fn renamed_outer_host_alias_is_canonicalized() {
    let names = binding_names(
        r#"(function(Q, s, F, z) {
            var $ops = [function(context) { return Q.value; }];
            return $ops;
        })(typeof window !== "undefined" ? window : this, 0, 0, [1, 0]);"#,
    );
    assert!(names.contains("host"), "bindings: {names:?}");
    assert!(!names.contains("Q"), "the alias spelling leaked: {names:?}");
}

/// The same spelling `b` is an ordinary external when the entry call does not
/// receive a host expression. It must stay `b`.
#[test]
fn ordinary_external_b_is_not_rewritten() {
    let names = binding_names(
        r#"(function(b, s, F, z) {
            var $ops = [function(context) { return b.value; }];
            return $ops;
        })(someNamespace, 0, 0, [1, 0]);"#,
    );
    assert!(names.contains("b"), "bindings: {names:?}");
    assert!(!names.contains("host"), "bindings: {names:?}");
}

/// A local `b` inside a handler is a temporary, not the captured host, even
/// when the entry parameter is also named `b`.
#[test]
fn shadowed_local_b_is_not_canonicalized() {
    let names = binding_names(
        r#"(function(b, s, F, z) {
            var $ops = [function(context) {
                var b = { value: 1 };
                return b.value;
            }];
            return $ops;
        })(typeof self !== "undefined" ? self : this, 0, 0, [1, 0]);"#,
    );
    assert!(!names.contains("host"), "bindings: {names:?}");
    assert!(!names.contains("b"), "bindings: {names:?}");
}

/// Only the proven host parameter is canonicalized; another captured external
/// keeps its own name.
#[test]
fn other_captured_externals_keep_their_names() {
    let names = binding_names(
        r#"(function(b, s, F, z) {
            var $ops = [function(context) { return [b.value, vmStringTable]; }];
            var vmStringTable = ["x"];
            return $ops;
        })(typeof global !== "undefined" ? global : this, 0, 0, [1, 0]);"#,
    );
    assert!(names.contains("host"), "bindings: {names:?}");
    assert!(names.contains("vmStringTable"), "bindings: {names:?}");
    assert!(!names.contains("b"), "bindings: {names:?}");
}

/// A string and an object property key spelled like the alias are data, not
/// bindings; nothing may rewrite them.
#[test]
fn strings_and_property_keys_are_untouched() {
    let names = binding_names(
        r#"(function(b, s, F, z) {
            var $ops = [function(context) { return { b: "b", value: 1 }; }];
            return $ops;
        })(typeof window !== "undefined" ? window : this, 0, 0, [1, 0]);"#,
    );
    assert!(names.is_empty(), "bindings: {names:?}");
}

/// `host` is reserved by the emitted cleanup. A local binding using that name
/// means canonicalizing would shadow it, so the source spelling is kept.
#[test]
fn local_host_binding_declines_canonicalization() {
    let names = binding_names(
        r#"(function(b, s, F, z) {
            var $ops = [function(context) {
                var host = { value: 2 };
                return b.value + host.value;
            }];
            return $ops;
        })(typeof window !== "undefined" ? window : this, 0, 0, [1, 0]);"#,
    );
    assert!(names.contains("b"), "bindings: {names:?}");
    assert!(!names.contains("host"), "bindings: {names:?}");
}

/// A distinct captured external named `host` also declines canonicalization:
/// renaming the host parameter to `host` would collide with it.
#[test]
fn captured_external_named_host_declines_canonicalization() {
    let names = binding_names(
        r#"(function(b, s, F, z) {
            var $ops = [function(context) { return b.value + host.value; }];
            var host = {};
            return $ops;
        })(typeof window !== "undefined" ? window : this, 0, 0, [1, 0]);"#,
    );
    assert!(names.contains("b"), "bindings: {names:?}");
}
