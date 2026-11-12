use shape::naming::name_threads;
const ROLE: &str = "isLowercaseAsciiLetterDigitOrHyphen";
const EVALUATOR: &str = "shared_ascii_label_predicate";
const DIRECT: &str = r#"function main(){
let code=function(text){return text.charCodeAt();};
let accessor=code;
let digitLow=accessor("0"),digitHigh=accessor("9"),letterLow=accessor("a"),letterHigh=accessor("z");
function $THREAD_1_7(value){return value>=digitLow && value<=digitHigh || value>=letterLow && value<=letterHigh || value===45;}
return $THREAD_1_7;
}main();"#;
const LATE: &str = r#"function main(){
let digitLow,digitHigh,letterLow,letterHigh,accessor;
function $THREAD_1_7(value){return value>=digitLow && value<=digitHigh || value>=letterLow && value<=letterHigh || value===45;}
let code=(function(text){return text.charCodeAt();});
accessor=code;
letterLow=accessor("a");letterHigh=accessor("z");digitLow=accessor("0");digitHigh=accessor("9");
return $THREAD_1_7;
}main();"#;
fn claim(source: &str) -> bool {
    name_threads(source, true)
        .expect("fixture must parse")
        .report
        .outcomes
        .iter()
        .any(|o| {
            o.applied
                && o.role_name.as_deref() == Some(ROLE)
                && o.evaluator.as_deref() == Some(EVALUATOR)
        })
}
#[test]
fn bound_character_accessor_and_late_setup_variants() {
    for source in [
        DIRECT.to_string(),
        LATE.to_string(),
        LATE.replace("digitLow", "lowerDigit")
            .replace("letterHigh", "upperLetter")
            .replace("value", "unit")
            .replace("code", "unitAccessor")
            .replace("$THREAD_1_7", "$THREAD_91_904_2"),
        DIRECT.replace(
            "function(text){return text.charCodeAt();}",
            "text=>text.charCodeAt()",
        ),
        DIRECT.replace(
            "function(text){return text.charCodeAt();}",
            "text=>{return text.charCodeAt();}",
        ),
        DIRECT.replace("let accessor=code;", "const alias=code;let accessor=alias;"),
        DIRECT
            .replace("return value>=", "return (value)>=")
            .replace("value===45", "(value)===(45)"),
        DIRECT.replace(
            "return $THREAD_1_7;",
            "function sibling(){let digitLow=999;return digitLow;}return $THREAD_1_7;",
        ),
    ] {
        assert!(claim(&source), "{source}");
    }
}
#[test]
fn rejects_wrong_dataflow_operators_endpoints_and_effects() {
    for (from, to) in [
        ("value>=digitLow", "other>=digitLow"),
        ("value<=letterHigh", "other<=letterHigh"),
        ("value===45", "other===45"),
        ("value>=digitLow", "value>digitLow"),
        ("value<=letterHigh", "value<letterHigh"),
        ("value===45", "value==45"),
        ("value===45", "value===46"),
        ("accessor(\"a\")", "accessor(\"A\")"),
        ("accessor(\"z\")", "accessor(\"Z\")"),
        ("accessor(\"0\")", "accessor(\"1\")"),
        ("accessor(\"9\")", "accessor(\"8\")"),
        ("accessor(\"0\")", "accessor(...other)"),
        ("accessor(\"0\")", "accessor(\"0\",1)"),
        ("text.charCodeAt()", "other.charCodeAt()"),
        ("text.charCodeAt()", "text.charCodeAt(1)"),
        ("text.charCodeAt()", "text.codePointAt()"),
        ("text.charCodeAt()", "text.charCodeAt?.()"),
        (
            "return text.charCodeAt();",
            "evil();return text.charCodeAt();",
        ),
        ("function(text)", "function(text=other)"),
        ("function(text)", "function(text,...rest)"),
        (
            "function $THREAD_1_7(value)",
            "async function $THREAD_1_7(value)",
        ),
        (
            "function $THREAD_1_7(value)",
            "function $THREAD_1_7(value=1)",
        ),
        (
            "function $THREAD_1_7(value)",
            "function $THREAD_1_7(value,...rest)",
        ),
        ("{return value>=", "{value=other;return value>="),
        ("{return value>=", "{evil();return value>="),
        ("let accessor=code;", "let accessor=code;accessor=other;"),
        ("return $THREAD_1_7;", "digitLow=other;return $THREAD_1_7;"),
    ] {
        let source = DIRECT.replace(from, to);
        assert_ne!(source, DIRECT);
        assert!(!claim(&source), "{source}");
    }
}
#[test]
fn refuses_nested_and_conditional_origins_and_unrelated_callable_donors() {
    for (from, to) in [
        (
            "digitLow=accessor(\"0\");",
            "function donor(){digitLow=accessor(\"0\");}",
        ),
        (
            "digitLow=accessor(\"0\");",
            "if(other){digitLow=accessor(\"0\");}",
        ),
        ("accessor=code;", "function donor(){accessor=code;}"),
        ("accessor=code;", "if(other){accessor=code;}"),
        (
            "return text.charCodeAt();",
            "function donor(){return text.charCodeAt();}return 0;",
        ),
        (
            "let code=(function(text){return text.charCodeAt();});",
            "function donor(){let code=function(text){return text.charCodeAt();};}let code=other;",
        ),
        (
            "accessor=code;",
            "accessor=other;function donor(){return code;}",
        ),
        ("accessor=code;", "accessor=code;accessor=other;"),
    ] {
        let source = LATE.replace(from, to);
        assert_ne!(source, LATE);
        assert!(!claim(&source), "{source}");
    }
    let foreign = LATE
        .replace(
            "digitLow=accessor(\"0\");",
            "digitLow=otherAccessor(\"0\");",
        )
        .replace(
            "let code=",
            "let otherAccessor=function(text){return text.charCodeAt();};let code=",
        );
    assert!(
        !claim(&foreign),
        "four endpoints must share one exact callable"
    );
    let nested = LATE
        .replace(
            "function $THREAD_1_7(value){return",
            "function $THREAD_1_7(value){function donor(){return",
        )
        .replace("value===45;}\n", "value===45;}return donor;}\n");
    assert!(!claim(&nested));
}
