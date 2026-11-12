use super::*;
const ACTUAL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/submit-forwarding/stateful-submit.js"
));
fn inspect(source: &str, check: impl FnOnce(&Model<'_, '_>)) {
    let a = oxc_allocator::Allocator::default();
    let parsed = crate::analysis::parse(&a, source);
    assert!(parsed.errors.is_empty());
    let built = oxc_semantic::SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    assert!(built.errors.is_empty());
    let m = Model::new(
        source,
        &parsed.program,
        built.semantic.scoping(),
        built.semantic.nodes(),
        ProgramPropertiesLimits::default(),
    )
    .unwrap();
    check(&m)
}
#[test]
fn actual_first_failed_guard() {
    inspect(ACTUAL, |m| {
        let site = *m
            .all_sites()
            .iter()
            .find(|s| s.span.start == 49059)
            .unwrap();
        let (_, body) = parts(m, site).unwrap();
        let [Statement::TryStatement(t)] = body.statements.as_slice() else {
            panic!()
        };
        let receiver = member(
            &statement_call(&t.block.body[0]).unwrap().callee,
            "removeEventListener",
        )
        .unwrap();
        let s = data_capture(m, site, receiver).unwrap();
        assert_eq!(m.binding_writes(s), 2);
        assert!(captured_unchanged(m, site, receiver).is_none());
        eprintln!("FIRST_FAILED_GUARD notifier@49059 captured_unchanged(receiver): two exclusive branch writes; prior notifier refuses before optional flag. Forwarder@49770 links this notifier.");
        assert!(notifier(m, site).is_some());
    });
}

fn protocol_names(m: &Model<'_, '_>) -> Vec<String> {
    m.all_sites()
        .iter()
        .filter_map(|&s| {
            (notifier(m, s).is_some()
                || forwarder(m, s).is_some()
                || parent::trusted(m, s).is_some())
            .then(|| match m.nodes.kind(s.node_id) {
                AstKind::Function(f) => f.id.as_ref().unwrap().name.to_string(),
                _ => unreachable!(),
            })
        })
        .collect()
}
const STATEFUL_SUBMIT_NAMES: [&str; 4] = [
    "$THREAD_0_106290",
    "$THREAD_8_108359",
    "$THREAD_0_65125",
    "$THREAD_2_153105",
];
const COMPACT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/submit-forwarding/stateful-submit-compact.js"
));
fn check(source: &str, names: &[&str]) {
    inspect(source, |m| {
        let got = protocol_names(m);
        for name in names {
            assert!(got.iter().any(|s| s == name), "missing {name}, got {got:?}");
        }
    });
}
#[test]
fn actual_stateful_submit_whole_stateful_protocol() {
    check(ACTUAL, &STATEFUL_SUBMIT_NAMES);
}
#[test]
fn finite_other_five_actual_contexts() {
    for (source, names) in [
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/submit-forwarding/forwarding-baseline.js"
            )),
            vec![
                "$THREAD_5_100374",
                "$THREAD_5_57355",
                "$THREAD_3_108742",
                "$THREAD_8_46728",
            ],
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/submit-forwarding/checkbox-hook.js"
            )),
            vec![
                "$THREAD_7_28660",
                "$THREAD_5_24289",
                "$THREAD_6_4422",
                "$THREAD_4_4512",
            ],
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/submit-forwarding/initialized-range.js"
            )),
            vec![
                "$THREAD_2_131206",
                "$THREAD_4_70572",
                "$THREAD_8_80862",
                "$THREAD_5_23082",
            ],
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/submit-forwarding/captured-submit.js"
            )),
            vec!["$THREAD_5_61777", "$THREAD_5_47205"],
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/submit-forwarding/forwarding-alternate.js"
            )),
            vec!["$THREAD_7_116708"],
        ),
    ] {
        check(source, &names);
    }
}
#[test]
fn prior_named_protocols() {
    check(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/submit-notification/direct-notification.js"
        )),
        &[
            "$THREAD_8_52378",
            "$THREAD_5_46332",
            "$THREAD_0_121572",
            "$THREAD_7_25094",
        ],
    );
    check(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/submit-notification/stateful-notification.js"
        )),
        &["$THREAD_4_139072", "$THREAD_1_86348", "$THREAD_7_110216"],
    );
}
#[test]
fn compact_alpha_and_inert_declaration_permutation() {
    check(COMPACT, &STATEFUL_SUBMIT_NAMES);
    check(
        &COMPACT
            .replace("$KEY_11", "receiver")
            .replace("$KEY_3", "state")
            .replace("$DM", "event"),
        &STATEFUL_SUBMIT_NAMES,
    );
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/submit-forwarding/forwarding-baseline-compact.js"
    ));
    check(
        source,
        &["$THREAD_5_57355", "$THREAD_3_108742", "$THREAD_8_46728"],
    );
    // Move the trusted declaration past both forwarder publications; its
    // publication stays last. This changes no executable statement order.
    let start = source.find("function $THREAD_5_57355").unwrap();
    let end = source.find("function $THREAD_3_108742").unwrap();
    let declaration = &source[start..end];
    let moved = source.replacen(declaration, "", 1).replace(
        "$KEY_3 = $THREAD_5_57355;",
        &format!("{declaration}$KEY_3 = $THREAD_5_57355;"),
    );
    check(
        &moved,
        &["$THREAD_5_57355", "$THREAD_3_108742", "$THREAD_8_46728"],
    );
    let compact_submit = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/submit-forwarding/captured-submit-compact.js"
    ));
    check(
        &compact_submit.replace("$AY", "cursor").replace("$AZ", "predicate"),
        &["$THREAD_5_61777"],
    );
}
#[test]
fn compact_whole_protocol_refusals() {
    for (from, to) in [
        ("$KEY_11.removeEventListener", "$KEY_8.removeEventListener"),
        ("$DM.target !== $KEY_8", "$DM.target !== $KEY_11"),
        ("$KEY_12($DM);", "$KEY_12($KEY_8);"),
        ("$KEY_12($DM);", "return $KEY_12($DM);"),
        ("$KEY_12($DM);", "$KEY_12.call(null,$DM);"),
        ("$KEY_0 = null;", "$KEY_0 = false;"),
        ("$KEY_3 = true;", "$KEY_3 = false;"),
        ("$KEY_3 = false;", "$KEY_3 = 0;"),
        ("2.27:", "2.29:"),
        ("2.28:", "2.27:"),
        ("form: $KEY_8", "form: $KEY_11"),
        ("event: $DM", "event: $KEY_8"),
        ("$KEY_229.notify({", "$KEY_2.notify({"),
        ("$KEY_12 = $THREAD_0_106290;", "$KEY_12 = unknown;"),
        ("$KEY_11 = $KEY_8;", "$KEY_11 = $DJ;"),
        ("$KEY_11 = $DK;", "$KEY_11 = $DH;"),
        ("$DJ = $DL ?? $DJ;", "$DJ = $DL || $DJ;"),
        ("$KEY_12();", "publish($KEY_12); $KEY_12();"),
        ("$KEY_12();", "$KEY_12(); $KEY_12 = unknown;"),
        ("$KEY_11.addEventListener", "$KEY_11.otherEventListener"),
        (
            "$KEY_11.addEventListener(\"submit\", $KEY_12);",
            "$KEY_11.addEventListener(\"submit\", $KEY_0);",
        ),
        ("$KEY_2.notify($KEY_8);", "$KEY_2.notify($KEY_8); effect();"),
        ("$KEY_2.notify($KEY_8);", "$KEY_2.notify($KEY_11);"),
        (
            "$KEY_12 = $THREAD_0_106290;",
            "immediate(); $KEY_12 = $THREAD_0_106290; function immediate(){ $THREAD_0_106290(); }",
        ),
        (
            "$KEY_12 = $THREAD_0_106290;",
            "publish(()=> $THREAD_0_106290()); $KEY_12 = $THREAD_0_106290;",
        ),
        ("let $KEY_12;", "$KEY_11 = other; let $KEY_12;"),
        ("let $KEY_12;", "let $KEY_12 = source.callback;"),
        ("$KEY_12();", "$KEY_0(); $KEY_12();"),
        ("$KEY_11 = $KEY_8;", "$KEY_11 = $KEY_8; $KEY_12();"),
        ("$KEY_12($DM);", "$KEY_0 = null; $KEY_12($DM);"),
        ("$KEY_229.notify({", "publish($KEY_12); $KEY_229.notify({"),
    ] {
        assert!(COMPACT.contains(from), "missing {from}");
        let mutated = COMPACT.replace(from, to);
        inspect(&mutated, |m| {
            assert!(
                protocol_names(m).is_empty(),
                "accepted mutation {from} -> {to}: {:?}",
                protocol_names(m)
            )
        });
    }
}

#[test]
fn real_whole_context_early_publication_refused() {
    let from = "$KEY_12 = $THREAD_0_106290;";
    assert!(ACTUAL.contains(from));
    let early=ACTUAL.replacen(from,"knownImmediate(); $KEY_12 = $THREAD_0_106290; function knownImmediate(){ $THREAD_0_106290(); }",1);
    inspect(&early, |m| {
        let got = protocol_names(m);
        for name in STATEFUL_SUBMIT_NAMES {
            assert!(!got.iter().any(|n| n == name), "early call accepted {name}");
        }
    });
}
#[test]
fn selector_variants_do_not_accept_changed_producers() {
    for (source, from, to, trusted) in [
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/submit-forwarding/captured-submit-compact.js"
            )),
            "$AZ = $AY.parentNode != null;",
            "$AZ = $AY.parentNode == null;",
            "$THREAD_5_61777",
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/submit-forwarding/initialized-range-compact.js"
            )),
            "$W.parentNode.nodeType !== 1",
            "$W.parentNode.nodeType === 1",
            "$THREAD_4_70572",
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/submit-forwarding/forwarding-baseline-compact.js"
            )),
            "let $KEY_2 = $BX;",
            "let $KEY_2 = $BW;",
            "$THREAD_5_57355",
        ),
    ] {
        assert!(source.contains(from), "missing variant mutation {from}");
        inspect(&source.replace(from, to), |m| {
            assert!(!protocol_names(m).iter().any(|n| n == trusted))
        });
    }
}
