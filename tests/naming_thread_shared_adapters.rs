use shape::naming::name_threads;

const BATCH: &str = r#"function main(){let callbacks=[],buffer={},registry={},subscriptions=[];
function $THREAD_7_123(){callbacks.forEach(item=>item());buffer.empty();registry.clear();subscriptions.forEach(item=>item.teardown());}
return $THREAD_7_123;}main();"#;

fn named(source: &str, target: &str, name: &str) -> bool {
    name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .any(|o| o.binding.name == target && o.applied && o.proposed_name.as_deref() == Some(name))
}

#[test]
fn teardown_proves_both_owned_callbacks_and_ordered_cleanup() {
    for s in [
        BATCH.to_string(),
        BATCH
            .replace("$THREAD_7_123", "$THREAD_912_99881_2")
            .replace("callbacks", "$KEY_8")
            .replace("item", "entry"),
        BATCH
            .replace("item=>item()", "item=>{item();}")
            .replace("item=>item.teardown()", "function(item){item.teardown();}"),
    ] {
        let target = if s.contains("$THREAD_912") {
            "$THREAD_912_99881_2"
        } else {
            "$THREAD_7_123"
        };
        assert!(named(&s, target, "teardownSubscriptions"), "{s}");
    }
}

#[test]
fn teardown_rejects_api_argument_return_and_callback_changes() {
    for (from, to) in [
        ("callbacks.forEach", "callbacks.map"),
        ("buffer.empty()", "buffer.clear()"),
        ("registry.clear()", "registry.empty()"),
        ("item.teardown()", "item.unsubscribe()"),
        ("item=>item()", "item=>other()"),
        ("item=>item()", "item=>item(1)"),
        ("item=>item()", "item=>{return item();}"),
        ("item=>item()", "async item=>item()"),
        ("item=>item()", "(item=other)=>item()"),
        ("item=>item()", "(...item)=>item()"),
        ("item=>item()", "({item})=>item()"),
        ("item=>item()", "item=>{item=other;item();}"),
        ("item=>item.teardown()", "item=>other.teardown()"),
        ("item=>item.teardown()", "item=>{let item;item.teardown();}"),
        (
            "buffer.empty();registry.clear();",
            "registry.clear();buffer.empty();",
        ),
        ("buffer.empty()", "buffer.empty(1)"),
        ("buffer.empty()", "buffer?.empty()"),
        ("registry.clear()", "registry.clear?.()"),
        ("registry.clear()", "return registry.clear()"),
        ("subscriptions.forEach", "callbacks.forEach"),
        ("registry.clear()", "buffer.clear()"),
        ("function $THREAD_7_123()", "function $THREAD_7_123(extra)"),
        (
            "function $THREAD_7_123()",
            "function $THREAD_7_123(...extra)",
        ),
        ("function $THREAD_7_123()", "async function $THREAD_7_123()"),
    ] {
        let s = BATCH.replace(from, to);
        assert!(
            !named(&s, "$THREAD_7_123", "teardownSubscriptions"),
            "{from} -> {to}"
        );
    }
}

#[test]
fn teardown_rejects_redeclared_local_and_unresolved_receivers() {
    for s in [
        BATCH
            .replace("let callbacks=[],", "var callbacks=[],")
            .replace("return $THREAD_7_123", "var callbacks;return $THREAD_7_123"),
        BATCH.replace(
            "(){callbacks.forEach",
            "(){let callbacks=[];callbacks.forEach",
        ),
        BATCH.replace(
            "let callbacks=[],buffer={},registry={},subscriptions=[];",
            "",
        ),
    ] {
        assert!(!named(&s, "$THREAD_7_123", "teardownSubscriptions"), "{s}");
    }
}

fn wrapper(origin: &str, extra: &str, params: &str) -> String {
    BATCH
        .replace(
            "return $THREAD_7_123;",
            &format!(
                "{origin}function $THREAD_8_456({params}){{alias();}}{extra}return $THREAD_8_456;"
            ),
        )
        .replace("$THREAD_7_123", "cleanup")
}

#[test]
fn small_composition_uses_independent_callee_body_and_stable_aliases() {
    for origin in [
        "let alias=$THREAD_7_123;",
        "var alias;alias=$THREAD_7_123;",
        "const first=$THREAD_7_123;const alias=first;",
    ] {
        assert!(named(
            &wrapper(origin, "", ""),
            "$THREAD_8_456",
            "teardownSubscriptions"
        ));
    }
}

#[test]
fn composition_refuses_ambiguous_runtime_and_mutated_origins() {
    for s in [
        wrapper("let alias=$THREAD_7_123;", "alias=other;", ""),
        wrapper("var alias=$THREAD_7_123;var alias;", "", ""),
        wrapper("", "", "alias"),
        wrapper("", "", "alias= $THREAD_7_123"),
        wrapper("let {alias}={alias:$THREAD_7_123};", "", ""),
        wrapper("var alias;for(alias of list){}alias=$THREAD_7_123;", "", ""),
        wrapper("let alias=$THREAD_7_123;", "$THREAD_7_123=other;", ""),
        wrapper("let alias=$THREAD_7_123;", "function $THREAD_7_123(){}", ""),
        wrapper("let alias=$THREAD_7_123;", "", "").replace("item.teardown()", "item.other()"),
        wrapper("let alias=$THREAD_7_123;", "", "").replace("{alias();}", "{return alias();}"),
        wrapper("let alias=$THREAD_7_123;", "", "").replace("{alias();}", "{alias(1);}"),
    ] {
        assert!(!named(&s, "$THREAD_8_456", "teardownSubscriptions"), "{s}");
    }
}

#[test]
fn composition_refuses_captured_parameter_catch_destructuring_and_loop_origins() {
    let catch = wrapper("", "", "")
        .replace(
            "function $THREAD_8_456",
            "try{throw 1;}catch(alias){alias=cleanup;function $THREAD_8_456",
        )
        .replace(
            "return $THREAD_8_456;}main()",
            "return $THREAD_8_456;}}main()",
        );
    for source in [
        wrapper("alias=$THREAD_7_123;", "", "").replace("main(){", "main(alias){"),
        wrapper("alias=$THREAD_7_123;", "", "").replace("main(){", "main(alias=other){"),
        wrapper("alias=$THREAD_7_123;", "", "").replace("main(){", "main(...alias){"),
        wrapper("alias=$THREAD_7_123;", "", "").replace("main(){", "main({alias}){"),
        wrapper("let {alias}={};alias=$THREAD_7_123;", "", ""),
        wrapper("for(var alias of list){}alias=$THREAD_7_123;", "", ""),
        wrapper("for(var alias in table){}alias=$THREAD_7_123;", "", ""),
        wrapper(
            "let first=$THREAD_7_123;const alias=first;",
            "first=other;",
            "",
        ),
        catch,
    ] {
        assert!(
            !named(&source, "$THREAD_8_456", "teardownSubscriptions"),
            "{source}"
        );
    }
}

#[test]
fn closed_getters_prove_exact_this_path_and_void_comparison() {
    for (path, expected) in [
        ("this._observer", "isSubscriptionClosed"),
        (
            "this._subscription._observer",
            "isObserverSubscriptionClosed",
        ),
    ] {
        for strict in ["", "\"use strict\";"] {
            let s=format!("function main(){{function $THREAD_88_999(){{{strict}return {path} === void 0;}}return $THREAD_88_999;}}main();");
            assert!(named(&s, "$THREAD_88_999", expected));
        }
    }
}

#[test]
fn closed_getters_reject_semantic_changes() {
    let s="function main(){function $THREAD_88_999(){return this._subscription._observer === void 0;}return $THREAD_88_999;}main();";
    for (from, to) in [
        ("===", "!=="),
        ("===", "=="),
        ("void 0", "null"),
        ("void 0", "void 1"),
        ("_observer", "observer"),
        ("_subscription", "other"),
        ("this._subscription", "other._subscription"),
        ("this._subscription", "this?._subscription"),
        ("() {", "(extra) {"),
        ("(){return", "(extra){return"),
    ] {
        let changed = s.replace(from, to);
        if changed == s {
            continue;
        }
        assert!(
            !named(&changed, "$THREAD_88_999", "isObserverSubscriptionClosed"),
            "{from}->{to}"
        );
    }
}

#[test]
fn captured_method_adapters_forward_exact_formals_without_return() {
    for (params, call, expected) in [
        ("", "sink.complete()", "forwardComplete"),
        ("", "sink.teardown()", "teardownSubscription"),
        ("value", "sink.next(value)", "forwardNext"),
    ] {
        let s=format!("function main(){{let sink={{}};function $THREAD_77_888({params}){{{call};}}return $THREAD_77_888;}}main();");
        assert!(named(&s, "$THREAD_77_888", expected), "{s}");
        assert!(named(
            &s.replace("sink", "$KEY_511").replace("value", "data"),
            "$THREAD_77_888",
            expected
        ));
        for changed in [
            s.replace(call, &format!("return {call}")),
            s.replace("sink.", "other."),
            s.replace("let sink={};", "let sink={};sink=other;"),
            s.replace("let sink={};", "var sink={};var sink;"),
            s.replace("sink.", "sink?."),
        ] {
            assert!(!named(&changed, "$THREAD_77_888", expected), "{changed}");
        }
    }
}

#[test]
fn next_adapter_rejects_changed_arity_arguments_and_parameter_shapes() {
    let s="function main(){let sink={};function $THREAD_77_888(value){sink.next(value);}return $THREAD_77_888;}main();";
    for (from, to) in [
        ("(value){", "(value=1){"),
        ("(value){", "(...value){"),
        ("(value){", "({value}){"),
        ("next(value)", "next(other)"),
        ("next(value)", "next(value,1)"),
        ("next(value)", "next(...value)"),
        ("next(value)", "next?.(value)"),
        ("{sink.next", "{value=1;sink.next"),
        ("next(value)", "complete(value)"),
    ] {
        assert!(
            !named(&s.replace(from, to), "$THREAD_77_888", "forwardNext"),
            "{from}->{to}"
        );
    }
}

#[test]
fn human_names_and_canonical_name_collisions_are_preserved() {
    let human = BATCH.replace("$THREAD_7_123", "humanCleanup");
    assert_eq!(name_threads(&human, true).unwrap().source, human);
    let collided = BATCH.replace("let callbacks", "let teardownSubscriptions=1;let callbacks");
    assert!(!named(&collided, "$THREAD_7_123", "teardownSubscriptions"));
    let descendant = BATCH.replace(
        "(){callbacks.forEach",
        "(){function teardownSubscriptions(){}callbacks.forEach",
    );
    assert!(!named(
        &descendant,
        "$THREAD_7_123",
        "teardownSubscriptions"
    ));
}

#[test]
fn teardown_accepts_reset_data_receivers_but_keeps_callable_origins_strict() {
    for name in ["callbacks", "buffer", "registry", "subscriptions"] {
        let source = BATCH.replace(
            "return $THREAD_7_123",
            &format!("{name}=other;return $THREAD_7_123"),
        );
        assert!(
            named(&source, "$THREAD_7_123", "teardownSubscriptions"),
            "{source}"
        );
    }
    let reset = wrapper(
        "let alias=$THREAD_7_123;",
        "callbacks=[];subscriptions=[];",
        "",
    );
    assert!(named(&reset, "$THREAD_8_456", "teardownSubscriptions"));
    assert!(!named(
        &wrapper("let alias=$THREAD_7_123;", "callbacks=[];alias=other;", ""),
        "$THREAD_8_456",
        "teardownSubscriptions"
    ));
    for source in [
        BATCH.replace(
            "let callbacks=[],buffer={},registry={},subscriptions=[];",
            "let buffer={},registry={},subscriptions=[];",
        ),
        BATCH.replace(
            "let callbacks=[],buffer={},registry={},subscriptions=[];",
            "let {callbacks}=other;let buffer={},registry={},subscriptions=[];",
        ),
        BATCH.replace(
            "let callbacks=[],buffer={},registry={},subscriptions=[];",
            "for(var callbacks of other){}let buffer={},registry={},subscriptions=[];",
        ),
        BATCH
            .replace(
                "let callbacks=[],buffer={},registry={},subscriptions=[];",
                "let buffer={},registry={},subscriptions=[];",
            )
            .replace(
                "return $THREAD_7_123",
                "let callbacks=[];return $THREAD_7_123",
            ),
    ] {
        assert!(
            !named(&source, "$THREAD_7_123", "teardownSubscriptions"),
            "{source}"
        );
    }
}

#[test]
fn composed_cleanup_refuses_future_initialization_at_each_alias_hop() {
    for source in [
        wrapper("", "const alias=cleanup;", ""),
        wrapper("const alias=first;", "const first=cleanup;", ""),
        wrapper("var alias;", "alias=cleanup;", ""),
    ] {
        assert!(
            !named(&source, "$THREAD_8_456", "teardownSubscriptions"),
            "{source}"
        );
    }
}

#[test]
fn composed_cleanup_refuses_setup_conditional_and_loop_callable_origins() {
    for origin in [
        "let alias;function setup(){alias=cleanup;}",
        "var alias;for(;ready;){alias=cleanup;}",
        "var alias;if(ready){alias=cleanup;}",
        "if(ready){var alias=cleanup;}",
        "using alias=cleanup;",
    ] {
        let source = wrapper(origin, "", "");
        assert!(
            !named(&source, "$THREAD_8_456", "teardownSubscriptions"),
            "{source}"
        );
    }
}
