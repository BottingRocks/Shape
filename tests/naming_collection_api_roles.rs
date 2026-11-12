use shape::naming::name_threads;

fn named(source: &str, name: &str) -> bool {
    name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .any(|o| o.applied && o.proposed_name.as_deref() == Some(name))
}

fn visibility_named(source: &str) -> bool {
    name_threads(source, true).is_ok_and(|r| {
        r.report.function_roles.iter().any(|role| {
            role.candidates
                .iter()
                .any(|candidate| candidate == "recordVisibilityChangeSample")
                && role
                    .evaluators
                    .iter()
                    .any(|evaluator| evaluator == "shared_anchor_visibility_publish")
        }) && r.report.outcomes.iter().any(|outcome| {
            outcome.applied
                && outcome.proposed_name.as_deref() == Some("recordVisibilityChangeSample")
        })
    })
}

fn unchanged(source: &str) -> bool {
    name_threads(source, true).unwrap().source == source
}

// ---------------------------------------------------------------------------
// reportErrorToConsole
// ---------------------------------------------------------------------------

const CONSOLE: &str =
    "function main(){function $THREAD_9_78434(){console.error(\"no global\");}return $THREAD_9_78434;}main();";

#[test]
fn console_error_report_proves_global_path_and_single_string_argument() {
    for source in [
        CONSOLE.to_string(),
        CONSOLE
            .replace("$THREAD_9_78434", "$THREAD_5_88112_2")
            .replace("\"no global\"", "\"missing host object\""),
    ] {
        assert!(named(&source, "reportErrorToConsole"), "{source}");
    }
    let human = CONSOLE.replace("$THREAD_9_78434", "reportNoGlobal");
    assert!(unchanged(&human));
}

#[test]
fn console_error_report_rejects_changed_api_arguments_and_shapes() {
    for (from, to) in [
        ("console.error", "console.warn"),
        ("console.error", "other.error"),
        ("console.error", "console.log"),
        ("console.error(\"no global\")", "console.error()"),
        ("console.error(\"no global\")", "console.error(messageName)"),
        (
            "console.error(\"no global\")",
            "console.error(\"a\", \"b\")",
        ),
        ("console.error", "console?.error"),
        (
            "console.error(\"no global\");",
            "console.error(\"no global\");return void 0;",
        ),
        (
            "console.error(\"no global\");",
            "return console.error(\"no global\");",
        ),
        ("(){console.error", "(value){console.error"),
    ] {
        assert!(
            !named(&CONSOLE.replace(from, to), "reportErrorToConsole"),
            "{from} -> {to}"
        );
    }
    let shadowed = CONSOLE.replace(
        "function main(){",
        "function main(){let console={error:function(){}};",
    );
    assert!(!named(&shadowed, "reportErrorToConsole"));
}

// ---------------------------------------------------------------------------
// syncCapturedFieldsIfChanged
// ---------------------------------------------------------------------------

const SYNC: &str = r#"function main(){let target={a:1,b:2};let first=3;let second=4;
function $THREAD_1_42949(){if(target.a!==first){target.a=first;}if(target.b!==second){target.b=second;}}
return $THREAD_1_42949;}main();"#;

#[test]
fn field_sync_proves_receiver_keys_values_and_ordered_guards() {
    for source in [
        SYNC.to_string(),
        SYNC.replace("$THREAD_1_42949", "$THREAD_3_71234_9")
            .replace("target", "shared")
            .replace("first", "savedOne")
            .replace("second", "savedTwo"),
    ] {
        assert!(named(&source, "syncCapturedFieldsIfChanged"), "{source}");
    }
    // Computed keys with a captured key binding prove the same operation.
    let computed = r#"function main(){let target={};let saved=7;let field="count";
function $THREAD_1_9(){if(target[field]!==saved){target[field]=saved;}}
return $THREAD_1_9;}main();"#;
    assert!(named(computed, "syncCapturedFieldsIfChanged"));
    let human = SYNC.replace("$THREAD_1_42949", "restoreCachedFields");
    assert!(unchanged(&human));
}

#[test]
fn field_sync_refuses_mismatched_keys_receivers_and_origins() {
    for (from, to) in [
        (
            "if(target.a!==first){target.a=first;}",
            "if(target.a!==first){target.b=first;}",
        ),
        (
            "if(target.a!==first){target.a=first;}",
            "if(target.a!==first){target.a=second;}",
        ),
        (
            "if(target.a!==first){target.a=first;}",
            "if(target.a!==first){other.a=first;}",
        ),
        (
            "if(target.a!==first){target.a=first;}",
            "if(target.a!==first){target.a=first;}else{target.a=first;}",
        ),
        (
            "if(target.a!==first){target.a=first;}",
            "if(target.a!=first){target.a=first;}",
        ),
        (
            "if(target.a!==first){target.a=first;}",
            "if(target.a===first){target.a=first;}",
        ),
        ("if(target.a!==first){target.a=first;}", "target.a=first;"),
        (
            "if(target.a!==first){target.a=first;}",
            "if(target.a!==first){target.a=first;target.b=second;}",
        ),
        (
            "if(target.b!==second){target.b=second;}",
            "if(target.a!==second){target.a=second;}",
        ),
        (
            "function $THREAD_1_42949()",
            "function $THREAD_1_42949(first)",
        ),
    ] {
        assert!(
            !named(&SYNC.replace(from, to), "syncCapturedFieldsIfChanged"),
            "{from} -> {to}"
        );
    }
    let redeclared = SYNC.replace("let target={a:1,b:2};", "let target={a:1,b:2};var target;");
    assert!(!named(&redeclared, "syncCapturedFieldsIfChanged"));
}

// ---------------------------------------------------------------------------
// recordVisibilityChangeSample
// ---------------------------------------------------------------------------

const VISIBILITY: &str = r#"function main(){let counter=0;let stream=createStream();let active=false;let sink=getSink();function clock(){return 0;}
function $THREAD_0_144796(event){try{++counter;let target=stream;let put=stream.put;put.call(target,{visible:event.target.visibilityState==="visible",timestamp:clock()});active=true;}catch(error){sink.error(error);}}
return $THREAD_0_144796;}main();"#;

#[test]
fn visibility_publish_proves_stream_receiver_state_and_error_sink() {
    for source in [
        VISIBILITY.to_string(),
        VISIBILITY
            .replace("$THREAD_0_144796", "$THREAD_4_5510_3")
            .replace("event", "evt")
            .replace("counter", "seenCount")
            .replace("active", "notified"),
        VISIBILITY.replace(
            "{visible:event.target.visibilityState===\"visible\",timestamp:clock()}",
            "{timestamp:clock(),visible:event.target.visibilityState===\"visible\"}",
        ),
    ] {
        assert!(visibility_named(&source), "{source}");
    }
    let human = VISIBILITY.replace("$THREAD_0_144796", "publishVisibilityState");
    assert!(unchanged(&human));
}

#[test]
fn visibility_publish_refuses_changed_state_operations_and_origins() {
    for (from, to) in [
        ("visibilityState", "hiddenState"),
        ("===\"visible\"", "===\"hidden\""),
        ("visible:event", "shown:event"),
        ("put.call", "put.apply"),
        ("timestamp:clock()", "timestamp:clock(1)"),
        ("timestamp:clock()", "timestamp:clock"),
        ("++counter;", ""),
        ("sink.error", "sink.warn"),
        ("sink.error(error)", "sink.error(counter)"),
        ("active=true", "active=false"),
        ("active=true", "active=1"),
        ("let put=stream.put;", "let put=other.put;"),
        (
            "function $THREAD_0_144796(event)",
            "function $THREAD_0_144796(event=other)",
        ),
        (
            "active=true;}catch",
            "active=true;if(counter>1){return;}}catch",
        ),
        ("catch(error)", "catch({error})"),
    ] {
        assert!(
            !visibility_named(&VISIBILITY.replace(from, to)),
            "{from} -> {to}"
        );
    }
}

#[test]
fn existing_readable_name_and_sibling_collision_are_preserved() {
    // A readable existing binding reserves its spelling; the generated claim
    // never reuses it.
    let collision = VISIBILITY.replace(
        "function main(){",
        "function main(){let recordVisibilityChangeSample=1;use(recordVisibilityChangeSample);",
    );
    assert!(!visibility_named(&collision));
    // Independent unrelated functions keep their readable names.
    let preserved = r#"function main(){function readableOne(){return 1;}function readableTwo(){return 2;}return [readableOne,readableTwo];}main();"#;
    assert!(unchanged(preserved));
}

// ---------------------------------------------------------------------------
// The timestamp clock must be an ordered, plain, single-origin callable
// ---------------------------------------------------------------------------

#[test]
fn visibility_publish_requires_an_ordered_callable_clock() {
    let clock = "function clock(){return 0;}";
    let cases = [
        // Initializer inside an if-block (not a FunctionBody/Program container).
        VISIBILITY.replace(clock, "if(enabled){var clock=function(){return 0;};}"),
        // Initializer assigned only inside a setup callback (foreign container).
        VISIBILITY.replace(
            clock,
            "let clock;function setup(){clock=function(){return 0;};}",
        ),
        // Initializer inside a loop body.
        VISIBILITY.replace(
            clock,
            "for(let i=0;i<1;i++){var clock=function(){return 0;};}",
        ),
        // Resource binding as the callable origin.
        VISIBILITY.replace(clock, "using clock=makeClock();"),
        // Initializer appears after the call site.
        VISIBILITY.replace(clock, "").replace(
            "return $THREAD_0_144796;",
            "var clock=function(){return 0;};return $THREAD_0_144796;",
        ),
        // Sole assignment appears after the call site.
        VISIBILITY.replace(clock, "let clock;").replace(
            "return $THREAD_0_144796;",
            "clock=function(){return 0;};return $THREAD_0_144796;",
        ),
    ];
    for source in cases {
        assert!(!visibility_named(&source), "{source}");
    }
}

// ---------------------------------------------------------------------------
// Computed keys must have stable literal origins, canonicalized to
// their ToPropertyKey, so same-value and [1]/["1"] collisions are refused
// ---------------------------------------------------------------------------

#[test]
fn field_sync_requires_literal_origin_keys_and_refuses_collisions() {
    // Distinct literal-origin computed keys still prove the operation.
    let distinct = r#"function main(){let target={};let saved1=1;let saved2=2;let keyA="a",keyB="b";
function $THREAD_1_3(){if(target[keyA]!==saved1){target[keyA]=saved1;}if(target[keyB]!==saved2){target[keyB]=saved2;}}
return $THREAD_1_3;}main();"#;
    assert!(named(distinct, "syncCapturedFieldsIfChanged"), "{distinct}");

    // A computed key binding without a literal initializer proves no key.
    let dynamic = r#"function main(){let target={};let saved=7;let field=getKey();
function $THREAD_1_9(){if(target[field]!==saved){target[field]=saved;}}
return $THREAD_1_9;}main();"#;
    // A reassigned key binding is not stable.
    let reassigned = r#"function main(){let target={};let saved=7;let field="count";field=other;
function $THREAD_1_10(){if(target[field]!==saved){target[field]=saved;}}
return $THREAD_1_10;}main();"#;
    // Two bindings holding the same property name are not distinct.
    let same_value = r#"function main(){let target={a:0};let saved1=1,saved2=2;let key1="a",key2="a";
function $THREAD_1_11(){if(target[key1]!==saved1){target[key1]=saved1;}if(target[key2]!==saved2){target[key2]=saved2;}}
return $THREAD_1_11;}main();"#;
    // Numeric and string spellings of the same key collide.
    let numeric_collision = r#"function main(){let target={};let saved1=1,saved2=2;
function $THREAD_1_12(){if(target[1]!==saved1){target[1]=saved1;}if(target["1"]!==saved2){target["1"]=saved2;}}
return $THREAD_1_12;}main();"#;
    // A literal key declared after the function that reads it has no
    // initialization-before-reference proof.
    let future = r#"function main(){let target={};let saved=7;
function $THREAD_1_13(){if(target[field]!==saved){target[field]=saved;}}
let field="count";return $THREAD_1_13;}main();"#;
    // A literal key in a conditional block does not dominate the use site.
    let conditional = r#"function main(){let target={};let saved=7;if(true){var field="count";}
function $THREAD_1_14(){if(target[field]!==saved){target[field]=saved;}}
return $THREAD_1_14;}main();"#;
    for (label, source) in [
        ("dynamic", dynamic),
        ("reassigned", reassigned),
        ("same_value", same_value),
        ("numeric_collision", numeric_collision),
        ("future", future),
        ("conditional", conditional),
    ] {
        assert!(
            !named(source, "syncCapturedFieldsIfChanged"),
            "{label}: {source}"
        );
    }
}

// ---------------------------------------------------------------------------
// Resource (using / await using) bindings are never accepted origins
// ---------------------------------------------------------------------------

#[test]
fn visibility_publish_refuses_resource_bindings_as_origins() {
    for (label, (from, to)) in [
        (
            "captured receiver",
            ("let stream=createStream();", "using stream=createStream();"),
        ),
        (
            "local alias",
            ("let target=stream;", "using target=stream;"),
        ),
        (
            "local callee",
            ("let put=stream.put;", "using put=stream.put;"),
        ),
    ] {
        let source = VISIBILITY.replace(from, to);
        assert!(!visibility_named(&source), "{label}: {source}");
    }
}

// ---------------------------------------------------------------------------
// An unwritten plain computed-key declaration without an initializer has
// the proven property name "undefined"; distinctness is by property name, never
// by SymbolId, so two uninitialized keys collide
// ---------------------------------------------------------------------------

#[test]
fn field_sync_accepts_uninitialized_captured_keys_with_undefined_identity() {
    // The actual two-guard shape: one static `ns_collect` key and one computed
    // key declared `let key;` with no initializer and no writes anywhere.
    let actual = r#"function main(){let $KEY_3={ns_collect:0};let $KEY_6=1;let $KEY_1=2;let $KEY_261;
function $THREAD_0_129852(){if($KEY_3.ns_collect!==$KEY_6){$KEY_3.ns_collect=$KEY_6;}if($KEY_3[$KEY_261]!==$KEY_1){$KEY_3[$KEY_261]=$KEY_1;}}
return $THREAD_0_129852;}main();"#;
    assert!(named(actual, "syncCapturedFieldsIfChanged"), "{actual}");

    // The same proof is spelling-independent: the uninitialized key may be
    // named `alpha` and every other symbol may be renamed.
    let alpha = r#"function main(){let shared={count:0};let first=1;let last=2;let alpha;
function $THREAD_2_7(){if(shared.count!==first){shared.count=first;}if(shared[alpha]!==last){shared[alpha]=last;}}
return $THREAD_2_7;}main();"#;
    assert!(named(alpha, "syncCapturedFieldsIfChanged"), "{alpha}");

    // Old literal-origin keys keep proving their decoded ToPropertyKey name and
    // coexist with a new uninitialized key: "ns_collect", "count" and
    // "undefined" are three distinct properties.
    let literalbase = r#"function main(){let target={ns_collect:0};let first=1;let second=2;let third=3;let literalbase="count";let alpha;
function $THREAD_3_11(){if(target.ns_collect!==first){target.ns_collect=first;}if(target[literalbase]!==second){target[literalbase]=second;}if(target[alpha]!==third){target[alpha]=third;}}
return $THREAD_3_11;}main();"#;
    assert!(
        named(literalbase, "syncCapturedFieldsIfChanged"),
        "{literalbase}"
    );
}

#[test]
fn field_sync_refuses_uninitialized_key_uncertainty_and_undefined_collisions() {
    let base = r#"function main(){let target={ns_collect:0};let savedOne=1;let savedTwo=2;let key;
function $THREAD_0_129852(){if(target.ns_collect!==savedOne){target.ns_collect=savedOne;}if(target[key]!==savedTwo){target[key]=savedTwo;}}
return $THREAD_0_129852;}main();"#;
    assert!(named(base, "syncCapturedFieldsIfChanged"), "{base}");

    // A second uninitialized key has the same ToPropertyKey value `undefined`
    // even though it is a different SymbolId.
    let duplicate_undefined = r#"function main(){let target={};let savedOne=1;let savedTwo=2;let keyA,keyB;
function $THREAD_0_129852(){if(target[keyA]!==savedOne){target[keyA]=savedOne;}if(target[keyB]!==savedTwo){target[keyB]=savedTwo;}}
return $THREAD_0_129852;}main();"#;
    // A literal "undefined" names the same property as an uninitialized key.
    let literal_undefined = r#"function main(){let target={};let savedOne=1;let savedTwo=2;let key;let literal="undefined";
function $THREAD_0_129852(){if(target[key]!==savedOne){target[key]=savedOne;}if(target[literal]!==savedTwo){target[literal]=savedTwo;}}
return $THREAD_0_129852;}main();"#;
    // A static `.undefined` write is the same property name again.
    let static_undefined = r#"function main(){let target={};let savedOne=1;let savedTwo=2;let key;
function $THREAD_0_129852(){if(target.undefined!==savedOne){target.undefined=savedOne;}if(target[key]!==savedTwo){target[key]=savedTwo;}}
return $THREAD_0_129852;}main();"#;
    // The key binding is written, so its value is not the proven `undefined`.
    let reassigned = base.replace("let key;", "let key;key=savedOne;");
    // A formal parameter is not a plain declaration before the site.
    let parameter = base.replace(
        "function $THREAD_0_129852()",
        "function $THREAD_0_129852(key)",
    );
    // An inner declaration shadows the uninitialized outer binding.
    let shadowed = base.replace(
        "function $THREAD_0_129852(){",
        "function $THREAD_0_129852(){let key=getKey();",
    );
    // The write goes to a different receiver than the read.
    let wrong_receiver = base
        .replace(
            "let target={ns_collect:0};",
            "let target={ns_collect:0};let other={};",
        )
        .replace("{target[key]=savedTwo;}", "{other[key]=savedTwo;}");
    // The assigned value is not the guarded saved binding.
    let wrong_value = base.replace("{target[key]=savedTwo;}", "{target[key]=savedOne;}");
    // A conditional declaration does not dominate the use site.
    let nested = base.replace("let key;", "if(true){var key;}");
    // A non-literal initializer is still an unknown computed field.
    let dynamic = base.replace("let key;", "let key=getKey();");

    for (label, source) in [
        ("duplicate_undefined", duplicate_undefined),
        ("literal_undefined", literal_undefined),
        ("static_undefined", static_undefined),
        ("reassigned", &reassigned),
        ("parameter", &parameter),
        ("shadowed", &shadowed),
        ("wrong_receiver", &wrong_receiver),
        ("wrong_value", &wrong_value),
        ("nested", &nested),
        ("dynamic", &dynamic),
    ] {
        assert!(
            !named(&source, "syncCapturedFieldsIfChanged"),
            "{label}: {source}"
        );
    }
}
