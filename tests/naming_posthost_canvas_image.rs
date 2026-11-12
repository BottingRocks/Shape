//! Focused, portable coverage for the 5x5 canvas image probe family.
//!
//! The family owns the outer canvas/Image setup and its nested `image.onload` callback.
//! Minimal carriers cover inline or grouped locals, for/while loops, direct slice calls or
//! a `[data, data.slice]` holder through Function.prototype.call, and random-word
//! truncation at declaration or each use. Negatives change operations, receivers, dataflow,
//! ownership, or shadows and must stay unclaimed by `shared_posthost_canvas_image`. The
//! existing canvas package supplies the integration environment.
use shape::naming::name_threads;

const EVALUATOR: &str = "shared_posthost_canvas_image";
const STARTER: &str = "startCanvasPixelProbe";
const HANDLER: &str = "handleCanvasProbeImageLoad";

const POSITIVE: &str = r#"function bootstrap(){
let $KEY_16 = document;
let $KEY_14 = Image;
let $KEY_17 = function($p, $s){};
let $KEY_282 = function($n){ return function(){ return 0; }; };
let $KEY_323 = function($state, $bytes, $n){ return 0; };
let $KEY_21 = [];
let $KEY_10 = 0;
let $KEY_2 = { next: function($value){}, complete: function(){} };
function $THREAD_0_127639($BP, $BQ) {
function $THREAD_1_106729() {
try {
$KEY_0.drawImage($KEY_5, 0, 0);
try {
let $BS = $KEY_0.getImageData(0, 0, 5, 5).data;
let i = 0;
for (; i < $BS.length / 4; i++) {
let $BT = i * 4;
let $BU = $BS.slice($BT, $BT + 4);
$KEY_17($BU, $KEY_1.b);
}
return void 0;
} catch (e) { return void 0; }
} catch (e_2) { return void 0; }
finally {
++$KEY_10;
if ($KEY_10 >= 4) {
let $BV = Math.random() * 4294967296 | 0;
let $BW = [$BV >>> 24, $BV >> 16 & 255, $BV >> 8 & 255, $BV & 255];
let $BX = $KEY_282($BV ^ 3816890618);
let $BY = $KEY_21.length;
let $BZ = $KEY_323($BX, $BY, 3);
$KEY_323($BX, $BW, 3);
$KEY_2.next([[], $BW]);
$KEY_2.complete();
}
}
}
let $KEY_1 = $BQ;
let $KEY_5;
let $KEY_3 = $KEY_16.createElement("canvas");
$KEY_3.width = 5;
$KEY_3.height = 5;
let $KEY_0 = $KEY_3.getContext("2d", { willReadFrequently: true });
$KEY_5 = new $KEY_14(5, 5);
$KEY_5.src = $BP;
$KEY_5.onload = $THREAD_1_106729;
}
return $THREAD_0_127639;
}
bootstrap();"#;

fn target_outcome<'a>(
    result: &'a shape::naming::NamedThreads,
    target: &str,
) -> &'a shape::naming::ThreadName {
    result
        .report
        .outcomes
        .iter()
        .find(|outcome| outcome.binding.name == target)
        .unwrap_or_else(|| panic!("target binding {target} missing from outcomes"))
}

fn named(source: &str, target: &str, name: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        let outcome = target_outcome(&result, target);
        outcome.applied
            && outcome.proposed_name.as_deref() == Some(name)
            && outcome.evaluator.as_deref() == Some(EVALUATOR)
    })
}

fn our_claim(source: &str, target: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        target_outcome(&result, target)
            .evaluator
            .as_deref()
            == Some(EVALUATOR)
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
    assert!(!our_claim(source, target), "unexpected fast claim for:\n{source}");
    assert!(!named(source, target, STARTER), "unexpected {STARTER} for:\n{source}");
    assert!(!named(source, target, HANDLER), "unexpected {HANDLER} for:\n{source}");
}

fn refuses(template: &str, target: &str, changes: &[(&str, &str)]) {
    for &(from, to) in changes {
        assert!(template.contains(from), "missing replacement {from}");
        denied(&template.replace(from, to), target);
    }
}

/// Replace the nested `image.onload` callback body while leaving the rest of
/// the setup intact.
fn replace_callback(source: &str, replacement: &str) -> String {
    const START: &str = "function $THREAD_1_106729() {";
    const END: &str = "\nlet $KEY_1 = $BQ;";
    let start = source.find(START).expect("callback declaration");
    let end = source[start..].find(END).expect("callback end") + start;
    let mut replaced = String::with_capacity(source.len());
    replaced.push_str(&source[..start]);
    replaced.push_str(replacement);
    replaced.push_str(&source[end..]);
    replaced
}

#[test]
fn witness_setup_and_owned_callback_are_named() {
    assert!(named(POSITIVE, "$THREAD_0_127639", STARTER), "{POSITIVE}");
    assert!(named(POSITIVE, "$THREAD_1_106729", HANDLER), "{POSITIVE}");
    assert!(our_claim(POSITIVE, "$THREAD_0_127639"));
    assert!(our_claim(POSITIVE, "$THREAD_1_106729"));
}

#[test]
fn portable_spelling_variants_are_claimed() {
    // Bare-word variant: while loop, holder-array slice through `.call`, and an
    // untruncated random word used directly.
    let bare_word = POSITIVE
        .replace(
            "let i = 0;\nfor (; i < $BS.length / 4; i++) {\nlet $BT = i * 4;\nlet $BU = $BS.slice($BT, $BT + 4);\n$KEY_17($BU, $KEY_1.b);\n}",
            "let $BX = 0;\nwhile ($BX < $BS.length / 4) {\nlet $BY = $BX * 4;\nlet $BZ = [$BS, $BS.slice];\nlet $BU = $BZ[1].call($BZ[0], $BY, $BY + 4);\n$KEY_17($BU, $KEY_1.b);\n++$BX;\n}",
        )
        .replace(
            "let $BV = Math.random() * 4294967296 | 0;",
            "let $BV = Math.random() * 4294967296;",
        );
    // Grouped-local variant: the context and image are pre-declared together,
    // the index is declared inside the `for` header and the slice stays direct.
    let grouped_local = POSITIVE
        .replace("let $KEY_5;", "let $KEY_0, $KEY_5;")
        .replace(
            "let $KEY_0 = $KEY_3.getContext(\"2d\", { willReadFrequently: true });",
            "$KEY_0 = $KEY_3.getContext(\"2d\", { willReadFrequently: true });",
        )
        .replace(
            "let i = 0;\nfor (; i < $BS.length / 4; i++) {",
            "for (let j = 0; j < $BS.length / 4; j++) {",
        )
        .replace("let $BT = i * 4;", "let $BT = j * 4;");
    // Plain local renaming and an inline offset expression.
    let inlined = POSITIVE
        .replace("$THREAD_0_127639", "$THREAD_71_887766_2")
        .replace("$THREAD_1_106729", "$THREAD_72_887767_2")
        .replace("$KEY_16", "$KEY_160")
        .replace("$KEY_14", "$KEY_140")
        .replace("$KEY_17", "$KEY_170")
        .replace("$KEY_282", "$KEY_2820")
        .replace("$KEY_323", "$KEY_3230")
        .replace("$KEY_10", "$KEY_100")
        .replace("$KEY_2 =", "$KEY_20 =")
        .replace("$KEY_2.", "$KEY_20.")
        .replace(
            "let $BT = i * 4;\nlet $BU = $BS.slice($BT, $BT + 4);",
            "let $BU = $BS.slice(i * 4, i * 4 + 4);",
        );
    // Use-site truncation: the random word is declared without truncation and coerced with `|
    // 0` in every byte lane and in the hash-state XOR.
    let coerced_lanes = POSITIVE
        .replace(
            "let $BV = Math.random() * 4294967296 | 0;",
            "let $BV = Math.random() * 4294967296;",
        )
        .replace(
            "let $BW = [$BV >>> 24, $BV >> 16 & 255, $BV >> 8 & 255, $BV & 255];",
            "let $BW = [($BV | 0) >>> 24, ($BV | 0) >> 16 & 255, ($BV | 0) >> 8 & 255, ($BV | 0) & 255];",
        )
        .replace(
            "$KEY_282($BV ^ 3816890618)",
            "$KEY_282(($BV | 0) ^ 3816890618)",
        );
    // Grouped context/image declarations use a separate loop index, holder-array slice through
    // `.call`, and use-site truncation.
    let grouped_holder_coerced = POSITIVE
        .replace("let $KEY_5;", "let $KEY_0, $KEY_5;")
        .replace(
            "let $KEY_0 = $KEY_3.getContext(\"2d\", { willReadFrequently: true });",
            "$KEY_0 = $KEY_3.getContext(\"2d\", { willReadFrequently: true });",
        )
        .replace(
            "let $BU = $BS.slice($BT, $BT + 4);\n$KEY_17($BU, $KEY_1.b);",
            "let $BI = [$BS, $BS.slice];\nlet $BJ = $BI[1].call($BI[0], $BT, $BT + 4);\n$KEY_17($BJ, $KEY_1.b);",
        )
        .replace(
            "let $BV = Math.random() * 4294967296 | 0;",
            "let $BV = Math.random() * 4294967296;",
        )
        .replace(
            "let $BW = [$BV >>> 24, $BV >> 16 & 255, $BV >> 8 & 255, $BV & 255];",
            "let $BW = [($BV | 0) >>> 24, ($BV | 0) >> 16 & 255, ($BV | 0) >> 8 & 255, ($BV | 0) & 255];",
        )
        .replace(
            "$KEY_282($BV ^ 3816890618)",
            "$KEY_282(($BV | 0) ^ 3816890618)",
        );
    for (source, outer, inner) in [
        (bare_word, "$THREAD_0_127639", "$THREAD_1_106729"),
        (grouped_local, "$THREAD_0_127639", "$THREAD_1_106729"),
        (inlined, "$THREAD_71_887766_2", "$THREAD_72_887767_2"),
        (coerced_lanes, "$THREAD_0_127639", "$THREAD_1_106729"),
        (grouped_holder_coerced, "$THREAD_0_127639", "$THREAD_1_106729"),
    ] {
        assert!(parses(&source), "variant did not parse:\n{source}");
        assert!(named(&source, outer, STARTER), "outer:\n{source}");
        assert!(named(&source, inner, HANDLER), "inner:\n{source}");
    }
}

#[test]
fn operation_and_receiver_negatives_are_refused() {
    refuses(
        POSITIVE,
        "$THREAD_0_127639",
        &[
            // operations
            ("getImageData(0, 0, 5, 5)", "getImageData(0, 0, 6, 5)"),
            ("$KEY_3.width = 5;", "$KEY_3.width = 6;"),
            ("$KEY_3.height = 5;", "$KEY_3.height = 4;"),
            ("Math.random() * 4294967296", "Math.random() * 4294967295"),
            ("$KEY_10 >= 4", "$KEY_10 >= 3"),
            ("drawImage($KEY_5, 0, 0)", "drawImage($KEY_5, 1, 0)"),
            ("$BS.slice($BT, $BT + 4)", "$BS.slice($BT, $BT + 3)"),
            ("let $BT = i * 4;", "let $BT = i * 5;"),
            ("$BV ^ 3816890618", "$BV + 3816890618"),
            (
                "$BV ^ 3816890618",
                "($BV + $KEY_21) ^ 3816890618",
            ),
            // receivers
            ("$KEY_16.createElement(\"canvas\")", "$KEY_16.createElement(\"div\")"),
            ("{ willReadFrequently: true }", "{}"),
            ("$KEY_3.getContext(\"2d\"", "$KEY_3.getContext(\"webgl\""),
            ("new $KEY_14(5, 5)", "new $KEY_14(6, 5)"),
            ("$KEY_0.drawImage", "$KEY_15.drawImage"),
            ("$KEY_0.getImageData", "$KEY_15.getImageData"),
            ("$KEY_17($BU, $KEY_1.b)", "$KEY_17($BU, $KEY_99.b)"),
        ],
    );
}

#[test]
fn use_site_random_truncation_keeps_the_exact_word() {
    // Declaration and use-site truncation produce the same 32-bit word and must both be named.
    // The use-site form must still refuse a different word, mask, surrounding arithmetic, or
    // local alias.
    let base = POSITIVE
        .replace(
            "let $BV = Math.random() * 4294967296 | 0;",
            "let $BV = Math.random() * 4294967296;",
        )
        .replace(
            "let $BW = [$BV >>> 24, $BV >> 16 & 255, $BV >> 8 & 255, $BV & 255];",
            "let $BW = [($BV | 0) >>> 24, ($BV | 0) >> 16 & 255, ($BV | 0) >> 8 & 255, ($BV | 0) & 255];",
        )
        .replace(
            "$KEY_282($BV ^ 3816890618)",
            "$KEY_282(($BV | 0) ^ 3816890618)",
        );
    const SEED: &str = "$KEY_282(($BV | 0) ^ 3816890618)";
    for replacement in [
        // A different binding, even another captured local, is not the word.
        "$KEY_282(($KEY_21 | 0) ^ 3816890618)",
        "$KEY_282(($KEY_5 | 0) ^ 3816890618)",
        // Masking with a non-zero constant changes the value.
        "$KEY_282(($BV | 1) ^ 3816890618)",
        // Arithmetic around the word is not the word.
        "$KEY_282(($BV + 0) ^ 3816890618)",
        "$KEY_282(($BV | 0) * 1 ^ 3816890618)",
    ] {
        let changed = base.replace(SEED, replacement);
        assert!(changed != base, "missing seed {SEED}");
        denied(&changed, "$THREAD_1_106729");
    }
    // A local alias of the truncated word is dataflow-equivalent but is not the
    // exact random binding, so neither the callback nor its setup is claimed.
    let aliased = base.replace(
        "let $BX = $KEY_282(($BV | 0) ^ 3816890618);",
        "let $BD = $BV | 0;\nlet $BX = $KEY_282($BD ^ 3816890618);",
    );
    assert!(aliased != base, "missing seed declaration");
    denied(&aliased, "$THREAD_1_106729");
    denied(&aliased, "$THREAD_0_127639");
}

#[test]
fn dataflow_and_ownership_negatives_are_refused() {
    refuses(
        POSITIVE,
        "$THREAD_0_127639",
        &[
            ("$KEY_5.src = $BP;", "$KEY_5.src = \"data:image/png;base64,\";"),
            ("let $KEY_1 = $BQ;", "let $KEY_1 = {};"),
            ("$KEY_5.onload = $THREAD_1_106729;", "$KEY_5.onload = $KEY_17;"),
            ("$KEY_2.next([[], $BW]);", "$KEY_2.next([[], $BV]);"),
            ("$KEY_2.complete();", "$KEY_2.error();"),
            ("$KEY_2 = { next:", "$KEY_20 = { next:"),
            ("$KEY_2.next", "$KEY_20.next"),
            ("$KEY_2.complete", "$KEY_20.complete"),
        ],
    );
    // A leading statement before the owned callback breaks the setup body
    // ordering, so neither the wrapper nor its nested callback is claimed.
    denied(
        &POSITIVE.replace(
            "function $THREAD_0_127639($BP, $BQ) {\nfunction $THREAD_1_106729()",
            "function $THREAD_0_127639($BP, $BQ) {\nlet $KEY_99 = 1;\nfunction $THREAD_1_106729()",
        ),
        "$THREAD_0_127639",
    );
    // Shadowing the shared load counter inside the callback must be refused.
    denied(
        &POSITIVE.replace(
            "try {\n$KEY_0.drawImage($KEY_5, 0, 0);",
            "try {\nlet $KEY_10 = 0;\n$KEY_0.drawImage($KEY_5, 0, 0);",
        ),
        "$THREAD_1_106729",
    );
    // A canvas image probe whose installed callback is not the pixel-hash
    // body stays unclaimed: the setup and its callback both decline.
    let flat = replace_callback(
        POSITIVE,
        "function $THREAD_1_106729() {\n$KEY_0.drawImage($KEY_5, 0, 0);\n}",
    );
    denied(&flat, "$THREAD_0_127639");
    denied(&flat, "$THREAD_1_106729");
}

#[test]
fn human_names_are_preserved_and_existing_names_not_overridden() {
    let human = POSITIVE.replace("$THREAD_0_127639", "humanCanvasProbe");
    let result = name_threads(&human, true).unwrap();
    assert!(result.source.contains("function humanCanvasProbe("));
    assert!(!result
        .report
        .outcomes
        .iter()
        .any(|outcome| outcome.binding.name == "humanCanvasProbe" && outcome.applied));
    // The owned callback is still the same proven role and may be named.
    assert!(named(&human, "$THREAD_1_106729", HANDLER));

    let collision = POSITIVE.replace(
        "function bootstrap(){",
        "function bootstrap(){let startCanvasPixelProbe=17;useValue(startCanvasPixelProbe);",
    );
    assert!(parses(&collision));
    let result = name_threads(&collision, true).unwrap();
    let outer = target_outcome(&result, "$THREAD_0_127639");
    assert!(!outer.applied);
    assert_eq!(outer.reason.as_deref(), Some("NameCollision"));
    assert!(result.source.contains("let startCanvasPixelProbe=17"));
    assert!(result.source.contains("$THREAD_0_127639"));
}

#[test]
fn edits_are_exact_identifier_only_replacements() {
    let result = name_threads(POSITIVE, true).unwrap();
    let applied: Vec<_> = result
        .report
        .outcomes
        .iter()
        .filter(|outcome| {
            outcome.applied
                && matches!(outcome.proposed_name.as_deref(), Some(STARTER) | Some(HANDLER))
        })
        .collect();
    assert_eq!(applied.len(), 2, "expected both owned roles");
    // The report carries original-coordinate edits in plan order, not source
    // order; replay them from the highest offset down. Every edit must replace
    // exactly one generated thread identifier.
    let mut replay = POSITIVE.to_string();
    let mut edits = result.report.edits.clone();
    edits.sort_by(|a, b| b.start.cmp(&a.start));
    for edit in edits {
        let original = &POSITIVE[edit.start as usize..edit.end as usize];
        assert!(
            original.starts_with("$THREAD_"),
            "edit targets a non-identifier: {original:?}"
        );
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, result.source);
}

// Exact owned function bodies from the three cached cleaned sources; only
// indentation is normalized. Captured dependencies have meaningful stubs.
const SHARED_CATCH_CATCH_STATE: &str = r#"function bootstrap() {
function samplePixel(pixel, config) { return [pixel, config]; }
function seedHashState(seed) { return function nextHashByte() { return seed & 255; }; }
function encodeNumber(state, value, bytes, offset) { return offset + 1; }
let observer = { next: function observeBytes(payload) {}, complete: function finishProbe() {} };
let $KEY_7 = [0];
let $KEY_9 = [];
let $KEY_12 = samplePixel;
let $KEY_13 = observer;
let $KEY_21 = Image;
let $KEY_25 = document;
let $KEY_34 = 0;
let $KEY_251 = seedHashState;
let $KEY_344 = encodeNumber;
function $THREAD_3_58686($DC, $DD) {
function $THREAD_0_46138() {
let $DE, $DF;
try {
$KEY_2.drawImage($KEY_3, 0, 0);
try {
let $DH = $KEY_2.getImageData(0, 0, 5, 5).data;
let i = 0;
for (; i < $DH.length / 4; i++) {
let $DI = i * 4;
let $DJ = [$DH, $DH.slice];
let $DK = $DJ[1].call($DJ[0], $DI, $DI + 4);
$KEY_12($DK, $KEY_0.b);
}
return void 0;
} catch (e) {
return void 0;
}
} catch (e_2) {
$DE = 46521;
$DF = 1;
return void 0;
} finally {
++$KEY_34;
if ($KEY_34 >= 4) {
let $DL = Math.random() * 4294967296 | 0;
let $DM = [$DL >>> 24, $DL >> 16 & 255, $DL >> 8 & 255, $DL & 255];
let $DN = $KEY_251($DL ^ 3402586920);
let $DO = $KEY_9.length;
let $DP = 0;
let $DQ;
let $DR = $KEY_344($DN, $DO, $DM, 3);
while ($DP < $DO) {
let $DS = $KEY_9[$DP];
let $DT = $KEY_344($DN, $DS.a.b, $DM, $DR);
let $DU = $KEY_344($DN, $DS.b.a.length, $DM, $DT);
if ($DE === void 0) {
throw new TypeError("Cannot read properties of undefined (reading '0')");
}
if ($DF === void 0) {
throw new TypeError("Cannot read properties of undefined (reading 'undefined')");
}
let $DV = $DS.b.a.length;
let $DW = $DQ;
let $DX = $DU;
for (;;) {
let $DY = $DV - 1;
if (!($DY >= 0)) break;
let $DZ = $KEY_344($DN, $DS.b.a[$DY][2], $DM, $DX);
let $EA = $KEY_344($DN, $DS.b.a[$DY][1], $DM, $DZ);
$DX = $KEY_344($DN, $DS.b.a[$DY][0], $DM, $KEY_344($DN, $DS.b.a[$DY][3], $DM, $EA));
$DV = Number($DY);
$DW = $DS.b.a[$DY];
}
let $EB = $KEY_344($DN, $DS.b.b, $DM, $DX);
let $EC, $ED;
if ($DS.c == null) {
let $EE = $EB + 1;
$DM[$EE] = $DN() ^ 130;
$EC = $DM;
$ED = $EE;
} else {
$EC = $DW;
$ED = $KEY_344($DN, $DS.c, $DM, $EB);
}
let j = 0;
$DQ = $EC;
$DR = $KEY_344($DN, $DS.a.a.length, $DM, $ED);
for (; j < $DS.a.a.length; j++) {
let $EF = $KEY_344($DN, $DS.a.a[j][3], $DM, $DR);
let $EG = $KEY_344($DN, $DS.a.a[j][0], $DM, $EF);
$DR = $KEY_344($DN, $DS.a.a[j][1], $DM, $KEY_344($DN, $DS.a.a[j][2], $DM, $EG));
$DQ = $DS.a.a[j];
}
++$DP;
}
$KEY_344($DN, $KEY_7[0] < 1 ? $KEY_7[0] : $KEY_7[0] - $KEY_7[0] >>> 0, $DM, $DR);
$KEY_13.next([[], $DM]);
$KEY_13.complete();
}
}
}
let $KEY_0 = $DD;
let $KEY_2, $KEY_3;
let $KEY_4 = $KEY_25.createElement("canvas");
$KEY_4.width = 5;
$KEY_4.height = 5;
$KEY_2 = $KEY_4.getContext("2d", { willReadFrequently: true });
$KEY_3 = new $KEY_21(5, 5);
$KEY_3.src = $DC;
$KEY_3.onload = $THREAD_0_46138;
}
return $THREAD_3_58686;
}
bootstrap();"#;

#[test]
fn shared_catch_catch_state_setup_and_handler_are_claimed() {
    assert_state_pair(SHARED_CATCH_CATCH_STATE, "$THREAD_3_58686", "$THREAD_0_46138");
}

const SPLIT_CATCH_CATCH_STATE: &str = r#"function bootstrap() {
function samplePixel(pixel, config) { return [pixel, config]; }
function seedHashState(seed) { return function nextHashByte() { return seed & 255; }; }
function encodeNumber(state, value, bytes, offset) { return offset + 1; }
let observer = { next: function observeBytes(payload) {}, complete: function finishProbe() {} };
let $KEY_2 = 0;
let $KEY_17 = [0];
let $KEY_18 = observer;
let $KEY_21 = Image;
let $KEY_27 = document;
let $KEY_32 = samplePixel;
let $KEY_34 = [];
let $KEY_243 = encodeNumber;
let $KEY_390 = seedHashState;
function $THREAD_6_112814($DE, $DF) {
function $THREAD_6_109494() {
let $DG, $DH;
try {
$KEY_4.drawImage($KEY_1, 0, 0);
try {
let $DJ = $KEY_4.getImageData(0, 0, 5, 5).data;
let i = 0;
for (; i < $DJ.length / 4; i++) {
let $DK = i * 4;
let $DL = $DJ.slice($DK, $DK + 4);
$KEY_32($DL, $KEY_5.b);
}
return void 0;
} catch (e) {
return void 0;
}
} catch (e_2) {
$DH = 110013;
$DG = 7;
return void 0;
} finally {
++$KEY_2;
if ($KEY_2 >= 4) {
let $DM = Math.random() * 4294967296;
let $DN = [($DM | 0) >>> 24, ($DM | 0) >> 16 & 255, ($DM | 0) >> 8 & 255, ($DM | 0) & 255];
let $DO = $KEY_390(($DM | 0) ^ 2084548485);
let $DP = $KEY_34.length;
let $DQ = 0;
let $DR = $KEY_243($DO, $DP, $DN, 3);
while ($DQ < $DP) {
let $DS = $KEY_34[$DQ];
let $DT = $KEY_243($DO, $DS.a.b, $DN, $DR);
let $DU;
if ($DS.c == null) {
let $DV = $DT + 1;
$DN[$DV] = $DO() ^ 130;
$DU = $DV;
} else {
$DU = $KEY_243($DO, $DS.c, $DN, $DT);
}
let j = 0;
let $DW = $KEY_243($DO, $DS.b.a.length, $DN, $DU);
for (; j < $DS.b.a.length; j++) {
let $DX = $KEY_243($DO, $DS.b.a[j][3], $DN, $DW);
let $DY = $KEY_243($DO, $DS.b.a[j][0], $DN, $DX);
let $DZ = $KEY_243($DO, $DS.b.a[j][1], $DN, $DY);
$DW = $KEY_243($DO, $DS.b.a[j][2], $DN, $DZ);
$DS.b.a[j];
}
let $EA = $KEY_243($DO, $DS.b.b, $DN, $DW);
let $EB = $KEY_243($DO, $DS.a.a.length, $DN, $EA);
if ($DH === void 0) {
throw new TypeError("Cannot read properties of undefined (reading '0')");
}
if ($DG === void 0) {
throw new TypeError("Cannot read properties of undefined (reading 'undefined')");
} else {
let k = 0;
$DR = $EB;
for (; k < $DS.a.a.length; k++) {
let $EC = $KEY_243($DO, $DS.a.a[k][0], $DN, $DR);
let $ED = $KEY_243($DO, $DS.a.a[k][2], $DN, $EC);
let $EE = $KEY_243($DO, $DS.a.a[k][3], $DN, $ED);
$DR = $KEY_243($DO, $DS.a.a[k][1], $DN, $EE);
}
++$DQ;
continue;
}
}
$KEY_243($DO, $KEY_17[0] < 1 ? $KEY_17[0] : $KEY_17[0] - $KEY_17[0] >>> 0, $DN, $DR);
$KEY_18.next([[], $DN]);
$KEY_18.complete();
}
}
}
let $KEY_5 = $DF;
let $KEY_3 = $KEY_27.createElement("canvas");
$KEY_3.width = 5;
$KEY_3.height = 5;
let $KEY_4 = $KEY_3.getContext("2d", { willReadFrequently: true });
let $KEY_1 = new $KEY_21(5, 5);
$KEY_1.src = $DE;
$KEY_1.onload = $THREAD_6_109494;
}
return $THREAD_6_112814;
}
bootstrap();"#;

#[test]
fn split_catch_catch_state_setup_and_handler_are_claimed() {
    assert_state_pair(SPLIT_CATCH_CATCH_STATE, "$THREAD_6_112814", "$THREAD_6_109494");
}

const GROUPED_SETUP_CATCH_STATE: &str = r#"function bootstrap() {
function samplePixel(pixel, config) { return [pixel, config]; }
function seedHashState(seed) { return function nextHashByte() { return seed & 255; }; }
function encodeNumber(state, value, bytes, offset) { return offset + 1; }
let observer = { next: function observeBytes(payload) {}, complete: function finishProbe() {} };
let $KEY_5 = [];
let $KEY_7 = document;
let $KEY_15 = 0;
let $KEY_16 = observer;
let $KEY_22 = samplePixel;
let $KEY_23 = Image;
let $KEY_29 = seedHashState;
let $KEY_33 = [0];
let $KEY_96 = encodeNumber;
function $THREAD_3_96794($CN, $CO) {
function $THREAD_3_40038() {
let $CQ, $CR, $CS, $CT;
try {
$KEY_4.drawImage($KEY_2, 0, 0);
try {
let $CV = $KEY_4.getImageData(0, 0, 5, 5).data;
let i = 0;
for (; i < $CV.length / 4; i++) {
let $CW = i * 4;
let $CX = [$CV, $CV.slice];
let $CY = $CX[1].call($CX[0], $CW, $CW + 4);
$KEY_22($CY, $KEY_1.b);
}
return void 0;
} catch (e) {
return void 0;
}
} catch (e_2) {
$CT = 40218;
$CR = 4;
$CS = 40396;
$CQ = 7;
return void 0;
} finally {
++$KEY_15;
if ($KEY_15 >= 4) {
let $CZ = Math.random() * 4294967296;
let $DA = [($CZ | 0) >>> 24, ($CZ | 0) >> 16 & 255, ($CZ | 0) >> 8 & 255, ($CZ | 0) & 255];
let $DB = $KEY_29(($CZ | 0) ^ 1356229728);
let $DC = $KEY_96($DB, $KEY_33[0] < 1 ? $KEY_33[0] : $KEY_33[0] - $KEY_33[0] >>> 0, $DA, 3);
let $DD = $KEY_5.length;
let $DE = 0;
let $DF = $KEY_96($DB, $DD, $DA, $DC);
while ($DE < $DD) {
let $DG = $KEY_5[$DE];
let $DH = $KEY_96($DB, $DG.a.a.length, $DA, $DF);
if ($CS === void 0) {
throw new TypeError("Cannot read properties of undefined (reading '0')");
}
if ($CQ === void 0) {
throw new TypeError("Cannot read properties of undefined (reading 'undefined')");
}
let $DI = $DG.a.a.length;
let $DJ = $DH;
for (;;) {
let $DK = $DI - 1;
if (!($DK >= 0)) break;
let $DL = $KEY_96($DB, $DG.a.a[$DK][0], $DA, $DJ);
let $DM = $KEY_96($DB, $DG.a.a[$DK][1], $DA, $DL);
$DJ = $KEY_96($DB, $DG.a.a[$DK][3], $DA, $KEY_96($DB, $DG.a.a[$DK][2], $DA, $DM));
$DI = Number($DK);
}
let $DN = $KEY_96($DB, $DG.b.b, $DA, $DJ);
let $DO = $DG.b.a.length;
let $DP = $KEY_96($DB, $DO, $DA, $DN);
for (;;) {
let $DQ = $DO - 1;
if (!($DQ >= 0)) break;
let $DR = $KEY_96($DB, $DG.b.a[$DQ][0], $DA, $DP);
let $DS = $KEY_96($DB, $DG.b.a[$DQ][3], $DA, $DR);
$DP = $KEY_96($DB, $DG.b.a[$DQ][1], $DA, $KEY_96($DB, $DG.b.a[$DQ][2], $DA, $DS));
$DO = Number($DQ);
}
let $DT = $KEY_96($DB, $DG.a.b, $DA, $DP);
if ($DG.c == null) {
let $DU = $DT + 1;
$DA[$DU] = $DB() ^ 130;
$DF = $DU;
} else {
$DF = $KEY_96($DB, $DG.c, $DA, $DT);
}
++$DE;
}
$KEY_16.next([[], $DA]);
$KEY_16.complete();
if ($CT === void 0) {
throw new TypeError("Cannot read properties of undefined (reading '0')");
}
if ($CR === void 0) {
throw new TypeError("Cannot read properties of undefined (reading 'undefined')");
}
}
}
}
let $KEY_1 = $CO;
let $KEY_2, $KEY_4;
let $CP = $KEY_7.createElement("canvas");
$CP.width = 5;
$CP.height = 5;
$KEY_4 = $CP.getContext("2d", { willReadFrequently: true });
$KEY_2 = new $KEY_23(5, 5);
$KEY_2.src = $CN;
$KEY_2.onload = $THREAD_3_40038;
}
return $THREAD_3_96794;
}
bootstrap();"#;

#[test]
fn grouped_setup_catch_state_setup_and_handler_are_claimed() {
    assert_state_pair(GROUPED_SETUP_CATCH_STATE, "$THREAD_3_96794", "$THREAD_3_40038");
}

fn assert_state_pair(source: &str, setup: &str, handler: &str) {
    let result = name_threads(source, true).expect("stateful fixture parses");
    for (target, expected) in [(setup, STARTER), (handler, HANDLER)] {
        let outcome = target_outcome(&result, target);
        assert!(outcome.applied, "{target}: {source}");
        assert_eq!(outcome.proposed_name.as_deref(), Some(expected));
        assert_eq!(outcome.evaluator.as_deref(), Some(EVALUATOR));
    }
}

fn refuse_state_pair(template: &str, setup: &str, handler: &str, from: &str, to: &str) {
    assert!(template.contains(from), "missing mutation {from}");
    let source = template.replacen(from, to, 1);
    let result = name_threads(&source, true).expect("negative fixture parses");
    for target in [setup, handler] {
        assert_ne!(
            target_outcome(&result, target).evaluator.as_deref(),
            Some(EVALUATOR),
            "unexpected stateful claim for {target}: {source}"
        );
    }
}

#[test]
fn stateful_setup_link_image_context_config_and_dimensions_negatives() {
    for (from, to) in [
        (
            "$KEY_3.onload = $THREAD_0_46138;",
            "$KEY_3.onload = samplePixel;",
        ),
        (
            "$KEY_3.onload = $THREAD_0_46138;",
            "$KEY_7.onload = $THREAD_0_46138;",
        ),
        (
            "$KEY_2.drawImage($KEY_3, 0, 0)",
            "$KEY_2.drawImage($KEY_7, 0, 0)",
        ),
        (
            "$KEY_2.getImageData(0, 0, 5, 5)",
            "$KEY_25.getImageData(0, 0, 5, 5)",
        ),
        ("$KEY_12($DK, $KEY_0.b)", "$KEY_12($DK, $KEY_7.b)"),
        (
            "$KEY_2.getImageData(0, 0, 5, 5)",
            "$KEY_2.getImageData(0, 0, 4, 5)",
        ),
        ("new $KEY_21(5, 5)", "new $KEY_21(5, 4)"),
        ("$KEY_4.width = 5;", "$KEY_4.width = 4;"),
        (
            "$DJ[1].call($DJ[0], $DI, $DI + 4)",
            "$KEY_7[1].call($KEY_7[0], $DI, $DI + 4)",
        ),
        ("$KEY_13.next([[], $DM]);", "$KEY_13.next([[], $DN]);"),
        ("$KEY_13.complete();", "$KEY_25.complete();"),
    ] {
        refuse_state_pair(
            SHARED_CATCH_CATCH_STATE,
            "$THREAD_3_58686",
            "$THREAD_0_46138",
            from,
            to,
        );
    }
}

#[test]
fn stateful_catch_stores_and_exception_flags_are_owned() {
    for (from, to) in [
        ("$DE = 46521;", "$KEY_34 = 46521;"),
        ("$DF = 1;", "$DE = 1;"),
        ("$DF = 1;", "$KEY_7 = 1;"),
        ("$DE = 46521;", "$DE = samplePixel();"),
        ("$DF = 1;", "function deferredStore() { $DF = 1; }"),
        ("$DE === void 0", "$DF === void 0"),
        ("$DF === void 0", "$DE === void 0"),
        ("$DF === void 0", "$KEY_7 === void 0"),
        ("$DF === void 0", "$DF !== void 0"),
        ("$DF = 1;", "$DF = 1; $DF = 2;"),
    ] {
        refuse_state_pair(
            SHARED_CATCH_CATCH_STATE,
            "$THREAD_3_58686",
            "$THREAD_0_46138",
            from,
            to,
        );
    }
}

#[test]
fn stateful_throw_paths_and_finalizer_effects_are_required() {
    for (from, to) in [
        ("$DM[$EE] = $DN() ^ 130", "$DM[$DP] = $DN() ^ 130"),
        (
            "$KEY_344($DN, $DS.a.b, $DM, $DR)",
            "$KEY_344($DN, $DS.a.b, $DM, $DQ)",
        ),
        (
            "$KEY_344($DN, $DS.a.b, $DM, $DR)",
            "$KEY_344($DN, $DS.a.b, $DM, $DP)",
        ),
        ("$ED = $EE;", "$ED = $DP;"),
        (
            "throw new TypeError(\"Cannot read properties of undefined (reading '0')\");",
            "return void 0;",
        ),
        (
            "throw new TypeError(\"Cannot read properties of undefined (reading 'undefined')\");",
            "samplePixel();",
        ),
        ("throw new TypeError", "throw new Error"),
        ("let $DE, $DF;", "let $DE = 46521, $DF;"),
        ("++$KEY_34;", "++$KEY_7;"),
        (
            "$KEY_344($DN, $DO, $DM, 3)",
            "$KEY_344($DN, $DO, $KEY_7, 3)",
        ),
        ("$DM[$EE] = $DN() ^ 130", "$KEY_7[$EE] = $DN() ^ 130"),
        ("$DM[$EE] = $DN() ^ 130", "$DM[$EE] = samplePixel() ^ 130"),
        ("let $DR = $KEY_344", "let $DR = samplePixel"),
        ("let $DQ;", "let $DQ; samplePixel();"),
        ("$DL >>> 24", "$DL >>> 16"),
        ("while ($DP < $DO) {", "while ($DP < $DO) { break;"),
        ("let $DQ;", "let $DQ; $KEY_7[0] = 1;"),
        ("let $DQ;", "let $DQ; function unexecutedEvidence() {}"),
    ] {
        refuse_state_pair(
            SHARED_CATCH_CATCH_STATE,
            "$THREAD_3_58686",
            "$THREAD_0_46138",
            from,
            to,
        );
    }
    // The grouped setup has two further throw checks after observer completion. Neither may
    // disappear merely because next/complete has already been proved.
    for (from, to) in [
        ("if ($CT === void 0) {\nthrow new TypeError(\"Cannot read properties of undefined (reading '0')\");\n}", ""),
        ("$CT === void 0", "$CS === void 0"),
        ("$CR === void 0", "$CQ === void 0"),
        ("$CT = 40218;", "$CS = 40218;"),
        ("$CR = 4;", "$CQ = 4;"),
    ] {
        refuse_state_pair(GROUPED_SETUP_CATCH_STATE, "$THREAD_3_96794", "$THREAD_3_40038", from, to);
    }
}

#[test]
fn stateful_numeric_values_and_local_spellings_are_not_evidence() {
    let renamed = SHARED_CATCH_CATCH_STATE
        .replace("$DE", "$catchTag")
        .replace("$DF", "$exceptionFlag")
        .replace("46521", "19")
        .replace("$exceptionFlag = 1;", "$exceptionFlag = 0;")
        .replace("$KEY_344", "$numberEncoder")
        .replace("$KEY_251", "$hashSeeder");
    assert_state_pair(&renamed, "$THREAD_3_58686", "$THREAD_0_46138");
}

const GROUPED_SETUP_TOP_LEVEL_TAG_GUARD: &str = "if ($CT === void 0) {\nthrow new TypeError(\"Cannot read properties of undefined (reading '0')\");\n}";
const GROUPED_SETUP_SAMPLE_TAG_GUARD: &str = "if ($CS === void 0) {\nthrow new TypeError(\"Cannot read properties of undefined (reading '0')\");\n}";

#[test]
fn cursor_readiness_mandatory_finalizer_guard_cannot_be_donated_by_zero_iteration_loop() {
    for prefix in ["while (0)", "for (; 0;)"] {
        let replacement = format!("{prefix} {{\n{GROUPED_SETUP_TOP_LEVEL_TAG_GUARD}\n}}");
        refuse_state_pair(
            GROUPED_SETUP_CATCH_STATE,
            "$THREAD_3_96794",
            "$THREAD_3_40038",
            GROUPED_SETUP_TOP_LEVEL_TAG_GUARD,
            &replacement,
        );
    }
}

#[test]
fn cursor_readiness_sample_guard_cannot_be_donated_by_nested_zero_iteration_loop() {
    // The original guard is legitimately conditional on the sample walk.
    // A further optional loop must not supply that direct protocol position.
    for prefix in ["while (0)", "for (; 0;)"] {
        let replacement = format!("{prefix} {{\n{GROUPED_SETUP_SAMPLE_TAG_GUARD}\n}}");
        refuse_state_pair(
            GROUPED_SETUP_CATCH_STATE,
            "$THREAD_3_96794",
            "$THREAD_3_40038",
            GROUPED_SETUP_SAMPLE_TAG_GUARD,
            &replacement,
        );
    }
}

#[test]
fn cursor_readiness_used_cursor_requires_a_value_on_the_zero_iteration_path() {
    // Keep the first encoder call direct. This isolates cursor donation from
    // the separate rule that refuses a conditional first encoder call.
    for prefix in ["while (0)", "for (; 0;)"] {
        let replacement = format!(
            "let $initialCursor = $KEY_344($DN, $DO, $DM, 3);\nlet $DR;\n{prefix} {{ $DR = $initialCursor; }}"
        );
        refuse_state_pair(
            SHARED_CATCH_CATCH_STATE,
            "$THREAD_3_58686",
            "$THREAD_0_46138",
            "let $DR = $KEY_344($DN, $DO, $DM, 3);",
            &replacement,
        );
    }
}

/// Keep setup, sample read, catch stores, random-byte seed, observer, and all four guards.
/// Replace only the finalizer encoder walk to isolate encoder presence from unrelated
/// cursor-use rejection.
fn grouped_setup_with_encoder_only_in_optional_path(optional_encoder: &str) -> String {
    const START: &str = "let $DC = $KEY_96($DB, $KEY_33[0] < 1 ? $KEY_33[0] : $KEY_33[0] - $KEY_33[0] >>> 0, $DA, 3);";
    const END: &str = "$KEY_16.next([[], $DA]);";
    let start = GROUPED_SETUP_CATCH_STATE
        .find(START)
        .expect("exact initial encoder statement");
    let end = GROUPED_SETUP_CATCH_STATE[start..]
        .find(END)
        .expect("exact observer statement")
        + start;
    let replacement = format!(
        "let $DD = $KEY_5.length;\nlet $DE = 0;\n{optional_encoder}\nwhile ($DE < $DD) {{\n{GROUPED_SETUP_SAMPLE_TAG_GUARD}\nif ($CQ === void 0) {{\nthrow new TypeError(\"Cannot read properties of undefined (reading 'undefined')\");\n}}\n++$DE;\n}}\n"
    );
    let mut source = GROUPED_SETUP_CATCH_STATE.to_owned();
    source.replace_range(start..end, &replacement);
    source
}

#[test]
fn cursor_readiness_first_encoder_in_dead_branch_loop_or_conditional_is_insufficient() {
    for optional_encoder in [
        "if (0) { $KEY_96($DB, 0, $DA, 3); }",
        "while (0) { $KEY_96($DB, 0, $DA, 3); }",
        "for (; 0;) { $KEY_96($DB, 0, $DA, 3); }",
        "let $deadResult = 0 ? $KEY_96($DB, 0, $DA, 3) : 0;",
    ] {
        let source = grouped_setup_with_encoder_only_in_optional_path(optional_encoder);
        // Parse and reject both roles, rather than treating a parse failure as
        // a successful negative. No cursor use follows the optional call.
        let result = name_threads(&source, true).expect("optional-encoder fixture parses");
        for target in ["$THREAD_3_96794", "$THREAD_3_40038"] {
            assert_ne!(
                target_outcome(&result, target).evaluator.as_deref(),
                Some(EVALUATOR),
                "optional encoder supplied evidence for {target}: {source}"
            );
        }
    }
}

#[test]
fn cursor_readiness_existing_cursor_survives_a_loop_that_may_not_execute() {
    let source = SHARED_CATCH_CATCH_STATE.replace(
        "let $DR = $KEY_344($DN, $DO, $DM, 3);",
        "let $DR = $KEY_344($DN, $DO, $DM, 3);\nfor (; 0;) { $DR = $DR; }",
    );
    assert_state_pair(&source, "$THREAD_3_58686", "$THREAD_0_46138");
}

#[test]
fn cursor_readiness_break_and_continue_cannot_skip_a_required_cursor_reinitialization() {
    // The loop entered with a definite $DR. A later syntactic reinitialization
    // must not hide an exit that leaves $DR as an ordinary number.
    for exit in ["if (!($DO >= 0)) break;", "continue;"] {
        let replacement = format!(
            "let $DR = $KEY_344($DN, $DO, $DM, 3);\nfor (; $DO < 1;) {{\n$DR = 0;\n{{ {exit} }}\n$DR = $KEY_344($DN, $DO, $DM, $initialCursor);\n}}"
        );
        // A separate incoming cursor keeps the reinitialization valid and
        // prevents unrelated offset rejection from masking the early exit.
        let replacement = replacement.replacen(
            "let $DR = $KEY_344($DN, $DO, $DM, 3);",
            "let $DR = $KEY_344($DN, $DO, $DM, 3);\nlet $initialCursor = $DR;",
            1,
        );
        refuse_state_pair(
            SHARED_CATCH_CATCH_STATE,
            "$THREAD_3_58686",
            "$THREAD_0_46138",
            "let $DR = $KEY_344($DN, $DO, $DM, 3);",
            &replacement,
        );
    }
}

#[test]
fn cursor_readiness_top_level_guard_cannot_move_into_the_real_conditional_sample_loop() {
    let source = GROUPED_SETUP_CATCH_STATE.replacen(GROUPED_SETUP_TOP_LEVEL_TAG_GUARD, "", 1);
    let replacement = format!("while ($DE < $DD) {{\n{GROUPED_SETUP_TOP_LEVEL_TAG_GUARD}");
    refuse_state_pair(
        &source,
        "$THREAD_3_96794",
        "$THREAD_3_40038",
        "while ($DE < $DD) {",
        &replacement,
    );
}
