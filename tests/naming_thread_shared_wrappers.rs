use shape::naming::name_threads;

fn claim(source: &str, target: &str, expected: &str) -> bool {
    name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .any(|o| {
            o.binding.name == target && o.applied && o.proposed_name.as_deref() == Some(expected)
        })
}

fn wrap(body: &str) -> String {
    format!("function main(){{let sink={{}};function $THREAD_91_804_2(a,b){{{body}}}return $THREAD_91_804_2;}}main();")
}

#[test]
fn pure_aliases_prove_slot_identity_and_literal_returns() {
    for (body, name) in [
        ("let x=a;const y=x;return y;", "identity"),
        ("var x=b,y=x;return y;", "identity"),
        ("const x=true;let y=x;return y;", "returnTrue"),
        ("const x=false;let y=x;return y;", "returnFalse"),
        ("const x='';let y=x;return y;", "returnEmptyString"),
    ] {
        assert!(claim(&wrap(body), "$THREAD_91_804_2", name), "{body}");
    }
    let source = wrap("let x=a;const y=x;return y;")
        .replace("$THREAD_91_804_2", "$THREAD_823_99999_7")
        .replace("(a,b)", "($KEY_848,second)")
        .replace("x=a", "x=$KEY_848");
    assert!(claim(&source, "$THREAD_823_99999_7", "identity"));
}

#[test]
fn alias_preludes_refuse_effects_and_ambiguous_bindings() {
    for body in [
        "var x=a;var x;return x;",
        "let x=a;x=b;return x;",
        "let x=a;x++;return x;",
        "let {x}=a;return x;",
        "let x=a.value;return x;",
        "let x=unknown();return x;",
        "let x=unknown;return a;",
        "var x=y,y=a;return x;",
        "let x=a;sideEffect();return x;",
        "let x=a;return other;",
        "let x=a;return [x];",
        "let x=a;return x+0;",
        "using x=a;return x;",
    ] {
        assert!(
            !claim(&wrap(body), "$THREAD_91_804_2", "identity"),
            "{body}"
        );
    }
    for params in ["a=other,b", "...a", "{a},b", "[a],b"] {
        let source = wrap("let x=a;return x;").replace("(a,b)", &format!("({params})"));
        assert!(!claim(&source, "$THREAD_91_804_2", "identity"), "{source}");
    }
}

const OWN: &str = "function main(){const method=Object.prototype.hasOwnProperty;const second=method;function $THREAD_91_804_2(a,b){let receiver=a;const key=b;return second.call(receiver,key);}return $THREAD_91_804_2;}main();";

#[test]
fn api_aliases_keep_receiver_and_key_order() {
    assert!(claim(OWN, "$THREAD_91_804_2", "hasOwnProperty"));
    assert!(claim(
        &wrap("let x=a,y=b;return Object.prototype.hasOwnProperty.call(x,y);"),
        "$THREAD_91_804_2",
        "hasOwnProperty"
    ));
    for (from, to) in [
        ("hasOwnProperty", "propertyIsEnumerable"),
        (".call(receiver,key)", ".call(key,receiver)"),
        (".call(receiver,key)", ".call(receiver,key,0)"),
        (".call(receiver,key)", ".apply(receiver,key)"),
        (".call(receiver,key)", ".call(receiver,...key)"),
        (".call(receiver,key)", "?.call(receiver,key)"),
        ("return second.call", "second.call"),
        ("const key=b", "const key=a"),
        ("const method=", "let method=other;method="),
        ("const method=", "var method;var method="),
        ("function main()", "function main(Object)"),
        ("(a,b)", "(a,b=other)"),
        ("let receiver=a;", "var a;let receiver=a;"),
    ] {
        assert!(
            !claim(&OWN.replace(from, to), "$THREAD_91_804_2", "hasOwnProperty"),
            "{from} -> {to}"
        );
    }
}

#[test]
fn api_paths_reject_parameter_catch_destructuring_and_loop_origins() {
    for source in [
        OWN.replace(
            "function main(){const method=Object.prototype.hasOwnProperty;",
            "function main(method){method=Object.prototype.hasOwnProperty;",
        ),
        OWN.replace(
            "const method=Object.prototype.hasOwnProperty;",
            "let {method}=Object.prototype.hasOwnProperty;",
        ),
        OWN.replace(
            "const method=Object.prototype.hasOwnProperty;",
            "var method;for(method of values){}",
        ),
        OWN.replace(
            "const method=Object.prototype.hasOwnProperty;",
            "try{}catch(method){method=Object.prototype.hasOwnProperty;}let method;",
        ),
        OWN.replace("const second=method;", "const second=method;method=other;"),
    ] {
        assert!(
            !claim(&source, "$THREAD_91_804_2", "hasOwnProperty"),
            "{source}"
        );
    }
}

#[test]
fn direct_api_proves_prototype_setter_and_refuses_changed_arguments() {
    let source = wrap("let obj=a;let proto=b;return Object.setPrototypeOf(obj,proto);");
    assert!(claim(&source, "$THREAD_91_804_2", "setObjectPrototype"));
    for (from, to) in [
        ("setPrototypeOf", "getPrototypeOf"),
        ("obj,proto)", "proto,obj)"),
        ("obj,proto)", "obj,proto,0)"),
        ("return Object", "Object"),
        ("(a,b)", "(a,b,Object)"),
        ("let obj=a", "let obj=b"),
    ] {
        assert!(
            !claim(
                &source.replace(from, to),
                "$THREAD_91_804_2",
                "setObjectPrototype"
            ),
            "{from} -> {to}"
        );
    }
}

const FORWARD: &str = "function main(){let receiver={};function $THREAD_33_400(e){let value=e;let sink=receiver;sink.error(value);}return $THREAD_33_400;}main();";

#[test]
fn error_forwarding_proves_alias_receiver_and_void_behavior() {
    assert!(claim(FORWARD, "$THREAD_33_400", "forwardError"));
    for (from, to) in [
        ("sink.error(value)", "sink.next(value)"),
        ("sink.error(value)", "sink.error(receiver)"),
        ("sink.error(value)", "sink.error(value,0)"),
        ("sink.error(value)", "return sink.error(value)"),
        ("sink.error(value)", "sink?.error(value)"),
        ("sink.error(value)", "sink.error(...value)"),
        ("let sink=receiver", "let sink=e"),
        ("let value=e", "let value=receiver"),
        ("let sink=receiver", "var sink=receiver;var sink"),
        ("let value=e", "let value=e;value=receiver"),
        ("(e)", "(e=receiver)"),
        ("(e)", "(...e)"),
    ] {
        assert!(
            !claim(&FORWARD.replace(from, to), "$THREAD_33_400", "forwardError"),
            "{from} -> {to}"
        );
    }
}

#[test]
fn prevent_default_requires_exact_formal_receiver_and_no_return() {
    let source = "function main(){function $THREAD_47_602(event){let input=event;input.preventDefault();}return $THREAD_47_602;}main();";
    assert!(claim(source, "$THREAD_47_602", "preventDefaultHandler"));
    for (from, to) in [
        ("input.preventDefault()", "other.preventDefault()"),
        ("input.preventDefault()", "input.preventDefault(0)"),
        ("input.preventDefault()", "return input.preventDefault()"),
        ("preventDefault", "stopPropagation"),
        ("(event)", "({event})"),
    ] {
        assert!(
            !claim(
                &source.replace(from, to),
                "$THREAD_47_602",
                "preventDefaultHandler"
            ),
            "{from} -> {to}"
        );
    }
}

const COMPOSE: &str = "function main(){function helper(a,b){let x=a,y=b;return Object.setPrototypeOf(x,y);}let alias=helper;function $THREAD_92_805(a,b){let x=a,y=b;return alias(x,y);}return $THREAD_92_805;}main();";

#[test]
fn direct_composition_reproves_body_and_all_formal_slots() {
    assert!(claim(COMPOSE, "$THREAD_92_805", "setObjectPrototype"));
    let renamed = COMPOSE
        .replace("helper", "humanLabelIsIrrelevant")
        .replace("alias", "$KEY_443");
    assert!(claim(&renamed, "$THREAD_92_805", "setObjectPrototype"));
    for (from, to) in [
        ("alias(x,y)", "alias(y,x)"),
        ("alias(x,y)", "alias(x)"),
        ("alias(x,y)", "alias(x,y,0)"),
        ("alias(x,y)", "alias.call(x,y)"),
        ("return alias(x,y)", "alias(x,y)"),
        ("let alias=helper;", "let alias=helper;alias=other;"),
        ("let alias=helper;", "var alias=helper;var alias;"),
        ("let alias=helper;", "let alias=helper;helper=other;"),
        (
            "let alias=helper;",
            "function helper(a,b){return false;}let alias=helper;",
        ),
        ("Object.setPrototypeOf(x,y)", "Object.getPrototypeOf(x,y)"),
        ("function helper(a,b)", "function helper(a,b=other)"),
        ("function helper(a,b)", "async function helper(a,b)"),
        ("function main(){", "function main(alias){"),
    ] {
        assert!(
            !claim(
                &COMPOSE.replace(from, to),
                "$THREAD_92_805",
                "setObjectPrototype"
            ),
            "{from} -> {to}"
        );
    }
}

#[test]
fn bounded_composition_accepts_single_initless_assignment_and_rejects_runtime_origins() {
    let initless = COMPOSE.replace("let alias=helper;", "let alias;alias=helper;");
    assert!(claim(&initless, "$THREAD_92_805", "setObjectPrototype"));
    let parameter = COMPOSE
        .replace("function main(){", "function main(alias){")
        .replace("let alias=helper;", "alias=helper;");
    assert!(!claim(&parameter, "$THREAD_92_805", "setObjectPrototype"));
    let destructured = COMPOSE.replace("let alias=helper;", "let {alias}=helper;");
    assert!(!claim(
        &destructured,
        "$THREAD_92_805",
        "setObjectPrototype"
    ));
}

#[test]
fn collision_and_human_names_remain_guarded() {
    let source = wrap("let x=a;return x;");
    let human = source.replace("$THREAD_91_804_2", "myIdentity");
    assert_eq!(name_threads(&human, true).unwrap().source, human);
    let collision = source
        .replace("function main(){", "function main(){let identity=42;")
        .replace(
            "return $THREAD_91_804_2;",
            "consume(identity);return $THREAD_91_804_2;",
        );
    assert!(!claim(&collision, "$THREAD_91_804_2", "identity"));
    let unrelated = source.replace(
        "function main(){",
        "function main(){function other(a){return false;}",
    );
    assert!(claim(&unrelated, "$THREAD_91_804_2", "identity"));
}

#[test]
fn detached_function_call_alias_cannot_supply_the_builtin_receiver() {
    let source = "function main(){const detached=Object.prototype.hasOwnProperty.call;function $THREAD_91_804_2(a,b){let x=a,y=b;return detached(x,y);}return $THREAD_91_804_2;}main();";
    assert!(!claim(source, "$THREAD_91_804_2", "hasOwnProperty"));
}

#[test]
fn immutable_nullary_thunk_uses_owned_body_not_human_callee_label() {
    let source = "function main(){const value=function(){let flag=false;return flag;};let thunk=value;function $THREAD_57_700(){return thunk();}return $THREAD_57_700;}main();";
    assert!(claim(source, "$THREAD_57_700", "returnFalse"));
    for (from, to) in [
        ("return thunk()", "thunk()"),
        ("return thunk()", "return thunk(0)"),
        ("let flag=false", "let flag=unknown()"),
        ("let thunk=value;", "let thunk=value;thunk=other;"),
        ("let thunk=value;", "var thunk=value;var thunk;"),
        ("let thunk=value;", "for(var thunk of [value]){}"),
    ] {
        assert!(
            !claim(&source.replace(from, to), "$THREAD_57_700", "returnFalse"),
            "{from} -> {to}"
        );
    }
    let human_name_only = "function main(){function returnFalse(){return unknown();}function $THREAD_57_700(){return returnFalse();}return $THREAD_57_700;}main();";
    assert!(!claim(human_name_only, "$THREAD_57_700", "returnFalse"));
}

#[test]
fn temporal_capture_aliases_and_mutated_receivers_do_not_prove_wrappers() {
    let api = OWN.replace(
        "const method=Object.prototype.hasOwnProperty;const second=method;",
        "var second=method;var method=Object.prototype.hasOwnProperty;",
    );
    assert!(!claim(&api, "$THREAD_91_804_2", "hasOwnProperty"));
    let callee = COMPOSE
        .replace("let alias=helper;", "var alias=later;var later=helper;")
        .replace("function main(){", "function main(){let scopePadding=0;");
    assert!(!claim(&callee, "$THREAD_92_805", "setObjectPrototype"));
    let receiver = FORWARD.replace("let receiver={};", "let receiver={};receiver=other;");
    assert!(!claim(&receiver, "$THREAD_33_400", "forwardError"));
}

const SENTINEL: &str = "function main(){const token=Object.create(null);function $THREAD_470_662(a){const value=a;return value===token;}return $THREAD_470_662;}main();";

#[test]
fn captured_null_prototype_sentinel_proves_exact_identity_and_getter() {
    for source in [
        SENTINEL.to_string(),
        SENTINEL
            .replace("token", "uniqueState")
            .replace("value", "candidate")
            .replace("$THREAD_470_662", "$THREAD_982_11144_2"),
        SENTINEL
            .replace("const value=a;", "const value=a;const sentinel=token;")
            .replace("value===token", "value===sentinel"),
        SENTINEL.replace(
            "const token=Object.create(null);",
            "const creator=Object.create;const token=creator(null);",
        ),
    ] {
        let target = if source.contains("$THREAD_982") {
            "$THREAD_982_11144_2"
        } else {
            "$THREAD_470_662"
        };
        assert!(
            claim(&source, target, "isNullPrototypeSentinel"),
            "{source}"
        );
    }
    let getter = SENTINEL.replace(
        "(a){const value=a;return value===token;",
        "(){const captured=token;return captured;",
    );
    assert!(claim(
        &getter,
        "$THREAD_470_662",
        "getNullPrototypeSentinel"
    ));
    for (from, to) in [
        ("return captured", "return {captured}"),
        ("(){const captured", "(unused){const captured"),
        ("Object.create(null)", "Object.create({})"),
    ] {
        assert!(!claim(
            &getter.replace(from, to),
            "$THREAD_470_662",
            "getNullPrototypeSentinel"
        ));
    }
}

#[test]
fn sentinel_refuses_changed_operations_origins_shadowing_and_forward_aliases() {
    for (from, to) in [
        ("value===token", "value==token"),
        ("value===token", "value!==token"),
        ("value===token", "token===value"),
        ("value===token", "value===other"),
        ("return value===token", "value===token"),
        ("const value=a", "const value=other"),
        ("Object.create(null)", "Object.create({})"),
        ("Object.create(null)", "Object.create(null,{})"),
        ("Object.create(null)", "Object.create(null,...extras)"),
        (
            "const token=Object.create(null);",
            "using token=Object.create(null);",
        ),
        ("Object.create(null)", "Object.create?.(null)"),
        ("Object.create(null)", "Object.assign(null)"),
        (
            "const token=Object.create(null);",
            "var token=Object.create(null);var token;",
        ),
        (
            "const token=Object.create(null);",
            "let token=Object.create(null);token=other;",
        ),
        (
            "const token=Object.create(null);",
            "let token;token=Object.create(null);",
        ),
        (
            "const token=Object.create(null);",
            "const {token}=Object.create(null);",
        ),
        (
            "const token=Object.create(null);",
            "for(const token of [Object.create(null)]){}",
        ),
        (
            "function main(){const token=Object.create(null);",
            "function main(token){token=Object.create(null);",
        ),
        ("function main(){", "function main(Object){"),
        ("function main(){", "function main(){const Object=custom;"),
        ("const value=a;", "var value=later;var later=a;"),
        ("const value=a;", "const value=a;let token=other;"),
    ] {
        assert!(
            !claim(
                &SENTINEL.replace(from, to),
                "$THREAD_470_662",
                "isNullPrototypeSentinel"
            ),
            "{from} -> {to}"
        );
    }
    let future = SENTINEL
        .replace("const token=Object.create(null);", "")
        .replace(
            "return $THREAD_470_662;",
            "const token=Object.create(null);return $THREAD_470_662;",
        );
    assert!(!claim(
        &future,
        "$THREAD_470_662",
        "isNullPrototypeSentinel"
    ));
    let future_api = SENTINEL.replace(
        "const token=Object.create(null);",
        "const token=creator(null);const creator=Object.create;",
    );
    assert!(!claim(
        &future_api,
        "$THREAD_470_662",
        "isNullPrototypeSentinel"
    ));
}

#[test]
fn sentinel_roles_preserve_names_and_obey_scope_collisions() {
    for (source, role) in [
        (SENTINEL.to_string(), "isNullPrototypeSentinel"),
        (
            SENTINEL.replace(
                "(a){const value=a;return value===token;",
                "(){return token;",
            ),
            "getNullPrototypeSentinel",
        ),
    ] {
        let human = source.replace("$THREAD_470_662", "chosenSentinelHelper");
        assert!(name_threads(&human, true)
            .unwrap()
            .source
            .contains("function chosenSentinelHelper("));
        let collision = source.replace(
            "function main(){",
            &format!("function main(){{let {role}=0;consume({role});"),
        );
        assert!(!claim(&collision, "$THREAD_470_662", role));
        let descendant = source
            .replace(
                "return token;",
                &format!("let {role}=0;consume({role});return token;"),
            )
            .replace(
                "return value===token;",
                &format!("let {role}=0;consume({role});return value===token;"),
            );
        assert!(!claim(&descendant, "$THREAD_470_662", role));
    }
}

#[test]
fn sentinel_refuses_runtime_capture_origins_and_conflicting_labels() {
    for prefix in [
        "try{throw external;}catch(token){",
        "for(let token of items){",
        "for(let token in items){",
    ] {
        let source = SENTINEL
            .replace("const token=Object.create(null);", prefix)
            .replace(";}main();", ";}}main();");
        assert!(
            !claim(&source, "$THREAD_470_662", "isNullPrototypeSentinel"),
            "{source}"
        );
    }
    for declaration in [
        "using creator=Object.create;const token=creator(null);",
        "let creator=Object.create;creator=other;const token=creator(null);",
        "var creator=Object.create;var creator;const token=creator(null);",
        "const detached=Object.create.call;const token=detached(null);",
    ] {
        let source = SENTINEL.replace("const token=Object.create(null);", declaration);
        assert!(
            !claim(&source, "$THREAD_470_662", "isNullPrototypeSentinel"),
            "{source}"
        );
    }
    let human = SENTINEL.replace("$THREAD_470_662", "differentHumanRole");
    let reference =
        shape::naming::labels::build_label_reference(SENTINEL, &human).unwrap();
    let result = shape::naming::name_threads_with_labels(
        SENTINEL,
        true,
        Some(&reference),
    )
    .unwrap();
    let target = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_470_662")
        .unwrap();
    assert!(!target.applied);
    assert!(target.reason.as_deref().unwrap().contains("conflicting"));
}

#[test]
fn composed_wrappers_refuse_setup_conditional_and_loop_callable_origins() {
    for origin in [
        "let alias;function setup(){alias=helper;}",
        "var alias;if(ready){alias=helper;}",
        "var alias;for(;ready;){alias=helper;}",
        "if(ready){var alias=helper;}",
        "using alias=helper;",
    ] {
        let source = COMPOSE.replace("let alias=helper;", origin);
        assert!(
            !claim(&source, "$THREAD_92_805", "setObjectPrototype"),
            "{source}"
        );
    }
    let conditional = SENTINEL.replace(
        "const token=Object.create(null);",
        "if(ready){var token=Object.create(null);}",
    );
    assert!(!claim(
        &conditional,
        "$THREAD_470_662",
        "isNullPrototypeSentinel"
    ));
}

#[test]
fn direct_sentinel_specialization_supersedes_generic_captured_state() {
    for (params, expression, name, evaluator) in [
        (
            "",
            "token",
            "getNullPrototypeSentinel",
            "shared_immutable_wrapper",
        ),
        (
            "value",
            "value===token",
            "isNullPrototypeSentinel",
            "shared_immutable_wrapper",
        ),
    ] {
        let source=format!("function main(){{const token=Object.create(null);function $THREAD_470_662({params}){{return {expression};}}return $THREAD_470_662;}}main();");
        let result = name_threads(&source, true).unwrap();
        let outcome = result
            .report
            .outcomes
            .iter()
            .find(|o| o.binding.name == "$THREAD_470_662")
            .unwrap();
        assert!(outcome.applied);
        assert_eq!(outcome.proposed_name.as_deref(), Some(name));
        assert!(
            !outcome
                .reason
                .as_deref()
                .is_some_and(|r| r.contains("conflicting")),
            "{:?}",
            outcome.reason
        );
        let role = result
            .report
            .function_roles
            .iter()
            .find(|r| r.current_name.as_deref() == Some("$THREAD_470_662"))
            .unwrap();
        assert_eq!(role.status, "identified");
        assert_eq!(role.candidates, vec![name.to_string()]);
        assert_eq!(role.evaluators, vec![evaluator.to_string()]);
        assert!(result.source.contains(&format!("function {name}(")));
    }
}
