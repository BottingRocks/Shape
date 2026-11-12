//! Focused, portable coverage for the custom cookie-collector registration
//! pair: the zero-formal stage-registrar plugin a collector's `ctor` returns and
//! the one-formal adapter that wraps it with the captured ctor factory.
//!
//! The inner cookie callback keeps its existing `emitMatchingCookieValue` name
//! and the document capture keeps `captureGlobalDocument`; this rule claims only
//! the plugin wrapper and its ctor adapter. Negatives change the setup registrar,
//! the cookie dataflow, the stage-object shape and the captured factory, add an
//! unused nested donor, and drop the flag / error gate. Existing readable names
//! and collisions are preserved.
use shape::naming::{name_threads, NamedThreads, ThreadName};

const EVALUATOR: &str = "cookie_plugin_adapter";
const PLUGIN: &str = "createCookieCollectorPlugin";
const ADAPTER: &str = "createCookieCollectorCtorAdapter";
const COOKIE: &str = "emitMatchingCookieValue";
const CAPTURE: &str = "captureGlobalDocument";

const POSITIVE: &str = r##"function bootstrap(){
let ctorFactory;
function $THREAD_9_50177(){
function $THREAD_2_119779($BS){
let $KEY_3 = $BS;
$KEY_0.cookie.split('; ').forEach($BT => {
let $BU = $BT.split('=');
if ($BU.length > 0 && $BU[0] === 'session') {
$KEY_3.next({ value: $BU[1] });
$KEY_3.complete();
}
});
$KEY_3.next({});
$KEY_3.complete();
}
let $BR = { setup: void 0, immediately: void 0 };
let $KEY_0;
function $THREAD_6_101230($BS){
$KEY_0 = $BS.global.document;
}
$BR.setup = $THREAD_6_101230;
$BR.immediately = $THREAD_2_119779;
return $BR;
}
function $THREAD_1_113370($BR){
let $KEY_4 = $BR;
return ctorFactory(($BS, $BT, $BU, $BV, $BW) => {
let $BX = !!$KEY_4.needsDOMReady;
let $BY, $BZ, $CA, $CB, $KEY_9;
try {
if ($KEY_4.setup) { $KEY_4.setup($BS); }
$BY = $BT;
$BZ = $BV;
$CA = $BW;
$CB = $BU;
} catch (e) {
$KEY_9 = e;
$BY = $BT;
$BZ = $BV;
$CA = $BW;
$CB = $BU;
}
if ($KEY_4.immediately) { $BY($CC => { if ($KEY_9 != null) { $CC.error($KEY_9); return void 0; } else { $KEY_4.immediately($CC); return void 0; } }); }
if ($KEY_4.afterReady) { $CB($CC => { if ($KEY_9 != null) { $CC.error($KEY_9); return void 0; } else { $KEY_4.afterReady($CC); return void 0; } }, $BX); }
if ($KEY_4.lastChance) { $BZ($CC => { if ($KEY_9 != null) { $CC.error($KEY_9); return void 0; } else { $KEY_4.lastChance($CC); return void 0; } }); }
if ($KEY_4.teardown) { $CA(() => { if ($KEY_9 != null) { return void 0; } else { $KEY_4.teardown(); return void 0; } }); }
});
}
$THREAD_9_50177();
$THREAD_1_113370(null);
}
bootstrap();"##;

// Alpha variant: reordered children and assignments, renamed identifiers and
// reordered stage gates. The stage-object shape and the captured factory are
// unchanged, so both roles stay proven.
const ALPHA: &str = r##"function bootstrap(){
let $KEY_800;
function $THREAD_903_4($KEY_820){
let $KEY_821 = $KEY_820;
return $KEY_800(($KEY_830, $KEY_831, $KEY_832, $KEY_833, $KEY_834) => {
let $KEY_835 = !!$KEY_821.needsDOMReady;
let $KEY_836, $KEY_837, $KEY_838, $KEY_839, $KEY_840;
try {
if ($KEY_821.setup) { $KEY_821.setup($KEY_830); }
$KEY_836 = $KEY_831;
$KEY_837 = $KEY_833;
$KEY_838 = $KEY_834;
$KEY_839 = $KEY_832;
} catch (caught) {
$KEY_840 = caught;
$KEY_836 = $KEY_831;
$KEY_837 = $KEY_833;
$KEY_838 = $KEY_834;
$KEY_839 = $KEY_832;
}
if ($KEY_821.teardown) { $KEY_838(() => { if ($KEY_840 != null) { return void 0; } else { $KEY_821.teardown(); return void 0; } }); }
if ($KEY_821.immediately) { $KEY_836($KEY_841 => { if ($KEY_840 != null) { $KEY_841.error($KEY_840); return void 0; } else { $KEY_821.immediately($KEY_841); return void 0; } }); }
if ($KEY_821.afterReady) { $KEY_839($KEY_841 => { if ($KEY_840 != null) { $KEY_841.error($KEY_840); return void 0; } else { $KEY_821.afterReady($KEY_841); return void 0; } }, $KEY_835); }
if ($KEY_821.lastChance) { $KEY_837($KEY_841 => { if ($KEY_840 != null) { $KEY_841.error($KEY_840); return void 0; } else { $KEY_821.lastChance($KEY_841); return void 0; } }); }
});
}
function $THREAD_900_1(){
let $KEY_810 = { setup: void 0, immediately: void 0 };
let $KEY_811;
function $THREAD_902_3($KEY_812){
$KEY_811 = $KEY_812.global.document;
}
function $THREAD_901_2($KEY_813){
let $KEY_814 = $KEY_813;
$KEY_811.cookie.split('; ').forEach($KEY_815 => {
let $KEY_816 = $KEY_815.split('=');
if ($KEY_816.length > 0 && $KEY_816[0] === 'theme') {
$KEY_814.next({ value: $KEY_816[1] });
$KEY_814.complete();
}
});
$KEY_814.next({});
$KEY_814.complete();
}
$KEY_810.immediately = $THREAD_901_2;
$KEY_810.setup = $THREAD_902_3;
return $KEY_810;
}
$THREAD_900_1();
$THREAD_903_4(null);
}
bootstrap();"##;

fn target_outcome<'a>(result: &'a NamedThreads, target: &str) -> &'a ThreadName {
    result
        .report
        .outcomes
        .iter()
        .find(|outcome| outcome.binding.name == target)
        .unwrap_or_else(|| panic!("target binding {target} missing from outcomes"))
}

fn our_claim(source: &str, target: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        target_outcome(&result, target).evaluator.as_deref() == Some(EVALUATOR)
    })
}

fn named(source: &str, target: &str, name: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        let outcome = target_outcome(&result, target);
        outcome.applied && outcome.proposed_name.as_deref() == Some(name)
    })
}

fn parses(source: &str) -> bool {
    match name_threads(source, true) {
        Ok(_) => true,
        Err(error) => {
            eprintln!("name_threads error: {error}");
            false
        }
    }
}

fn denied(source: &str, target: &str) {
    assert!(parses(source), "fixture did not parse:\n{source}");
    assert!(!our_claim(source, target), "unexpected claim for:\n{source}");
    assert!(!named(source, target, PLUGIN), "unexpected {PLUGIN}:\n{source}");
    assert!(!named(source, target, ADAPTER), "unexpected {ADAPTER}:\n{source}");
}

fn refuses(template: &str, target: &str, changes: &[(&str, &str)]) {
    for &(from, to) in changes {
        assert!(template.contains(from), "missing replacement {from}");
        denied(&template.replace(from, to), target);
    }
}

fn claims(source: &str, target: &str, name: &str) {
    assert!(parses(source), "fixture did not parse:\n{source}");
    assert!(our_claim(source, target), "not our claim for {target}:\n{source}");
    assert!(named(source, target, name), "not named {name} for {target}:\n{source}");
}

fn named_only(source: &str, target: &str, name: &str) {
    assert!(parses(source), "fixture did not parse:\n{source}");
    assert!(named(source, target, name), "not named {name} for {target}:\n{source}");
}

#[test]
fn witness_plugin_and_adapter_are_claimed() {
    claims(POSITIVE, "$THREAD_9_50177", PLUGIN);
    claims(POSITIVE, "$THREAD_1_113370", ADAPTER);
}

#[test]
fn existing_inner_names_are_preserved() {
    named_only(POSITIVE, "$THREAD_2_119779", COOKIE);
    named_only(POSITIVE, "$THREAD_6_101230", CAPTURE);
    named_only(ALPHA, "$THREAD_901_2", COOKIE);
    named_only(ALPHA, "$THREAD_902_3", CAPTURE);
}

#[test]
fn alpha_variant_is_claimed_independently_of_spelling_and_order() {
    claims(ALPHA, "$THREAD_900_1", PLUGIN);
    claims(ALPHA, "$THREAD_903_4", ADAPTER);
}

#[test]
fn plugin_rejects_wrong_setup_cookie_dataflow_and_capture() {
    refuses(
        POSITIVE,
        "$THREAD_9_50177",
        &[
            (
                "$KEY_0 = $BS.global.document;",
                "$KEY_0 = $BS.global.body;",
            ),
            ("$KEY_0.cookie.split('; ')", "$KEY_0.storage.split('; ')"),
            (
                "$KEY_0 = $BS.global.document;",
                "$KEY_0 = $BS.global.document;\n$KEY_0 = window.document;",
            ),
        ],
    );
}

#[test]
fn plugin_rejects_flag_abi_unknown_keys_and_unused_donor() {
    refuses(
        POSITIVE,
        "$THREAD_9_50177",
        &[
            (
                "{ setup: void 0, immediately: void 0 }",
                "{ setup: false, immediately: void 0 }",
            ),
            (
                "{ setup: void 0, immediately: void 0 }",
                "{ setup: void 0, immediately: void 0, priority: false }",
            ),
            (
                "{ setup: void 0, immediately: void 0 }",
                "{ setup: void 0 }",
            ),
        ],
    );
    denied(
        &POSITIVE.replace(
            "return $BR;\n}",
            "function $THREAD_7_700(){return 1;}\nreturn $BR;\n}",
        ),
        "$THREAD_9_50177",
    );
}

#[test]
fn adapter_rejects_uncaptured_factory_and_non_arrow_stage_registrar() {
    refuses(
        POSITIVE,
        "$THREAD_1_113370",
        &[
            ("return ctorFactory((", "return unboundFactory(("),
            (
                "return ctorFactory(($BS, $BT, $BU, $BV, $BW) => {",
                "return ctorFactory(function ($BS, $BT, $BU, $BV, $BW) {",
            ),
            (
                "return ctorFactory(($BS, $BT, $BU, $BV, $BW) => {",
                "return ctorFactory(($BS, $BT, $BU, $BV) => {",
            ),
        ],
    );
}

#[test]
fn adapter_rejects_missing_flag_error_gate_and_wrong_setup() {
    refuses(
        POSITIVE,
        "$THREAD_1_113370",
        &[
            (
                "let $BX = !!$KEY_4.needsDOMReady;",
                "let $BX = false;",
            ),
            (
                "$BY($CC => { if ($KEY_9 != null) { $CC.error($KEY_9); return void 0; } else { $KEY_4.immediately($CC); return void 0; } });",
                "$BY($CC => { $KEY_4.immediately($CC); });",
            ),
            (
                "if ($KEY_4.setup) { $KEY_4.setup($BS); }",
                "if ($KEY_4.setup) { $KEY_4.other($BS); }",
            ),
        ],
    );
}

#[test]
fn human_names_and_collisions_are_preserved() {
    let human = POSITIVE
        .replace("$THREAD_9_50177", "humanCookiePlugin")
        .replace("$THREAD_1_113370", "humanCookieAdapter");
    let result = name_threads(&human, true).unwrap();
    assert!(result.source.contains("function humanCookiePlugin("));
    assert!(!result
        .report
        .outcomes
        .iter()
        .any(|outcome| outcome.binding.name == "humanCookiePlugin" && outcome.applied));
    assert!(named(&human, "$THREAD_2_119779", COOKIE));

    let collision = POSITIVE.replace(
        "function bootstrap(){",
        "function bootstrap(){\"let createCookieCollectorPlugin=17\";createCookieCollectorPlugin;",
    );
    assert!(parses(&collision));
    let result = name_threads(&collision, true).unwrap();
    let plugin = target_outcome(&result, "$THREAD_9_50177");
    assert!(!plugin.applied);
    assert_eq!(plugin.reason.as_deref(), Some("NameCollision"));
    assert!(result.source.contains("let createCookieCollectorPlugin=17"));
    assert!(result.source.contains("$THREAD_9_50177"));
}
