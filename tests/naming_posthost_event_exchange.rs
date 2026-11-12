use shape::naming::name_threads;

// Fast-loop family: custom-event exchange handshake bootstrap.
const EVALUATOR: &str = "posthost_event_exchange_handshake";
const NAME: &str = "bootstrapEventExchangeHandshake";
const TARGET: &str = "$THREAD_2_55708";

// Portable copy of the exchange handshake. Every consumer input is a scoped
// declaration: the created receiver and the exchange/capture callbacks are
// locals of the outer function, the instrumented event-name array and the
// session class are captured from the enclosing scope.
const POSITIVE: &str = r#"function host(){
let $KEY_50 = ["x", "y"];
let $KEY_279 = class { init(){} };
function $THREAD_2_55708($BO, $BP, $BQ, $BR, $BS, $BT, $BU, $BV, $BW, $BX){
function $THREAD_1_111406($CC){ $CC.preventDefault(); }
let $BY = document.createEvent("CustomEvent");
let $KEY_4 = [];
let $KEY_12 = [];
let $KEY_11 = 0;
function $THREAD_9_115548($CC, $CD, $CE){ $KEY_4 = $CC; $KEY_12 = $CD; $KEY_11 = $CE; }
let $BZ = { exchange: $THREAD_9_115548 };
let $CA = 0;
while ($CA < $KEY_50.length){
$BY.initCustomEvent($KEY_50[$CA], false, true, $BZ);
if (dispatchEvent($BY)){ ++$CA; continue; } else { return void 0; }
}
let i = 0;
for (; i < $KEY_50.length; i++){
addEventListener($KEY_50[i], $THREAD_1_111406, true);
}
let $CB = new $KEY_279($BO, $BP, $BQ, $BR, $BS, $BT, $BU, $BV, $BW, $BX, $KEY_4, $KEY_12, $KEY_11);
$CB.init();
}
return $THREAD_2_55708;
}
host();"#;

// Alpha/spelling variant: generated thread names respelled, every local and
// captured spelling changed, declaration order rearranged, and the capture loop
// declares its own counter. Only symbol identity may decide the family.
const ALPHA: &str = r#"function host(){
let $HK_50 = ["x", "y"];
let $HK_279 = class { init(){} };
function $THREAD_90_777_2($P0, $P1, $P2, $P3, $P4, $P5, $P6, $P7, $P8, $P9){
let $R0 = document.createEvent("CustomEvent");
function $THREAD_42_31337($E0){ $E0.preventDefault(); }
let $S2 = 0;
let $S0 = [];
let $S1 = [];
function $THREAD_43_31337($E0, $E1, $E2){ $S2 = $E2; $S0 = $E0; $S1 = $E1; }
let $D0 = { exchange: $THREAD_43_31337 };
let $C0 = 0;
while ($C0 < $HK_50.length){
$R0.initCustomEvent($HK_50[$C0], false, true, $D0);
if (dispatchEvent($R0)){ ++$C0; continue; } else { return void 0; }
}
for (let i = 0; i < $HK_50.length; i++){
addEventListener($HK_50[i], $THREAD_42_31337, true);
}
let $X0 = new $HK_279($P0, $P1, $P2, $P3, $P4, $P5, $P6, $P7, $P8, $P9, $S0, $S1, $S2);
$X0.init();
}
return $THREAD_90_777_2;
}
host();"#;

// Node-shape variant: the init is lowered to `[receiver, receiver.initCustomEvent]
// .call(receiver, ...)`, and the detail placeholder is filled by a later store.
const NODE_SHAPE: &str = r#"function host(){
let $HK_50 = ["x", "y"];
let $HK_279 = class { init(){} };
function $THREAD_2_55708($F0, $F1, $F2, $F3, $F4, $F5, $F6, $F7, $F8, $F9){
function $THREAD_1_111406($CC){ $CC.preventDefault(); }
let $BY = document.createEvent("CustomEvent");
let $KEY_4 = [];
let $KEY_12 = [];
let $KEY_11 = 0;
function $THREAD_9_115548($CC, $CD, $CE){ $KEY_4 = $CC; $KEY_12 = $CD; $KEY_11 = $CE; }
let $BZ = { exchange: void 0 };
$BZ.exchange = $THREAD_9_115548;
let $BB = [$BY, $BY.initCustomEvent];
let $CA = 0;
while ($CA < $HK_50.length){
$BB[1].call($BB[0], $HK_50[$CA], false, true, $BZ);
if (dispatchEvent($BY)){ ++$CA; continue; } else { return void 0; }
}
let i = 0;
for (; i < $HK_50.length; i++){
addEventListener($HK_50[i], $THREAD_1_111406, true);
}
let $CB = new $HK_279($F0, $F1, $F2, $F3, $F4, $F5, $F6, $F7, $F8, $F9, $KEY_4, $KEY_12, $KEY_11);
$CB.init();
}
return $THREAD_2_55708;
}
host();"#;

// Extra-statement variant: the bootstrap is interleaved with unrelated owned
// work, so statement order and unrelated statements never decide the family.
const INTERLEAVED: &str = r#"function host(){
let $KEY_50 = ["x", "y"];
let $KEY_279 = class { init(){} };
function $THREAD_2_55708($BO, $BP, $BQ, $BR, $BS, $BT, $BU, $BV, $BW, $BX){
function $THREAD_1_111406($CC){ $CC.preventDefault(); }
let $BY = document.createEvent("CustomEvent");
let $KEY_4 = [];
let $KEY_12 = [];
let $KEY_11 = 0;
function $THREAD_9_115548($CC, $CD, $CE){ $KEY_4 = $CC; $KEY_12 = $CD; $KEY_11 = $CE; }
let $BZ = { exchange: $THREAD_9_115548 };
for (let k = 0; k < $KEY_50.length; k++){ console.log($KEY_50[k]); }
let $CA = 0;
while ($CA < $KEY_50.length){
$BY.initCustomEvent($KEY_50[$CA], false, true, $BZ);
if (dispatchEvent($BY)){ ++$CA; continue; } else { return void 0; }
}
let i = 0;
for (; i < $KEY_50.length; i++){
addEventListener($KEY_50[i], $THREAD_1_111406, true);
}
let $CB = new $KEY_279($BO, $BP, $BQ, $BR, $BS, $BT, $BU, $BV, $BW, $BX, $KEY_4, $KEY_12, $KEY_11);
$CB.init();
$KEY_279.prototype = $KEY_279;
}
return $THREAD_2_55708;
}
host();"#;

// The instrumented event-name array is a local of the bootstrap, not a captured
// consumer binding.
const LOCAL_NAMES: &str = r#"function host(){
let $KEY_279 = class { init(){} };
function $THREAD_2_55708($BO, $BP, $BQ, $BR, $BS, $BT, $BU, $BV, $BW, $BX){
function $THREAD_1_111406($CC){ $CC.preventDefault(); }
let $BY = document.createEvent("CustomEvent");
let $KEY_50 = ["x", "y"];
let $KEY_4 = [];
let $KEY_12 = [];
let $KEY_11 = 0;
function $THREAD_9_115548($CC, $CD, $CE){ $KEY_4 = $CC; $KEY_12 = $CD; $KEY_11 = $CE; }
let $BZ = { exchange: $THREAD_9_115548 };
let $CA = 0;
while ($CA < $KEY_50.length){
$BY.initCustomEvent($KEY_50[$CA], false, true, $BZ);
if (dispatchEvent($BY)){ ++$CA; continue; } else { return void 0; }
}
let i = 0;
for (; i < $KEY_50.length; i++){
addEventListener($KEY_50[i], $THREAD_1_111406, true);
}
let $CB = new $KEY_279($BO, $BP, $BQ, $BR, $BS, $BT, $BU, $BV, $BW, $BX, $KEY_4, $KEY_12, $KEY_11);
$CB.init();
}
return $THREAD_2_55708;
}
host();"#;

// The capture-phase loop installs the listener on a different array than the
// probe loop dispatches over.
const SECOND_ARRAY: &str = r#"function host(){
let $KEY_50 = ["x", "y"];
let $KEY_51 = ["x", "y"];
let $KEY_279 = class { init(){} };
function $THREAD_2_55708($BO, $BP, $BQ, $BR, $BS, $BT, $BU, $BV, $BW, $BX){
function $THREAD_1_111406($CC){ $CC.preventDefault(); }
let $BY = document.createEvent("CustomEvent");
let $KEY_4 = [];
let $KEY_12 = [];
let $KEY_11 = 0;
function $THREAD_9_115548($CC, $CD, $CE){ $KEY_4 = $CC; $KEY_12 = $CD; $KEY_11 = $CE; }
let $BZ = { exchange: $THREAD_9_115548 };
let $CA = 0;
while ($CA < $KEY_50.length){
$BY.initCustomEvent($KEY_50[$CA], false, true, $BZ);
if (dispatchEvent($BY)){ ++$CA; continue; } else { return void 0; }
}
let i = 0;
for (; i < $KEY_51.length; i++){
addEventListener($KEY_51[i], $THREAD_1_111406, true);
}
let $CB = new $KEY_279($BO, $BP, $BQ, $BR, $BS, $BT, $BU, $BV, $BW, $BX, $KEY_4, $KEY_12, $KEY_11);
$CB.init();
}
return $THREAD_2_55708;
}
host();"#;

// The probe loop lives in a nested decoy function, so the bootstrap's own body
// no longer owns the handshake.
const NESTED_DECOY: &str = r#"function host(){
let $KEY_50 = ["x", "y"];
let $KEY_279 = class { init(){} };
function $THREAD_2_55708($BO, $BP, $BQ, $BR, $BS, $BT, $BU, $BV, $BW, $BX){
function $THREAD_1_111406($CC){ $CC.preventDefault(); }
let $BY = document.createEvent("CustomEvent");
let $KEY_4 = [];
let $KEY_12 = [];
let $KEY_11 = 0;
function $THREAD_9_115548($CC, $CD, $CE){ $KEY_4 = $CC; $KEY_12 = $CD; $KEY_11 = $CE; }
let $BZ = { exchange: $THREAD_9_115548 };
function $THREAD_3_9(){
let $CA = 0;
while ($CA < $KEY_50.length){
$BY.initCustomEvent($KEY_50[$CA], false, true, $BZ);
if (dispatchEvent($BY)){ ++$CA; continue; } else { return void 0; }
}
}
$THREAD_3_9();
let i = 0;
for (; i < $KEY_50.length; i++){
addEventListener($KEY_50[i], $THREAD_1_111406, true);
}
let $CB = new $KEY_279($BO, $BP, $BQ, $BR, $BS, $BT, $BU, $BV, $BW, $BX, $KEY_4, $KEY_12, $KEY_11);
$CB.init();
}
return $THREAD_2_55708;
}
host();"#;

// The exchange callback is a captured helper rather than an owned nested
// function, so it cannot donate the session slots.
const CAPTURED_CALLBACK: &str = r#"function host(){
let $KEY_50 = ["x", "y"];
let $KEY_279 = class { init(){} };
function $THREAD_9_115548($CC, $CD, $CE){ this.values = $CC; }
function $THREAD_2_55708($BO, $BP, $BQ, $BR, $BS, $BT, $BU, $BV, $BW, $BX){
function $THREAD_1_111406($CC){ $CC.preventDefault(); }
let $BY = document.createEvent("CustomEvent");
let $KEY_4 = [];
let $KEY_12 = [];
let $KEY_11 = 0;
let $BZ = { exchange: $THREAD_9_115548 };
let $CA = 0;
while ($CA < $KEY_50.length){
$BY.initCustomEvent($KEY_50[$CA], false, true, $BZ);
if (dispatchEvent($BY)){ ++$CA; continue; } else { return void 0; }
}
let i = 0;
for (; i < $KEY_50.length; i++){
addEventListener($KEY_50[i], $THREAD_1_111406, true);
}
let $CB = new $KEY_279($BO, $BP, $BQ, $BR, $BS, $BT, $BU, $BV, $BW, $BX, $KEY_4, $KEY_12, $KEY_11);
$CB.init();
}
return $THREAD_2_55708;
}
host();"#;

fn named(source: &str, target: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        result.report.outcomes.iter().any(|outcome| {
            outcome.applied
                && outcome.binding.name == target
                && outcome.proposed_name.as_deref() == Some(NAME)
        })
    })
}

fn our_claim(source: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        result.report.function_roles.iter().any(|role| {
            role.evaluators.iter().any(|evaluator| evaluator == EVALUATOR)
        })
    })
}

fn parses(source: &str) -> bool {
    name_threads(source, true).is_ok()
}

fn denied(source: &str, target: &str) {
    assert!(parses(source), "fixture did not parse: {source}");
    assert!(!named(source, target), "unexpected {NAME} for: {source}");
    assert!(!our_claim(source), "unexpected exchange-handshake claim for: {source}");
}

fn refuses(template: &str, target: &str, changes: &[(&str, &str)]) {
    for &(from, to) in changes {
        assert!(template.contains(from), "missing replacement {from}");
        denied(&template.replace(from, to), target);
    }
}

#[test]
fn accepts_portable_family_shapes() {
    for (label, source, target) in [
        ("positive", POSITIVE, TARGET),
        ("alpha", ALPHA, "$THREAD_90_777_2"),
        ("node_shape", NODE_SHAPE, TARGET),
        ("interleaved", INTERLEAVED, TARGET),
    ] {
        assert!(parses(source), "{label} did not parse");
        assert!(named(source, target), "{label} was not named");
        assert!(our_claim(source), "{label} was not our claim");
    }
}

#[test]
fn claims_only_the_outer_bootstrap() {
    let result = name_threads(POSITIVE, true).unwrap();
    let claimed: Vec<_> = result
        .report
        .function_roles
        .iter()
        .filter(|role| role.evaluators.iter().any(|e| e == EVALUATOR))
        .collect();
    assert_eq!(claimed.len(), 1, "expected exactly the outer bootstrap");
    assert_eq!(claimed[0].current_name.as_deref(), Some(TARGET));
}

#[test]
fn denies_wrong_receivers_details_and_names() {
    refuses(
        POSITIVE,
        TARGET,
        &[
            // The cancel guard dispatches the detail object instead of the receiver.
            ("if (dispatchEvent($BY))", "if (dispatchEvent($BZ))"),
            // The probe event is initialized on the detail object.
            ("$BY.initCustomEvent($KEY_50[$CA]", "$BZ.initCustomEvent($KEY_50[$CA]"),
            // The detail key is not `exchange`.
            ("{ exchange: $THREAD_9_115548 }", "{ handler: $THREAD_9_115548 }"),
            // The detail value is a captured class rather than an owned callback.
            ("{ exchange: $THREAD_9_115548 }", "{ exchange: $KEY_279 }"),
            // The capture flag is lowered away.
            (
                "addEventListener($KEY_50[i], $THREAD_1_111406, true)",
                "addEventListener($KEY_50[i], $THREAD_1_111406, false)",
            ),
            // The listener is not the owned preventDefault stub.
            ("$CC.preventDefault();", "$CC.stopPropagation();"),
            // The capture loop installs no listener.
            (
                "addEventListener($KEY_50[i], $THREAD_1_111406, true);",
                "void 0;",
            ),
        ],
    );
    denied(&LOCAL_NAMES, TARGET);
    denied(&SECOND_ARRAY, TARGET);
}

#[test]
fn denies_disconnected_and_decoy_dataflow() {
    refuses(
        POSITIVE,
        TARGET,
        &[
            // The cancel guard no longer aborts to undefined.
            ("return void 0;", "return 0;"),
            // The callback copies a computed value instead of a formal.
            ("$KEY_4 = $CC;", "$KEY_4 = $CC + 1;"),
            // A session slot is dropped from the constructor.
            (", $KEY_4, $KEY_12, $KEY_11);", ", $KEY_4, $KEY_12);"),
            // The session instance is never initialized.
            ("$CB.init();", ""),
            // The session class is a bootstrap local, not a captured binding.
            ("new $KEY_279(", "new $BZ("),
        ],
    );
    // The constructor receives different bindings than the callback writes.
    let disconnected = POSITIVE
        .replace(
            "let $CB = new $KEY_279($BO, $BP, $BQ, $BR, $BS, $BT, $BU, $BV, $BW, $BX, $KEY_4, $KEY_12, $KEY_11);",
            "let $CB = new $KEY_279($BO, $BP, $BQ, $BR, $BS, $BT, $BU, $BV, $BW, $BX, $KEY_6, $KEY_7, $KEY_8);",
        )
        .replace(
            "let $KEY_11 = 0;",
            "let $KEY_11 = 0;\nlet $KEY_6 = [], $KEY_7 = [], $KEY_8 = 0;",
        );
    denied(&disconnected, TARGET);
    denied(&NESTED_DECOY, TARGET);
    denied(&CAPTURED_CALLBACK, TARGET);
}

#[test]
fn denies_shadowed_host_bindings() {
    denied(
        &POSITIVE.replace(
            "function host(){\n",
            "function host(){\nlet dispatchEvent = function(){ return true; };\n",
        ),
        TARGET,
    );
    denied(
        &POSITIVE.replace(
            "function host(){\n",
            "function host(){\nlet addEventListener = function(){ };\n",
        ),
        TARGET,
    );
    denied(
        &POSITIVE.replace(
            "function host(){\n",
            "function host(){\nlet document = { createEvent: function(){ return {}; } };\n",
        ),
        TARGET,
    );
    // The listener is a captured helper rather than an owned nested function.
    let captured_listener = POSITIVE
        .replace("function $THREAD_1_111406($CC){ $CC.preventDefault(); }\n", "")
        .replace(
            "function host(){\n",
            "function host(){\nfunction $THREAD_1_111406($CC){ $CC.preventDefault(); }\n",
        );
    denied(&captured_listener, TARGET);
}

#[test]
fn preserves_human_names_and_collision_guards() {
    let human = POSITIVE.replace(TARGET, NAME);
    let result = name_threads(&human, true).unwrap();
    assert!(result.source.contains(&format!("function {NAME}(")));
    assert!(!result
        .report
        .outcomes
        .iter()
        .any(|outcome| outcome.binding.name == NAME && outcome.applied));

    for collision in [
        POSITIVE.replace(
            "function host(){\n",
            &format!("function host(){{let {NAME}=17;useValue({NAME});\n"),
        ),
        POSITIVE.replace(
            "function host(){\n",
            &format!("function host(){{function {NAME}(){{}}useValue({NAME});\n"),
        ),
    ] {
        assert!(parses(&collision), "{collision}");
        assert!(!named(&collision, TARGET), "{collision}");
    }
}

#[test]
fn edits_are_identifier_only_replacements() {
    let source = POSITIVE.replace("$THREAD_9_115548", "applyExchangeValues");
    let result = name_threads(&source, true).unwrap();
    let applied: Vec<_> = result
        .report
        .outcomes
        .iter()
        .filter(|outcome| outcome.applied && outcome.proposed_name.as_deref() == Some(NAME))
        .collect();
    assert_eq!(applied.len(), 1, "expected exactly one applied target");
    let mut replay = source;
    let mut edits: Vec<_> = result.report.edits.iter().collect();
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.start));
    for edit in edits {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, result.source);
}

// The capture-phase handler can be the owned exchange-teardown sibling rather than an
// inline preventDefault stub. This witness keeps that sibling and the 11-argument runtime
// constructor. The enclosing scope supplies the captured names array and runtime class.
const OWNED_TEARDOWN_WITNESS_TARGET: &str = "$THREAD_3_68119";
const OWNED_TEARDOWN_WITNESS: &str = r#"function host(){
let $KEY_19 = ["x", "y"];
let $KEY_139 = class { init(){} };
function $THREAD_3_68119($BZ, $CA, $CB, $CC, $CD, $CE, $CF, $CG, $CH, $CI) {
					let $KEY_9 = $BZ;
					let $KEY_11, $KEY_12, $KEY_15, $KEY_0, $KEY_2, $KEY_6;
					function $THREAD_1_6187($CN) {
						let i = 0;
						for (; i < $KEY_19.length; i++) {
							removeEventListener($KEY_19[i], $KEY_0, true);
						}
						clearTimeout(0);
						const $CO = $KEY_15 ? $KEY_12 + (Date.now() - $KEY_15) / 1e3 / 60 : -1;
						if ($CN.detail != null && typeof $CN.detail.exchange === "function") {
							$KEY_2.push("AECdLXChAQAA_N2P8qmtrsbUIY-Ueq12pE8e2W2z0FtugcHAEdC0aOhVBF8k");
							$KEY_11.push($KEY_9);
							$CN.detail.exchange($KEY_2, $KEY_11, $CO);
						}
						$KEY_6.teardown();
					}
					$KEY_0 = $THREAD_1_6187;
					let $CJ = document.createEvent("CustomEvent");
					$KEY_2 = [];
					$KEY_11 = [];
					$KEY_12 = 0;
					function $THREAD_4_18341($CN, $CO, $CP) {
						$KEY_2 = $CN;
						$KEY_11 = $CO;
						$KEY_12 = $CP;
					}
					let $CK = { exchange: $THREAD_4_18341 };
					let $CL = 0;
					while ($CL < $KEY_19.length) {
						let $CM = [$CJ, $CJ.initCustomEvent];
						$CM[1].call($CM[0], $KEY_19[$CL], false, true, $CK);
						if (dispatchEvent($CJ)) {
							++$CL;
							continue;
						} else {
							return void 0;
						}
					}
					let i = 0;
					for (; i < $KEY_19.length; i++) {
						addEventListener($KEY_19[i], $KEY_0, true);
					}
					$KEY_6 = new $KEY_139($KEY_9, $CA, $CB, $CC, $CD, $CE, $CG, $CI, $KEY_2, $KEY_11, $KEY_12);
					$KEY_15 = Date !== null && typeof Date.now === "function" ? Date.now() : 0;
					$KEY_6.init();
				}
return $THREAD_3_68119;
}
host();"#;

// The alpha variant changes every local and capture spelling, rearranges declarations,
// gives the capture loop its own counter, reorders session arguments, and respells the
// teardown guard and removal loop. Only symbol identity may determine the family.
const OWNED_TEARDOWN_ALPHA: &str = r#"function host(){
let $HK_19 = ["x", "y"];
let $HK_139 = class { init(){} };
function $THREAD_90_777_2($P0, $P1, $P2, $P3, $P4, $P5, $P6, $P7, $P8, $P9){
let $A0 = $P1;
let $H0, $S0, $S1, $S2, $R0;
function $THREAD_42_31337($E){
for (let k = 0; k < $HK_19.length; k = k + 1){
removeEventListener($HK_19[k], $H0, true);
}
if (typeof $E.detail.exchange === "function" && $E.detail != null){
$S0.push("tok");
$S1.push($A0);
$E.detail.exchange($S0, $S1, 0);
}
}
$H0 = $THREAD_42_31337;
let $R1 = document.createEvent("CustomEvent");
$S1 = [];
$S0 = [];
$S2 = 0;
function $THREAD_43_31337($E0, $E1, $E2){ $S2 = $E2; $S1 = $E1; $S0 = $E0; }
let $D0 = { exchange: $THREAD_43_31337 };
let $C0 = 0;
while ($C0 < $HK_19.length){
$R1.initCustomEvent($HK_19[$C0], false, true, $D0);
if (dispatchEvent($R1)){ ++$C0; continue; } else { return void 0; }
}
for (let m = 0; m < $HK_19.length; m++){
addEventListener($HK_19[m], $H0, true);
}
let $X0 = new $HK_139($A0, $P2, $S0, $S1, $S2);
$X0.init();
}
return $THREAD_90_777_2;
}
host();"#;

#[test]
fn accepts_owned_exchange_teardown_handler_owned_teardown_witness() {
    for (label, source, target) in [
        ("cursor_witness", OWNED_TEARDOWN_WITNESS, OWNED_TEARDOWN_WITNESS_TARGET),
        ("cursor_alpha", OWNED_TEARDOWN_ALPHA, "$THREAD_90_777_2"),
    ] {
        assert!(parses(source), "{label} did not parse");
        assert!(named(source, target), "{label} was not named");
        assert!(our_claim(source), "{label} was not our claim");
    }
}

#[test]
fn claims_only_the_outer_bootstrap_for_owned_teardown_witness() {
    let result = name_threads(OWNED_TEARDOWN_WITNESS, true).unwrap();
    let claimed: Vec<_> = result
        .report
        .function_roles
        .iter()
        .filter(|role| role.evaluators.iter().any(|e| e == EVALUATOR))
        .collect();
    assert_eq!(claimed.len(), 1, "expected exactly the outer bootstrap");
    assert_eq!(claimed[0].current_name.as_deref(), Some(OWNED_TEARDOWN_WITNESS_TARGET));
}

#[test]
fn denies_broken_owned_teardown_handler() {
    refuses(
        OWNED_TEARDOWN_WITNESS,
        OWNED_TEARDOWN_WITNESS_TARGET,
        &[
            // The removal loop removes a different handler, not the installed one.
            (
                "removeEventListener($KEY_19[i], $KEY_0, true);",
                "removeEventListener($KEY_19[i], $KEY_9, true);",
            ),
            // The removal loop drops the literal capture flag.
            (
                "removeEventListener($KEY_19[i], $KEY_0, true);",
                "removeEventListener($KEY_19[i], $KEY_0, false);",
            ),
            // The removal loop removes a local array, not the captured name list.
            (
                "removeEventListener($KEY_19[i], $KEY_0, true);",
                "removeEventListener($KEY_2[i], $KEY_0, true);",
            ),
            // The self-removal loop disappears.
            ("removeEventListener($KEY_19[i], $KEY_0, true);", "void 0;"),
            // The guarded emission targets a non-formal object.
            (
                "$CN.detail.exchange($KEY_2, $KEY_11, $CO);",
                "$KEY_9.detail.exchange($KEY_2, $KEY_11, $CO);",
            ),
            // The emission disappears.
            ("$CN.detail.exchange($KEY_2, $KEY_11, $CO);", "void 0;"),
            // The null/typeof guard is replaced by an unconditional body.
            (
                "if ($CN.detail != null && typeof $CN.detail.exchange === \"function\") {",
                "if (true) {",
            ),
        ],
    );
}

#[test]
fn denies_unowned_and_twice_written_handlers() {
    // The installed handler is a host-declared helper, not an owned sibling.
    let captured = OWNED_TEARDOWN_WITNESS
        .replace(
            "let $KEY_139 = class { init(){} };",
            "let $KEY_139 = class { init(){} };\nfunction $THREAD_9_1($CN){ $CN.preventDefault(); }",
        )
        .replace("$KEY_0 = $THREAD_1_6187;", "$KEY_0 = $THREAD_9_1;");
    denied(&captured, OWNED_TEARDOWN_WITNESS_TARGET);

    // The handler binding is written twice, so its origin is not stable.
    let rewritten = OWNED_TEARDOWN_WITNESS.replace(
        "$KEY_0 = $THREAD_1_6187;",
        "$KEY_0 = $THREAD_1_6187;\n$KEY_0 = $THREAD_4_18341;",
    );
    denied(&rewritten, OWNED_TEARDOWN_WITNESS_TARGET);
}

#[test]
fn preserves_base_prevent_default_path() {
    assert!(named(POSITIVE, TARGET));
    assert!(our_claim(POSITIVE));
}

#[test]
fn owned_teardown_witness_edits_are_identifier_only_replacements() {
    let source = OWNED_TEARDOWN_WITNESS.replace("$THREAD_4_18341", "applyExchangeValues");
    let result = name_threads(&source, true).unwrap();
    let applied: Vec<_> = result
        .report
        .outcomes
        .iter()
        .filter(|outcome| outcome.applied && outcome.proposed_name.as_deref() == Some(NAME))
        .collect();
    assert_eq!(applied.len(), 1, "expected exactly one applied target");
    let mut replay = source;
    let mut edits: Vec<_> = result.report.edits.iter().collect();
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.start));
    for edit in edits {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, result.source);
}
