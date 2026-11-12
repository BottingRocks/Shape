//! One naming decision path: predicate ownership, span-based identity joins,
//! one role -> one name vocabulary, and the serialized proposal contract.
//!
//! Every case is synthetic and self-contained.

use std::collections::{BTreeMap, BTreeSet};

use shape::signals::extract::scanner::extract_program_properties;
use shape::naming::semantic_names::{plan_semantic_names, plan_semantic_names_with_claims, plan_semantic_names_with_properties, vocabulary_name, FunctionSpanClaim, SemanticNamesError, SemanticNamingPlan, NAMING_VOCABULARY};
use shape::signals::schema::{Confidence, ProgramProperties, SourceSpan};
use shape::naming::program::{identify_and_transform, IdentifyOptions};

fn proposed(plan: &SemanticNamingPlan) -> BTreeMap<String, String> {
    plan.candidates
        .iter()
        .map(|candidate| {
            (
                candidate.proposed_name.clone(),
                candidate.original_name.clone(),
            )
        })
        .collect()
}

fn names(plan: &SemanticNamingPlan) -> BTreeSet<String> {
    plan.candidates
        .iter()
        .map(|candidate| candidate.proposed_name.clone())
        .collect()
}

fn has_role_conflict(plan: &SemanticNamingPlan) -> bool {
    plan.ambiguous
        .iter()
        .any(|ambiguity| ambiguity.role.starts_with("role_conflict"))
}

// ===========================================================================
// Predicate ownership
// ===========================================================================

/// MAIN declares an Observable factory (a class it does not own) and also
/// uses observable members elsewhere. Under a descendant-wide predicate MAIN
/// itself matched and the role conflict cost both names.
const MAIN_CONTAINS_OBSERVABLE: &str = "\
function outer() {
  function main() {
    var alpha = 1, beta = 2, gamma = 3;
    function makeObservable() {
      class Stream {
        constructor(subscriber) {
          this._subscriber = subscriber;
        }
        subscribe(observer) {
          return this._subscriber(observer);
        }
        pipe() {
          return this;
        }
      }
      var Ctor = Stream;
      return Ctor;
    }
    var streamHolder = makeObservable();
    function consume(source) {
      source._subscriber;
      source.subscribe(function () {});
      return source.pipe();
    }
    function entry() {
      return consume(new streamHolder(function () {})) + alpha + beta + gamma;
    }
    return { init: entry };
  }
  return main();
}
outer();
";

#[test]
fn main_containing_observable_code_names_both_independently() {
    let plan = plan_semantic_names(MAIN_CONTAINS_OBSERVABLE).expect("plan");
    let proposed = proposed(&plan);
    assert_eq!(
        proposed.get("createAgentModule").map(String::as_str),
        Some("main"),
        "{proposed:?} {:#?}",
        plan.ambiguous
    );
    assert_eq!(
        proposed.get("createObservableClass").map(String::as_str),
        Some("makeObservable"),
        "{proposed:?}"
    );
    assert_eq!(
        proposed.get("Observable").map(String::as_str),
        Some("streamHolder"),
        "{proposed:?}"
    );
    assert!(!has_role_conflict(&plan), "{:#?}", plan.ambiguous);
}

#[test]
fn constructor_function_observable_factory_is_recognized() {
    // The prototype-assignment form: the constructor function and its
    // `prototype` members are written at the factory's own level.
    let source = "\
function outer() {
  function main() {
    var alpha = 1, beta = 2;
    function makeObservable() {
      var Ctor;
      function Stream() {
        this[\"_subscriber\"] = arguments[0];
      }
      Ctor = Stream;
      Stream[\"prototype\"][\"subscribe\"] = function () { return this._subscriber(arguments[0]); };
      Stream[\"prototype\"][\"pipe\"] = function () { return this; };
      return Ctor;
    }
    var streamHolder;
    streamHolder = makeObservable();
    function entry() { return new streamHolder(function () {}) + alpha + beta; }
    return { init: entry };
  }
  return main();
}
outer();
";
    let plan = plan_semantic_names(source).expect("plan");
    let proposed = proposed(&plan);
    assert_eq!(
        proposed.get("createObservableClass").map(String::as_str),
        Some("makeObservable"),
        "{proposed:?}"
    );
    assert!(proposed.contains_key("createAgentModule"), "{proposed:?}");
    assert!(!has_role_conflict(&plan), "{:#?}", plan.ambiguous);
}

#[test]
fn a_function_that_only_calls_into_an_observable_is_not_the_factory() {
    let source = "\
function outer() {
  function main() {
    var alpha = 1, beta = 2;
    function useOnly() {
      var stream = new Thing(function () {});
      stream._subscriber = null;
      stream.subscribe(function () {});
      return stream.pipe();
    }
    function entry() { return useOnly() + alpha + beta; }
    return { init: entry };
  }
  return main();
}
outer();
";
    let plan = plan_semantic_names(source).expect("plan");
    let names = names(&plan);
    assert!(!names.contains("createObservableClass"), "{names:?}");
    assert!(names.contains("createAgentModule"), "{names:?}");
    assert!(!has_role_conflict(&plan), "{:#?}", plan.ambiguous);
}

#[test]
fn escaping_descendants_do_not_lend_their_facts_to_the_parent() {
    // Two separate, escaping functions supply the AES facts between them.
    // Neither is an encryptor and their parent does not own their code.
    let source = "\
function outer() {
  function main() {
    var alpha = 1, beta = 2;
    function holder() {
      function keys() {
        var alg = { name: \"AES-GCM\" };
        var key = crypto.subtle.generateKey(alg, true, [\"encrypt\"]);
        return crypto.subtle.encrypt(alg, key, alpha);
      }
      function exporter() {
        var encoder = new TextEncoder();
        crypto.subtle.exportKey(\"raw\", beta);
        return encoder.encode(\"x\");
      }
      return { keys: keys, exporter: exporter };
    }
    function entry() { return holder(); }
    return { init: entry };
  }
  return main();
}
outer();
";
    let plan = plan_semantic_names(source).expect("plan");
    let names = names(&plan);
    assert!(!names.contains("encryptStringAesGcm"), "{names:?}");
    assert!(
        !plan
            .ambiguous
            .iter()
            .any(|ambiguity| ambiguity.role == "encrypt_string_aes_gcm"),
        "{:#?}",
        plan.ambiguous
    );
}

#[test]
fn inline_and_handed_off_callbacks_are_owned_by_their_function() {
    // The encryptor's facts live in promise callbacks: one inline, one a
    // declaration that is only handed to `then`.
    let source = "\
function outer() {
  function main() {
    var alpha = 1, beta = 2, gamma = 3, delta = 4, epsilon = 5;
    function encrypt(text) {
      var bytes = new TextEncoder().encode(text);
      var alg = { name: \"AES-GCM\" };
      function exported(raw) { return raw.byteLength; }
      return crypto.subtle.generateKey(alg, true, [\"encrypt\"]).then(function (key) {
        return crypto.subtle.encrypt(alg, key, bytes).then(function () {
          return crypto.subtle.exportKey(\"raw\", key).then(exported);
        });
      });
    }
    function entry() { return encrypt(\"x\") + alpha + beta + gamma + delta + epsilon; }
    return { init: entry };
  }
  return main();
}
outer();
";
    let plan = plan_semantic_names(source).expect("plan");
    let proposed = proposed(&plan);
    assert_eq!(
        proposed.get("encryptStringAesGcm").map(String::as_str),
        Some("encrypt"),
        "{proposed:?} {:#?}",
        plan.ambiguous
    );
}

// ===========================================================================
// Cross-parse identity: SHA-256 plus validated identifier spans
// ===========================================================================

/// Rewrite every report symbol ordinal (`BindingRef.symbol`, `bindings[].id`)
/// so it can only match the current parse by accident; spans stay valid.
fn shift_ordinals(properties: &ProgramProperties, by: u64) -> ProgramProperties {
    fn walk(value: &mut serde_json::Value, by: u64) {
        match value {
            serde_json::Value::Object(map) => {
                let is_ref = map.contains_key("identifier") && map.contains_key("name");
                for key in ["symbol", "id"] {
                    if is_ref {
                        if let Some(serde_json::Value::Number(number)) = map.get(key) {
                            if let Some(ordinal) = number.as_u64() {
                                map.insert(key.to_string(), serde_json::json!(ordinal + by));
                            }
                        }
                    }
                }
                for child in map.values_mut() {
                    walk(child, by);
                }
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    walk(item, by);
                }
            }
            _ => {}
        }
    }
    let mut value = serde_json::to_value(properties).expect("serialize properties");
    walk(&mut value, by);
    serde_json::from_value(value).expect("deserialize properties")
}

#[test]
fn different_symbol_ordinals_with_valid_spans_still_resolve() {
    let source = MAIN_CONTAINS_OBSERVABLE;
    let properties = extract_program_properties(source).expect("properties");
    let shifted = shift_ordinals(&properties, 100_000);
    assert_ne!(
        properties
            .program_scope
            .main
            .binding
            .as_ref()
            .unwrap()
            .symbol,
        shifted.program_scope.main.binding.as_ref().unwrap().symbol
    );
    let baseline = plan_semantic_names(source).expect("plan");
    let joined = plan_semantic_names_with_properties(source, &shifted).expect("plan");
    assert_eq!(joined, baseline);
    assert!(names(&joined).contains("createAgentModule"));
}

#[test]
fn invalid_identifier_spans_fail_closed_with_a_refusal() {
    let source = MAIN_CONTAINS_OBSERVABLE;
    let properties = extract_program_properties(source).expect("properties");
    let main = properties.program_scope.main.binding.clone().expect("main");

    let cases: Vec<(SourceSpan, &str)> = vec![
        // Off by one: spells a different text.
        (
            SourceSpan::new(main.identifier.start + 1, main.identifier.end + 1),
            "does not spell",
        ),
        // Out of bounds.
        (
            SourceSpan::new(source.len() as u32 + 4, source.len() as u32 + 8),
            "outside",
        ),
    ];
    for (span, reason) in cases {
        let mut broken = properties.clone();
        if let Some(binding) = broken.program_scope.main.binding.as_mut() {
            binding.identifier = span;
        }
        let plan = plan_semantic_names_with_properties(source, &broken).expect("plan");
        assert!(!names(&plan).contains("createAgentModule"), "{span:?}");
        let refusal = plan
            .skipped
            .iter()
            .find(|skip| skip.role == "agent_module")
            .unwrap_or_else(|| panic!("refusal recorded for {span:?}: {:#?}", plan.skipped));
        assert!(refusal.reason.contains(reason), "{refusal:?}");
        assert_eq!(refusal.involved[0].span, span);
    }
}

#[test]
fn multibyte_identifier_spans_are_authoritative_and_never_repaired() {
    // `mäin` is five bytes; its valid byte span resolves unchanged.
    let source = MAIN_CONTAINS_OBSERVABLE
        .replace("function main()", "function mäin()")
        .replace("return main();", "return mäin();");
    let properties = extract_program_properties(&source).expect("properties");
    let main = properties.program_scope.main.binding.clone().expect("main");
    assert_eq!(main.identifier.len(), 5, "byte span, not char span");
    let plan = plan_semantic_names_with_properties(&source, &properties).expect("plan");
    assert_eq!(
        proposed(&plan).get("createAgentModule").map(String::as_str),
        Some("mäin")
    );

    // A span that ends inside `ä` is not on a char boundary: refused, not
    // widened or narrowed to a neighbouring boundary.
    let mut broken = properties.clone();
    if let Some(binding) = broken.program_scope.main.binding.as_mut() {
        binding.identifier = SourceSpan::new(main.identifier.start, main.identifier.start + 2);
    }
    let plan = plan_semantic_names_with_properties(&source, &broken).expect("plan");
    assert!(!names(&plan).contains("createAgentModule"));
    assert!(plan
        .skipped
        .iter()
        .any(|skip| skip.role == "agent_module" && skip.reason.contains("UTF-8 char boundaries")));
}

#[test]
fn a_report_from_another_source_is_refused() {
    let properties = extract_program_properties(MAIN_CONTAINS_OBSERVABLE).expect("properties");
    let other = MAIN_CONTAINS_OBSERVABLE.replace("alpha = 1", "alpha = 2");
    match plan_semantic_names_with_properties(&other, &properties) {
        Err(SemanticNamesError::SourceMismatch { report, source }) => {
            assert_eq!(report, properties.source.sha256);
            assert_ne!(report, source);
        }
        other => panic!("expected a source mismatch, got {other:?}"),
    }
}

// ===========================================================================
// One decision path, one vocabulary
// ===========================================================================

#[test]
fn identify_renames_come_from_the_unified_plan() {
    let identified = identify_and_transform(
        MAIN_CONTAINS_OBSERVABLE,
        IdentifyOptions {
            rename: true,
            annotate: false,
        },
    )
    .expect("identify");
    let plan = plan_semantic_names(MAIN_CONTAINS_OBSERVABLE).expect("plan");
    assert_eq!(identified.naming_plan, plan);
    let from_identify: Vec<(u32, String)> = identified
        .rename_plans
        .iter()
        .map(|rename| (rename.declaration_start, rename.canonical_name.clone()))
        .collect();
    let from_plan: Vec<(u32, String)> = plan
        .candidates
        .iter()
        .map(|candidate| (candidate.identifier.start, candidate.proposed_name.clone()))
        .collect();
    assert_eq!(from_identify, from_plan);
    assert!(identified
        .transform
        .transformed_source
        .contains("function createAgentModule()"));
    // The retired second vocabulary never appears.
    for retired in [
        "mainEntry",
        "sessionConstructor",
        "writeString",
        "observerFactory",
    ] {
        assert!(
            !identified.transform.transformed_source.contains(retired),
            "{retired}"
        );
    }
}

#[test]
fn each_role_yields_one_name() {
    let plan = plan_semantic_names(MAIN_CONTAINS_OBSERVABLE).expect("plan");
    let mut by_role: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for candidate in &plan.candidates {
        by_role
            .entry(candidate.role.as_str())
            .or_default()
            .insert(candidate.proposed_name.as_str());
    }
    for (role, names) in by_role {
        assert_eq!(names.len(), 1, "{role}: {names:?}");
    }
}

#[test]
fn two_same_role_helpers_give_no_rename_on_either_path() {
    let source = "\
function outer() {
  function main() {
    var alpha = 1;
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
    function entry() { return first() + second() + alpha; }
    return { init: entry };
  }
  return main();
}
outer();
";
    let identified = identify_and_transform(source, IdentifyOptions::full()).expect("identify");
    assert!(identified
        .rename_plans
        .iter()
        .all(|rename| rename.canonical_name != "encryptStringAesGcm"));
    assert!(identified
        .naming_plan
        .ambiguous
        .iter()
        .any(
            |ambiguity| ambiguity.role == "encrypt_string_aes_gcm" && ambiguity.involved.len() == 2
        ));
    let output = &identified.transform.transformed_source;
    assert!(output.contains("function first()") && output.contains("function second()"));
}

#[test]
fn every_vocabulary_role_is_named_ambiguous_refused_or_unproven() {
    let plan = plan_semantic_names(MAIN_CONTAINS_OBSERVABLE).expect("plan");
    for candidate in &plan.candidates {
        if let Some(name) = vocabulary_name(&candidate.role) {
            assert_eq!(candidate.proposed_name, name, "{}", candidate.role);
        }
    }
    for (role, _, _) in NAMING_VOCABULARY {
        let entries = plan.candidates.iter().filter(|c| c.role == *role).count()
            + plan.ambiguous.iter().filter(|a| a.role == *role).count()
            + plan.skipped.iter().filter(|s| s.role == *role).count()
            + plan.unproven.iter().filter(|u| u.role == *role).count();
        assert!(entries >= 1, "{role} is not accounted for");
    }
    assert!(plan.unproven.iter().any(|u| u.role == "write_number"));
}

#[test]
fn a_span_claim_cannot_introduce_a_second_name_for_a_vocabulary_role() {
    let properties = extract_program_properties(SYNTHETIC).expect("properties");
    let span = function_span(SYNTHETIC, "function () { return alpha + 1; }");
    let plan = plan_semantic_names_with_claims(
        SYNTHETIC,
        &properties,
        &[claim("observable_class", span, "makeStreamClass", None)],
    )
    .expect("plan");
    assert!(!names(&plan).contains("makeStreamClass"));
    assert!(plan
        .skipped
        .iter()
        .any(|skip| skip.role == "observable_class" && skip.reason.contains("vocabulary")));
}

#[test]
fn a_refused_properties_report_degrades_per_role() {
    // Two equally broad main candidates: the extractor refuses the report as
    // ambiguous. The plan keeps the structural roles and records the rest.
    let source = "\
function outer() {
  function main() {
    var alpha = 1, beta = 2;
    function encrypt(text) {
      var bytes = new TextEncoder().encode(text);
      var alg = { name: \"AES-GCM\" };
      function exported(raw) { return raw.byteLength; }
      return crypto.subtle.generateKey(alg, true, [\"encrypt\"]).then(function (key) {
        return crypto.subtle.encrypt(alg, key, bytes).then(function () {
          return crypto.subtle.exportKey(\"raw\", key).then(exported);
        });
      });
    }
    function entry() { return encrypt(\"x\") + alpha + beta; }
    return { init: entry };
  }
  return main();
}
outer();
";
    assert!(matches!(
        extract_program_properties(source),
        Err(shape::signals::schema::ProgramPropertiesError::Ambiguous { .. })
    ));
    let plan = plan_semantic_names(source).expect("degraded plan");
    let refused = plan
        .ambiguous
        .iter()
        .find(|ambiguity| ambiguity.reason.contains("properties report refused"))
        .expect("the refused structure is an ambiguity");
    assert_eq!(refused.role, "agent_module");
    assert!(!refused.involved.is_empty());
    assert!(names(&plan).contains("encryptStringAesGcm"), "{:#?}", plan);
    let session = plan
        .unproven
        .iter()
        .find(|unproven| unproven.role == "session_runtime")
        .expect("dependent role recorded");
    assert!(session.reason.contains("refused"), "{session:?}");
}

#[test]
fn the_value_holder_gets_the_role_and_its_alias_the_alias_role() {
    let source = "\
function outer() {
  function main() {
    var alpha = 1, beta = 2, gamma = 3;
    var specs = [{ name: \"a\", property: \"b\", type: \"c\", isConstructor: false, location: 1 }];
    var specsRef = specs;
    function entry() { return specsRef.length + alpha + beta + gamma; }
    return { init: entry };
  }
  return main();
}
outer();
";
    let plan = plan_semantic_names(source).expect("plan");
    let proposed = proposed(&plan);
    assert_eq!(
        proposed.get("interceptorSpecs").map(String::as_str),
        Some("specs")
    );
    assert_eq!(
        proposed.get("interceptorSpecsAlias").map(String::as_str),
        Some("specsRef")
    );
    let alias = plan
        .candidates
        .iter()
        .find(|candidate| candidate.role == "interceptor_specs_alias")
        .unwrap();
    let holder = plan
        .candidates
        .iter()
        .find(|candidate| candidate.role == "interceptor_specs")
        .unwrap();
    assert_eq!(
        alias.entity.target.as_deref(),
        Some(holder.entity.id.as_str())
    );
}

// ===========================================================================
// Proposal vocabulary
// ===========================================================================

#[test]
fn every_proposal_exposes_entity_binding_role_evidence_and_confidence() {
    let plan = plan_semantic_names(MAIN_CONTAINS_OBSERVABLE).expect("plan");
    let json = serde_json::to_value(&plan).expect("serialize plan");
    assert!(json["contract"]
        .as_str()
        .unwrap()
        .contains("does not claim"));
    let candidates = json["candidates"].as_array().unwrap();
    assert!(!candidates.is_empty());
    for candidate in candidates {
        for key in [
            "role",
            "entity",
            "original_name",
            "proposed_name",
            "binding_kind",
            "identifier",
            "declaration",
            "confidence",
            "evidence",
            "behavior",
        ] {
            assert!(candidate.get(key).is_some(), "{key} missing: {candidate}");
        }
        assert!(candidate.get("symbol").is_none(), "no cross-parse ordinal");
        assert_eq!(candidate["behavior"], "role_only");
        let entity = &candidate["entity"];
        let kind = entity["kind"].as_str().unwrap();
        let id = entity["id"].as_str().unwrap();
        let span = format!(
            "{}..{}",
            entity["span"]["start"].as_u64().unwrap(),
            entity["span"]["end"].as_u64().unwrap()
        );
        match kind {
            "function" => assert_eq!(id, format!("fn@{span}")),
            "main_data" | "alias_holder" => assert_eq!(id, format!("mv@{span}")),
            other => panic!("unexpected entity kind {other}"),
        }
        assert!(!candidate["evidence"].as_array().unwrap().is_empty());
    }
    let observable = candidates
        .iter()
        .find(|candidate| candidate["role"] == "observable_class")
        .unwrap();
    let start = MAIN_CONTAINS_OBSERVABLE
        .find("function makeObservable")
        .unwrap();
    assert_eq!(observable["entity"]["span"]["start"], start as u64);
}

#[test]
fn ambiguities_and_refusals_carry_reasons_and_spans() {
    let source = "\
function outer() {
  function main() {
    var alpha = 1;
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
    var r = [{ ctor: null, priority: false }];
    r = [{ ctor: null, priority: false }];
    function entry() { return first() + second() + alpha + r; }
    return { init: entry };
  }
  return main();
}
outer();
";
    let plan = plan_semantic_names(source).expect("plan");
    let json = serde_json::to_value(&plan).expect("serialize plan");
    let ambiguity = json["ambiguous"]
        .as_array()
        .unwrap()
        .iter()
        .find(|ambiguity| ambiguity["role"] == "encrypt_string_aes_gcm")
        .expect("ambiguity");
    assert!(!ambiguity["reason"].as_str().unwrap().is_empty());
    let involved = ambiguity["involved"].as_array().unwrap();
    assert_eq!(involved.len(), 2);
    for entry in involved {
        assert!(entry["span"]["start"].is_u64());
        assert!(entry["id"].as_str().unwrap().starts_with("fn@"));
    }
    for skip in json["skipped"].as_array().unwrap() {
        assert!(!skip["reason"].as_str().unwrap().is_empty(), "{skip}");
        assert!(skip.get("involved").is_some(), "{skip}");
    }
}

// ===========================================================================
// Span-keyed claims (the purpose-finder entry point)
// ===========================================================================

const SYNTHETIC: &str = "\
function outer() {
  function main() {
    var alpha = 1;
    var helper = function () { return alpha + 1; };
    var helperAlias = helper;
    var mutable = function () { return 2; };
    mutable = null;
    function declared() { return alpha; }
    function entry() { return helperAlias() + mutable + declared(); }
    return { init: entry };
  }
  return main();
}
outer();
";

fn function_span(source: &str, needle: &str) -> SourceSpan {
    let start = source.find(needle).expect("needle") as u32;
    // The synthetic function bodies are single-line `{ ... }` blocks.
    let end = start + source[start as usize..].find('}').expect("end") as u32 + 1;
    SourceSpan::new(start, end)
}

fn claim(
    role: &str,
    span: SourceSpan,
    name: &str,
    alias: Option<(&str, &str)>,
) -> FunctionSpanClaim {
    FunctionSpanClaim {
        role: role.to_string(),
        function_span: span,
        proposed_name: name.to_string(),
        alias: alias.map(|(name, role)| (name.to_string(), role.to_string())),
        confidence: Confidence::Medium,
        evidence: vec!["synthetic structural finding".to_string()],
    }
}

#[test]
fn a_function_span_claim_resolves_its_holder_and_alias() {
    let properties = extract_program_properties(SYNTHETIC).expect("properties");
    let span = function_span(SYNTHETIC, "function () { return alpha + 1; }");
    let plan = plan_semantic_names_with_claims(
        SYNTHETIC,
        &properties,
        &[claim(
            "next_value",
            span,
            "computeNextValue",
            Some(("nextValueComputer", "next_value_alias")),
        )],
    )
    .expect("plan");
    let helper = plan
        .candidates
        .iter()
        .find(|candidate| candidate.role == "next_value")
        .unwrap_or_else(|| panic!("{:#?}", plan.skipped));
    assert_eq!(helper.original_name, "helper");
    assert_eq!(helper.proposed_name, "computeNextValue");
    assert_eq!(helper.entity.span, span);
    assert_eq!(helper.entity.id, format!("fn@{}..{}", span.start, span.end));
    let alias = plan
        .candidates
        .iter()
        .find(|candidate| candidate.role == "next_value_alias")
        .expect("alias");
    assert_eq!(alias.original_name, "helperAlias");
    assert_eq!(
        alias.entity.target.as_deref(),
        Some(helper.entity.id.as_str())
    );

    // A declaration resolves to its own name.
    let declared = function_span(SYNTHETIC, "function declared()");
    let plan = plan_semantic_names_with_claims(
        SYNTHETIC,
        &properties,
        &[claim("declared_role", declared, "readAlpha", None)],
    )
    .expect("plan");
    assert_eq!(
        proposed(&plan).get("readAlpha").map(String::as_str),
        Some("declared")
    );
}

#[test]
fn a_function_span_claim_reuses_every_fail_closed_proof() {
    let properties = extract_program_properties(SYNTHETIC).expect("properties");
    let helper = function_span(SYNTHETIC, "function () { return alpha + 1; }");
    let mutable = function_span(SYNTHETIC, "function () { return 2; }");
    let not_a_function = SourceSpan::new(helper.start + 1, helper.end);
    let plan = plan_semantic_names_with_claims(
        SYNTHETIC,
        &properties,
        &[
            claim("mutable_role", mutable, "mutableAction", None),
            claim("nowhere_role", not_a_function, "nowhereAction", None),
            claim("first_role", helper, "firstAction", None),
            claim("second_role", helper, "secondAction", None),
        ],
    )
    .expect("plan");
    let names = names(&plan);
    for name in [
        "mutableAction",
        "nowhereAction",
        "firstAction",
        "secondAction",
    ] {
        assert!(!names.contains(name), "{name}: {names:?}");
    }
    let refused: BTreeSet<&str> = plan.skipped.iter().map(|skip| skip.role.as_str()).collect();
    assert!(refused.contains("mutable_role"), "{:#?}", plan.skipped);
    assert!(refused.contains("nowhere_role"), "{:#?}", plan.skipped);
    assert!(plan
        .ambiguous
        .iter()
        .any(|ambiguity| ambiguity.role == "role_conflict:first_role|second_role"));
}

// ===========================================================================
// CLI
// ===========================================================================

fn run_cli(args: &[&str]) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_shape"))
        .args(args)
        .output()
        .expect("run shape")
}

#[test]
fn cli_plan_and_inventory_print_reports() {
    let dir = std::env::temp_dir().join(format!("shape-naming-unified-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let input = dir.join("program.js");
    std::fs::write(&input, MAIN_CONTAINS_OBSERVABLE).expect("write fixture");
    let input = input.to_str().unwrap();

    let plan = run_cli(&["naming", "plan", input]);
    assert!(
        plan.status.success(),
        "{}",
        String::from_utf8_lossy(&plan.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&plan.stdout).expect("plan json");
    assert_eq!(
        json["source"]["sha256"].as_str().unwrap(),
        shape::source::sha256_hex(MAIN_CONTAINS_OBSERVABLE.as_bytes())
    );
    assert!(json["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .any(|candidate| candidate["proposed_name"] == "createObservableClass"));

    let inventory = run_cli(&["naming", "inventory", input, "--json"]);
    assert!(
        inventory.status.success(),
        "{}",
        String::from_utf8_lossy(&inventory.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&inventory.stdout).expect("inventory");
    assert!(
        json["function_index"]["functions"]
            .as_array()
            .unwrap()
            .len()
            >= 4
    );
    assert!(json["role_graph"]["nodes"].is_array());

    let summary = run_cli(&["naming", "inventory", input]);
    assert!(summary.status.success());
    assert!(String::from_utf8_lossy(&summary.stdout).contains("fn@"));

    let help = run_cli(&["naming", "--help"]);
    let help = String::from_utf8_lossy(&help.stdout);
    for command in ["inventory", "plan", "threads", "collectors"] {
        assert!(help.contains(command), "{command} in help");
    }

    let _ = std::fs::remove_dir_all(&dir);
}
