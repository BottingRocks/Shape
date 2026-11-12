use shape::naming::name_threads;

// Portable complete constructor; intentionally no encoding table, extend
// sibling or installer. No captured seed source is needed for this contract.
const SOURCE: &str = r#"function host(){
let self;
function $THREAD_1_10(){"use strict";
if(this instanceof self){this.extensions=Object.create(null);return void 0;}
else{throw new TypeError("Cannot call a class as a function");}}
self=$THREAD_1_10;
return self;
}host();"#;
const ROLE: &str = "ExtensionRegistry";

fn check(source: &str, expected: bool) {
    let allocator = oxc_allocator::Allocator::default();
    let parsed =
        oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::unambiguous()).parse();
    assert!(
        !parsed.panicked && parsed.errors.is_empty(),
        "{:?}\n{source}",
        parsed.errors
    );
    let result = name_threads(source, true).expect("every fixture must build a valid Model");
    let outcome = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_1_10")
        .expect("fixture target must be inventoried");
    assert_eq!(
        outcome.applied && outcome.proposed_name.as_deref() == Some(ROLE),
        expected,
        "{source}\n{outcome:?}"
    );
    if expected {
        assert!(outcome
            .explanation
            .iter()
            .any(|e| e.contains("complete own constructor")));
        let mut replay = source.to_string();
        for edit in result.report.edits.iter().rev() {
            replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
        }
        assert_eq!(replay, result.source);
        assert_eq!(
            name_threads(&result.source, true).unwrap().source,
            result.source
        );
        assert!(result
            .source
            .contains("this.extensions=Object.create(null)"));
    }
}

#[test]
fn proves_own_body_and_ordered_self_capture_without_donor_methods() {
    for source in [
        SOURCE.to_string(),
        SOURCE.replace("self", "capturedClass"),
        SOURCE.replace("let self;", "var self;"),
        SOURCE
            .replace("let self;", "")
            .replace("self=$THREAD_1_10;", "const self=$THREAD_1_10;"),
        SOURCE
            .replace("let self;", "")
            .replace("self=$THREAD_1_10;", "let self=$THREAD_1_10;"),
        SOURCE.replace("let self;", "let self; function irrelevant(){ return 3; }"),
        SOURCE.replace("return self;", "return new self();"),
    ] {
        check(&source, true);
    }
}

#[test]
fn rejects_real_behavior_changes_in_valid_models() {
    for (from, to) in [
        ("Object.create(null)", "Object.create({})"),
        ("Object.create(null)", "Object.create(null,{})"),
        ("Object.create(null)", "Object.assign(null)"),
        ("this.extensions", "this.other"),
        ("this.extensions=", "other.extensions="),
        ("this.extensions=", "this.extensions||="),
        ("return void 0", "return this"),
        ("return void 0", "return void sideEffect()"),
        ("throw new TypeError", "throw new Error"),
        ("Cannot call a class as a function", "Other error"),
        ("this instanceof self", "other instanceof self"),
        ("this instanceof self", "this instanceof other"),
        ("function $THREAD_1_10()", "function $THREAD_1_10(unused)"),
        ("function $THREAD_1_10()", "async function $THREAD_1_10()"),
        ("function $THREAD_1_10()", "function* $THREAD_1_10()"),
        ("\"use strict\";", ""),
        ("return void 0;", "sideEffect();return void 0;"),
        ("let self;", "let self; const Object={create:x=>x};"),
        ("let self;", "let self; const TypeError=OtherError;"),
    ] {
        assert!(SOURCE.contains(from));
        check(&SOURCE.replace(from, to), false);
    }
}

#[test]
fn refuses_mutable_conditional_nested_shadowed_or_early_self_origins() {
    for source in [
        SOURCE.replace("self=$THREAD_1_10;", "self=$THREAD_1_10;self=other;"),
        SOURCE.replace("self=$THREAD_1_10;", "if(flag){self=$THREAD_1_10;}"),
        SOURCE.replace(
            "self=$THREAD_1_10;",
            "function dormant(){self=$THREAD_1_10;}",
        ),
        SOURCE.replace("self=$THREAD_1_10;", "$THREAD_1_10();self=$THREAD_1_10;"),
        SOURCE.replace(
            "self=$THREAD_1_10;",
            "expose($THREAD_1_10);self=$THREAD_1_10;",
        ),
        SOURCE.replace("let self;", "let self=other;"),
        SOURCE.replace("\"use strict\";", "\"use strict\";let self=other;"),
        SOURCE.replace("function host(){\nlet self;", "function host(self){"),
        SOURCE.replace(
            "self=$THREAD_1_10;",
            "function sibling(){let self=$THREAD_1_10;}",
        ),
    ] {
        check(&source, false);
    }
}
