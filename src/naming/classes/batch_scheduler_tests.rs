use crate::signals::schema::ProgramPropertiesLimits;
use super::*;
const DIRECT_STORE_ALIAS: &str =
    include_str!("../../../tests/fixtures/program/thread-batch-scheduler-direct-store-alternate.js");
const DIRECT_STORE: &str =
    include_str!("../../../tests/fixtures/program/thread-batch-scheduler-direct-store.js");
const CARRIED_MEAN: &str =
    include_str!("../../../tests/fixtures/program/thread-batch-scheduler-carried-mean.js");
const DELAYED_MEAN: &str =
    include_str!("../../../tests/fixtures/program/thread-batch-scheduler-delayed-mean.js");

fn prove(source: &str) -> Vec<FinderClaim> {
    let allocator = oxc_allocator::Allocator::default();
    let program = crate::analysis::parse_program(&allocator, source).expect("negative must parse");
    let (scoping, nodes) = crate::analysis::analyze(&program);
    let model = Model::new(
        source,
        &program,
        &scoping,
        &nodes,
        ProgramPropertiesLimits::default(),
    )
    .expect("negative must build Model");
    let AstKind::Function(main) = model.nodes.kind(model.main_node) else {
        panic!("fixture MAIN must be the complete bootstrap function")
    };
    assert_eq!(main.id.as_ref().unwrap().name.as_str(), "bootstrap");
    let ctx = FinderCtx {
        source,
        program: &program,
        scoping: &scoping,
        nodes: &nodes,
    };
    let finder_claims = crate::signals::analysis::roles::vm_exec::classes::find(&ctx);
    let found = claims_with_finder_claims(&model, &finder_claims);
    assert_eq!(
        found,
        claims(&model),
        "cached and standalone APIs must agree"
    );
    assert!(
        claims_with_finder_claims(&model, &[]).is_empty(),
        "missing elapsed-clock evidence must refuse"
    );
    found
}
fn accepts(source: &str) {
    let claims = prove(source);
    assert_eq!(claims.len(), 1, "{source}");
    let FinderClaim::ClassBinding {
        role, class_span, ..
    } = &claims[0]
    else {
        panic!("function gain")
    };
    assert_eq!(*role, "owned_batch_scheduler_class");
    assert!(source[class_span.start as usize..class_span.end as usize].starts_with("class "));
}
fn refuses(source: &str) {
    assert!(prove(source).is_empty(), "{source}");
}
fn changed(source: &str, old: &str, new: &str) -> String {
    assert!(source.contains(old), "missing mutation: {old}");
    source.replace(old, new)
}

#[test]
fn real_installed_classes_clock_transport_and_work_consumers() {
    accepts(DIRECT_STORE_ALIAS);
    accepts(DIRECT_STORE);
}

#[test]
fn edge_count_protocol_preserves_old_count_and_continuing_edge_order() {
    let source =
        include_str!("../../../tests/fixtures/program/thread-batch-scheduler-edge-count.js");
    accepts(source);
    accepts(
        &source
            .replace("$BQ", "mean")
            .replace("$BS", "nextCount")
            .replace("$BN", "input"),
    );
    let found = prove(source);
    let FinderClaim::ClassBinding { evidence, .. } = &found[0] else {
        panic!("class proof only")
    };
    assert!(evidence[0].contains("old count at inline deadline"));
    for (from, to) in [
        ("for (;; i = $BS)", "for (;; $BQ = $BS)"),
        ("Number(i) + 1", "Number(i) + 2"),
        ("$BS = Number(i) + 1;", "$BS = Number(i) + 1; i = $BS;"),
        ("$BQ * i", "$BQ * $BS"),
        ("$BU - $BT", "$BT - $BU"),
        ("if ($BR)", "if (!$BR)"),
        (
            "this._process.bind(this, $BN)",
            "this._process.bind(null, $BN)",
        ),
        (
            "$BR = $BN();",
            "$BR = $BN(); function donor() { return $BQ; }",
        ),
        (
            "this.targetBatchTimeMs = $BN;",
            "this.targetBatchTimeMs = 8;",
        ),
        (
            "let $KEY_203 = $THREAD_0_76570;",
            "let $KEY_203 = $THREAD_0_76570; $KEY_203 = otherClock;",
        ),
    ] {
        refuses(&changed(source, from, to));
    }
    // Move the mean store above the break: the stop edge must not commit it.
    let store = "\t\t\t\t\t$BQ = ($BQ * i + ($BU - $BT)) / (i + 1);";
    assert!(source.contains(store));
    let moved = source.replace(store, "").replace(
        "$BS = Number(i) + 1;",
        &format!("$BS = Number(i) + 1;\n{store}"),
    );
    refuses(&moved);
}

#[test]
fn old_direct_store_evidence_is_byte_exact() {
    // Literal scheduler explanation. Keep this independent of the production string so metadata
    // changes fail.
    let expected = "unique own constructor/now/schedule/_process; stable scoped elapsed-clock origin and host selection; budget formal; zero-delay receiver/work binding; ordered clock-work-clock, positive elapsed running mean, deadline/work short circuit and pending-only reschedule";
    for source in [DIRECT_STORE_ALIAS, DIRECT_STORE] {
        let claims = prove(source);
        assert_eq!(claims.len(), 1);
        let FinderClaim::ClassBinding { evidence, .. } = &claims[0] else {
            panic!("expected class claim")
        };
        assert_eq!(evidence.len(), 1);
        assert_eq!(evidence[0].as_bytes(), expected.as_bytes());
    }
}

#[test]
fn real_carried_and_delayed_protocols_keep_distinct_evidence() {
    for (source, evidence) in [
        (
            CARRIED_MEAN,
            "next mean used by deadline then carried on for-update edge",
        ),
        (
            DELAYED_MEAN,
            "increment before inline deadline mean; delayed mean store on continuing edge only",
        ),
    ] {
        accepts(source);
        let claims = prove(source);
        let FinderClaim::ClassBinding {
            evidence: found, ..
        } = &claims[0]
        else {
            panic!("expected class claim")
        };
        assert!(found.iter().any(|item| item.contains(evidence)));
    }
}

#[test]
fn carried_mean_requires_exact_update_deadline_and_count() {
    for (old, new) in [
        ("for (;; i = $BY)", "for (;; i = $BW)"),
        ("$CA + $BY <", "$CA + i <"),
        ("($BW + 1)", "($BW + 2)"),
    ] {
        refuses(&changed(CARRIED_MEAN, old, new));
    }
}

#[test]
fn delayed_mean_requires_exact_store_host_and_stable_clock_origin() {
    for (old, new) in [
        (
            "$BV = ($BV * $BW + ($BZ - $BY)) / ($BW + 1);",
            "$BV = ($BV * $BW + ($BY - $BZ)) / ($BW + 1);",
        ),
        ("++$BW;", "++$BW; $BV = 0;"),
        ("$BT = self;", "$BT = global;"),
        ("$BT = void 0;", "$BT = window;"),
        (
            "$KEY_311 = $THREAD_9_129514;",
            "$KEY_311 = $THREAD_9_129514; $KEY_311 = other;",
        ),
    ] {
        refuses(&changed(DELAYED_MEAN, old, new));
    }
}

fn synthetic(process: &str) -> String {
    // Keep the real clock installation and consuming caller, replacing only
    // the owned native class with the readable behavioral reference.
    let start = DIRECT_STORE_ALIAS.find("class ").unwrap();
    let end = DIRECT_STORE_ALIAS[start..].find("\n$KEY_63 =").unwrap() + start;
    let class = format!("class $THREAD_4_63087 {{ constructor(budget) {{ this.targetBatchTimeMs = budget; this._internalNow = $KEY_250({HOST}); }} {NOW} {SCHEDULE} {process} }}");
    format!("{}{}{}", &DIRECT_STORE_ALIAS[..start], class, &DIRECT_STORE_ALIAS[end..])
}
#[test]
fn direct_conjunction_while_and_duration_temporary() {
    for source in [
        PROCESS.to_string(),
        PROCESS.replace("for (;;)", "while (true)"),
        PROCESS.replace(
            "after + mean < start + budget ? pending : after + mean < start + budget",
            "after + mean < start + budget && pending",
        ),
        PROCESS.replace(
            "mean = (mean * count + (after - before))",
            "let elapsed = after - before; mean = (mean * count + elapsed)",
        ),
    ] {
        accepts(&synthetic(&source));
    }
}
#[test]
fn wrong_arithmetic_budget_receiver_argument_or_order_refuses() {
    for (old, new) in [
        ("after - before", "before - after"),
        ("count + 1", "count + 2"),
        ("mean * count", "mean + count"),
        ("start + budget", "start - budget"),
        ("start + budget", "start + 45"),
        ("this.now()", "other.now()"),
        ("work()", "work(this)"),
        ("work()", "work.call(this)"),
        (
            "this._process.bind(this, work)",
            "this._process.bind(other, work)",
        ),
        (
            "this._process.bind(this, work)",
            "this._process.bind(this, other)",
        ),
        ("if (pending)", "if (true)"),
        (
            "pending = work();\n        let after = this.now();",
            "let after = this.now(); pending = work();",
        ),
        ("++count;", "count = 0;"),
    ] {
        refuses(&synthetic(&changed(PROCESS, old, new)));
    }
}
#[test]
fn native_api_traps_and_donor_fail_closed() {
    for (old, new) in [
        ("now() {", "static now() {"),
        ("now() {", "['now']() {"),
        ("now() {", "get now() {"),
        ("now() {", "extra() {} now() {"),
        ("now() {", "now() {} now() {"),
        (
            "this.targetBatchTimeMs = budget",
            "this.targetBatchTimeMs = 45",
        ),
        ("this._internalNow()", "other._internalNow()"),
        (
            "setTimeout(this._process.bind(this, work), 0)",
            "setTimeout(this._process.bind(this, work), 1)",
        ),
    ] {
        refuses(&changed(&synthetic(PROCESS), old, new));
    }
    refuses(&synthetic(&format!(
        "_process(work) {{ function donor(work) {{ {} }} }}",
        PROCESS
            .strip_prefix("_process(work) {")
            .unwrap()
            .trim_end_matches('}')
    )));
}
#[test]
fn clock_alias_mutation_shadowing_and_host_contract_refuse() {
    let source = synthetic(PROCESS);
    for (old, new) in [
        (
            "this._internalNow = $KEY_250(",
            "this._internalNow = unknownClock(",
        ),
        (
            "let $KEY_250 = $THREAD_0_59817;",
            "let $KEY_250 = $THREAD_0_59817; $KEY_250 = unknownClock;",
        ),
        ("global.global === global", "global.global !== global"),
        ("function bootstrap() {", "function bootstrap(setTimeout) {"),
        ("function bootstrap() {", "function bootstrap(Number) {"),
    ] {
        refuses(&changed(&source, old, new));
    }
}

#[test]
fn class_alias_and_prototype_mutation_refuse() {
    for mutation in [
        "$KEY_63 = other;",
        "$KEY_63.prototype._process = other;",
        "$THREAD_4_63087.prototype.now = other;",
        "mutate($KEY_63);",
    ] {
        refuses(&changed(
            DIRECT_STORE_ALIAS,
            "$KEY_63 = $THREAD_4_63087;",
            &format!("$KEY_63 = $THREAD_4_63087; {mutation}"),
        ));
    }
}

const COUNT_FIRST: &str =
    include_str!("../../../tests/fixtures/program/thread-batch-scheduler-count-first.js");
const COUNT_FIRST_ALIAS: &str =
    include_str!("../../../tests/fixtures/program/thread-batch-scheduler-count-first-alternate.js");

#[test]
fn real_count_first_guard_and_closed_direct_store_alternative() {
    let allocator = oxc_allocator::Allocator::default();
    let program = crate::analysis::parse_program(&allocator, COUNT_FIRST).unwrap();
    let (scoping, nodes) = crate::analysis::analyze(&program);
    let actual = nodes
        .iter()
        .find_map(|n| {
            let AstKind::Class(c) = n.kind() else {
                return None;
            };
            c.body.body.iter().find_map(|e| {
                let ClassElement::MethodDefinition(m) = e else {
                    return None;
                };
                (property_key_name(&m.key).as_deref() == Some("_process"))
                    .then(|| method(&scoping, &m.value))
                    .flatten()
            })
        })
        .unwrap();
    // Reproduce the exact pre-extension failed guard on a real own method:
    // declaration-numbered binders differ, while the contrasting old order
    // continues to match its original reference.
    assert_ne!(actual, reference(&format!("class Pattern {{ {PROCESS} }}")));
    assert_eq!(
        actual,
        reference(&format!(
            "class Pattern {{ {} }}",
            PROCESS.replace(
                "let mean = 0;\n    let count = 0;",
                "let count = 0;\n    let mean = 0;"
            )
        ))
    );
    accepts(DIRECT_STORE_ALIAS);
    accepts(COUNT_FIRST);
    accepts(COUNT_FIRST_ALIAS);
    for (from, to) in [
        ("$AP * $AO", "$AO * $AP"),
        ("++$AO;", "++$AP;"),
        ("$AS + $AP <", "$AS + $AO <"),
        ("$AS - $AR", "$AR - $AS"),
        ("let $AO = 0;", "let $AO = sideEffect();"),
        ("let $AP = 0;", "let $AP = this.now();"),
        ("$AQ = $AL();", "$AQ = other();"),
        ("if ($AQ)", "if (true)"),
        (
            "this._process.bind(this, $AL)",
            "this._process.bind(null, $AL)",
        ),
        ("global.global === global", "global.global !== global"),
        ("function bootstrap() {", "function bootstrap(global) {"),
        (
            "let $KEY_289 = $THREAD_2_127412;",
            "let $KEY_289 = $THREAD_2_127412; $KEY_289 = other;",
        ),
        (
            "new $THREAD_3_75778(8);",
            "new $THREAD_3_75778(8); $THREAD_3_75778.prototype.now = other;",
        ),
    ] {
        refuses(&changed(COUNT_FIRST, from, to));
    }
    let declaration = "let $KEY_289 = $THREAD_2_127412;";
    let late = COUNT_FIRST.replace(declaration, "").replace(
        "new $THREAD_3_75778(8);",
        &format!("new $THREAD_3_75778(8); {declaration}"),
    );
    refuses(&late);
    // The old-count/continuing-mean protocol remains independently required.
    accepts(include_str!(
        "../../../tests/fixtures/program/thread-batch-scheduler-edge-count.js"
    ));
}

#[test]
fn staged_global_constructor_closes_host_and_native_clock() {
    let direct = synthetic(PROCESS);
    let staged = direct.replace(
        &format!("this._internalNow = $KEY_250({HOST});"),
        &format!("{} this._internalNow = $KEY_250(safe && global.global === global ? global : typeof self !== 'undefined' && self != null && self.self === self ? self : typeof window !== 'undefined' && window != null && window.window === window ? window : void 0);",
            "let safe; if (typeof global !== 'undefined') { safe = global != null; } else { safe = false; }"),
    );
    assert_ne!(direct, staged);
    accepts(&staged);
    accepts(
        &staged
            .replace("safe", "hostReady")
            .replace("budget", "duration"),
    );
    for process in [CARRIED_PROCESS, PROCESS] {
        let source = staged.replace(PROCESS, process);
        accepts(&source);
        for (from, to) in [
            ("global != null", "self != null"),
            ("safe = false", "safe = true"),
            ("global.global === global", "self.self === self"),
            ("window : void 0", "window : global"),
            ("function bootstrap()", "function bootstrap(global)"),
            ("function bootstrap()", "function bootstrap(Date)"),
            ("function bootstrap()", "function bootstrap(Number)"),
            (
                "$BR.performance.now.bind($BR.performance)",
                "$BR.performance.now.bind($BR)",
            ),
            ("return $KEY_2() - $KEY_0", "return $KEY_0 - $KEY_2()"),
            ("let $KEY_0 = $BS();", "let $KEY_0 = $BS(); $KEY_0 = 0;"),
            ("return Number(new Date())", "return Number(otherClock())"),
            (
                "safe = global != null;",
                "safe = global != null; otherEffect();",
            ),
            ("mean * count", "count * mean"),
            ("count + 1", "count + 2"),
            ("++count", "++mean"),
            (
                "this._process.bind(this, work)",
                "this._process.bind(null, work)",
            ),
            ("pending = work();", "pending = work(); sideEffect();"),
            (
                "let $KEY_250 = $THREAD_0_59817;",
                "let $KEY_250 = $THREAD_0_59817; $KEY_250 = other;",
            ),
        ] {
            refuses(&changed(&source, from, to));
        }
    }
    // A valid donor beside the class cannot fill a missing own process body.
    refuses(&staged.replace(PROCESS, "_process(work) { return work(); }"));
    let carried = staged.replace(PROCESS, CARRIED_PROCESS);
    refuses(&changed(
        &carried,
        "for (;; mean = next)",
        "for (;; count = next)",
    ));
    refuses(&changed(&carried, "++count;", "++count; mean = next;"));
}

const UPDATE_ADD: &str =
    include_str!("../../../tests/fixtures/program/thread-batch-scheduler-update-add.js");

#[test]
fn update_add_actual_update_add_whole_model_and_alpha() {
    accepts(UPDATE_ADD);
    accepts(&UPDATE_ADD.replace("$BM", "task").replace("$BP", "average")
        .replace("$BR", "savedNumericCount").replace("$THREAD_1_143030", "Scheduler"));
    let allocator = oxc_allocator::Allocator::default();
    let program = crate::analysis::parse_program(&allocator, UPDATE_ADD).unwrap();
    let (scoping, nodes) = crate::analysis::analyze(&program);
    let model = Model::new(UPDATE_ADD, &program, &scoping, &nodes,
        ProgramPropertiesLimits::default()).unwrap();
    let class = model.nodes.iter().find_map(|n| match n.kind() {
        AstKind::Class(c) => Some(c), _ => None,
    }).unwrap();
    let process = class.body.body.iter().find_map(|e| match e {
        ClassElement::MethodDefinition(m) if property_key_name(&m.key).as_deref() == Some("_process") =>
            method(model.scoping, &m.value),
        _ => None,
    }).unwrap();
    assert!(class_uses(&model, class.id.as_ref().unwrap().symbol_id.get().unwrap(),
        class.span, false, &mut BTreeSet::new()), "actual publication passes unchanged gate");
    assert!(grammar().process.iter().filter(|(_, e)| !e.is_some_and(|e| e.contains("saved numeric count plus one")))
        .all(|(p, _)| p != &process), "first failed old guard is the complete process grammar");
    assert_eq!(process, reference(&format!("class Pattern {{ {UPDATE_ADD_PROCESS} }}")));
    // This remains a separate protocol even when the arithmetic happens to

    // agree for immutable numeric inputs. No old grammar entry proves it.
    let actual = reference(&format!("class Pattern {{ {UPDATE_ADD_PROCESS} }}"));
    let matching: Vec<_> = grammar().process.iter().filter(|(p, _)| p == &actual).collect();
    assert_eq!(matching.len(), 1);
    assert!(matching[0].1.unwrap().contains("saved numeric count plus one"));
    println!("whole Model accepts only the complete ordered process; legacy process patterns do not match");
}

#[test]
fn update_add_order_arithmetic_shortcircuit_and_ports_refuse() {
    for (from, to) in [
        ("for (;; i = $BR + 1)", "for (;; i = $BR + 2)"),
        ("for (;; i = $BR + 1)", "for (;; i = Number(i) + 1)"),
        ("$BR = Number(i);", "$BR = Number(i); i = $BR + 1;"),
        ("$BR = Number(i);", "i = Number(i);"),
        ("$BP * i", "$BP * $BR"),
        ("/ (i + 1)", "/ ($BR + 1)"),
        ("/ (i + 1)", "/ i"),
        ("($BT - $BS)", "($BS - $BT)"),
        ("$BT + $BP <", "$BT + i <"),
        ("? $BQ :", "? !$BQ :"),
        ("? $BQ : $BT + $BP < $BO + $BN", "&& $BQ"),
        ("for (;; i = $BR + 1)", "while ($BQ)"),
        ("if ($BQ)", "if (!$BQ)"),
        ("if ($BQ)", "if (true)"),
        ("this._process.bind(this, $BM)", "this._process.bind(null, $BM)"),
        ("this._process.bind(this, $BM)", "this._process.bind(this, $BM, 1)"),
        ("this._process.bind(this, $BM), 0", "this._process.bind(this, $BM), 1"),
        ("this._process.bind(this, $BM), 0", "this._process.bind(this, $BM), 0, 1"),
        ("$BQ = $BM();", "$BQ = $BM(this);"),
        ("this.targetBatchTimeMs = $BM;", "this.targetBatchTimeMs = 8;"),
        ("return this._internalNow();", "return this._internalNow(1);"),
        ("$BR = Number(i);", "$BR = Number($BP);"),
        ("$BR = Number(i);", "$BR = otherNumber(i);"),
        ("_process($BM) {", "_process($BM, Number) {"),
    ] {
        refuses(&changed(UPDATE_ADD, from, to));
    }
    // Keep source coercion placement exact: both moves alter observable order.
    let coercion = "$BR = Number(i);";
    let mean = "$BP = ($BP * i + ($BT - $BS)) / (i + 1);";
    let without = UPDATE_ADD.replace(coercion, "");
    refuses(&changed(&without, mean, &format!("{coercion} {mean}")));
    let after = "let $BT = this.now();";
    refuses(&changed(UPDATE_ADD, "$BQ = $BM();", "")
        .replace(after, &format!("{after} $BQ = $BM();")));
    let before = "let $BS = this.now();";
    refuses(&changed(UPDATE_ADD, "$BQ = $BM();", "")
        .replace(before, &format!("$BQ = $BM(); {before}")));
}

#[test]
fn update_add_scopes_captures_publication_lifetimes_refuse() {
    for (from, to) in [
        ("$KEY_434 = $THREAD_6_20734;", "$KEY_434 = $THREAD_6_20734; $KEY_434 = otherClock;"),
        ("$KEY_434 = $THREAD_6_20734;", ""),
        ("typeof global", "typeof hiddenGlobal"),
        ("function bootstrap() {", "function bootstrap(global) {"),
        ("constructor($BM) {", "constructor($BM, global) {"),
        ("this._internalNow = $KEY_434(", "this._internalNow = unknownClock("),
        ("let $KEY_83;", "let $KEY_83 = other;"),
        ("$KEY_83 = $THREAD_1_143030;", "$KEY_83 = $THREAD_1_143030; $KEY_83 = other;"),
        ("$KEY_83 = $THREAD_1_143030;", "$KEY_83 = other; $KEY_83 = $THREAD_1_143030; $KEY_83 = other;"),
        ("class $THREAD_1_143030 {", "let early = $THREAD_1_143030; class $THREAD_1_143030 {"),
        ("$KEY_83 = $THREAD_1_143030;", "let early = $KEY_83; $KEY_83 = $THREAD_1_143030;"),
        ("class $THREAD_1_143030 {", "class $THREAD_1_143030 extends UnknownBase {"),
    ] {
        refuses(&changed(UPDATE_ADD, from, to));
    }
    let assignment = "$KEY_434 = $THREAD_6_20734;";
    refuses(&UPDATE_ADD.replace(assignment, "").replace("let scheduler =", &format!("{assignment} let scheduler =")));
}
