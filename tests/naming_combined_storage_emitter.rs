//! Split flag aliases preserve exact lane provenance through both frame paths.
use shape::naming::name_threads;
const NAME: &str = "emitStorageRemovalSignal";
const BODY_DIRECT_CURSOR: &str = r#"function $THREAD_2_125597($CB) {
				try {
					if ($CB.key === null || $CB.key === $KEY_14 && $CB.newValue === null) {
						$KEY_28 = true;
						$KEY_16 = $CB.oldValue ?? null;
						let $CC = Math.random() * 4294967296;
						let $CD = [($CC | 0) >>> 24, ($CC | 0) >> 16 & 255, ($CC | 0) >> 8 & 255, ($CC | 0) & 255];
						$CC = $KEY_310(($CC | 0) ^ 636298419);
						let $CE = $KEY_145($CC);
						let $CF = $KEY_16;
						if ($CF == null) {
							$CD[4] = $CC() ^ 130;
							$CE = 4;
						} else {
							$CE = $CE($CF, $CD, 3);
						}
						$CD[$CE + 1] = ($KEY_9 | 0 | $KEY_28 << 1 | $KEY_20 << 2) ^ $CC();
						$KEY_17.next([[], $CD]);
					}
				} catch (e) {
					$KEY_20 = true;
					let $CG = Math.random() * 4294967296;
					let $CH = [($CG | 0) >>> 24, ($CG | 0) >> 16 & 255, ($CG | 0) >> 8 & 255, ($CG | 0) & 255];
					$CG = $KEY_310(($CG | 0) ^ 636298419);
					let $CI = $KEY_145($CG);
					let $CJ = $KEY_16;
					if ($CJ == null) {
						$CH[4] = $CG() ^ 130;
						$CI = 4;
					} else {
						$CI = $CI($CJ, $CH, 3);
					}
					$CH[$CI + 1] = ($KEY_9 | 0 | $KEY_28 << 1 | $KEY_20 << 2) ^ $CG();
					$KEY_17.next([[], $CH]);
				}
			}"#;
const DIRECT_CURSOR_PREFIX: &str = r#"function main(){
let $KEY_14 = 0;
let $KEY_28 = false;
let $KEY_16 = null;
let $KEY_20 = false;
let $KEY_9 = false;
let $KEY_17 = { next(value){ return value; } };
let $KEY_284 = 0;
function $KEY_207($k, $o, $b, $p) { return $p; }
function $KEY_166($c, $i) { return 0; }
function $THREAD_9_7577($BR) {
    let $KEY_2, $KEY_3, $KEY_4, $KEY_5;
    function $THREAD_9_44384() {
        let $BS = $KEY_2;
        $KEY_2 = ($BS + 1) % 4;
        if (($BS + 1) % 4 === 0) {
            $KEY_5 = $KEY_3 + $KEY_4;
            let $BT = $KEY_4;
            $KEY_4 = $BT ^ $KEY_3;
            $KEY_3 = $BT ^ $KEY_3 ^ ($KEY_3 << 26 | $KEY_3 >>> 6) ^ $KEY_4 << 9;
            $KEY_4 = $KEY_4 << 13 | $KEY_4 >>> 19;
            return $KEY_5 & 255;
        }
        if (($BS + 1) % 4 === 1) {
            return $KEY_5 >> 8 & 255;
        }
        if (($BS + 1) % 4 === 2) {
            return $KEY_5 >> 16 & 255;
        }
        if (($BS + 1) % 4 === 3) {
            return $KEY_5 >>> 24 & 255;
        } else {
            return void 0;
        }
    }
    $KEY_3 = $KEY_284;
    $KEY_4 = $BR;
    $KEY_2 = -1;
    $THREAD_9_44384();
    $THREAD_9_44384();
    $THREAD_9_44384();
    $THREAD_9_44384();
    return $THREAD_9_44384;
}
function $THREAD_1_58232($BR) {
    let $KEY_0 = $BR;
    let $KEY_1, $KEY_2;
    $KEY_1 = Object.create(null);
    $KEY_2 = 0;
    function $THREAD_8_14209($BS, $BT, $BU) {
        let $KEY_11;
        if ($KEY_1[$BS] != null) {
            $KEY_11 = $BU;
            $KEY_11 = Number($BU) + 1;
            $BT[$KEY_11] = $KEY_0() ^ 129;
            $KEY_11 = $KEY_207($KEY_0, $KEY_1[$BS], $BT, $KEY_11);
        } else {
            let $BV = $BS.length;
            let $BW = 0;
            let $BX = $BU;
            while ($BW < $BV) {
                let $BY = $KEY_166($BS[$BW], 0);
                if ($BY === 0) {
                    $BT[++$BX] = $KEY_0() ^ 192;
                    $BY = 128;
                } else if (!($BY < 128)) if ($BY < 2048) {
                    $BT[++$BX] = ($BY >> 6 | 192) ^ $KEY_0();
                    $BY = $BY & 63 | 128;
                } else {
                    const $BZ = $BY >= 55296 && $BY <= 57343 && $BY < 56320 && $BW + 1 < $BV ? $KEY_166($BS[$BW + 1], 0) : null;
                    if ($BZ == null || $BZ < 56320 || $BZ > 57343) {
                        $BT[++$BX] = ($BY >> 12 | 224) ^ $KEY_0();
                        $BT[++$BX] = ($BY >> 6 & 63 | 128) ^ $KEY_0();
                        $BY = $BY & 63 | 128;
                    } else {
                        ++$BW;
                        $BT[++$BX] = (65536 + ($BY - 55296 << 10) + ($BZ - 56320) >> 18 | 240) ^ $KEY_0();
                        $BT[++$BX] = (65536 + ($BY - 55296 << 10) + ($BZ - 56320) >> 12 & 63 | 128) ^ $KEY_0();
                        $BT[++$BX] = (65536 + ($BY - 55296 << 10) + ($BZ - 56320) >> 6 & 63 | 128) ^ $KEY_0();
                        $BY = 65536 + ($BY - 55296 << 10) + ($BZ - 56320) & 63 | 128;
                    }
                }
                $BT[++$BX] = $BY ^ $KEY_0();
                ++$BW;
            }
            $KEY_11 = Number($BX) + 1;
            $BT[$KEY_11] = $KEY_0();
            let $CA = $KEY_1;
            let $CB = Number($KEY_2);
            $KEY_2 = $CB + 1;
            $CA[$BS] = $CB;
        }
        return $KEY_11;
    }
    return $THREAD_8_14209;
}
let $KEY_310 = $THREAD_9_7577;
let $KEY_145 = $THREAD_1_58232;
"#;
const DIRECT_CURSOR_SUFFIX: &str = r#"
let $KEY_5 = $THREAD_2_125597;
let $KEY_8 = { global: { addEventListener(){ return void 0; } } };
$KEY_8.global.addEventListener("storage", $KEY_5);
}
main();"#;
fn checked(source: &str, target: &str, accepted: bool) {
    let allocator = oxc_allocator::Allocator::default();
    let parsed =
        oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::unambiguous()).parse();
    assert!(
        !parsed.panicked && parsed.errors.is_empty(),
        "{:?}",
        parsed.errors
    );
    let result = name_threads(source, true).expect("ISO fixture must analyze");
    assert!(
        result
            .report
            .outcomes
            .iter()
            .any(|o| o.binding.name == target),
        "missing {target}"
    );
    let claimed = result.report.function_roles.iter().any(|r| {
        r.current_name.as_deref() == Some(target)
            && r.evaluators
                .iter()
                .any(|e| e == "thread_posthost_storage_removal_emitter")
            && r.candidates.iter().any(|n| n == NAME)
    });
    assert_eq!(claimed, accepted, "family proof for {target}\n{source}");
    assert_eq!(
        result.report.outcomes.iter().any(|o| {
            o.binding.name == target && o.applied && o.proposed_name.as_deref() == Some(NAME)
        }),
        accepted,
        "applied role for {target}"
    );
    assert_eq!(
        result.report.outcomes.iter().any(|o| {
            o.binding.name == target
                && o.evaluator.as_deref() == Some("thread_posthost_storage_removal_emitter")
        }),
        accepted,
        "target evaluator for {target}"
    );
}
fn positive(layout: u8) -> String {
    let (zero, one, two) = match layout {
        0 => ("$KEY_9", "$KEY_28", "$KEY_20"),
        1 => ("$KEY_9", "$KEY_20", "$KEY_28"),
        2 => ("$KEY_20", "$KEY_28", "$KEY_9"),
        _ => unreachable!(),
    };
    let split = BODY_DIRECT_CURSOR.find("} catch (e)").unwrap();
    let success = BODY_DIRECT_CURSOR[..split]
        .replace(
            "let $CF = $KEY_16;",
            &format!("let firstFlags = {zero} | 0 | {one} << 1; let $CF = $KEY_16;"),
        )
        .replace(
            "$KEY_9 | 0 | $KEY_28 << 1 | $KEY_20 << 2",
            &format!("firstFlags | {two} << 2"),
        );
    let catch = BODY_DIRECT_CURSOR[split..]
        .replace(
            "let $CJ = $KEY_16;",
            &format!("let secondFlags = {zero} | 0 | {one} << 1; let $CJ = $KEY_16;"),
        )
        .replace(
            "$KEY_9 | 0 | $KEY_28 << 1 | $KEY_20 << 2",
            &format!("secondFlags | {two} << 2"),
        );
    format!("{DIRECT_CURSOR_PREFIX}{success}{catch}{DIRECT_CURSOR_SUFFIX}")
}
fn reject(source: &str, from: &str, to: &str) {
    assert!(source.contains(from), "missing mutation anchor {from}");
    let changed = source.replacen(from, to, 1);
    checked(&changed, "$THREAD_2_125597", false);
}
#[test]
fn combined_flag_variants_and_owned_dataflow_near_misses() {
    for layout in 0..=2 {
        let source = positive(layout);
        checked(&source, "$THREAD_2_125597", true);
        checked(
            &source
                .replace("firstFlags", "readableFlags")
                .replace("secondFlags", "otherFlags")
                .replace("636298419", "888"),
            "$THREAD_2_125597",
            true,
        );
        reject(&source, "firstFlags |", "$KEY_9 |");
        reject(
            &source,
            "let $CF = $KEY_16;",
            "let $CF = $KEY_16; firstFlags = 0;",
        );
        reject(&source, "<< 1;", "<< 3;");
        reject(
            &source,
            "$KEY_17.next([[], $CD]);",
            "$KEY_17.next([[], $CH]);",
        );
        reject(&source, "$KEY_28 = true;", "$KEY_9 = true;");
        reject(&source, "    $KEY_4 = $BR;", "    $KEY_4 = $KEY_284;");
        reject(
            &source,
            "firstFlags |",
            "(() => { let firstFlags = 0; return firstFlags; })() |",
        );
    }
}
