use shape::naming::name_threads;

const NAME: &str = "deleteCapturedKeysAndUnregisterCallback";
const EVALUATOR: &str = "network_captured_key_callback_removal";
const BODY: &str = r#"function $THREAD_1_234() {
let keys = first.concat(second);
let i = 0;
for (; i < keys.length; i++) { delete target[keys[i]]; }
if (holder.onBeforeInvoke != null) { holder.onBeforeInvoke.unregister(callback); }
}"#;
const CAPTURES: &str = "let first, second, target, holder, callback;";

fn check(source: &str, name: &str, positive: bool) {
    let wrapped = if source == BODY {
        format!("function main(){{ {source} return 0; }}main();")
    } else {
        format!("{source} main();")
    };
    let source = wrapped.as_str();
    let result = name_threads(source, true).expect("valid scoped JavaScript");
    let outcome = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == name)
        .unwrap();
    if positive {
        assert!(outcome.applied, "{outcome:?}");
        assert_eq!(outcome.evaluator.as_deref(), Some(EVALUATOR));
        assert_eq!(outcome.role_name.as_deref(), Some(NAME));
        assert!(outcome
            .explanation
            .iter()
            .any(|s| s.contains("activation are unproved")));
    } else {
        assert_ne!(outcome.evaluator.as_deref(), Some(EVALUATOR), "{outcome:?}");
        assert_ne!(outcome.role_name.as_deref(), Some(NAME), "{outcome:?}");
    }
    let mut replay = source.to_owned();
    let mut edits = result.report.edits.clone();
    edits.sort_by_key(|e| std::cmp::Reverse(e.start));
    for edit in edits {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, result.source);
    for property in ["concat", "length", "onBeforeInvoke", "unregister"] {
        assert_eq!(
            source.matches(property).count(),
            result.source.matches(property).count()
        );
    }
}

#[test]
fn own_protocol_and_alpha_lowering() {
    check(
        &format!("function main(){{ {CAPTURES} {BODY} }}"),
        "$THREAD_1_234",
        true,
    );
    let alpha = BODY
        .replace("$THREAD_1_234", "$THREAD_2_456")
        .replace("keys", "list")
        .replace("callback", "listener");
    check(
        &format!(
            "function main(){{ {} {alpha} }}",
            CAPTURES.replace("callback", "listener")
        ),
        "$THREAD_2_456",
        true,
    );
    // Declaration order and sibling evidence are irrelevant to the protocol.
    check(
        &format!("function main(){{ {BODY} {CAPTURES} }}"),
        "$THREAD_1_234",
        true,
    );
}

#[test]
fn four_original_own_bodies_with_exact_captures() {
    for (body, captures, name) in [
        (
            include_str!("fixtures/network-captured-key-removal/hook-unregister-guarded.js"),
            "let $KEY_281,$KEY_264,$KEY_5,$KEY_2,$KEY_11;",
            "$THREAD_7_5196",
        ),
        (
            include_str!("fixtures/network-captured-key-removal/hook-unregister-compact.js"),
            "let $KEY_281,$KEY_264,$KEY_5,$KEY_2,$KEY_11;",
            "$THREAD_7_5196",
        ),
        (
            include_str!("fixtures/network-captured-key-removal/hook-unregister-alternate-guarded.js"),
            "let $KEY_390,$KEY_78,$KEY_18,$KEY_10,$KEY_1;",
            "$THREAD_1_114447",
        ),
        (
            include_str!("fixtures/network-captured-key-removal/hook-unregister-alternate-compact.js"),
            "let $KEY_390,$KEY_78,$KEY_18,$KEY_10,$KEY_1;",
            "$THREAD_1_114447",
        ),
    ] {
        check(
            &format!("function main(){{ {captures} {body} }}"),
            name,
            true,
        );
    }
}

#[test]
fn reject_incomplete_or_changed_protocols() {
    for (from, to) in [
        ("first.concat(second)", "first.concat(first)"),
        ("first.concat(second)", "first.concat(second, callback)"),
        ("let i = 0", "let i = 1"),
        ("i < keys.length", "i <= keys.length"),
        ("i++", "++i"),
        ("i++", "i--"),
        ("delete target[keys[i]]", "delete target[i]"),
        ("delete target[keys[i]]", "delete callback[keys[i]]"),
        ("delete target[keys[i]]", "target[keys[i]] = null"),
        (
            "holder.onBeforeInvoke != null",
            "holder.onBeforeInvoke == null",
        ),
        (
            "holder.onBeforeInvoke.unregister(callback)",
            "target.onBeforeInvoke.unregister(callback)",
        ),
        ("unregister(callback)", "unregister(target)"),
        ("unregister(callback)", "register(callback)"),
        ("unregister(callback)", "unregister(callback, first)"),
        ("let keys =", "callback(); let keys ="),
        ("function $THREAD_1_234()", "async function $THREAD_1_234()"),
        ("function $THREAD_1_234()", "function $THREAD_1_234(extra)"),
    ] {
        check(
            &format!("function main(){{ {CAPTURES} {} }}", BODY.replace(from, to)),
            "$THREAD_1_234",
            false,
        );
    }
}

#[test]
fn reject_unresolved_parameter_catch_and_redeclared_capture_origins() {
    check(BODY, "$THREAD_1_234", false);
    check(
        &format!("function main(first, second, target, holder, callback){{ {BODY} }}"),
        "$THREAD_1_234",
        false,
    );
    check(&format!("function main(){{ let first, second, target, holder; try {{ work(); }} catch(callback) {{ {BODY} }} }}"), "$THREAD_1_234", false);
    check(
        &format!(
            "function main(){{ var first, second, target, holder, callback; var first; {BODY} }}"
        ),
        "$THREAD_1_234",
        false,
    );
    // Same spelling in a sibling cannot provide declaration evidence.
    check(
        &format!("function sibling(){{ {CAPTURES} }} function main(){{ {BODY} }}"),
        "$THREAD_1_234",
        false,
    );
}
