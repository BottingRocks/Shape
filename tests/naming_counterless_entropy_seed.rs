use shape::naming::name_threads;
#[path = "fixtures/counterless-entropy-witnesses.rs"]
mod fixtures;
const ROLE: &str = "deriveSignalCodecSeed";
const EVALUATOR: &str = "encoding_counterless_entropy_seed";

fn check(function: &str, setup: &str, expected: bool) {
    let name = function
        .trim_start()
        .strip_prefix("function ")
        .unwrap()
        .split('(')
        .next()
        .unwrap();
    let source = format!("function bootstrap(){{let carrierA=1;let carrierB=2;let carrierC=3;let carrierD=4;{setup}{function}return [carrierA,carrierB,carrierC,carrierD,{name}];}}bootstrap();");
    let result = name_threads(&source, true).expect("static fixture analyzes");
    let outcome = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == name)
        .unwrap();
    if expected {
        assert!(outcome.applied, "{outcome:?}");
        assert_eq!(outcome.role_name.as_deref(), Some(ROLE));
        assert_eq!(outcome.evaluator.as_deref(), Some(EVALUATOR));
    } else {
        assert_ne!(outcome.evaluator.as_deref(), Some(EVALUATOR), "{outcome:?}");
        assert!(
            !(outcome.applied && outcome.role_name.as_deref() == Some(ROLE)),
            "unexpected seed role: {outcome:?}"
        );
        assert!(!result
            .report
            .function_roles
            .iter()
            .filter(|r| r.current_name.as_deref() == Some(name))
            .any(|r| r.evaluators.iter().any(|e| e == EVALUATOR)));
    }
}
fn ternary() -> &'static str {
    fixtures::WITNESSES[1].1
}
#[test]
fn nine_existing_if_and_ternary_bodies() {
    for (seed, body) in fixtures::WITNESSES {
        check(body, "", *seed != "null-guarded-clock-reassignment");
    }
}
#[test]
fn readable_alpha_and_commuted_xor_variants() {
    let alpha = ternary()
        .replace("$KEY_3", "randomWord")
        .replace("$KEY_5", "clockWord")
        .replace("$KEY_4", "dateWord")
        .replace("$KEY_1", "cryptoWord");
    check(&alpha, "", true);
    check(
        &alpha
            .replace("randomWord ^ clockWord", "clockWord ^ randomWord")
            .replace("clockWord ^ dateWord", "dateWord ^ clockWord")
            .replace("dateWord ^ cryptoWord", "cryptoWord ^ dateWord")
            .replace("randomWord ^ 1103515245", "1103515245 ^ randomWord")
            .replace("Math.random() * 4294967296", "4294967296 * Math.random()"),
        "",
        true,
    );
}
#[test]
fn every_host_root_must_be_unshadowed() {
    for root in [
        "Math",
        "performance",
        "Number",
        "Date",
        "crypto",
        "Uint32Array",
    ] {
        check(ternary(), &format!("let {root}=foreign;"), false);
    }
}
#[test]
fn source_operation_near_misses() {
    for (from, to) in [
        ("4294967296", "4294967295"),
        ("1103515245", "1103515246"),
        ("new Uint32Array(1)", "new Uint32Array(2)"),
        ("Uint32Array", "Int32Array"),
        ("))[0]", "))[1]"),
        ("performance.now()", "other.now()"),
        ("performance.now()", "performance.now?.()"),
        ("Math.random()", "Math?.random()"),
        ("Number(new Date())", "Number(Date())"),
        ("catch (e) {}", "catch (e) { throw e; }"),
        ("catch (e) {}", "catch ({x=effect()}) {}"),
        ("let $KEY_4 = Number", "const $KEY_4 = Number"),
    ] {
        assert!(ternary().contains(from), "missing mutation {from}");
        check(&ternary().replacen(from, to, 1), "", false);
    }
}
#[test]
fn disconnected_or_wrong_final_mix_refuses() {
    for (from, to) in [
        ("$KEY_3 ^ $KEY_5", "$KEY_3 ^ foreign"),
        ("$KEY_5 === $KEY_5", "$KEY_5 === $KEY_4"),
        ("$KEY_5 ^ $KEY_4", "$KEY_3 ^ $KEY_4"),
        ("$KEY_3;\n", "foreign;\n"),
        ("($KEY_4 ^ $KEY_1)", "($KEY_3 ^ $KEY_1)"),
        ("($KEY_4 ^ $KEY_1)", "($KEY_4 ^ $KEY_1 ^ counter)"),
        ("return ($KEY_4", "extra(); return ($KEY_4"),
    ] {
        assert!(ternary().contains(from), "missing mutation {from}");
        check(&ternary().replacen(from, to, 1), "", false);
    }
}
#[test]
fn if_merge_wrong_branch_or_foreign_target_refuses() {
    let body = fixtures::WITNESSES[0].1;
    for (from, to) in [
        ("$KEY_0 = $KEY_0 ^ $KEY_2", "$KEY_5 = $KEY_0 ^ $KEY_2"),
        ("$KEY_2 === $KEY_2", "$KEY_2 !== $KEY_2"),
        ("if ($KEY_2 === $KEY_2)", "if (foreign === foreign)"),
    ] {
        assert!(body.contains(from));
        check(&body.replacen(from, to, 1), "", false);
    }
}
#[test]
fn nested_donors_and_owned_shadowing_refuse() {
    let nested = ternary().replace(
        "$KEY_5 = performance.now() >>> 0;",
        "function donor(){ $KEY_5 = performance.now() >>> 0; } donor();",
    );
    check(&nested, "", false);
    let shadow = ternary().replace(
        "$KEY_5 = performance.now() >>> 0;",
        "let $KEY_5=performance.now() >>> 0;",
    );
    check(&shadow, "", false);
    let swapped = ternary().replace(
        "$KEY_1 = crypto.getRandomValues",
        "$KEY_5 = crypto.getRandomValues",
    );
    check(&swapped, "", false);
}
#[test]
fn signatures_return_width_and_extra_effects_refuse() {
    check(&ternary().replacen("() {", "(input) {", 1), "", false);
    check(&ternary().replacen("() {", "(...rest) {", 1), "", false);
    check(
        &ternary().replace(
            "return ($KEY_4 ^ $KEY_1) >>> 0;",
            "return ($KEY_4 ^ $KEY_1) | 0;",
        ),
        "",
        false,
    );
    check(
        &ternary().replacen("() {", "() { external();", 1),
        "",
        false,
    );
}
#[test]
fn captured_counter_variant_keeps_existing_evaluator() {
    let function = ternary().replace(
        "return ($KEY_4 ^ $KEY_1) >>> 0;",
        "++counter;return ($KEY_4 ^ $KEY_1 ^ counter) >>> 0;",
    );
    let source = format!(
        "function bootstrap(){{let counter=0;{function}return $THREAD_1_80340;}}bootstrap();"
    );
    let r = name_threads(&source, true).unwrap();
    let o = r
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_1_80340")
        .unwrap();
    assert!(o.applied, "{o:?}");
    assert_eq!(o.role_name.as_deref(), Some(ROLE));
    assert_eq!(o.evaluator.as_deref(), Some("posthost_entropy_seed"));
}
