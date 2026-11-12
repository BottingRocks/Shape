//! Descriptive semantic naming over synthetic sources.
//!
//! These tests exercise only the public API: the analysis plan
//! (`shape::naming::semantic_names::plan_semantic_names`) and the rewrite
//! (`shape::naming::program::rename_semantic_names`). Every case uses a
//! hand-written source; no external capture or corpus variable is needed.

use std::collections::BTreeSet;
use std::path::PathBuf;

use oxc_allocator::Allocator;

use shape::analysis;
use shape::naming::semantic_names::{plan_semantic_names, SemanticNamingPlan};
use shape::naming::program::rename_semantic_names;


fn assert_reparses(source: &str) {
    let allocator = Allocator::default();
    let parsed = analysis::parse(&allocator, source);
    assert!(
        parsed.errors.is_empty() && !parsed.panicked,
        "output did not parse: {:?}",
        parsed.errors
    );
}

// ===========================================================================
// Name-independent property projection
// ===========================================================================

// ===========================================================================
// Alpha-rename and constant mutation
// ===========================================================================

fn proposed_names(plan: &SemanticNamingPlan) -> BTreeSet<String> {
    plan.candidates
        .iter()
        .map(|candidate| candidate.proposed_name.clone())
        .collect()
}

// ===========================================================================
// Corpus tests
// ===========================================================================

// ===========================================================================
// Fail-closed decoys
// ===========================================================================

#[test]
fn a_binding_with_conflicting_assignments_is_not_named() {
    // Two writes to the same window slot: not a single, unambiguous origin.
    let source = "\
function outer() {
  function main() {
    var w;
    w = window;
    w = window;
    var u = w + \"\";
    return { init: function inner() { return u; } };
  }
  return main();
}
outer();
";
    let plan = plan_semantic_names(source).expect("plan");
    let names = proposed_names(&plan);
    assert!(!names.contains("hostWindow"), "{names:?}");
    assert!(!names.contains("instrumentationWindow"), "{names:?}");
}

#[test]
fn a_shadowed_same_spelling_local_is_not_renamed() {
    let source = "\
function outer() {
  function main() {
    var w;
    w = window;
    var u = w[\"location\"] + \"\";
    function inner() { var w = 7; return w; }
    return { init: inner };
  }
  return main();
}
outer();
";
    let result = rename_semantic_names(source).expect("plan");
    assert!(proposed_names(&result.plan).contains("hostWindow"));
    let output = &result.transformed_source;
    // The main slot moved; the shadowed local kept its spelling.
    assert!(output.contains("hostWindow[\"location\"]"), "{output}");
    assert!(output.contains("var w = 7"), "{output}");
    assert!(output.contains("return w;"), "{output}");
    assert_reparses(output);
}

#[test]
fn two_identical_helper_candidates_fail_closed_as_ambiguous() {
    let source = "\
function outer() {
  function main() {
    function first() {
      var encoder = new TextEncoder();
      var alg = { name: \"AES-GCM\" };
      var key = crypto.subtle.generateKey(alg, true, [\"encrypt\"]);
      crypto.subtle.exportKey(\"raw\", key);
      return crypto.subtle.encrypt(alg, key, encoder.encode(\"one\"));
    }
    function second() {
      var encoder = new TextEncoder();
      var alg = { name: \"AES-GCM\" };
      var key = crypto.subtle.generateKey(alg, true, [\"encrypt\"]);
      crypto.subtle.exportKey(\"raw\", key);
      return crypto.subtle.encrypt(alg, key, encoder.encode(\"two\"));
    }
    return { init: first };
  }
  return main();
}
outer();
";
    let plan = plan_semantic_names(source).expect("plan");
    let names = proposed_names(&plan);
    assert!(!names.contains("encryptStringAesGcm"), "{names:?}");
    assert!(
        plan.ambiguous
            .iter()
            .any(|ambiguity| ambiguity.role == "encrypt_string_aes_gcm"),
        "an equally-matching pair must be reported as ambiguous"
    );
}

/// Two structurally equal functions bound in the same scope and claimed by one
/// package-finder role: the role is an equal-evidence tie there, so the plan
/// records exactly one ambiguity and resolves neither candidate. The rename CLI
/// would otherwise fail closed later with a simultaneous-collision error.
#[test]
fn two_finder_candidates_for_one_role_are_one_ambiguity_and_no_candidates() {
    let source = "\
(function main(host) {
  function first(text) { return text.charCodeAt(); }
  function second(text) { return text.charCodeAt(); }
  first(\"a\");
  second(\"b\");
})(this);
";
    let plan = plan_semantic_names(source).expect("plan");
    let names = proposed_names(&plan);

    assert!(
        !names.contains("firstCharCode"),
        "neither tied function may be proposed: {names:?}"
    );
    let ties: Vec<_> = plan
        .ambiguous
        .iter()
        .filter(|ambiguity| ambiguity.role == "first_char_code")
        .collect();
    assert_eq!(
        ties.len(),
        1,
        "one role tie is one ambiguity entry: {:#?}",
        plan.ambiguous
    );
    let tie = ties[0];
    assert_eq!(tie.involved.len(), 2, "{tie:#?}");
    assert_eq!(tie.candidates.len(), 2, "{tie:#?}");
    assert!(
        tie.reason.contains("firstCharCode") && tie.reason.contains("2 proven functions"),
        "the tie names the proposed name and both functions: {tie:#?}"
    );
    assert!(
        !plan
            .candidates
            .iter()
            .any(|candidate| candidate.role == "first_char_code"),
        "no candidate is emitted for an ambiguous role: {:#?}",
        plan.candidates
    );

    // Fail closed: the rename still applies and leaves both names unchanged.
    let result = rename_semantic_names(source).expect("rename");
    let output = &result.transformed_source;
    assert!(output.contains("function first("), "{output}");
    assert!(output.contains("function second("), "{output}");
    assert!(output.contains("first(\"a\")"), "{output}");
    assert!(output.contains("second(\"b\")"), "{output}");
    assert_reparses(output);
}

/// Unbound (anonymous) functions cannot collide with a binding rename, so two
/// equal claims of one role stay two anonymous names and are never a tie.
#[test]
fn two_unbound_finder_candidates_are_anonymous_not_ambiguous() {
    let source = "\
(function main(host) {
  let first = [1].map(text => text.charCodeAt());
  let second = [2].map(text => text.charCodeAt());
  return [first, second];
})(this);
";
    let plan = plan_semantic_names(source).expect("plan");
    assert!(
        !plan
            .ambiguous
            .iter()
            .any(|ambiguity| ambiguity.role == "first_char_code"),
        "unbound claims have no binding name to collide: {:#?}",
        plan.ambiguous
    );
    assert!(
        !plan
            .candidates
            .iter()
            .any(|candidate| candidate.role == "first_char_code"),
        "an anonymous function is not a binding rename: {:#?}",
        plan.candidates
    );
    let anonymous: Vec<_> = plan
        .anonymous_functions
        .iter()
        .filter(|named| named.role == "first_char_code")
        .collect();
    assert_eq!(anonymous.len(), 2, "{:#?}", plan.anonymous_functions);
    assert!(
        anonymous
            .iter()
            .all(|named| named.proposed_name == "firstCharCode"),
        "{anonymous:#?}"
    );

    // Anonymous names are not renames, so the source keeps both arrows.
    let result = rename_semantic_names(source).expect("rename");
    let output = &result.transformed_source;
    assert!(output.contains("[1].map(text => text.charCodeAt())"), "{output}");
    assert!(output.contains("[2].map(text => text.charCodeAt())"), "{output}");
    assert_reparses(output);
}

/// A role tied in one scope does not tie its other claims: two equal functions
/// declared in different function scopes both resolve to the same name, which
/// the rename engine can apply because the bindings never see each other.
#[test]
fn two_finder_candidates_in_different_scopes_both_resolve() {
    let source = "\
(function main(host) {
  function firstScope() {
    function read(text) { return text.charCodeAt(); }
    return read(\"a\");
  }
  function secondScope() {
    function read(text) { return text.charCodeAt(); }
    return read(\"b\");
  }
  return [firstScope(), secondScope()];
})(this);
";
    let plan = plan_semantic_names(source).expect("plan");
    assert!(
        !plan
            .ambiguous
            .iter()
            .any(|ambiguity| ambiguity.role == "first_char_code"),
        "different scopes are independent: {:#?}",
        plan.ambiguous
    );
    let candidates: Vec<_> = plan
        .candidates
        .iter()
        .filter(|candidate| candidate.role == "first_char_code")
        .collect();
    assert_eq!(candidates.len(), 2, "{:#?}", plan.candidates);
    assert!(
        candidates
            .iter()
            .all(|candidate| candidate.proposed_name == "firstCharCode"
                && candidate.original_name == "read"),
        "{candidates:#?}"
    );

    // Both renames apply: the two `read` bindings live in sibling scopes.
    let result = rename_semantic_names(source).expect("rename");
    assert_eq!(result.applied, result.plan.proposed(), "{:#?}", result.outcomes);
    let output = &result.transformed_source;
    assert_eq!(output.matches("function firstCharCode(").count(), 2, "{output}");
    assert!(output.contains("return firstCharCode(\"a\")"), "{output}");
    assert!(output.contains("return firstCharCode(\"b\")"), "{output}");
    assert_reparses(output);
}

/// A finder role with exactly one candidate still resolves exactly as before.
#[test]
fn a_single_finder_candidate_still_resolves() {
    let source = "\
(function main(host) {
  function first(text) { return text.charCodeAt(); }
  first(\"a\");
})(this);
";
    let plan = plan_semantic_names(source).expect("plan");
    assert!(
        plan.candidates
            .iter()
            .any(|candidate| candidate.role == "first_char_code"
                && candidate.proposed_name == "firstCharCode"),
        "{:#?}",
        plan.candidates
    );
    assert!(
        !plan
            .ambiguous
            .iter()
            .any(|ambiguity| ambiguity.role == "first_char_code"),
        "a single candidate is not ambiguous: {:#?}",
        plan.ambiguous
    );

    let result = rename_semantic_names(source).expect("rename");
    let output = &result.transformed_source;
    assert!(output.contains("function firstCharCode("), "{output}");
    assert!(output.contains("firstCharCode(\"a\")"), "{output}");
    assert_reparses(output);
}

/// A finder function claim's `alias_role` is a different role and is claimed
/// from the single main-scope alias when the function role itself is not tied.
#[test]
fn a_finder_alias_role_still_produces_the_alias_candidate() {
    let source = "\
(function main(host) {
  let first = function (text) { return text.charCodeAt(); };
  let firstAlias = first;
  firstAlias(\"a\");
})(this);
";
    let plan = plan_semantic_names(source).expect("plan");
    let names = proposed_names(&plan);
    assert!(names.contains("firstCharCode"), "{names:?}");
    assert!(names.contains("firstCharCodeReader"), "{names:?}");
    assert!(
        plan.candidates
            .iter()
            .any(|candidate| candidate.role == "first_char_code_alias"),
        "{:#?}",
        plan.candidates
    );
    assert!(
        !plan
            .ambiguous
            .iter()
            .any(|ambiguity| ambiguity.role == "first_char_code"
                || ambiguity.role == "first_char_code_alias"),
        "{:#?}",
        plan.ambiguous
    );
}

/// The main-binding half of the grouping rule: one role claimed for two
/// distinct identifier spans is one ambiguity, and neither binding is named.
#[test]
fn two_finder_main_bindings_for_one_role_are_one_ambiguity() {
    let source = "\
(function main(host) {
  let first = Array.call.bind(\"\".charCodeAt);
  let second = Array.call.bind(\"\".charCodeAt);
  return [first, second];
})(this);
";
    let plan = plan_semantic_names(source).expect("plan");
    let names = proposed_names(&plan);
    assert!(!names.contains("charCodeUnitReader"), "{names:?}");
    let ties: Vec<_> = plan
        .ambiguous
        .iter()
        .filter(|ambiguity| ambiguity.role == "char_code_unit_reader")
        .collect();
    assert_eq!(
        ties.len(),
        1,
        "one tie is one ambiguity entry: {:#?}",
        plan.ambiguous
    );
    let tie = ties[0];
    assert_eq!(tie.involved.len(), 2, "{tie:#?}");
    assert!(
        tie.reason.contains("charCodeUnitReader") && tie.reason.contains("2 proven main bindings"),
        "{tie:#?}"
    );
    assert!(
        tie.involved.iter().all(|involved| involved
            .id
            .as_deref()
            .is_some_and(|id| id.starts_with("mv@"))),
        "tied MAIN bindings keep their mv@ ids: {tie:#?}"
    );
    assert!(
        !plan
            .candidates
            .iter()
            .any(|candidate| candidate.role == "char_code_unit_reader"),
        "{:#?}",
        plan.candidates
    );

    let result = rename_semantic_names(source).expect("rename");
    let output = &result.transformed_source;
    assert!(output.contains("let first ="), "{output}");
    assert!(output.contains("let second ="), "{output}");
    assert_reparses(output);
}

#[test]
fn a_reassigned_container_role_is_skipped_not_guessed() {
    // A descriptor-shaped array that is overwritten cannot carry a certain role.
    let source = "\
function outer() {
  function main() {
    var a = 1, b = 2, c = 3, d = 4;
    var r = [{ ctor: null, priority: false }];
    r = [{ ctor: null, priority: false }, { ctor: null, priority: false }];
    return { init: function inner() { return [a, b, c, d, r]; } };
  }
  return main();
}
outer();
";
    let plan = plan_semantic_names(source).expect("plan");
    assert!(!proposed_names(&plan).contains("signalDescriptorRegistry"));
    assert!(
        plan.skipped
            .iter()
            .any(|skip| skip.role == "signal_descriptor_registry"),
        "a conflicting container binding is reported as skipped"
    );
}

// ===========================================================================
// Production hygiene: no generated spellings or seed constants in recognition
// ===========================================================================

#[test]
fn recognition_source_contains_no_generated_spellings_or_seed_constants() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/naming/semantic_names.rs");
    let source = std::fs::read_to_string(&path).expect("read recognition source");
    for needle in [
        "$KEY_",
        "$THREAD_",
        "zeohreqohkhm",
        "WMyakuIkn",
        "FTzyeq12rr",
        "4294967295",
        "307069178630199",
    ] {
        assert!(
            !source.contains(needle),
            "recognition source must not contain {needle}"
        );
    }
}

// ===========================================================================
// CLI
// ===========================================================================

// ===========================================================================
// Structural-fault regressions
// ===========================================================================

fn aes_encryptor_body(value: &str) -> String {
    format!(
        r#""AES-GCM";
      const encoder = new TextEncoder();
      const key = crypto.subtle.generateKey({{ name: "AES-GCM" }}, true, ["encrypt"]);
      crypto.subtle.exportKey("raw", key);
      return crypto.subtle.encrypt({{ name: "AES-GCM" }}, key, encoder.encode({value}));"#
    )
}

fn two_encryptors(first: &str, second: &str) -> String {
    format!(
        "(function main() {{\n  var i = 0;\n  function {first}() {{\n    {}\n  }}\n  function {second}() {{\n    {}\n  }}\n  return {first};\n}})();",
        aes_encryptor_body("i"),
        aes_encryptor_body("i + 1"),
    )
}

#[test]
fn unrelated_encryptors_stay_ambiguous_regardless_of_byte_length() {
    // Two unrelated, equally plausible AES functions of different body length.
    let unequal = two_encryptors("$THREAD_7_1", "$THREAD_7_222222");
    // An alpha rename swaps the length ordering without changing semantics.
    let swapped = two_encryptors("$THREAD_7_111111", "$THREAD_7_2");
    for source in [unequal, swapped] {
        let plan = plan_semantic_names(&source).expect("plan");
        assert!(
            !proposed_names(&plan).contains("encryptStringAesGcm"),
            "byte length must not select one unrelated candidate"
        );
        assert!(
            plan.ambiguous
                .iter()
                .any(|ambiguity| ambiguity.role == "encrypt_string_aes_gcm"),
            "the unrelated pair must be reported ambiguous"
        );
    }
}

#[test]
fn conflicting_roles_are_ambiguous_not_first_wins() {
    let source = "\
function outer() {
  function main() {
    var alpha = 1, beta = 2, gamma = 3;
    function both() {
      var hooks = {};
      hooks.onBeforeInvoke = 1;
      hooks.onAfterInvoke = 2;
      hooks.onBeforeSet = 3;
      hooks.onBeforeGet = 4;
      return hooks;
    }
    function entry() { return both(); }
    return { init: entry };
  }
  return main();
}
outer();
";
    let plan = plan_semantic_names(source).expect("plan");
    let names = proposed_names(&plan);
    assert!(!names.contains("installMethodInterceptor"), "{names:?}");
    assert!(!names.contains("installAccessorInterceptor"), "{names:?}");
    assert!(
        plan.ambiguous.iter().any(|ambiguity| {
            ambiguity.role.contains("role_conflict")
                && ambiguity.reason.contains("install_method_interceptor")
                && ambiguity.reason.contains("install_accessor_interceptor")
        }),
        "a binding with two incompatible roles must be ambiguous: {:#?}",
        plan.ambiguous
    );
}

#[test]
fn unused_string_and_member_bags_are_not_rename_proofs() {
    let source = "\
function outer() {
  function main() {
    var alpha = 1, beta = 2, gamma = 3;
    function bag() {
      var label = \"AES-GCM\";
      api.generateKey;
      api.exportKey;
      api.encrypt;
      document.querySelectorAll;
      return label;
    }
    function entry() { return bag(); }
    return { init: entry };
  }
  return main();
}
outer();
";
    let plan = plan_semantic_names(source).expect("plan");
    let names = proposed_names(&plan);
    for name in [
        "encryptStringAesGcm",
        "decryptPackedAesGcmBlob",
        "collectDomFields",
    ] {
        assert!(!names.contains(name), "a bag became {name}: {names:?}");
    }
}

#[test]
fn a_reassigned_or_duplicated_function_is_not_certain() {
    let body = aes_encryptor_body("1");
    let reassigned = format!(
        "(function main() {{\n  function $THREAD_7_1() {{\n    {body}\n  }}\n  $THREAD_7_1 = null;\n  return $THREAD_7_1;\n}})();"
    );
    let duplicated = format!(
        "(function main() {{\n  function $THREAD_7_1() {{\n    {body}\n  }}\n  function $THREAD_7_1() {{ return 1; }}\n  return $THREAD_7_1;\n}})();"
    );
    for source in [reassigned, duplicated] {
        let plan = plan_semantic_names(&source).expect("plan");
        assert!(
            !proposed_names(&plan).contains("encryptStringAesGcm"),
            "a mutable/duplicate function binding became a certain role"
        );
        assert!(
            plan.skipped
                .iter()
                .any(|skip| skip.role == "encrypt_string_aes_gcm"),
            "the refusal must be recorded as an explicit skip"
        );
    }
}

// ===========================================================================
// Class declarations
// ===========================================================================

/// A proven ring buffer class in MAIN plus an unrelated class of the same
/// original spelling in a nested scope.
const RING_CLASS_IN_NESTED_SCOPE: &str = r#"(function main() {
  class Ring {
    constructor(size) {
      this.i = 0;
      this.length = 0;
      this.n = size === 0 || typeof size !== "number" ? Infinity : size;
      this.arr = [];
    }
    put(value) {
      this.arr[this.i] = value;
      this.i = Number(this.i) + 1;
      if (this.length < this.n) { this.length = Number(this.length) + 1; }
      if (this.i >= this.n) { this.i = 0; }
    }
    toArray() {
      if (this.length < this.n) { return this.arr.slice(0, this.length); }
      return this.arr.slice(this.i, this.n).concat(this.arr.slice(0, this.i));
    }
    clear() { this.arr = []; this.length = 0; this.i = 0; }
  }
  function unrelated() {
    class Ring { constructor() { this.value = 1; } }
    return new Ring().value;
  }
  let ring = new Ring(2);
  ring.put(1);
  return [ring.toArray(), unrelated()];
})();"#;

#[test]
fn a_class_claim_renames_its_declaration_and_references_and_reparses() {
    let result = rename_semantic_names(RING_CLASS_IN_NESTED_SCOPE).expect("rename");
    let out = &result.transformed_source;
    assert!(out.contains("class RingBuffer {"), "{out}");
    assert!(out.contains("new RingBuffer(2)"), "{out}");
    assert_eq!(
        out.matches("class Ring {").count(),
        1,
        "only the unrelated nested class keeps the original spelling:\n{out}"
    );
    assert!(
        result
            .outcomes
            .iter()
            .all(|outcome| outcome.applied),
        "{:#?}",
        result.outcomes
    );
    assert_reparses(out);
}

#[test]
fn a_class_rename_that_would_duplicate_a_binding_is_refused() {
    let source = RING_CLASS_IN_NESTED_SCOPE.replace(
        "  let ring = new Ring(2);",
        "  let RingBuffer = 1;\n  let ring = new Ring(2);",
    );
    let plan = plan_semantic_names(&source).expect("plan");
    assert!(
        !plan
            .candidates
            .iter()
            .any(|candidate| candidate.role == "ring_buffer_class"),
        "a colliding class rename must not be proposed: {:#?}",
        plan.candidates
    );
    let refusals: Vec<_> = plan
        .skipped
        .iter()
        .filter(|skip| skip.role == "ring_buffer_class")
        .collect();
    assert_eq!(refusals.len(), 1, "{:#?}", plan.skipped);
    assert!(
        refusals[0].reason.contains("duplicate a binding"),
        "{:#?}",
        refusals[0]
    );
}
