//! Focused, portable coverage for the curried interned-string writer and elapsed-clock
//! seeded emitter with `slice(0, 50)`.
//!
//! The callback is wrapped with independently proved interned-string-writer, four-parameter
//! number-writer, and warm-up keystream factories. The keystream factory is initialized
//! once; the emitter sits in the branch returning the initializer object, while the
//! alternate base36 decoder tail writes `$KEY_10 = $KEY_25`. That sole write cannot reach
//! the call edge, exercising control-flow-disjoint resolution. Alpha spellings and the
//! preserved numeric base proof are covered. The literal-`3` cursor sibling remains
//! unclaimed by this bounded inline-cursor rule.
//!
//! The factory formal must feed returned state. Empty tests, discarded arguments, unread
//! locals, dead nested functions, and overwritten aliases must refuse even if the formal is
//! mentioned. Callable writes before the reference, foreign overwrites, fallthrough to the
//! tail write, declarations outside the branch, conditional var initialization, and
//! loop-owned declarations also refuse.
use sha2::{Digest, Sha256};
use shape::naming::name_threads;

const EVALUATOR: &str = "thread_posthost_curried_seeded_emitter";
const NAME: &str = "emitSeededHashEncoding";
const WITNESS_SHA256: &str = "5889ed813200a9f69693becedba460ca8e9f0741641dfcda48500e75f1c531ab";
/// The number-writer body is independently accepted by the mixed-index evaluator. The
/// curried rule re-proves this exact callee, so the positive witness depends on that
/// accepted source being present.
const NUMBER_WRITER_SHA256: &str =
    "88e9b082a6f605485d871c8fe89e6bf89a28ffd1a5ffcef208b96d2648b7c1b0";
const STRING_WRITER_FACTORY_SHA256: &str =
    "fe19f96cb27b95041d2c37fb070fd778c64badadaadc297008a0eaeb74f783f0";
const KEYSTREAM_FACTORY_SHA256: &str =
    "b54299820efb5bd573dc1e63d2fc096ff41dc614ccc9e65de1e7e7c0474f92a8";

/// The elapsed-clock emitter body, preserved verbatim.
const WITNESS_BODY: &str = r#"function $THREAD_6_48236($CK) {
									let $KEY_2 = $KEY_3.slice(0, 50);
									let $KEY_5 = Math.random() * 4294967296 | 0;
									let $CL = [$KEY_5 >>> 24, $KEY_5 >> 16 & 255, $KEY_5 >> 8 & 255, $KEY_5 & 255];
									let $KEY_11 = $KEY_10($KEY_5 ^ 581381427);
									$KEY_2 = $KEY_78($KEY_11)($KEY_2, $CL, $KEY_174($KEY_11, $CK, $CL, 3));
									$KEY_174($KEY_11, $KEY_8.now() - $KEY_6, $CL, $KEY_2);
									$KEY_9.next([[], $CL]);
									$KEY_9.complete();
								}"#;

const STRING_WRITER_FACTORY: &str = r#"function $THREAD_9_47888($BZ) {
					let $KEY_1 = $BZ;
					let $KEY_2;
					let $KEY_0 = Object.create(null);
					$KEY_2 = 0;
					function $THREAD_2_3512($CA, $CB, $CC) {
						let $KEY_8;
						if ($KEY_0[$CA] != null) {
							$KEY_8 = $CC;
							$KEY_8 = Number($CC) + 1;
							$CB[$KEY_8] = $KEY_1() ^ 129;
							$KEY_8 = $KEY_174($KEY_1, $KEY_0[$CA], $CB, $KEY_8);
						} else {
							let $CD = $CA.length;
							let $CE = 0;
							let $CF = $CC;
							while ($CE < $CD) {
								let $CG = $KEY_205($CA[$CE], 0);
								if ($CG === 0) {
									$CB[++$CF] = $KEY_1() ^ 192;
									$CG = 128;
								} else if (!($CG < 128)) if ($CG < 2048) {
									$CB[++$CF] = ($CG >> 6 | 192) ^ $KEY_1();
									$CG = $CG & 63 | 128;
								} else {
									let $CH = $CG >= 55296 && $CG <= 57343 && $CG < 56320 && $CE + 1 < $CD ? $KEY_205($CA[$CE + 1], 0) : null;
									if ($CH == null || $CH < 56320 || $CH > 57343) {
										$CB[++$CF] = ($CG >> 12 | 224) ^ $KEY_1();
										$CB[++$CF] = ($CG >> 6 & 63 | 128) ^ $KEY_1();
										$CG = $CG & 63 | 128;
									} else {
										++$CE;
										$CB[++$CF] = (65536 + ($CG - 55296 << 10) + ($CH - 56320) >> 18 | 240) ^ $KEY_1();
										$CB[++$CF] = (65536 + ($CG - 55296 << 10) + ($CH - 56320) >> 12 & 63 | 128) ^ $KEY_1();
										$CB[++$CF] = (65536 + ($CG - 55296 << 10) + ($CH - 56320) >> 6 & 63 | 128) ^ $KEY_1();
										$CG = 65536 + ($CG - 55296 << 10) + ($CH - 56320) & 63 | 128;
									}
								}
								$CB[++$CF] = $CG ^ $KEY_1();
								++$CE;
							}
							$KEY_8 = Number($CF) + 1;
							$CB[$KEY_8] = $KEY_1();
							let $CI = $KEY_0;
							let $CJ = Number($KEY_2);
							$KEY_2 = $CJ + 1;
							$CI[$CA] = $CJ;
						}
						return $KEY_8;
					}
					return $THREAD_2_3512;
				}"#;

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

/// A borrowed-helper negative: the donor factory returns and warms up the real
/// `nextKeystreamByte` body, but the captured `$THREAD_9_35814` factory returns
/// that donor-owned helper while ignoring its own formal. The captured factory
/// is nested in the donor only so it can name the helper; the helper is still
/// not lexically owned by it. The old arity/role-only proof accepted this.
const BORROWED_KEYSTREAM_FACTORY: &str = r#"function $THREAD_DONOR_1() {
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
					$KEY_2 = 1;
					$KEY_1 = -1;
					$THREAD_7_28359();
					$THREAD_7_28359();
					$THREAD_7_28359();
					$THREAD_7_28359();
					function $THREAD_9_35814($BZ) {
						return $THREAD_7_28359;
					}
					$KEY_10 = $THREAD_9_35814;
					return $THREAD_7_28359;
				}"#;

/// A `writeNumber`-shaped sibling used only by the foreign-writer negative.
const FOREIGN_WRITER: &str = r#"function $THREAD_176($k, $v, $b, $p) {
  $b[$p] = $v ^ $k();
  return $p;
}"#;

/// The sibling uses hash assignment and a literal `3` writer cursor. It belongs to the same
/// family but falls outside this bounded inline-cursor variant.
const SIBLING_BODY: &str = r#"function $THREAD_1_18501($CQ) {
								let $KEY_0 = $KEY_1.slice(0, 50);
								let $CR = Math.random() * 4294967296;
								let $KEY_4 = [($CR | 0) >>> 24, ($CR | 0) >> 16 & 255, ($CR | 0) >> 8 & 255, ($CR | 0) & 255];
								$CR = $KEY_297(($CR | 0) ^ 370509211);
								$KEY_0 = $KEY_87($CR)($KEY_0, $KEY_4, 3);
								$KEY_280($CR, $CQ, $KEY_4, $KEY_280($CR, $KEY_20.now() - $KEY_16, $KEY_4, $KEY_0));
								$KEY_9.next([[], $KEY_4]);
								$KEY_9.complete();
							}"#;

/// The numeric base proof already belongs to the anchor evaluator. It must retain that
/// claim and never be re-claimed here.
const BASE_FIXED: &str = r#"function main(){let observer={next(){},complete(){}},anchor=1;
function hashKey(v){return v;}function mix(h,v,b,s){return s;}
function $THREAD_4_4($S){let seed=Math.random()*4294967296;
let nonce=[(seed|0)>>>24,(seed|0)>>16&255,(seed|0)>>8&255,(seed|0)&255];
seed=hashKey((seed|0)^1234567);
let h=mix(seed,$S[3],nonce,3);h=mix(seed,$S[4],nonce,h);
h=mix(seed,$S[2],nonce,h);h=mix(seed,$S[1],nonce,h);mix(seed,$S[0],nonce,h);
observer.next([[],nonce]);observer.complete();}
return $THREAD_4_4;}main();"#;

/// The keystream factory is initialized once in the outer function. The emitter is defined
/// in the branch returning the initializer object; the alternate base36 decoder tail writes
/// `$KEY_10 = $KEY_25`. That write is unreachable on every path to the emitter call. This
/// tests control-flow-disjoint sole-write resolution: a stability check without that
/// control-flow evidence would refuse the later write.
fn carrier_with_actual_write(body: &str, returned: &str) -> String {
    let mut out = String::from("function main(){");
    out.push_str("let $KEY_3 = \"012345678901234567890123456789012345678901234567890123456789\";");
    out.push_str("let $KEY_8 = Date;");
    out.push_str("let $KEY_6 = 0;");
    out.push_str("let $KEY_9 = {next(){},complete(){}};");
    out.push_str("let $KEY_68 = 0, $KEY_187 = null, $KEY_202 = ArrayBuffer, $KEY_289 = [];");
    out.push_str("let $KEY_174, $KEY_176, $KEY_205;");
    out.push_str(STRING_WRITER_FACTORY);
    out.push_str(NUMBER_WRITER);
    out.push_str(FOREIGN_WRITER);
    out.push_str("$KEY_174 = $THREAD_1_96432;");
    out.push_str("$KEY_176 = $THREAD_176;");
    out.push_str("function $THREAD_205($c, $i) { return 0; }");
    out.push_str("$KEY_205 = $THREAD_205;");
    out.push_str("let $KEY_78 = $THREAD_9_47888;");
    out.push_str(KEYSTREAM_FACTORY);
    out.push_str("let $KEY_10 = $THREAD_9_35814;");
    out.push_str("let $BO = 1;");
    out.push_str("if (typeof $BO !== \"undefined\") {");
    out.push_str(body);
    out.push_str("return ");
    out.push_str(returned);
    out.push_str(";");
    out.push_str("}");
    out.push_str("let $KEY_B36 = [];");
    out.push_str("let $KEY_24 = 0, $KEY_30 = 0;");
    out.push_str("for (;;) {");
    out.push_str("let $KEY_25 = 0;");
    out.push_str("let $KEY_2 = 0;");
    out.push_str("$KEY_B36.splice($KEY_25, 0, Math.floor($KEY_2));");
    out.push_str("++$KEY_25;");
    out.push_str("$KEY_24 = 1;");
    out.push_str("$KEY_30 = 36;");
    out.push_str("$KEY_10 = $KEY_25;");
    out.push_str("break;");
    out.push_str("}");
    out.push_str("}main();");
    out
}

fn witness() -> String {
    carrier_with_actual_write(WITNESS_BODY, "$THREAD_6_48236")
}

fn alpha_body() -> String {
    WITNESS_BODY
        .replace("$THREAD_6_48236", "$THREAD_91_7712_6")
        .replace("$KEY_2", "$CJ")
        .replace("$KEY_5", "$CQ")
        .replace("$CL", "$CR")
        .replace("$KEY_11", "$CS")
        .replace("$CK", "$CP")
}

fn alpha() -> String {
    carrier_with_actual_write(&alpha_body(), "$THREAD_91_7712_6")
}

fn sibling() -> String {
    [
        "function main(){",
        "let $KEY_1 = \"012345678901234567890123456789012345678901234567890123456789\";",
        "let $KEY_20 = Date;",
        "let $KEY_16 = 0;",
        "let $KEY_9 = {next(){},complete(){}};",
        SIBLING_BODY,
        "return $THREAD_1_18501;}main();",
    ]
    .concat()
}

fn sha256_hex(bytes: impl AsRef<[u8]>) -> String {
    Sha256::digest(bytes.as_ref())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn claims(source: &str) -> usize {
    name_threads(source, true)
        .expect("source parses")
        .report
        .function_roles
        .iter()
        .filter(|role| {
            role.candidates.iter().any(|candidate| candidate == NAME)
                && role
                    .evaluators
                    .iter()
                    .any(|evaluator| evaluator == EVALUATOR)
        })
        .count()
}

fn applied(source: &str) -> usize {
    name_threads(source, true)
        .expect("source parses")
        .report
        .outcomes
        .iter()
        .filter(|outcome| {
            outcome.applied
                && outcome.proposed_name.as_deref() == Some(NAME)
                && outcome.evaluator.as_deref() == Some(EVALUATOR)
        })
        .count()
}

fn role_named(source: &str, target: &str, name: &str, evaluator: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        result.report.function_roles.iter().any(|role| {
            role.candidates.iter().any(|candidate| candidate == name)
                && role.evaluators.iter().any(|value| value == evaluator)
        }) && result.report.outcomes.iter().any(|outcome| {
            outcome.binding.name == target
                && outcome.applied
                && outcome.proposed_name.as_deref() == Some(name)
        })
    })
}

/// Both numeric call sites use the foreign four-formal writer. Before the
/// identity fix this mutation was accepted because the two sites matched by
/// symbol and the writer was only checked for arity.
fn both_sites_foreign_body() -> String {
    WITNESS_BODY.replace("$KEY_174", "$KEY_176")
}

/// The captured keystream factory returns a donor-owned helper and never uses
/// its own formal. The old one-hop role check accepted it.
fn borrowed_factory_source() -> String {
    witness()
        .replace(
            "let $KEY_174, $KEY_176, $KEY_205;",
            "let $KEY_174, $KEY_176, $KEY_205, $KEY_10;",
        )
        .replace(KEYSTREAM_FACTORY, BORROWED_KEYSTREAM_FACTORY)
        .replace("let $KEY_10 = $THREAD_9_35814;", "")
}

/// The captured keystream factory declares its formal but seeds the helper from
/// a captured global instead; a dead `$BZ;` mention must not count as use.
fn ignored_seed_source() -> String {
    witness().replace("$KEY_2 = $BZ;", "$BZ; $KEY_2 = $KEY_68;")
}

/// The accepted nested-dead counterexample: the helper's real state comes from
/// the captured `$KEY_68` global while an unused nested function still returns
/// the factory formal `$BZ`. The old formal-feed check counted any reference
/// inside the helper's span, including this nested dead function.
fn nested_dead_formal_source() -> String {
    witness()
        .replace("$KEY_2 = $BZ;", "$KEY_2 = $KEY_68;")
        .replace(
            "let $CA = $KEY_1;",
            "let $CA = $KEY_1;\n\t\t\t\t\t\tfunction $UNUSED_1() { return $BZ; }",
        )
}

/// A dead own local in the returned helper: `let $UNUSED_2 = $BZ;` is never
/// read, so the formal value never reaches the helper's state while the real
/// state comes from the captured `$KEY_68` global.
fn dead_local_formal_source() -> String {
    witness()
        .replace("$KEY_2 = $BZ;", "$KEY_2 = $KEY_68;")
        .replace(
            "let $CA = $KEY_1;",
            "let $CA = $KEY_1;\n\t\t\t\t\t\tlet $UNUSED_2 = $BZ;",
        )
}

/// The equivalent one-hop dead alias: the factory locally seeds `$UNUSED_3`
/// from `$BZ`, but the returned helper only copies it into an unread
/// `$UNUSED_4`, so the hop is dead while the state comes from `$KEY_68`.
fn dead_hop_alias_source() -> String {
    witness()
        .replace(
            "$KEY_2 = $BZ;",
            "$KEY_2 = $KEY_68;\n\t\t\t\t\tlet $UNUSED_3 = $BZ;",
        )
        .replace(
            "let $CA = $KEY_1;",
            "let $CA = $KEY_1;\n\t\t\t\t\t\tlet $UNUSED_4 = $UNUSED_3;",
        )
}

/// The accepted control-test counterexample: the helper state comes from the
/// captured `$KEY_68` global while the factory formal appears only in an empty
/// `if ($BZ) {}`. A bare control-flow test is not a state operation, so the
/// seed is ignored.
fn control_test_formal_source() -> String {
    witness()
        .replace("$KEY_2 = $BZ;", "$KEY_2 = $KEY_68;")
        .replace(
            "let $CA = $KEY_1;",
            "let $CA = $KEY_1;\n\t\t\t\t\t\tif ($BZ) {}",
        )
}

/// The equivalent one-hop control-test counterexample: the factory seeds
/// `$UNUSED_5` from `$BZ`, but the helper only tests it in `if ($UNUSED_5) {}`,
/// so the hop still does not feed state.
fn control_test_hop_source() -> String {
    witness()
        .replace(
            "$KEY_2 = $BZ;",
            "$KEY_2 = $KEY_68;\n\t\t\t\t\tlet $UNUSED_5 = $BZ;",
        )
        .replace(
            "let $CA = $KEY_1;",
            "let $CA = $KEY_1;\n\t\t\t\t\t\tif ($UNUSED_5) {}",
        )
}

/// The accepted discarded-call counterexample: the helper discards
/// `$KEY_10($BZ);` while the real state comes from `$KEY_68`. A call argument
/// that never feeds a byte/state operation does not consume the seed.
fn discarded_call_formal_source() -> String {
    witness()
        .replace("$KEY_2 = $BZ;", "$KEY_2 = $KEY_68;")
        .replace(
            "let $CA = $KEY_1;",
            "let $CA = $KEY_1;\n\t\t\t\t\t\t$KEY_10($BZ);",
        )
}

/// The seed is written from the formal and then overwritten by the captured
/// `$KEY_68` global before the helper is warmed, so the formal value never
/// reaches the helper's state.
fn overwritten_seed_source() -> String {
    witness().replace("$KEY_2 = $BZ;", "$KEY_2 = $BZ; $KEY_2 = $KEY_68;")
}

/// Remove the real tail write so a variant has exactly one `$KEY_10` store.
fn without_tail_write(source: String) -> String {
    source.replace("$KEY_10 = $KEY_25;", "")
}

/// The one `$KEY_10` write now precedes the branch that reaches the emitter's
/// call edge. The initializer is still the right factory, but the write is
/// reachable before the reference, so the control-flow-disjoint proof must not
/// accept it.
fn write_before_use_source() -> String {
    without_tail_write(witness()).replace(
        "let $BO = 1;",
        "let $KEY_25 = 0; let $BO = 1; $KEY_10 = $KEY_25;",
    )
}

/// The one later `$KEY_10` write overwrites the callable with a foreign
/// function before the call edge. Even though the initializer is the right
/// factory, a reachable overwrite must refuse the binding.
fn callable_overwrite_source() -> String {
    without_tail_write(witness()).replace("let $BO = 1;", "let $BO = 1; $KEY_10 = $THREAD_176;")
}

/// The reference branch no longer ends with an unconditional `return`, so the
/// later base36 tail write can be reached on the same invocation. The write is
/// not control-flow disjoint and must refuse the binding.
fn falling_branch_source() -> String {
    witness().replace("return $THREAD_6_48236;", "$THREAD_6_48236;")
}

/// The captured `$KEY_10` factory is declared once in the enclosing `outer`
/// scope and shared across two `run` invocations. The emitter branch and the
/// sole base36-tail write stay in `run`, so the branch and the write are
/// per-invocation disjoint, but `run(false)` mutates the shared binding before
/// `run(true)` creates the emitter. The declaration is therefore not freshly
/// bound in the invocation the evaluator claims, and the binding must be
/// refused because its owner is not the common function that owns the returning
/// branch and the write.
fn shared_outer_declaration_source() -> String {
    let mut out = String::from("function outer(){");
    out.push_str("let $KEY_3 = \"012345678901234567890123456789012345678901234567890123456789\";");
    out.push_str("let $KEY_8 = Date;");
    out.push_str("let $KEY_6 = 0;");
    out.push_str("let $KEY_9 = {next(){},complete(){}};");
    out.push_str("let $KEY_68 = 0, $KEY_187 = null, $KEY_202 = ArrayBuffer, $KEY_289 = [];");
    out.push_str("let $KEY_174, $KEY_176, $KEY_205;");
    out.push_str(STRING_WRITER_FACTORY);
    out.push_str(NUMBER_WRITER);
    out.push_str(FOREIGN_WRITER);
    out.push_str("$KEY_174 = $THREAD_1_96432;");
    out.push_str("$KEY_176 = $THREAD_176;");
    out.push_str("function $THREAD_205($c, $i) { return 0; }");
    out.push_str("$KEY_205 = $THREAD_205;");
    out.push_str("let $KEY_78 = $THREAD_9_47888;");
    out.push_str(KEYSTREAM_FACTORY);
    // The shared initializer lives in `outer`, not in the function whose branch
    // and tail write the evaluator proves disjoint.
    out.push_str("let $KEY_10 = $THREAD_9_35814;");
    out.push_str("function run($flag) {");
    out.push_str("let $BO = 1;");
    out.push_str("if (typeof $BO !== \"undefined\") {");
    out.push_str(WITNESS_BODY);
    out.push_str("return $THREAD_6_48236;");
    out.push_str("}");
    out.push_str("let $KEY_B36 = [];");
    out.push_str("let $KEY_24 = 0, $KEY_30 = 0;");
    out.push_str("for (;;) {");
    out.push_str("let $KEY_25 = 0;");
    out.push_str("let $KEY_2 = 0;");
    out.push_str("$KEY_B36.splice($KEY_25, 0, Math.floor($KEY_2));");
    out.push_str("++$KEY_25;");
    out.push_str("$KEY_24 = 1;");
    out.push_str("$KEY_30 = 36;");
    out.push_str("$KEY_10 = $KEY_25;");
    out.push_str("break;");
    out.push_str("}");
    out.push_str("return { init: $THREAD_6_48236 };");
    out.push_str("}");
    out.push_str("run(false); run(true);");
    out.push_str("}outer();");
    out
}

/// The initializer is a function-scoped `var` nested in an earlier conditional.
/// Its declaration is byte-earlier than the emitter branch and is owned by the
/// same `main`, but it may not have executed when the branch creates the
/// emitter (`$KEY_COND` false leaves the factory undefined), so byte order is
/// not dominance and the binding must be refused.
fn conditional_var_init_source() -> String {
    witness().replace(
        "let $KEY_10 = $THREAD_9_35814;",
        "let $KEY_COND = 0; if ($KEY_COND) { var $KEY_10 = $THREAD_9_35814; }",
    )
}

/// The initializer is a function-scoped `var` owned by an earlier loop body.
/// The loop may execute zero times or re-create the binding, so the covering
/// direct statement in `main` is the loop, not a `VariableDeclaration`, and the
/// declaration does not dominate the branch.
fn loop_owned_init_source() -> String {
    witness().replace(
        "let $KEY_10 = $THREAD_9_35814;",
        "while (0) { var $KEY_10 = $THREAD_9_35814; }",
    )
}

#[test]
fn recovered_inline_cursor_curried_emitter_witness_is_claimed_and_renamed() {
    assert_eq!(WITNESS_BODY.as_bytes().len(), 485);
    assert_eq!(sha256_hex(WITNESS_BODY), WITNESS_SHA256);
    assert_eq!(NUMBER_WRITER.as_bytes().len(), 3851);
    assert_eq!(sha256_hex(NUMBER_WRITER), NUMBER_WRITER_SHA256);
    assert_eq!(
        sha256_hex(STRING_WRITER_FACTORY),
        STRING_WRITER_FACTORY_SHA256
    );
    assert_eq!(sha256_hex(KEYSTREAM_FACTORY), KEYSTREAM_FACTORY_SHA256);
    let source = witness();
    // The positive retains the captured `$KEY_10` initializer, the emitter in the returning
    // branch, and the actual `$KEY_10 = $KEY_25` write in the alternate base36 tail. The sole
    // write cannot reach the call edge, so the stability guard is exercised with its real
    // control-flow relationship.
    assert!(source.contains("let $KEY_10 = $THREAD_9_35814;"));
    assert!(source.contains("if (typeof $BO !== \"undefined\")"));
    assert_eq!(source.matches("$KEY_10 = $KEY_25;").count(), 1);
    assert_eq!(claims(&source), 1);
    assert_eq!(applied(&source), 1);
}

#[test]
fn renamed_alpha_curried_emitter_is_claimed() {
    let source = alpha();
    assert_ne!(alpha_body(), WITNESS_BODY);
    assert_eq!(claims(&source), 1);
    assert_eq!(applied(&source), 1);
}

#[test]
fn known_numeric_base_proof_is_preserved() {
    assert!(role_named(
        BASE_FIXED,
        "$THREAD_4_4",
        NAME,
        "encoding_seeded_hash_encoding"
    ));
    assert_eq!(claims(BASE_FIXED), 0);
}

#[test]
fn cached_literal_cursor_sibling_stays_unclaimed() {
    let source = sibling();
    assert_eq!(claims(&source), 0);
}

#[test]
fn curried_emitter_operation_order_and_identity_negatives_are_refused() {
    let edits = [
        ("slice(0, 50)", "slice(0, 51)"),
        ("slice(0, 50)", "slice(1, 50)"),
        (
            "Math.random() * 4294967296 | 0",
            "Math.random() * 4294967295 | 0",
        ),
        ("$KEY_5 >>> 24", "$KEY_5 >>> 16"),
        ("$KEY_5 >> 16 & 255", "$KEY_5 >> 15 & 255"),
        ("$KEY_5 & 255", "$KEY_5 & 127"),
        ("$KEY_5 ^ 581381427", "$KEY_5 + 581381427"),
        ("$KEY_5 ^ 581381427", "$KEY_5 ^ $KEY_6"),
        ("$KEY_78($KEY_11)", "$KEY_78($KEY_5)"),
        (
            "$KEY_78($KEY_11)($KEY_2, $CL,",
            "$KEY_78($KEY_11)($CL, $KEY_2,",
        ),
        (
            "$KEY_174($KEY_11, $CK, $CL, 3)",
            "$KEY_174($KEY_11, $CK, $CL, 4)",
        ),
        (
            "$KEY_174($KEY_11, $CK, $CL, 3)",
            "$KEY_174($KEY_11, $CK, $CL, 3, 4)",
        ),
        (
            "$KEY_174($KEY_11, $CK, $CL, 3)",
            "$KEY_174($KEY_11, $CL, $CK, 3)",
        ),
        (
            "$KEY_174($KEY_11, $CK, $CL, 3)",
            "$KEY_176($KEY_11, $CK, $CL, 3)",
        ),
        ("$KEY_8.now() - $KEY_6", "$KEY_8.now() + $KEY_6"),
        ("$KEY_8.now() - $KEY_6", "$KEY_8.now()"),
        ("$KEY_8.now() - $KEY_6", "$KEY_8.now() - $KEY_8"),
        (
            "$KEY_174($KEY_11, $KEY_8.now() - $KEY_6, $CL, $KEY_2)",
            "$KEY_174($KEY_11, $KEY_8.now() - $KEY_6, $CL, 3)",
        ),
        (
            "$KEY_174($KEY_11, $KEY_8.now() - $KEY_6, $CL, $KEY_2)",
            "$KEY_176($KEY_11, $KEY_8.now() - $KEY_6, $CL, $KEY_2)",
        ),
        ("$KEY_9.next([[], $CL])", "$KEY_9.next([[], $KEY_2])"),
        ("$KEY_9.next([[], $CL])", "$KEY_9.next([[1], $CL])"),
        (
            "$KEY_9.next([[], $CL]);",
            "$KEY_2 = $KEY_2 + 1; $KEY_9.next([[], $CL]);",
        ),
        (
            "$KEY_9.next([[], $CL]);",
            "(() => 0)(); $KEY_9.next([[], $CL]);",
        ),
        ("$KEY_78($KEY_11)(", "$KEY_78.call(null, $KEY_11)("),
        ("$KEY_9.complete()", "$KEY_9.complete(1)"),
    ];
    for (from, to) in edits {
        let body = WITNESS_BODY.replace(from, to);
        assert_ne!(body, WITNESS_BODY, "edit did not apply: {from}");
        let source = carrier_with_actual_write(&body, "$THREAD_6_48236");
        assert_eq!(claims(&source), 0, "accepted {from} -> {to}");
    }
}

#[test]
fn source_proven_numeric_writer_and_owned_factory_negatives_are_refused() {
    // Both numeric call sites use the four-formal foreign writer at once.
    let both = both_sites_foreign_body();
    assert_ne!(both, WITNESS_BODY);
    assert_eq!(
        claims(&carrier_with_actual_write(&both, "$THREAD_6_48236")),
        0
    );

    // A factory returns a donor-owned helper and ignores its own formal.
    let borrowed = borrowed_factory_source();
    assert_ne!(borrowed, witness());
    assert_eq!(claims(&borrowed), 0);

    // The factory keeps a dead `$BZ;` mention but seeds the helper from a
    // captured global, so the formal is not actually used.
    let ignored = ignored_seed_source();
    assert_ne!(ignored, witness());
    assert_eq!(claims(&ignored), 0);
}

/// The factory formal only appears in a dead position inside the returned
/// helper (a nested unused function, an unread own local, or an unread one-hop
/// alias), while the helper's real state comes from the captured `$KEY_68`
/// global. None of these may count as a live seed capture.
#[test]
fn dead_and_nested_formal_references_are_not_live_seed_use() {
    let nested = nested_dead_formal_source();
    assert_ne!(nested, witness(), "nested counterexample did not apply");
    assert_eq!(claims(&nested), 0);

    let dead_local = dead_local_formal_source();
    assert_ne!(dead_local, witness(), "dead-local negative did not apply");
    assert_eq!(claims(&dead_local), 0);

    let dead_hop = dead_hop_alias_source();
    assert_ne!(dead_hop, witness(), "dead-hop alias negative did not apply");
    assert_eq!(claims(&dead_hop), 0);
}

/// The factory formal appears only in an empty control-flow test or a discarded
/// call inside the returned helper while the helper's real state comes from the
/// captured `$KEY_68` global. The earlier consumer classifier counted those
/// syntactic consumers as seed use; none of them reaches the helper's state or
/// byte operation, so all are refused. A seed overwritten by an unrelated
/// capture before the warm-up is refused too.
#[test]
fn control_flow_discarded_call_and_overwritten_seed_are_not_state_feed() {
    let control = control_test_formal_source();
    assert_ne!(
        control,
        witness(),
        "control-test counterexample did not apply"
    );
    assert_eq!(claims(&control), 0);

    let hop = control_test_hop_source();
    assert_ne!(
        hop,
        witness(),
        "control-test hop counterexample did not apply"
    );
    assert_eq!(claims(&hop), 0);

    let discarded = discarded_call_formal_source();
    assert_ne!(
        discarded,
        witness(),
        "discarded-call counterexample did not apply"
    );
    assert_eq!(claims(&discarded), 0);

    let overwritten = overwritten_seed_source();
    assert_ne!(
        overwritten,
        witness(),
        "overwritten-seed counterexample did not apply"
    );
    assert_eq!(claims(&overwritten), 0);
}

/// A captured callable sole later write is accepted only when control-flow disjoint from
/// the call edge. Writes before the reference, foreign callable overwrites, or fallthrough
/// to the tail write must refuse. These variants retain the initializer and base36 write
/// text so the control-flow guard is exercised directly.
#[test]
fn captured_callable_write_before_use_overwrite_and_fallthrough_are_refused() {
    let before = write_before_use_source();
    assert_ne!(before, witness(), "write-before-use variant did not apply");
    assert_eq!(before.matches("$KEY_10 = $KEY_25;").count(), 1);
    assert_eq!(claims(&before), 0);

    let overwrite = callable_overwrite_source();
    assert_ne!(
        overwrite,
        witness(),
        "callable-overwrite variant did not apply"
    );
    assert!(overwrite.contains("$KEY_10 = $THREAD_176;"));
    assert_eq!(overwrite.matches("$KEY_10 = $KEY_25;").count(), 0);
    assert_eq!(claims(&overwrite), 0);

    let fallthrough = falling_branch_source();
    assert_ne!(
        fallthrough,
        witness(),
        "falling-branch variant did not apply"
    );
    assert_eq!(claims(&fallthrough), 0);
}

/// The captured callable must be freshly bound in the same invocation as the
/// returning branch and the sole write. These valid programs declare the
/// initializer once but outside that function (shared across invocations), under
/// a conditional `var`, or in a loop that may be skipped or repeated. Each
/// mutation is applied, and the binding must be refused: only a declaration
/// owned by the common function whose covering direct statement is the earlier
/// `VariableDeclaration` proves that the initializer executes before the branch.
#[test]
fn captured_callable_declaration_scope_and_dominance_are_required() {
    let shared = shared_outer_declaration_source();
    assert_ne!(shared, witness(), "outer-declaration variant did not apply");
    assert!(shared.contains("function outer(){"));
    assert!(shared.contains("let $KEY_10 = $THREAD_9_35814;"));
    assert_eq!(shared.matches("$KEY_10 = $KEY_25;").count(), 1);
    assert_eq!(claims(&shared), 0);

    let conditional = conditional_var_init_source();
    assert_ne!(
        conditional,
        witness(),
        "conditional-var variant did not apply"
    );
    assert!(conditional.contains("var $KEY_10 = $THREAD_9_35814;"));
    assert_eq!(conditional.matches("$KEY_10 = $KEY_25;").count(), 1);
    assert_eq!(claims(&conditional), 0);

    let looped = loop_owned_init_source();
    assert_ne!(looped, witness(), "loop-owned variant did not apply");
    assert!(looped.contains("while (0) { var $KEY_10 = $THREAD_9_35814; }"));
    assert_eq!(looped.matches("$KEY_10 = $KEY_25;").count(), 1);
    assert_eq!(claims(&looped), 0);
}

// Keep syntax and target-presence checks separate from semantic refusal.
fn iso_curried_refused(source: &str) {
    let allocator = oxc_allocator::Allocator::default();
    let parsed =
        oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::unambiguous()).parse();
    assert!(
        !parsed.panicked && parsed.errors.is_empty(),
        "{:?}",
        parsed.errors
    );
    let result = name_threads(source, true).expect("ISO mutant must analyze");
    assert!(result
        .report
        .outcomes
        .iter()
        .any(|o| o.binding.name == "$THREAD_6_48236"));
    assert!(
        !result.report.function_roles.iter().any(|r| {
            r.evaluators.iter().any(|e| e == EVALUATOR) && r.candidates.iter().any(|n| n == NAME)
        }),
        "unexpected family evidence"
    );
    assert!(!result.report.outcomes.iter().any(|o| {
        o.binding.name == "$THREAD_6_48236" && o.applied && o.proposed_name.as_deref() == Some(NAME)
    }));
}

#[test]
fn factory_exit_curried_factory_alternate_exits_are_refused() {
    // The first row is the exact reported mutant. Both independently used
    // factory paths retain their real child and final return.
    for returned in ["$THREAD_2_3512", "$THREAD_7_28359"] {
        let original = witness();
        let anchor = format!("return {returned};");
        assert_eq!(original.matches(&anchor).count(), 1);
        for exit in [
            "if (true) return null;",
            "if ($BZ) return function () { return 0; };",
            "{ if ($BZ) { return void 0; } }",
        ] {
            let source = original.replace(&anchor, &format!("{exit} {anchor}"));
            iso_curried_refused(&source);
        }
    }
}

#[test]
fn factory_exit_curried_child_returns_and_outer_exclusive_rebinding_remain_supported() {
    let source = witness();
    let allocator = oxc_allocator::Allocator::default();
    let parsed =
        oxc_parser::Parser::new(&allocator, &source, oxc_span::SourceType::unambiguous()).parse();
    assert!(
        !parsed.panicked && parsed.errors.is_empty(),
        "{:?}",
        parsed.errors
    );
    assert!(role_named(&source, "$THREAD_6_48236", NAME, EVALUATOR));
    // An uncalled child with an unrelated return is not an exit of its parent.
    let source = source.replace(
        "return $THREAD_2_3512;",
        "function isoNestedExit() { return null; } return $THREAD_2_3512;",
    );
    assert!(role_named(&source, "$THREAD_6_48236", NAME, EVALUATOR));
}
