use shape::naming::{name_threads, ThreadName};

const BODY: &str = include_str!("fixtures/signal_report_holder/actual.js");
const TARGET: &str = "$THREAD_0_109096";
const CAPTURES: &str =
    "let $KEY_287=[], $KEY_17={}, $KEY_60=[], $KEY_152=[], $KEY_94=[], $KEY_221={};";
fn wrapped(body: &str) -> String {
    format!("function main(){{{CAPTURES}{body}return {TARGET};}}main();")
}
fn outcome(source: &str, target: &str) -> ThreadName {
    name_threads(source, true)
        .expect("valid model")
        .report
        .outcomes
        .into_iter()
        .find(|o| o.binding.name == target)
        .expect("owned function")
}
fn accepted(source: &str, target: &str) {
    let result = name_threads(source, true).unwrap();
    let o = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == target)
        .unwrap();
    assert!(o.applied, "{o:?}");
    assert_eq!(o.proposed_name.as_deref(), Some("getSignalSnapshot"));
    assert_eq!(o.evaluator.as_deref(), Some("signals_signal_snapshot"));
    let mut replay = source.to_string();
    let mut edits: Vec<_> = result.report.edits.iter().collect();
    edits.sort_by_key(|e| std::cmp::Reverse(e.start));
    for e in edits {
        replay.replace_range(e.start as usize..e.end as usize, &e.replacement);
    }
    assert_eq!(replay, result.source);
}
#[test]
fn actual_body_alpha_and_equivalent_declarations() {
    let source = wrapped(BODY);
    accepted(&source, TARGET);
    let mut alpha = source.clone();
    for (a, b) in [
        (TARGET, "$THREAD_99_876543"),
        ("$CL", "optionsValue"),
        ("$CM", "valuesList"),
        ("$CN", "namedList"),
        ("$CO", "unnamedList"),
        ("$CP", "numericList"),
        ("$CQ", "descriptor"),
        ("$CR", "receiver"),
        ("$CS", "method"),
        ("$CT", "payload"),
        ("$KEY_17", "callbacks"),
        ("$KEY_60", "rawValues"),
        ("$KEY_94", "descriptors"),
        ("$KEY_221", "failures"),
    ] {
        alpha = alpha.replace(a, b);
    }
    accepted(&alpha, "$THREAD_99_876543");
    // List roles come from return keys, not declaration order.
    for order in [["$CP", "$CO", "$CN"], ["$CO", "$CN", "$CP"]] {
        let s = source.replace(
            "let $CN = [];\n\t\t\t\tlet $CO = [];\n\t\t\t\tlet $CP = [];",
            &format!(
                "let {}=[];let {}=[];let {}=[];",
                order[0], order[1], order[2]
            ),
        );
        assert_ne!(source, s);
        accepted(&s, TARGET);
    }
    let s = source.replace(
        "$CS = $CO.push;\n\t\t\t\t\t\t$CR = $CO;",
        "$CR = $CO; $CS = $CO.push;",
    );
    assert_ne!(source, s);
    accepted(&s, TARGET);
}
#[test]
fn holder_producer_receiver_and_effect_mutations_refused() {
    for (from, to) in [
        ("$CS = $CR.push", "$CS = $CO.push"),
        (
            "$CS = $CR.push",
            "$CS = (typeof $CQ.name === \"number\" ? $CP : $CN).push",
        ),
        ("$CS.call($CR, $CT)", "$CS = $CO.push; $CS.call($CR, $CT)"),
        ("$CS.call($CR, $CT)", "$CR = $CO; $CS.call($CR, $CT)"),
        ("$CS.call($CR, $CT)", "$CS.call($CO, $CT)"),
        ("$CS.call($CR, $CT)", "$CS.call($CR, $CM[i])"),
        ("$CS.call($CR, $CT)", "$CS.call($CR, $CT, 1)"),
        (
            "$CS.call($CR, $CT)",
            "$CS.call($CR, $CT); $CS.call($CR, $CT)",
        ),
        ("$CS.call($CR, $CT)", "effect(); $CS.call($CR, $CT)"),
        ("? $CP : $CN", "? $CN : $CP"),
        (
            "typeof $CQ.name === \"number\"",
            "typeof $CQ.name !== \"number\"",
        ),
        (
            "hasOwnProperty.call($CQ, \"name\")",
            "hasOwnProperty.call($CN, \"name\")",
        ),
        (
            "Object.prototype.hasOwnProperty.call($CQ, \"name\")",
            "true",
        ),
        ("$CT = [$CQ.name, $CM[i]]", "$CT = [$CM[i], $CQ.name]"),
        ("$CT = $CM[i]", "$CT = [$CQ.name, $CM[i]]"),
        ("$CS = $CO.push", "$CS = $CP.push"),
        ("$CR = $CO", "$CR = $CN"),
        ("$KEY_221.toArray()", "$KEY_60.toArray()"),
        ("$KEY_221.toArray()", "$KEY_221.toArray.call($KEY_60)"),
        ("signals: $CN", "signals: $CP"),
        ("positionalSignals: $CO", "positionalSignals: $CN"),
        ("keyedSignals: $CP", "keyedSignals: $CO"),
        ("for (let i = 0;", "for (let i = 1;"),
        ("for (let i = 0;", "for (let i = 0, extra = effect();"),
        ("i < $CM.length", "i <= $CM.length"),
        ("i++)", "i--)"),
    ] {
        let b = BODY.replace(from, to);
        assert_ne!(b, BODY, "{from}");
        let o = outcome(&wrapped(&b), TARGET);
        assert!(!o.applied, "{from} -> {to}: {o:?}");
    }
    let b = BODY.replace(
        "$CS = $CR.push;\n\t\t\t\t\t\t$CT = [$CQ.name, $CM[i]];",
        "$CT = [$CQ.name, $CM[i]]; $CS = $CR.push;",
    );
    assert_ne!(b, BODY);
    assert!(
        !outcome(&wrapped(&b), TARGET).applied,
        "getter read order matters"
    );
}
#[test]
fn option_notification_debug_and_capture_mutations_refused() {
    for (from, to) in [
        ("$CK === void 0", "$CK == null"),
        ("includeDebugNames: false", "includeDebugNames: true"),
        ("notifyLastChance: true", "notifyLastChance: false"),
        ("signalFilter: $KEY_287", "signalFilter: $KEY_60"),
        (
            "$CL.notifyLastChance == null",
            "$CL.notifyLastChance === void 0",
        ),
        ("$KEY_17[$CV].call(void 0)", "$KEY_17[$CV].call(null)"),
        ("$KEY_17[$CV].call(void 0)", "$KEY_94[$CV].call(void 0)"),
        ("$KEY_17[$CV] != null", "$KEY_17[$CV] === null"),
        ("$KEY_60.map(($CV, $CW)", "$CL.signalFilter.map(($CV, $CW)"),
        ("signal: $KEY_152[$CW]", "signal: $KEY_152[$CV]"),
        ("value: $CV", "value: $KEY_60[$CW]"),
        (
            "$CL.signalFilter.map($CV => $KEY_60[$CV])",
            "$CL.signalFilter.map($CV => $KEY_152[$CV])",
        ),
        ("$KEY_94[$CL.signalFilter[i]]", "$KEY_94[i]"),
        ("$CL.includeDebugNames)", "$CL.includeDebugNames === false)"),
    ] {
        let b = BODY.replace(from, to);
        assert_ne!(b, BODY, "{from}");
        let o = outcome(&wrapped(&b), TARGET);
        assert!(!o.applied, "{from} -> {to}: {o:?}");
    }
    // Single-origin capture gate includes writers in callbacks and conditional
    // publication paths, not only stores in the builder's own body.
    for effect in [
        "$KEY_60=[];",
        "register(()=>{$KEY_17={};});",
        "if(flag){$KEY_152=[];}",
        "register(()=>{$KEY_94=[];});",
    ] {
        let s = wrapped(BODY).replace(
            &format!("return {TARGET};"),
            &format!("{effect}return {TARGET};"),
        );
        assert!(!outcome(&s, TARGET).applied, "{effect}");
    }
    let s = wrapped(BODY).replace("$KEY_17={}", "$KEY_17");
    assert!(
        !outcome(&s, TARGET).applied,
        "missing callback-table origin"
    );
}

#[test]
fn report_and_preloop_snapshot_protocols_preserved() {
    let source = include_str!("fixtures/signal_report_holder/baseline.js");
    let o = outcome(source, "$THREAD_8_505");
    assert!(o.applied);
    assert_eq!(o.proposed_name.as_deref(), Some("buildSignalReport"));
    assert_eq!(
        o.evaluator.as_deref(),
        Some("signals_signal_report_builder")
    );
    let preloop = BODY.replace("for (let i = 0;", "let i = 0; for (;");
    assert_ne!(preloop, BODY);
    accepted(&wrapped(&preloop), TARGET);
}
