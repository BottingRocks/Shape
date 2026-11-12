//! Real saved callback bodies with constructed captures from separate accepted
//! helper fixtures. The constructed capture harness is not source-backed.
use shape::naming::name_threads;
const NUMBER_WRITER: &str = r#"function $THREAD_1_96432($BZ, $CA, $CB, $CC) {
					let $CD;
					if (($CA | 0) === $CA) {
						let $CE;
						if ($CA < 0) {
							$CE = $CA < 0;
						} else if ($CA === 0) {
							$CE = 1 / $CA < 0;
						} else {
							$CE = false;
						}
						let $CF, $CG;
						if ($CE) {
							$CF = 64;
							$CG = -$CA;
						} else {
							$CF = 0;
							$CG = $CA;
						}
						if ($CG < 32) {
							$CD = Number($CC) + 1;
							$CB[$CD] = ($CF | $CG) ^ $BZ();
						} else if ($CG < 4096) {
							let $CH = Number($CC) + 1;
							$CB[$CH] = ($CF | ($CG & 31 | 32)) ^ $BZ();
							$CH = [$CH, $CB];
							$CD = $CH[0] + 1;
							$CH[1][$CD] = $CG >> 5 ^ $BZ();
						} else if ($CG < 524288) {
							let $CI = Number($CC) + 1;
							$CB[$CI] = ($CF | ($CG & 31 | 32)) ^ $BZ();
							$CB[$CI + 1] = ($CG >> 5 & 127 | 128) ^ $BZ();
							$CB[$CI + 2] = $CG >>> 12 ^ $BZ();
							$CD = $CI + 2;
						} else if ($CG < 67108864) {
							let $CJ = Number($CC) + 1;
							$CB[$CJ] = ($CF | ($CG & 31 | 32)) ^ $BZ();
							$CB[++$CJ] = ($CG >> 5 & 127 | 128) ^ $BZ();
							$CJ = [$CJ, $CB];
							let $CK = $CJ[0] + 1;
							$CJ[1][$CK] = ($CG >>> 12 & 127 | 128) ^ $BZ();
							$CK = [$CK, $CB];
							$CD = $CK[0] + 1;
							$CK[1][$CD] = $CG >>> 19 ^ $BZ();
						} else {
							let $CL = Number($CC) + 1;
							$CB[$CL] = ($CF | ($CG & 31 | 32)) ^ $BZ();
							$CB[++$CL] = ($CG >> 5 & 127 | 128) ^ $BZ();
							$CB[++$CL] = ($CG >>> 12 & 127 | 128) ^ $BZ();
							$CL = [$CL, $CB];
							let $CM = $CL[0] + 1;
							$CL[1][$CM] = ($CG >>> 19 & 127 | 128) ^ $BZ();
							$CB[$CM + 1] = $CG >>> 26 ^ $BZ();
							$CD = $CM + 1;
						}
					} else {
						$CD = Number($CC) + 1;
						$CB[$CD] = $BZ() ^ 128;
						let $CN;
						if ($KEY_187) {
							$KEY_187[0] = $CA;
							$CN = new $KEY_202($KEY_187.buffer);
						} else if ($CA !== $CA) {
							$CN = $KEY_289;
						} else {
							const $CO = $CA < 0 || $CA === 0 && 1 / $CA < 0;
							let $CP = Math.abs($CA);
							let $CQ, $KEY_3;
							if ($CP >= Math.pow(2, -1022)) {
								$CQ = Math.min(Math.floor(Math.log($CP) / Math.LN2), 1023);
								$CP = $CP / Math.pow(2, $CQ);
								let $CR = $CP < 1;
								$CQ = $CR ? $CQ - 1 : $CQ;
								$CR = $CR ? $CP * 2 : $CP;
								let $CS = $CR >= 2;
								$CQ = $CS ? $CQ + 1 : $CQ;
								$CS = $CS ? $CR / 2 : $CR;
								let $KEY_9 = Math.pow(2, 52);
								let $KEY_5 = Math.floor($CS * $KEY_9);
								$KEY_5 = $CS * $KEY_9 - $KEY_5 < .5 ? $KEY_5 : $CS * $KEY_9 - $KEY_5 > .5 ? $KEY_5 + 1 : $KEY_5 % 2 ? $KEY_5 + 1 : $KEY_5;
								let $CT = ($KEY_5 - $KEY_9) / $KEY_9 >= 1;
								$CQ = $CT ? $CQ + 1024 : $CQ + 1023;
								if ($CQ > 2046) {
									$CQ = 2047;
									$KEY_3 = 0;
								} else {
									$KEY_3 = $CT ? 0 : $KEY_5 - $KEY_9;
								}
							} else {
								$CP = $CP / Math.pow(2, -1074);
								$KEY_3 = Math.floor($CP);
								$CQ = 0;
								$KEY_3 = $CP - $KEY_3 < .5 ? $KEY_3 : $CP - $KEY_3 > .5 ? $KEY_3 + 1 : $KEY_3 % 2 ? $KEY_3 + 1 : $KEY_3;
							}
							let $CU = [];
							let j = 51;
							for (; j >= 0; j--) {
								$CU.unshift(!!($KEY_3 & 1));
								$KEY_3 = Math.floor($KEY_3 / 2);
							}
							let $CV = 11;
							let $CW;
							for (;;) {
								$CW = $CU;
								if ($CV <= 0) break;
								$CW.unshift(!!($CQ & 1));
								--$CV;
								$CQ = Math.floor($CQ / 2);
							}
							$CW.unshift($CO);
							$CN = [];
							let i = 56;
							for (; i >= 0; i = i - 8) {
								$CN.push(Number($CU[i]) << 7 | Number($CU[i + 1]) << 6 | Number($CU[i + 2]) << 5 | Number($CU[i + 3]) << 4 | Number($CU[i + 4]) << 3 | Number($CU[i + 5]) << 2 | Number($CU[i + 6]) << 1 | Number($CU[i + 7]));
							}
						}
						let k = 0;
						for (; k < 8; k++) {
							$CB[++$CD] = $CN[k] ^ $BZ();
						}
					}
					return $CD;
				}"#;

const KEYSTREAM_FACTORY: &str = r#"function $THREAD_9_35814($BZ) {
					let $KEY_1, $KEY_2, $KEY_3, $KEY_4;
					function $THREAD_7_28359() {
						let $CA = $KEY_1;
						$KEY_1 = ($CA + 1) % 4;
						if (($CA + 1) % 4 === 0) {
							$KEY_4 = $KEY_3 + $KEY_2;
							let $CB = $KEY_2;
							$KEY_2 = $CB ^ $KEY_3;
							$KEY_3 = $CB ^ $KEY_3 ^ ($KEY_3 << 26 | $KEY_3 >>> 6) ^ $KEY_2 << 9;
							$KEY_2 = $KEY_2 << 13 | $KEY_2 >>> 19;
							return $KEY_4 & 255;
						}
						if (($CA + 1) % 4 === 1) {
							return $KEY_4 >> 8 & 255;
						}
						if (($CA + 1) % 4 === 2) {
							return $KEY_4 >> 16 & 255;
						}
						if (($CA + 1) % 4 === 3) {
							return $KEY_4 >>> 24 & 255;
						} else {
							return void 0;
						}
					}
					$KEY_3 = $KEY_68;
					$KEY_2 = $BZ;
					$KEY_1 = -1;
					$THREAD_7_28359();
					$THREAD_7_28359();
					$THREAD_7_28359();
					$THREAD_7_28359();
					return $THREAD_7_28359;
				}"#;

const BODY: &str = r#"function $THREAD_2_108242($AW) {
						let $KEY_5 = Math.random() * 4294967296 | 0;
						let $AX = [$KEY_5 >>> 24, $KEY_5 >> 16 & 255, $KEY_5 >> 8 & 255, $KEY_5 & 255];
						let $KEY_1 = $KEY_10($KEY_5 ^ 784010925);
						let $AY = $AW.length;
						let $AZ = $KEY_174($KEY_1, $AW.length, $AX, 3);
						for (;;) {
							--$AY;
							if (!($AY >= 0)) break;
							$AZ = $KEY_174($KEY_1, $AW[$AY], $AX, $AZ);
							$AY = Number($AY);
						}
						$KEY_9.next([[], $AX]);
						$KEY_9.complete();
					}"#;
const OLD_BODY: &str = r#"function $THREAD_8_50596($AW) {
						let $KEY_3 = Math.random() * 4294967296 | 0;
						let $AX = [$KEY_3 >>> 24, $KEY_3 >> 16 & 255, $KEY_3 >> 8 & 255, $KEY_3 & 255];
						let $AY = [];
						$KEY_3 = $KEY_10($KEY_3 ^ 1766291345);
						let $AZ = $AW.length;
						let $BA = $KEY_174($KEY_3, $AZ, $AX, 3);
						while ($AZ - 1 >= 0) {
							$BA = $KEY_174($KEY_3, $AW[$AZ - 1], $AX, $BA);
							--$AZ;
						}
						$KEY_9.next([$AY, $AX]);
						$KEY_9.complete();
					}"#;
fn fixture(body: &str) -> String {
    format!("function main() {{ let observer={{}}, $KEY_9=observer, $KEY_68=17; {NUMBER_WRITER} {KEYSTREAM_FACTORY} let $KEY_174=$THREAD_1_96432; let $KEY_10=$THREAD_9_35814; {body} return $THREAD_2_108242; }}")
}
fn count(source: &str) -> usize {
    name_threads(source, true)
        .unwrap()
        .report
        .function_roles
        .iter()
        .filter(|r| {
            r.evaluators
                .iter()
                .any(|e| e == "owned_reverse_array_emitter")
        })
        .count()
}
#[test]
fn reverse_array_positive_alpha_and_arbitrary_u32_salt() {
    assert_eq!(count(&fixture(BODY)), 1);
    assert_eq!(
        count(
            &fixture(BODY)
                .replace("$AW", "values")
                .replace("$AY", "index")
                .replace("$AZ", "cursor")
                .replace("784010925", "4209900001")
        ),
        1
    );
}
#[test]
fn reverse_array_effect_protocol_and_scope_negatives() {
    for (from, to) in [
        ("$AY >= 0", "$AY > 0"),
        ("--$AY", "++$AY"),
        ("$AW[$AY]", "$AW[$AY - 1]"),
        ("$AX, $AZ)", "$AX, 3)"),
        ("$AX, 3)", "$AX, 4)"),
        ("$KEY_5 >>> 24", "$KEY_5 >>> 16"),
        ("^ 784010925", "+ 784010925"),
        ("^ 784010925", "^ 1.5"),
        ("$KEY_9.complete();", "observer.complete();"),
        ("$KEY_9.complete();", "$KEY_9.complete(); return $AX;"),
        (
            "let $AY = $AW.length;",
            "let $AY = $AW.length; observer.leak($AX);",
        ),
        (
            "let $AY = $AW.length;",
            "let $AY = $AW.length; let escape=()=> $AX;",
        ),
        ("$AW.length", "$AW.size"),
        ("$KEY_9.next([[], $AX])", "$KEY_9.next([$AX, []])"),
        ("$KEY_9.complete();", "$KEY_9.complete($AX);"),
        ("let $KEY_1 = $KEY_10", "let $KEY_1 = observer"),
        ("$AY = Number($AY);", "$AY = Number($AY) + 1;"),
        ("if (!($AY >= 0)) break;", "if (!($AY >= 0)) continue;"),
        ("for (;;) {", "for (;observer;) {"),
    ] {
        assert!(BODY.contains(from), "missing mutation {from}");
        assert_eq!(
            count(&fixture(&BODY.replace(from, to))),
            0,
            "accepted {from} -> {to}"
        );
    }
    for formal in ["Math", "Number"] {
        assert_eq!(
            count(
                &fixture(BODY).replace("function main() {", &format!("function main({formal}) {{"))
            ),
            0
        );
    }
    assert_eq!(
        count(&fixture(BODY).replace(
            "let $KEY_10=$THREAD_9_35814;",
            "let $KEY_10=$THREAD_9_35814; $KEY_10=observer;"
        )),
        0
    );
    assert_eq!(
        count(&fixture(BODY).replace("$KEY_2 = $BZ;", "$KEY_2 = $KEY_68;")),
        0
    );
    assert_eq!(
        count(&fixture(BODY).replace(
            "let $KEY_174=$THREAD_1_96432;",
            "let $KEY_174=function(a,b,c,d){return d;};"
        )),
        0
    );
}
#[test]
fn reverse_array_early_capture_shadow_and_old_overlap_control() {
    let source = fixture(BODY)
        .replace("let $KEY_10=$THREAD_9_35814;", "")
        .replace(
            "return $THREAD_2_108242;",
            "let $KEY_10=$THREAD_9_35814; return $THREAD_2_108242;",
        );
    assert_eq!(count(&source), 0);
    assert_eq!(
        count(&fixture(BODY).replace(
            "function $THREAD_2_108242($AW) {",
            "function $THREAD_2_108242($AW) { let $KEY_174=function(a,b,c,d){return d;};"
        )),
        0
    );
    let old = fixture(OLD_BODY).replace("return $THREAD_2_108242;", "return $THREAD_8_50596;");
    let report = name_threads(&old, true).unwrap().report;
    assert_eq!(count(&old), 0);
    let role = report
        .function_roles
        .iter()
        .find(|r| r.current_name.as_deref() == Some("$THREAD_8_50596"))
        .unwrap();
    assert_eq!(
        role.candidates,
        vec!["emitSeededHashEncoding", "emitSeededSignalFrame"]
    );
}
