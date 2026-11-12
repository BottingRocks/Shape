use shape::naming::name_threads;
const A: &str = include_str!("fixtures/naming/captured_source_teardown/captured-source-parent.js");
const B: &str = include_str!("fixtures/naming/captured_source_teardown/sample-expansion-parent.js");
fn gained(source: &str) -> Vec<String> {
    name_threads(source, true)
        .expect("valid whole-scope model")
        .report
        .outcomes
        .into_iter()
        .filter(|r| r.applied && r.evaluator.as_deref() == Some("captured_subscription_cleanup"))
        .map(|r| r.function_id)
        .collect()
}
#[test]
fn actual_whole_scopes_and_alpha_variants() {
    assert_eq!(gained(A).len(), 3);
    assert_eq!(gained(B).len(), 3);
    assert_eq!(
        gained(
            &A.replace("$KEY_", "local_")
                .replace("$THREAD_", "$THREAD_99_")
        )
        .len(),
        3
    );
}
fn check(source: &str, expected: bool) {
    let result =
        name_threads(&format!("{source} outer();"), true).expect("negative must parse/build");
    let target = result
        .report
        .outcomes
        .iter()
        .find(|r| r.binding.name == "$THREAD_1_1")
        .expect("exact target identity");
    if !expected {
        assert!(
            !target.applied,
            "negative target must remain unresolved: {source}"
        );
    }
    assert_eq!(
        target.evaluator.as_deref() == Some("captured_subscription_cleanup"),
        expected,
        "{source}"
    );
}
#[test]
fn store_epochs_and_whole_body() {
    let source = "function outer(input){let source=input; let subscription; function $THREAD_1_1(){return subscription.unsubscribe();} subscription=source.subscribe(handler); return $THREAD_1_1;}";
    check(source, true);
    for (from, to) in [
        (
            "return subscription.unsubscribe();",
            "effect(); return subscription.unsubscribe();",
        ),
        ("subscription.unsubscribe()", "subscription.unsubscribe(1)"),
        ("subscription.unsubscribe()", "subscription.unsubscribe?.()"),
        ("subscription.unsubscribe()", "subscription?.unsubscribe()"),
        ("subscription.unsubscribe()", "other.unsubscribe()"),
        (
            "subscription=source.subscribe(handler);",
            "if(flag){subscription=source.subscribe(handler);}",
        ),
        (
            "subscription=source.subscribe(handler);",
            "$THREAD_1_1(); subscription=source.subscribe(handler);",
        ),
        (
            "subscription=source.subscribe(handler);",
            "register($THREAD_1_1); subscription=source.subscribe(handler);",
        ),
        ("source.subscribe(handler)", "source.subscribe($THREAD_1_1)"),
        (
            "source.subscribe(handler)",
            "source.subscribe(()=>$THREAD_1_1())",
        ),
        (
            "return $THREAD_1_1;",
            "subscription=other; return $THREAD_1_1;",
        ),
        (
            "return $THREAD_1_1;",
            "function deferred(){subscription=other;} return $THREAD_1_1;",
        ),
        ("let source=input;", "let source=input; source=other;"),
        (
            "subscription=source.subscribe(handler);",
            "subscription=source.subscribe(handler); source=other;",
        ),
        (
            "subscription=source.subscribe(handler);",
            "let ignored = ()=>{subscription=source.subscribe(handler);};",
        ),
        ("let source=input;", "let source;"),
        ("function $THREAD_1_1()", "async function $THREAD_1_1()"),
        ("function $THREAD_1_1()", "function $THREAD_1_1(x=effect())"),
    ] {
        check(&source.replacen(from, to, 1), false);
    }
    check(
        &source
            .replace(
                "subscription=source.subscribe(handler);",
                "subscription=source.subscribe(handler);",
            )
            .replace(
                "return subscription.unsubscribe();",
                "return subscription['unsubscribe']();",
            ),
        true,
    );
    check(
        &source
            .replace(
                "let subscription;",
                "let subscription = source.subscribe(handler);",
            )
            .replace("subscription=source.subscribe(handler);", ""),
        true,
    );
    check(
        &source
            .replace(
                "subscription=source.subscribe(handler);",
                "let alias=$THREAD_1_1; subscription=source.subscribe(handler);",
            )
            .replace("return $THREAD_1_1;", "return alias;"),
        true,
    );
    check(
        &source.replace(
            "subscription=source.subscribe(handler);",
            "let alias=$THREAD_1_1; alias(); subscription=source.subscribe(handler);",
        ),
        false,
    );
}

fn sample_target(source: &str) -> bool {
    let result = name_threads(source, true).expect("sample mutation must parse/build");
    let target = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_0_64134")
        .expect("actual exact target identity");
    if target.evaluator.as_deref() == Some("append_sine_derived_samples") {
        assert!(target.applied);
        true
    } else {
        assert!(!target.applied, "negative target must remain unresolved");
        false
    }
}
#[test]
fn sample_expansion_whole_scope_and_changed_protocols() {
    assert!(sample_target(A));
    assert!(sample_target(
        &A.replace("$V", "inputArray")
            .replace("$W", "counterAlias")
            .replace("$X", "limitAlias")
            .replace("$Y", "stateAlias")
    ));
    for global in ["Math", "Number"] {
        let shadowed = A.replacen(
            "function $THREAD_0_64134(",
            &format!("let {global} = other; function $THREAD_0_64134("),
            1,
        );
        assert!(!sample_target(&shadowed), "shadowed global {global}");
    }
    let begin = A.find("function $THREAD_0_64134(").unwrap();
    let end = begin + A[begin..].find("\n\t\t\t\tlet ").unwrap();
    let own = &A[begin..end];
    for (from, to) in [
        ("let $W = 1", "let $W = 0"),
        ("let $X = 8", "let $X = 9"),
        ("* 63 +", "* 64 +"),
        ("$V[0]", "$V[1]"),
        ("$W <= $X", "$W < $X"),
        ("Math.fround", "Math.round"),
        ("Math.sin", "Math.cos"),
        ("Math.abs", "Math.floor"),
        ("* 2", "* 3"),
        ("$Y >= 1", "$Y > 1"),
        ("$X < 50", "$X < 51"),
        ("++$X", "--$X"),
        ("++$W", "--$W"),
        ("continue;", "break;"),
        ("($X - $W) / $X", "($X - $W) / $W"),
        ("$V.push($Y)", "$V.unshift($Y)"),
        ("Number($V[1]) + 1", "Number($V[0]) + 1"),
        ("$V[1] = Number", "$V[0] = Number"),
        ("let $W = 1;", "let Math = other; let $W = 1;"),
        ("let $W = 1;", "let Number = other; let $W = 1;"),
        ("$V.push($Y);", "$V.push($Y); effect();"),
    ] {
        assert!(own.contains(from), "missing exact protocol mutation {from}");
        let mut changed = A.to_string();
        changed.replace_range(begin..end, &own.replacen(from, to, 1));
        assert!(
            !sample_target(&changed),
            "accepted altered protocol {from} => {to}"
        );
    }
}
