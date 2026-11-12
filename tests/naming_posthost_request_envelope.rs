use shape::naming::name_threads;

// Source-only family: the request envelope builder's base64url variant.
// The evaluator is the existing p07 structural finder role. The previously
// accepted named base (notifyLastChance inside the collector literal, no charAt
// sextet loop) must stay named, and the new variant must be recognised by the
// same role from its own body only.
const EVALUATOR: &str = "function_finder::request_envelope_builder";
const NAME: &str = "getEncodedData";

// Actual cleaned bodies of the four shape variants (opaque spellings only).
// Each body carries the enclosing declarations of the captured locals exactly
// as the cleaned input does, so packet `f` and checksum `b` resolve the same
// outer transaction-token symbol. Tail groups use the traced arrow
// identity-call form, exactly as emitted.
const BASE_ORDER: &str = r##"let $KEY_253, $KEY_315, $KEY_330, $KEY_291, $KEY_268, $KEY_362;
function $THREAD_6_94713($BP) {
    let $KEY_13 = $BP != null ? $BP : Object.create(null);
    let $KEY_2 = $KEY_13.signalFilter === void 0 ? $KEY_253 : $KEY_13.signalFilter;
    let $KEY_12 = typeof ($KEY_13.encode === void 0 ? null : $KEY_13.encode) === "function" ? $KEY_13.encode === void 0 ? null : $KEY_13.encode : $KEY_315;
    let $BQ = Object.create(null);
    $KEY_2 = {
        signalFilter: $KEY_2,
        includeDebugNames: false
    };
    $KEY_2.notifyLastChance = ($KEY_13.notifyLastChance === void 0 || $KEY_13.notifyLastChance) ?? true;
    $KEY_12 = $KEY_12($KEY_330($KEY_2));
    let $KEY_11 = $KEY_291 ?? "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_=";
    let $BR = $KEY_12.length - $KEY_12.length % 3;
    let $BS = 0;
    let $BT = "";
    while ($BS < $BR) {
        let $BU = $KEY_12[$BS + 2];
        $BT = $BT + ($KEY_11.charAt($KEY_12[$BS] >> 2) + $KEY_11.charAt(($KEY_12[$BS] & 3) << 4 | $KEY_12[$BS + 1] >> 4) + $KEY_11.charAt(($KEY_12[$BS + 1] & 15) << 2 | $BU >> 6) + $KEY_11.charAt($BU & 63));
        $BS += 3;
    }
    if ($KEY_12.length % 3 === 2) {
        $BT = $BT + (($PARAMA, $PARAMB, $PARAMC, $PARAMD, $PARAME, $PARAMF) => {
        return $PARAMA + $PARAMC.call($PARAMB, $PARAMD) + $PARAME.charAt($PARAMF);
        })(($KEY_11.charAt($KEY_12[$BS] >> 2)), $KEY_11, ($KEY_11.charAt), (($KEY_12[$BS] & 3) << 4 | $KEY_12[$BS + 1] >> 4), $KEY_11, (($KEY_12[$BS + 1] & 15) << 2));
    } else if ($KEY_12.length % 3 === 1) {
        $KEY_12 = $KEY_12[$BS];
        $BT = $BT + ($KEY_11.charAt($KEY_12 >> 2) + $KEY_11.charAt(($KEY_12 & 3) << 4));
    }
    $BQ.f = $KEY_268;
    $BQ.b = $KEY_362($KEY_268 + $BT).toString(36);
    $BQ.c = "AEBe7IahAQAAA1KZWILPEsyBdRCwO5H7N9dugyFZcFdzAX3Zs9rTcIhlL0eE";
    $BQ.d = $KEY_13.descriptor === void 0 ? "ADYAhICBAKGAgQGAAYAQgISigaIAwAGAyPpCxg_13ocRnsD3CLzTmBgQ2tNwiGUvR4T_____7oV7YgF_UzGrK5Ma2UIU_3FUeFAs" : $KEY_13.descriptor;
    $BQ.z = "q";
    $BQ.a = $BT;
    return $BQ;
}"##;

const REORDERED: &str = r##"let $KEY_6, $KEY_61, $KEY_121, $KEY_368, $KEY_179, $KEY_359;
function $THREAD_5_159216($BH) {
    let $KEY_12 = $BH != null ? $BH : Object.create(null);
    let $KEY_2 = $KEY_12.signalFilter === void 0 ? $KEY_6 : $KEY_12.signalFilter;
    let $KEY_11 = typeof ($KEY_12.encode === void 0 ? null : $KEY_12.encode) === "function" ? $KEY_12.encode === void 0 ? null : $KEY_12.encode : $KEY_61;
    let $BI = Object.create(null);
    $KEY_2 = {
        signalFilter: $KEY_2,
        includeDebugNames: false
    };
    let $KEY_16 = $KEY_121 ?? "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_=";
    $KEY_2.notifyLastChance = ($KEY_12.notifyLastChance === void 0 || $KEY_12.notifyLastChance) ?? true;
    let $KEY_15 = $KEY_11($KEY_368($KEY_2));
    let $BJ = $KEY_15.length - $KEY_15.length % 3;
    let $BK = 0;
    let $BL = "";
    while ($BK < $BJ) {
        let $BM = $KEY_15[$BK + 2];
        $BL = $BL + ($KEY_16.charAt($KEY_15[$BK] >> 2) + $KEY_16.charAt(($KEY_15[$BK] & 3) << 4 | $KEY_15[$BK + 1] >> 4) + $KEY_16.charAt(($KEY_15[$BK + 1] & 15) << 2 | $BM >> 6) + $KEY_16.charAt($BM & 63));
        $BK += 3;
    }
    if ($KEY_15.length % 3 === 2) {
        $BL = $BL + ($KEY_16.charAt($KEY_15[$BK] >> 2) + $KEY_16.charAt(($KEY_15[$BK] & 3) << 4 | $KEY_15[$BK + 1] >> 4) + $KEY_16.charAt(($KEY_15[$BK + 1] & 15) << 2));
    } else if ($KEY_15.length % 3 === 1) {
        $KEY_15 = $KEY_15[$BK];
        $BL = $BL + ($KEY_16.charAt($KEY_15 >> 2) + $KEY_16.charAt(($KEY_15 & 3) << 4));
    }
    $BI.f = $KEY_179;
    $BI.b = $KEY_359($KEY_179 + $BL).toString(36);
    $BI.c = "AOBYO3ChAQAAQuBozFW-WgPbT1yWZiYYeRwpSJDr3Q29zTOGRXGJ_hoAaHMP";
    $BI.d = $KEY_12.descriptor === void 0 ? "ADYAhICBAKGAgQGAAYAQgISigaIAwAGAyPpCxg_13ocRnsD3CLzTmBgQcYn-GgBocw______pWNdWAIoUqqxER-awp7b5_Gok2e7" : $KEY_12.descriptor;
    $BI.z = "q";
    $BI.a = $BL;
    return $BI;
}"##;

const HOISTED_ENCODE: &str = r##"let $KEY_250, $KEY_313, $KEY_232, $KEY_215, $KEY_381, $KEY_196;
function $THREAD_9_98043($BO) {
    let $KEY_8 = $BO != null ? $BO : Object.create(null);
    let $BP;
    if ($KEY_8.encode === void 0) {
        $BP = null;
    } else {
        $BP = $KEY_8.encode;
    }
    let $BQ = $KEY_8.signalFilter === void 0 ? $KEY_250 : $KEY_8.signalFilter;
    $BP = typeof $BP === "function" ? $BP : $KEY_313;
    let $BR = Object.create(null);
    $BQ = {
        signalFilter: $BQ,
        includeDebugNames: false
    };
    $BQ.notifyLastChance = ($KEY_8.notifyLastChance === void 0 || $KEY_8.notifyLastChance) ?? true;
    $BQ = $BP($KEY_232($BQ));
    let $KEY_3 = $KEY_215 ?? "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_=";
    let $BS = $BQ.length - $BQ.length % 3;
    let $BT = 0;
    let $BU = "";
    while ($BT < $BS) {
        let $BV = $BQ[$BT + 2];
        $BU = $BU + ((($PARAMA, $PARAMB, $PARAMC, $PARAMD) => {
        return $PARAMA + $PARAMB + $PARAMC.charAt($PARAMD);
        })(($KEY_3.charAt($BQ[$BT] >> 2)), ($KEY_3.charAt(($BQ[$BT] & 3) << 4 | $BQ[$BT + 1] >> 4)), $KEY_3, (($BQ[$BT + 1] & 15) << 2 | $BV >> 6)) + $KEY_3.charAt($BV & 63));
        $BT += 3;
    }
    if ($BQ.length % 3 === 2) {
        $BU = $BU + (($PARAMA, $PARAMB, $PARAMC, $PARAMD) => {
        return $PARAMA + $PARAMB + $PARAMC.charAt($PARAMD);
        })(($KEY_3.charAt($BQ[$BT] >> 2)), ($KEY_3.charAt(($BQ[$BT] & 3) << 4 | $BQ[$BT + 1] >> 4)), $KEY_3, (($BQ[$BT + 1] & 15) << 2));
    } else if ($BQ.length % 3 === 1) {
        $BQ = $BQ[$BT];
        $BU = $BU + ($KEY_3.charAt($BQ >> 2) + $KEY_3.charAt(($BQ & 3) << 4));
    }
    $BR.f = $KEY_381;
    $BR.b = $KEY_196($KEY_381 + $BU).toString(36);
    $BR.c = "AMCQGWuhAQAADSiQISFxYxxrMaFhEYV1fnVG5eNCcZDLFzXYxpXvHkW-4-M4";
    $BR.d = $KEY_8.descriptor === void 0 ? "ADYAhICBAKGAgQGAAYAQgISigaIAwAGAyPpCxg_13ocRnsD3CLzTmBgQle8eRb7j4zgAAAAAdtsn6wKjEBmJcxq-ZlgCVxZ_TZ9Z" : $KEY_8.descriptor;
    $BR.z = "q";
    $BR.a = $BU;
    return $BR;
}"##;

const HOISTED_DESCRIPTOR: &str = r##"let $KEY_208, $KEY_52, $KEY_58, $KEY_101, $KEY_155, $KEY_370;
function $THREAD_3_133534($BX) {
    let $KEY_1 = $BX != null ? $BX : Object.create(null);
    let $KEY_5 = $KEY_1.signalFilter === void 0 ? $KEY_208 : $KEY_1.signalFilter;
    let $BY;
    if ($KEY_1.descriptor === void 0) {
        $BY = "ADYAhICBAKGAgQGAAYAQgISigaIAwAGAyPpCxg_13ocRnsD3CLzTmBgQbM5poCVBZ24AAAAAdtsn6wI9CRV2FuG8jj3HSw53zx70";
    } else {
        $BY = $KEY_1.descriptor;
    }
    let $BZ = typeof ($KEY_1.encode === void 0 ? null : $KEY_1.encode) === "function" ? $KEY_1.encode === void 0 ? null : $KEY_1.encode : $KEY_52;
    let $CA = Object.create(null);
    $KEY_5 = {
        signalFilter: $KEY_5,
        includeDebugNames: false
    };
    $KEY_5.notifyLastChance = ($KEY_1.notifyLastChance === void 0 || $KEY_1.notifyLastChance) ?? true;
    $BZ = $BZ($KEY_58($KEY_5));
    let $CB = $KEY_101 ?? "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_=";
    let $CC = $BZ.length - $BZ.length % 3;
    let $CD = 0;
    let $CE = "";
    while ($CD < $CC) {
        let $CF = $BZ[$CD + 2];
        $CE = $CE + ($CB.charAt($BZ[$CD] >> 2) + $CB.charAt(($BZ[$CD] & 3) << 4 | $BZ[$CD + 1] >> 4) + $CB.charAt(($BZ[$CD + 1] & 15) << 2 | $CF >> 6) + $CB.charAt($CF & 63));
        $CD += 3;
    }
    if ($BZ.length % 3 === 2) {
        $CE = $CE + ($CB.charAt($BZ[$CD] >> 2) + $CB.charAt(($BZ[$CD] & 3) << 4 | $BZ[$CD + 1] >> 4) + $CB.charAt(($BZ[$CD + 1] & 15) << 2));
    } else if ($BZ.length % 3 === 1) {
        $BZ = $BZ[$CD];
        $CE = $CE + ($CB.charAt($BZ >> 2) + $CB.charAt(($BZ & 3) << 4));
    }
    $CA.f = $KEY_155;
    $CA.b = $KEY_370($KEY_155 + $CE).toString(36);
    $CA.c = "AMCbGm6hAQAAIBYZ7zjRgB7JS0tstYxbGa-aWicZW8kSmwCExGzOaaAlQWdu";
    $CA.d = $BY;
    $CA.z = "q";
    $CA.a = $CE;
    return $CA;
}"##;

// The previously accepted named base: notifyLastChance inside the collector
// options literal and no base64url charAt loop.
const OLD_BASE: &str = r##"let $KEY_253, $KEY_315, $KEY_330, $KEY_268, $KEY_362;
function $THREAD_6_90001($BP) {
    let $KEY_13 = $BP != null ? $BP : Object.create(null);
    let $KEY_2 = $KEY_13.signalFilter === void 0 ? $KEY_253 : $KEY_13.signalFilter;
    let $KEY_12 = typeof ($KEY_13.encode === void 0 ? null : $KEY_13.encode) === "function" ? $KEY_13.encode === void 0 ? null : $KEY_13.encode : $KEY_315;
    let $BQ = Object.create(null);
    $KEY_2 = { signalFilter: $KEY_2, includeDebugNames: false, notifyLastChance: ($KEY_13.notifyLastChance === void 0 || $KEY_13.notifyLastChance) ?? true };
    $KEY_12 = $KEY_12($KEY_330($KEY_2));
    $BQ.f = $KEY_268;
    $BQ.b = $KEY_362($KEY_268 + $KEY_12).toString(36);
    $BQ.c = "AAA";
    $BQ.d = $KEY_13.descriptor === void 0 ? "BBB" : $KEY_13.descriptor;
    $BQ.z = "q";
    $BQ.a = $KEY_12;
    return $BQ;
}"##;

const ENCODER_ONLY_DECOY: &str = r##"function main(){function $THREAD_6_90010($A){let $R=$A!=null?$A:Object.create(null);let $F=$R.signalFilter===void 0?$X:$R.signalFilter;$F={signalFilter:$F,includeDebugNames:false};$F.notifyLastChance=($R.notifyLastChance===void 0||$R.notifyLastChance)??true;let $E=typeof($R.encode===void 0?null:$R.encode)==="function"?$R.encode:$Y;let $S="";let $I=0;let $L=$E.length-$E.length%3;while($I<$L){let $J=$E[$I+2];$S=$S+($Z.charAt($E[$I]>>2)+$Z.charAt(($E[$I]&3)<<4|$E[$I+1]>>4)+$Z.charAt(($E[$I+1]&15)<<2|$J>>6)+$Z.charAt($J&63));$I+=3;}return $S;}return $THREAD_6_90010;}main();"##;

const NESTED_DONOR_DECOY: &str = r##"function main(){function $THREAD_6_90011($A){function $THREAD_6_90012($B){let $S="";let $I=0;let $L=$B.length-$B.length%3;while($I<$L){let $J=$B[$I+2];$S=$S+($A.charAt($B[$I]>>2)+$A.charAt(($B[$I]&3)<<4|$B[$I+1]>>4)+$A.charAt(($B[$I+1]&15)<<2|$J>>6)+$A.charAt($J&63));$I+=3;}return $S;}return $THREAD_6_90012;}return $THREAD_6_90011;}main();"##;

const PACKET_DECOY: &str = r##"function main(){function $THREAD_6_90013(){let $R=Object.create(null);$R.f=1;$R.b=2;$R.c=3;$R.d=4;$R.z=5;$R.a=6;return $R;}return $THREAD_6_90013;}main();"##;

// The wrapper body is kept byte-for-byte. This role recognizes only that shape; callee
// meaning comes from the source-backed builder proof below.
const ACTUAL_WRAPPER_59_BYTES: &str =
    "function $THREAD_9_49854($BR) {\n\t\t\treturn $KEY_75($BR);\n\t\t}";

fn fixture(body: &str, name: &str) -> String {
    format!("function main(){{\n{body}\nreturn {name};\n}}\nmain();")
}

// The carrier main has a broader direct binding set than any sibling, remaining uniquely
// reachable even when a negative calls the builder by its declared name. `let $KEY_19,
// $KEY_75, $KEY_271;` gives main 11 bindings against the builder 9. A lone `let $KEY_75;`
// would tie them at 9 and trigger main-scope ambiguity before the forwarder proof runs.
fn forwarder_fixture_with_wrapper(body: &str, wrapper: &str) -> String {
    format!(
        "function main(){{\nlet $KEY_19, $KEY_75, $KEY_271;\n{body}\n$KEY_75 = $THREAD_6_94713;\n{wrapper}\nreturn [$THREAD_6_94713, $THREAD_9_49854];\n}}\nmain();"
    )
}

fn forwarder_fixture(body: &str, wrapper_body: &str) -> String {
    forwarder_fixture_with_wrapper(
        body,
        &format!("function $THREAD_9_49854($BR) {{{wrapper_body}}}"),
    )
}

fn forwarder_claimed(source: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        result.report.function_roles.iter().any(|role| {
            role.function_span.start > 0
                && role.candidates == vec!["forwardGetEncodedData".to_string()]
                && role
                    .evaluators
                    .iter()
                    .any(|e| e == "function_finder::request_envelope_forwarder")
        })
    })
}

fn parses(source: &str) -> bool {
    name_threads(source, true).is_ok()
}

fn named(source: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        result
            .report
            .outcomes
            .iter()
            .any(|outcome| outcome.applied && outcome.proposed_name.as_deref() == Some(NAME))
    })
}

fn our_claim(source: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        result
            .report
            .function_roles
            .iter()
            .any(|role| role.evaluators.iter().any(|e| e == EVALUATOR))
    })
}

fn denied(source: &str) {
    assert!(parses(source), "fixture did not parse: {source}");
    assert!(!named(source), "unexpected {NAME} for: {source}");
    assert!(!our_claim(source), "unexpected posthost claim for: {source}");
}

fn accepts(body: &str, name: &str) -> String {
    let source = fixture(body, name);
    assert!(parses(&source), "{name} did not parse");
    assert!(named(&source), "{name} was not named");
    assert!(our_claim(&source), "{name} was not our claim");
    source
}

/// The four actual cleaned shapes: base order, the reordered alphabet/notify
/// form, the hoisted-encode form and the hoisted-descriptor form.
#[test]
fn accepts_actual_variants() {
    accepts(BASE_ORDER, "$THREAD_6_94713");
    accepts(REORDERED, "$THREAD_5_159216");
    accepts(HOISTED_ENCODE, "$THREAD_9_98043");
    accepts(HOISTED_DESCRIPTOR, "$THREAD_3_133534");
}

#[test]
fn names_the_actual_one_hop_forwarder_after_the_proven_builder() {
    assert_eq!(ACTUAL_WRAPPER_59_BYTES.len(), 59);
    let source = forwarder_fixture_with_wrapper(BASE_ORDER, ACTUAL_WRAPPER_59_BYTES);
    let result = name_threads(&source, true).unwrap();
    assert!(result
        .report
        .outcomes
        .iter()
        .any(|outcome| outcome.applied
            && outcome.proposed_name.as_deref() == Some("getEncodedData")));
    assert!(result
        .report
        .outcomes
        .iter()
        .any(|outcome| outcome.applied
            && outcome.proposed_name.as_deref() == Some("forwardGetEncodedData")));
    assert!(forwarder_claimed(&source));
}

#[test]
fn forwarder_is_alpha_renamed_but_never_uses_generated_spellings_as_proof() {
    let source = forwarder_fixture(BASE_ORDER, "return $KEY_75($BR);")
        .replace("$KEY_75", "$CAPTURED")
        .replace("$THREAD_6_94713", "$THREAD_77_12345")
        .replace("$THREAD_9_49854", "$THREAD_88_54321")
        .replace("$BR", "input");
    let result = name_threads(&source, true).unwrap();
    assert!(result.source.contains("function forwardGetEncodedData(input)"));
    assert!(result
        .report
        .outcomes
        .iter()
        .any(|outcome| outcome.proposed_name.as_deref() == Some("getEncodedData")
            && outcome.applied));
}

#[test]
fn forwarder_rejects_nontrivial_near_misses() {
    let base = forwarder_fixture(BASE_ORDER, "return $KEY_75($BR);");
    // The shared carrier itself must be processed as a valid program before any
    // role is checked, so the positives cannot silently rely on an upstream
    // program-properties error being read as a rejection.
    assert!(parses(&base), "base carrier did not parse: {base}");
    let cases = [
        // A second statement means the wrapper is no longer a pure one-hop
        // return, even though its final call looks identical.
        base.replace(
            "function $THREAD_9_49854($BR) {return $KEY_75($BR);}",
            "function $THREAD_9_49854($BR) {let $LOCAL = $BR; return $KEY_75($BR);}",
        ),
        // The call must use the actual captured callee, not another function.
        base.replace("return $KEY_75($BR);", "return $THREAD_6_94713($BR);"),
        // A second write destroys the unique captured origin.
        base.replace(
            "$KEY_75 = $THREAD_6_94713;",
            "$KEY_75 = $THREAD_6_94713; $KEY_75 = $THREAD_6_94713;",
        ),
        // A captured non-envelope function is not promoted by the wrapper.
        base.replace(
            "$KEY_75 = $THREAD_6_94713;",
            "$KEY_75 = $THREAD_6_90014; function $THREAD_6_90014($BR){return $BR;}",
        ),
        // The formal must be passed unchanged and alone.
        base.replace("return $KEY_75($BR);", "return $KEY_75($BR + 0);"),
        // The return value must be the direct call result.
        base.replace("return $KEY_75($BR);", "return void $KEY_75($BR);"),
        // A written formal is not a stable forwarding input.
        base.replace(
            "function $THREAD_9_49854($BR) {return $KEY_75($BR);}",
            "function $THREAD_9_49854($BR){$BR = $BR; return $KEY_75($BR);}",
        ),
        // Async, rest, explicit receiver, and nested-donor forms are outside
        // the one-hop proof even when their body contains the same call.
        base.replace(
            "function $THREAD_9_49854($BR)",
            "async function $THREAD_9_49854($BR)",
        ),
        base.replace(
            "function $THREAD_9_49854($BR)",
            "function $THREAD_9_49854(...$BR)",
        ),
        base.replace("return $KEY_75($BR);", "return this.$KEY_75($BR);"),
        base.replace(
            "function $THREAD_9_49854($BR) {return $KEY_75($BR);}",
            "function $THREAD_9_49854($BR){function $THREAD_9_49855(x){return x;} return $KEY_75($BR);}",
        ),
    ];
    for source in cases {
        assert!(parses(&source), "near-miss did not parse: {source}");
        assert!(!forwarder_claimed(&source), "unexpected forwarder claim: {source}");
    }
}

/// The previously accepted named base is preserved: notifyLastChance inside the
/// collector literal with no charAt loop.
#[test]
fn preserves_old_no_base64_base() {
    accepts(OLD_BASE, "$THREAD_6_90001");

    // Moving notifyLastChance out of the literal does not by itself authorise a
    // claim: without the inline sextet loop this stays unclaimed.
    let split = OLD_BASE
        .replace(
            " { signalFilter: $KEY_2, includeDebugNames: false, notifyLastChance: ($KEY_13.notifyLastChance === void 0 || $KEY_13.notifyLastChance) ?? true }",
            " { signalFilter: $KEY_2, includeDebugNames: false }",
        )
        .replace(
            "$KEY_12 = $KEY_12($KEY_330($KEY_2));",
            "$KEY_2.notifyLastChance = ($KEY_13.notifyLastChance === void 0 || $KEY_13.notifyLastChance) ?? true;\n    $KEY_12 = $KEY_12($KEY_330($KEY_2));",
        );
    denied(&fixture(&split, "$THREAD_6_90001"));
}

/// Recognition never reads the alphabet literal, the c key, the d default or
/// the z constant.
#[test]
fn does_not_gate_alphabet_or_key_constants() {
    let alphabet = BASE_ORDER.replace(
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_=",
        "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ+/=",
    );
    assert!(named(&fixture(&alphabet, "$THREAD_6_94713")));
    assert!(our_claim(&fixture(&alphabet, "$THREAD_6_94713")));

    let key = BASE_ORDER.replace(
        "\"AEBe7IahAQAAA1KZWILPEsyBdRCwO5H7N9dugyFZcFdzAX3Zs9rTcIhlL0eE\"",
        "\"ZZZZ\"",
    );
    assert!(named(&fixture(&key, "$THREAD_6_94713")));

    let descriptor = BASE_ORDER.replace(
        "\"ADYAhICBAKGAgQGAAYAQgISigaIAwAGAyPpCxg_13ocRnsD3CLzTmBgQ2tNwiGUvR4T_____7oV7YgF_UzGrK5Ma2UIU_3FUeFAs\"",
        "\"Q0Q0Q0\"",
    );
    assert!(named(&fixture(&descriptor, "$THREAD_6_94713")));

    let zed = BASE_ORDER.replace("$BQ.z = \"q\";", "$BQ.z = \"r\";");
    assert!(named(&fixture(&zed, "$THREAD_6_94713")));
}

/// Generated spellings carry no role evidence; only symbol identity decides.
#[test]
fn accepts_renamed_spellings() {
    let renamed = BASE_ORDER
        .replace("$KEY_", "$HK")
        .replace("$THREAD_6_94713", "$THREAD_90_777_2")
        .replace("$BP", "originOptions");
    accepts(&renamed, "$THREAD_90_777_2");
}

#[test]
fn claims_only_the_envelope_builder() {
    let source = fixture(BASE_ORDER, "$THREAD_6_94713");
    let result = name_threads(&source, true).unwrap();
    let claimed: Vec<_> = result
        .report
        .function_roles
        .iter()
        .filter(|role| role.evaluators.iter().any(|e| e == EVALUATOR))
        .collect();
    assert_eq!(claimed.len(), 1, "expected exactly the envelope builder");
    assert_eq!(claimed[0].current_name.as_deref(), Some("$THREAD_6_94713"));
}

#[test]
fn denies_wrong_receiver_key_guard_shadow_and_decoys() {
    let cases = [
        // The sextet loop is no longer a loop.
        BASE_ORDER.replace("while ($BS < $BR) {", "if ($BS < $BR) {"),
        // The sextet emission never calls charAt.
        BASE_ORDER.replace(".charAt(", ".substring("),
        // The payload written to the packet is the raw encoder output, not the
        // local the loop appends into.
        BASE_ORDER.replace("$BQ.a = $BT;", "$BQ.a = $KEY_12;"),
        // The packet has no `a` field bound to the payload.
        BASE_ORDER.replace("$BQ.a = $BT;", "$BQ.x = $BT;"),
        // The integrity field drops the payload from its hash input.
        BASE_ORDER.replace(
            "$BQ.b = $KEY_362($KEY_268 + $BT).toString(36);",
            "$BQ.b = $KEY_362($KEY_268).toString(36);",
        ),
        // notifyLastChance is written onto the encoder local, not the collector.
        BASE_ORDER.replace("$KEY_2.notifyLastChance = ", "$KEY_12.notifyLastChance = "),
        // The notifyLastChance option is read under a different key.
        BASE_ORDER.replace("$KEY_13.notifyLastChance", "$KEY_13.otherChance"),
        // The options guard no longer falls back to a null-prototype object.
        BASE_ORDER.replace("$BP != null ? $BP : Object.create(null)", "$BP != null ? $BP : {}"),
    ];
    for body in cases {
        denied(&fixture(&body, "$THREAD_6_94713"));
    }

    // The global Object is shadowed in the owning scope.
    denied(&fixture(BASE_ORDER, "$THREAD_6_94713").replace(
        "function main(){",
        "function main(){let Object={create:function(){return {};}};",
    ));

    // A base64url byte encoder without the f/b/c/d/z/a null-prototype packet.
    denied(ENCODER_ONLY_DECOY);
    // A parent that only returns a nested byte-encoder child.
    denied(NESTED_DONOR_DECOY);
    // Six packet writes on a null-prototype object, but no options slot.
    denied(PACKET_DECOY);
}

/// The tightened base64url dataflow: exact `% 3` on the owned encoder-output
/// length, one alphabet symbol across the selected loop and the body, and the
/// checksum tied to the captured token written to packet field `f`.
#[test]
fn denies_tightened_emission_dataflow() {
    // The grouping is `% 2`, not the exact `% 3` byte grouping.
    denied(&fixture(&BASE_ORDER.replace("% 3", "% 2"), "$THREAD_6_94713"));

    // The `% 3` is applied to a local that never receives the encoder call.
    denied(&fixture(
        &BASE_ORDER.replace(
            "let $BR = $KEY_12.length - $KEY_12.length % 3;",
            "let $BR = $BP.length - $BP.length % 3;",
        ),
        "$THREAD_6_94713",
    ));

    // The byte index advances by four rather than the three-byte group.
    denied(&fixture(&BASE_ORDER.replace("$BS += 3;", "$BS += 4;"), "$THREAD_6_94713"));

    // The loop keeps one charAt sextet while the remaining quartet is moved to
    // disconnected statements outside the selected loop.
    let disconnected = BASE_ORDER
        .replace(
            "$BT = $BT + ($KEY_11.charAt($KEY_12[$BS] >> 2) + $KEY_11.charAt(($KEY_12[$BS] & 3) << 4 | $KEY_12[$BS + 1] >> 4) + $KEY_11.charAt(($KEY_12[$BS + 1] & 15) << 2 | $BU >> 6) + $KEY_11.charAt($BU & 63));",
            "$BT = $BT + $KEY_11.charAt($KEY_12[$BS] >> 2);",
        )
        .replace(
            "let $BR = $KEY_12.length",
            "let $DEAD = $KEY_11.charAt(0) + $KEY_11.charAt(1) + $KEY_11.charAt(2);\n    let $BR = $KEY_12.length",
        );
    denied(&fixture(&disconnected, "$THREAD_6_94713"));

    // One sextet of the append uses a second alphabet symbol.
    let mixed = BASE_ORDER
        .replace(
            "let $BR = $KEY_12.length",
            "let $KEY_99 = \"ZYX\";\n    let $BR = $KEY_12.length",
        )
        .replace("$KEY_11.charAt($BU & 63)", "$KEY_99.charAt($BU & 63)");
    denied(&fixture(&mixed, "$THREAD_6_94713"));

    // The checksum hashes a different captured token than packet field `f`.
    denied(&fixture(
        &BASE_ORDER.replace(
            "$BQ.b = $KEY_362($KEY_268 + $BT).toString(36);",
            "$BQ.b = $KEY_362($KEY_253 + $BT).toString(36);",
        ),
        "$THREAD_6_94713",
    ));

    // The checksum payload is the encoder output, not the packet `a` payload.
    denied(&fixture(
        &BASE_ORDER.replace(
            "$BQ.b = $KEY_362($KEY_268 + $BT).toString(36);",
            "$BQ.b = $KEY_362($KEY_268 + $KEY_12).toString(36);",
        ),
        "$THREAD_6_94713",
    ));

    // notifyLastChance is written onto a local that never holds the collector
    // options literal, disconnecting the collector from the envelope.
    denied(&fixture(
        &BASE_ORDER.replace("$KEY_2.notifyLastChance = ", "$KEY_13.notifyLastChance = "),
        "$THREAD_6_94713",
    ));
}

#[test]
fn preserves_human_names_and_collision_guards() {
    let human = fixture(BASE_ORDER, "$THREAD_6_94713").replace("$THREAD_6_94713", NAME);
    let result = name_threads(&human, true).unwrap();
    assert!(result.source.contains(&format!("function {NAME}(")));
    assert!(!result
        .report
        .outcomes
        .iter()
        .any(|outcome| outcome.binding.name == NAME && outcome.applied));

    for collision in [
        fixture(BASE_ORDER, "$THREAD_6_94713").replace(
            "function main(){",
            &format!("function main(){{let {NAME}=17;useValue({NAME});"),
        ),
        fixture(BASE_ORDER, "$THREAD_6_94713").replace(
            "function main(){",
            &format!("function main(){{function {NAME}(){{}}useValue({NAME});"),
        ),
    ] {
        assert!(parses(&collision), "{collision}");
        assert!(!named(&collision), "{collision}");
    }
}

#[test]
fn edits_are_identifier_only_replacements() {
    let source = fixture(BASE_ORDER, "$THREAD_6_94713");
    let mut result = name_threads(&source, true).unwrap();
    let applied: Vec<_> = result
        .report
        .outcomes
        .iter()
        .filter(|outcome| outcome.applied && outcome.proposed_name.as_deref() == Some(NAME))
        .collect();
    assert_eq!(applied.len(), 1, "expected exactly one applied target");

    // Replay the edits in descending byte order: report.edits is not guaranteed
    // sorted, so sort explicitly rather than reversing an unsorted list.
    result.report.edits.sort_by(|a, b| b.start.cmp(&a.start));
    let mut replay = source.clone();
    for edit in &result.report.edits {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, result.source);
}
