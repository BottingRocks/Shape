use super::*;
use crate::kernel::wasm_module::test_support::*;

#[test]
fn wasm_summary_preserves_import_export_for_valid_gc_types() {
    // Actual-properties never validated and never read the type section.
    assert_eq!(
        wasm_summary(&gc_memory_export_module()),
        Some((vec![], vec!["m".into()]))
    );
    assert_eq!(
        wasm_summary(&gc_import_export_module()),
        Some((vec!["env::f".into()], vec!["run".into()]))
    );
}

#[test]
fn wasm_summary_ignores_malformed_irrelevant_sections() {
    assert_eq!(
        wasm_summary(&malformed_type_memory_export_module()),
        Some((vec![], vec!["m".into()]))
    );
    assert_eq!(
        wasm_summary(&malformed_body_module()),
        Some((vec![], vec![]))
    );
    assert_eq!(
        wasm_summary(&malformed_table_memory_export_module()),
        Some((vec![], vec!["m".into()]))
    );
}

#[test]
fn wasm_summary_keeps_validation_ignored_but_reads_abi() {
    assert_eq!(
        wasm_summary(&invalid_export_module()),
        Some((vec![], vec!["x".into()]))
    );
}

#[test]
fn wasm_summary_keeps_compact_imports_and_group_errors() {
    assert_eq!(
        wasm_summary(&compact2_memory_import_module()),
        Some((vec!["env::m".into()], vec!["m".into()]))
    );
    // wasmparser validates the compact item block while reading the group,
    // so a truncated group is a top-level import error here.  The ABI
    // projection still flattens compact inner iterators defensively, but
    // this structurally pre-validated encoding cannot demonstrate it.
    assert_eq!(wasm_summary(&compact1_truncated_with_export_module()), None);
}

#[test]
fn wasm_summary_rejects_top_level_import_and_export_errors() {
    assert_eq!(wasm_summary(&truncated_import_module()), None);
    assert_eq!(wasm_summary(&truncated_export_after_import_module()), None);
}

#[test]
fn wasm_summary_accepts_large_module_without_new_default_cap() {
    let bytes = large_custom_section_module();
    assert!(bytes.len() > 8 * 1024 * 1024);
    assert_eq!(wasm_summary(&bytes), Some((vec![], vec!["m".into()])));
}
