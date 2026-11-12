use shape::naming::name_threads;
const BODY: &str = include_str!("fixtures/naming/software_float_byte_writer_family/software-float-original-function.js");
const NAME: &str = "$THREAD_5_16368";
const RULE: &str = "shared_software_float_arithmetic_writer";
fn scope(body: &str, prefix: &str, tail: &str) -> String {
    format!("function main() {{ let $KEY_357; {prefix} $KEY_357 = {{FLOAT32:236, DOUBLE64:237}}; {body} {tail} }} main();")
}
fn outcome(source: &str) -> shape::naming::ThreadName {
    name_threads(source, true)
        .expect("negative must parse")
        .report
        .outcomes
        .into_iter()
        .find(|o| o.binding.name == NAME)
        .expect("same target binding")
}
#[test]
fn exact_arithmetic_and_alpha() {
    for body in [
        BODY.to_owned(),
        BODY.replace("$DF", "input")
            .replace("$DG", "output")
            .replace("$DH", "absolute")
            .replace("$DI", "fraction")
            .replace("$DJ", "exponent"),
    ] {
        let o = outcome(&scope(&body, "", &format!("return {NAME};")));
        assert_eq!(o.evaluator.as_deref(), Some(RULE), "{o:?}");
        assert!(o.applied);
    }
}
#[test]
fn altered_complete_protocols_refused() {
    for body in [
        BODY.replace("536870911", "536870912"),
        BODY.replacen("$DI / 536870911", "$DI / 536870912", 1),
        BODY.replace("$DF === 0", "$DF == 0"),
        BODY.replace("1 / $DF < 0", "1 / $DF > 0"),
        BODY.replace("% 2", "% 3"),
        BODY.replace(".5", ".25"),
        BODY.replace("$DJ > 2046", "$DJ >= 2046"),
        BODY.replace("$DJ + 1023 - 127", "$DJ + 1023 - 128"),
        BODY.replace("1 - 127 - 23", "1 - 127 - 24"),
        BODY.replace("$DQ.unshift", "$DQ.push"),
        BODY.replace("$DG.push($DN)", "$DF.push($DN)"),
        BODY.replace("$DR += 8", "$DR += 7"),
        BODY.replace("let $DR = 0;", "sideEffect(); let $DR = 0;"),
        BODY.replace("$KEY_357.FLOAT32", "other.FLOAT32"),
    ] {
        let o = outcome(&scope(&body, "", &format!("return {NAME};")));
        assert_ne!(
            o.evaluator.as_deref(),
            Some(RULE),
            "accepted altered body: {body}"
        );
        assert!(!o.applied, "{o:?}");
    }
}
#[test]
fn capture_epochs_globals_and_reentrancy_refused() {
    for source in [
        scope(BODY, "let Math = foreign;", &format!("return {NAME};")),
        scope(BODY, "", &format!("$KEY_357 = {{FLOAT32:0,DOUBLE64:237}}; return {NAME};")),
        scope(BODY, "", &format!("expose($KEY_357); return {NAME};")),
        scope(BODY, "", &format!("$KEY_357.FLOAT32 = 1; return {NAME};")),
        scope(BODY, &format!("register({NAME});"), &format!("return {NAME};")),
        scope(BODY, &format!("if (condition) {{ {NAME}(0, []); }}"), &format!("return {NAME};")),
        format!("function main() {{ let $KEY_357; if (condition) {{ $KEY_357 = {{FLOAT32:236,DOUBLE64:237}}; }} {BODY} return {NAME}; }} main();"),
        format!("function main() {{ let $KEY_357; register(() => {NAME}(0, [])); $KEY_357 = {{FLOAT32:236,DOUBLE64:237}}; {BODY} return {NAME}; }} main();"),
        format!("function main() {{ let $KEY_357; $KEY_357 = {{FLOAT32:236,DOUBLE64:237}}; let holder = {BODY}; return holder; }} main();"),
    ] {
        let o = outcome(&source);
        assert_ne!(o.evaluator.as_deref(), Some(RULE), "accepted bad capture: {source}");
        assert!(!o.applied, "{o:?}");
    }
}
