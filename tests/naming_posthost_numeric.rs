//! Focused, portable coverage for the signal-codec number-to-XOR-byte-buffer encoder
//! writeNumber.
//!
//! Exact bodies cover direct, aliased, split-declaration, and base-plus-offset spellings in
//! minimal carriers. Negatives change operations, receivers, shadows, or nested callbacks
//! and must stay unclaimed. Full-input static coverage is separate.
use shape::naming::name_threads;

const EVALUATOR: &str = "thread_posthost_numeric";
const NAME: &str = "writeNumber";

const WITNESS_BODY: &str = r#"function $THREAD_6_6494($BW, $BX, $BY, $BZ) {
			let $KEY_2;
			if (($BX | 0) === $BX) {
				let $CA;
				if ($BX < 0) {
					$CA = $BX < 0;
				} else if ($BX === 0) {
					$CA = 1 / $BX < 0;
				} else {
					$CA = false;
				}
				let $CB, $CC;
				if ($CA) {
					$CB = 64;
					$CC = -$BX;
				} else {
					$CB = 0;
					$CC = $BX;
				}
				if ($CC < 32) {
					$KEY_2 = Number($BZ) + 1;
					$BY[$KEY_2] = ($CB | $CC) ^ $BW();
				} else if ($CC < 4096) {
					$KEY_2 = Number($BZ) + 1;
					$BY[$KEY_2] = ($CB | ($CC & 31 | 32)) ^ $BW();
					$BY[++$KEY_2] = $CC >> 5 ^ $BW();
				} else if ($CC < 524288) {
					$KEY_2 = Number($BZ) + 1;
					$BY[$KEY_2] = ($CB | ($CC & 31 | 32)) ^ $BW();
					$BY[++$KEY_2] = ($CC >> 5 & 127 | 128) ^ $BW();
					$BY[++$KEY_2] = $CC >>> 12 ^ $BW();
				} else if ($CC < 67108864) {
					$KEY_2 = Number($BZ) + 1;
					$BY[$KEY_2] = ($CB | ($CC & 31 | 32)) ^ $BW();
					$BY[++$KEY_2] = ($CC >> 5 & 127 | 128) ^ $BW();
					$BY[++$KEY_2] = ($CC >>> 12 & 127 | 128) ^ $BW();
					$BY[++$KEY_2] = $CC >>> 19 ^ $BW();
				} else {
					$KEY_2 = Number($BZ) + 1;
					$BY[$KEY_2] = ($CB | ($CC & 31 | 32)) ^ $BW();
					$BY[++$KEY_2] = ($CC >> 5 & 127 | 128) ^ $BW();
					$BY[++$KEY_2] = ($CC >>> 12 & 127 | 128) ^ $BW();
					$BY[++$KEY_2] = ($CC >>> 19 & 127 | 128) ^ $BW();
					$BY[++$KEY_2] = $CC >>> 26 ^ $BW();
				}
			} else {
				$KEY_2 = $BZ;
				$KEY_2 = Number($BZ) + 1;
				$BY[$KEY_2] = $BW() ^ 128;
				let $CD;
				if ($KEY_23) {
					$KEY_23[0] = $BX;
					$CD = new $KEY_223($KEY_23.buffer);
				} else if ($BX !== $BX) {
					$CD = $KEY_311;
				} else {
					let $KEY_7 = $BX < 0 || $BX === 0 && 1 / $BX < 0;
					let $CE = Math.abs($BX);
					let $CF, $CG;
					if ($CE >= Math.pow(2, -1022)) {
						let $KEY_3 = Math.min(Math.floor(Math.log($CE) / Math.LN2), 1023);
						$CE = $CE / Math.pow(2, $KEY_3);
						let $CH = $CE < 1;
						$CE = $CH ? $CE * 2 : $CE;
						$CH = $CH ? $KEY_3 - 1 : $KEY_3;
						if ($CE >= 2) {
							++$CH;
							$CE = $CE / 2;
						}
						let $KEY_5 = Math.pow(2, 52);
						let $KEY_6 = Math.floor($CE * $KEY_5);
						$KEY_6 = $CE * $KEY_5 - $KEY_6 < .5 ? $KEY_6 : $CE * $KEY_5 - $KEY_6 > .5 ? $KEY_6 + 1 : $KEY_6 % 2 ? $KEY_6 + 1 : $KEY_6;
						$CH += 1023;
						let $CI = ($KEY_6 - $KEY_5) / $KEY_5 >= 1;
						$CG = $CI ? $CH + 1 : $CH;
						if ($CG > 2046) {
							$CF = 0;
							$CG = 2047;
						} else {
							$CF = $CI ? 0 : $KEY_6 - $KEY_5;
						}
					} else {
						$CE = $CE / Math.pow(2, -1074);
						let $CJ = Math.floor($CE);
						$CF = $CE - $CJ < .5 ? $CJ : $CE - $CJ > .5 ? $CJ + 1 : $CJ % 2 ? $CJ + 1 : $CJ;
						$CG = 0;
					}
					let $CK = [];
					let k = 51;
					for (; k >= 0; k--) {
						$CK.unshift(!!($CF & 1));
						$CF = Math.floor($CF / 2);
					}
					let i = 11;
					for (; i > 0; i--) {
						$CK.unshift(!!($CG & 1));
						$CG = Math.floor($CG / 2);
					}
					$CK.unshift($KEY_7);
					$CD = [];
					let j = 56;
					for (; j >= 0; j = j - 8) {
						$CD.push(Number($CK[j]) << 7 | Number($CK[j + 1]) << 6 | Number($CK[j + 2]) << 5 | Number($CK[j + 3]) << 4 | Number($CK[j + 4]) << 3 | Number($CK[j + 5]) << 2 | Number($CK[j + 6]) << 1 | Number($CK[j + 7]));
					}
				}
				let i_2 = 0;
				for (; i_2 < 8; i_2++) {
					$BY[++$KEY_2] = $CD[i_2] ^ $BW();
				}
			}
			return $KEY_2;
		}"#;

const ALIASED_POSITION_BODY: &str = r#"function $THREAD_5_111700($BQ, $BR, $BS, $BT) {
					let $BU;
					if (($BR | 0) === $BR) {
						let $BV;
						if ($BR < 0) {
							$BV = $BR < 0;
						} else if ($BR === 0) {
							$BV = 1 / $BR < 0;
						} else {
							$BV = false;
						}
						let $BW, $BX;
						if ($BV) {
							$BW = 64;
							$BX = -$BR;
						} else {
							$BW = 0;
							$BX = $BR;
						}
						if ($BX < 32) {
							$BU = Number($BT) + 1;
							$BS[$BU] = ($BW | $BX) ^ $BQ();
						} else if ($BX < 4096) {
							let $BY = Number($BT) + 1;
							$BS[$BY] = ($BW | ($BX & 31 | 32)) ^ $BQ();
							$BU = $BY + 1;
							$BS[$BU] = $BX >> 5 ^ $BQ();
						} else if ($BX < 524288) {
							let $BZ = Number($BT) + 1;
							$BS[$BZ] = ($BW | ($BX & 31 | 32)) ^ $BQ();
							$BS[++$BZ] = ($BX >> 5 & 127 | 128) ^ $BQ();
							$BU = $BZ + 1;
							$BS[$BU] = $BX >>> 12 ^ $BQ();
						} else if ($BX < 67108864) {
							let $CA = Number($BT) + 1;
							$BS[$CA] = ($BW | ($BX & 31 | 32)) ^ $BQ();
							$BS[++$CA] = ($BX >> 5 & 127 | 128) ^ $BQ();
							$BS[++$CA] = ($BX >>> 12 & 127 | 128) ^ $BQ();
							$BU = $CA + 1;
							$BS[$BU] = $BX >>> 19 ^ $BQ();
						} else {
							let $CB = Number($BT) + 1;
							$BS[$CB] = ($BW | ($BX & 31 | 32)) ^ $BQ();
							$BS[++$CB] = ($BX >> 5 & 127 | 128) ^ $BQ();
							$BS[++$CB] = ($BX >>> 12 & 127 | 128) ^ $BQ();
							$BS[++$CB] = ($BX >>> 19 & 127 | 128) ^ $BQ();
							$BU = $CB + 1;
							$BS[$BU] = $BX >>> 26 ^ $BQ();
						}
					} else {
						$BU = Number($BT) + 1;
						$BS[$BU] = $BQ() ^ 128;
						let $CC, $CD;
						if ($KEY_377) {
							$KEY_377[0] = $BR;
							$CC = new $KEY_173($KEY_377.buffer);
							$CD = $BS;
						} else if ($BR !== $BR) {
							$CC = $KEY_90;
							$CD = $BS;
						} else {
							let $KEY_7 = $BR < 0 || $BR === 0 && 1 / $BR < 0;
							$CD = $BS;
							let $CE = Math.abs($BR);
							let $CF, $CG;
							if ($CE >= Math.pow(2, -1022)) {
								let $KEY_8 = Math.min(Math.floor(Math.log($CE) / Math.LN2), 1023);
								$CE = $CE / Math.pow(2, $KEY_8);
								let $CH = $CE < 1;
								$CE = $CH ? $CE * 2 : $CE;
								$CH = $CH ? $KEY_8 - 1 : $KEY_8;
								if ($CE >= 2) {
									++$CH;
									$CE = $CE / 2;
								}
								let $KEY_4 = Math.pow(2, 52);
								let $CI = Math.floor($CE * $KEY_4);
								$CE = $CE * $KEY_4;
								$CH += 1023;
								let $CJ = (($CE - $CI < .5 ? $CI : $CE - $CI > .5 ? $CI + 1 : $CI % 2 ? $CI + 1 : $CI) - $KEY_4) / $KEY_4 >= 1;
								$CG = $CJ ? $CH + 1 : $CH;
								if ($CG > 2046) {
									$CF = 0;
									$CG = 2047;
								} else {
									$CF = $CJ ? 0 : ($CE - $CI < .5 ? $CI : $CE - $CI > .5 ? $CI + 1 : $CI % 2 ? $CI + 1 : $CI) - $KEY_4;
								}
							} else {
								$CE = $CE / Math.pow(2, -1074);
								let $CK = Math.floor($CE);
								$CF = $CE - $CK < .5 ? $CK : $CE - $CK > .5 ? $CK + 1 : $CK % 2 ? $CK + 1 : $CK;
								$CG = 0;
							}
							let $CL = [];
							let k = 51;
							for (; k >= 0; k--) {
								$CL.unshift(!!($CF & 1));
								$CF = Math.floor($CF / 2);
							}
							let i = 11;
							for (; i > 0; i--) {
								$CL.unshift(!!($CG & 1));
								$CG = Math.floor($CG / 2);
							}
							$CL.unshift($KEY_7);
							let $CM = [];
							let j = 56;
							let $CN, $CO;
							for (;; j = $CO - 8) {
								$CN = $CM;
								$CO = j;
								if (!(j >= 0)) break;
								$CN.push(Number($CL[$CO]) << 7 | Number($CL[$CO + 1]) << 6 | Number($CL[$CO + 2]) << 5 | Number($CL[$CO + 3]) << 4 | Number($CL[$CO + 4]) << 3 | Number($CL[$CO + 5]) << 2 | Number($CL[$CO + 6]) << 1 | Number($CL[$CO + 7]));
							}
							$CC = $CN;
						}
						let i_2 = 0;
						for (; i_2 < 8; i_2++) {
							$CD[++$BU] = $CC[i_2] ^ $BQ();
						}
					}
					return $BU;
				}"#;
const SPLIT_POSITION_BODY: &str = r#"function $THREAD_7_102903($BH, $BI, $BJ, $BK) {
					let $BL;
					if (($BI | 0) === $BI) {
						let $BM;
						if ($BI < 0) {
							$BM = $BI < 0;
						} else if ($BI === 0) {
							$BM = 1 / $BI < 0;
						} else {
							$BM = $BI === 0;
						}
						let $BN, $BO;
						if ($BM) {
							$BN = -$BI;
							$BO = 64;
						} else {
							$BN = $BI;
							$BO = 0;
						}
						if ($BN < 32) {
							$BL = Number($BK) + 1;
							$BJ[$BL] = ($BO | $BN) ^ $BH();
						} else if ($BN < 4096) {
							let $BP = Number($BK) + 1;
							$BJ[$BP] = ($BO | ($BN & 31 | 32)) ^ $BH();
							$BL = $BP + 1;
							$BJ[$BL] = $BN >> 5 ^ $BH();
						} else if ($BN < 524288) {
							let $BQ = Number($BK) + 1;
							$BJ[$BQ] = ($BO | ($BN & 31 | 32)) ^ $BH();
							$BJ[++$BQ] = ($BN >> 5 & 127 | 128) ^ $BH();
							$BL = $BQ + 1;
							$BJ[$BL] = $BN >>> 12 ^ $BH();
						} else if ($BN < 67108864) {
							let $BR = Number($BK) + 1;
							$BJ[$BR] = ($BO | ($BN & 31 | 32)) ^ $BH();
							$BJ[++$BR] = ($BN >> 5 & 127 | 128) ^ $BH();
							$BJ[++$BR] = ($BN >>> 12 & 127 | 128) ^ $BH();
							$BL = $BR + 1;
							$BJ[$BL] = $BN >>> 19 ^ $BH();
						} else {
							let $BS = Number($BK) + 1;
							$BJ[$BS] = ($BO | ($BN & 31 | 32)) ^ $BH();
							$BJ[++$BS] = ($BN >> 5 & 127 | 128) ^ $BH();
							$BJ[++$BS] = ($BN >>> 12 & 127 | 128) ^ $BH();
							$BJ[++$BS] = ($BN >>> 19 & 127 | 128) ^ $BH();
							$BL = $BS + 1;
							$BJ[$BL] = $BN >>> 26 ^ $BH();
						}
					} else {
						$BL = Number($BK) + 1;
						$BJ[$BL] = $BH() ^ 128;
						let $BT;
						if ($KEY_62) {
							$KEY_62[0] = $BI;
							$BT = new $KEY_3($KEY_62.buffer);
						} else if ($BI !== $BI) {
							$BT = $KEY_424;
						} else {
							let $KEY_5 = $BI < 0 || $BI === 0 && 1 / $BI < 0;
							let $BU = Math.abs($BI);
							let $BV, $KEY_2;
							if ($BU >= Math.pow(2, -1022)) {
								$BV = Math.min(Math.floor(Math.log($BU) / Math.LN2), 1023);
								$BU = $BU / Math.pow(2, $BV);
								let $BW = $BU < 1;
								$BV = $BW ? $BV - 1 : $BV;
								$BW = $BW ? $BU * 2 : $BU;
								let $BX = $BW >= 2;
								$BV = $BX ? $BV + 1 : $BV;
								$KEY_2 = Math.pow(2, 52);
								let $BY = Math.floor(($BX ? $BW / 2 : $BW) * $KEY_2);
								let $BZ = ((($BX ? $BW / 2 : $BW) * $KEY_2 - $BY < .5 ? $BY : ($BX ? $BW / 2 : $BW) * $KEY_2 - $BY > .5 ? $BY + 1 : $BY % 2 ? $BY + 1 : $BY) - $KEY_2) / $KEY_2 >= 1;
								$BV = $BZ ? $BV + 1024 : $BV + 1023;
								if ($BV > 2046) {
									$BV = 2047;
									$KEY_2 = 0;
								} else {
									$KEY_2 = $BZ ? 0 : (($BX ? $BW / 2 : $BW) * $KEY_2 - $BY < .5 ? $BY : ($BX ? $BW / 2 : $BW) * $KEY_2 - $BY > .5 ? $BY + 1 : $BY % 2 ? $BY + 1 : $BY) - $KEY_2;
								}
							} else {
								$BU = $BU / Math.pow(2, -1074);
								let $CA = Math.floor($BU);
								$BV = 0;
								$KEY_2 = $BU - $CA < .5 ? $CA : $BU - $CA > .5 ? $CA + 1 : $CA % 2 ? $CA + 1 : $CA;
							}
							let $CB = [];
							let k = 51;
							for (; k >= 0; k--) {
								$CB.unshift(!!($KEY_2 & 1));
								$KEY_2 = Math.floor($KEY_2 / 2);
							}
							let i = 11;
							for (; i > 0; i--) {
								$CB.unshift(!!($BV & 1));
								$BV = Math.floor($BV / 2);
							}
							$CB.unshift($KEY_5);
							$BT = [];
							let j = 56;
							for (; j >= 0; j = j - 8) {
								$BT.push(Number($CB[j]) << 7 | Number($CB[j + 1]) << 6 | Number($CB[j + 2]) << 5 | Number($CB[j + 3]) << 4 | Number($CB[j + 4]) << 3 | Number($CB[j + 5]) << 2 | Number($CB[j + 6]) << 1 | Number($CB[j + 7]));
							}
						}
						let $CC = $BJ;
						let i_2 = 0;
						for (; i_2 < 8; i_2++) {
							$CC[++$BL] = $BT[i_2] ^ $BH();
						}
					}
					return $BL;
				}"#;
const BASE_PLUS_BODY: &str = r#"function $THREAD_0_111797($E, $F, $G, $H) {
		let $KEY_7;
		if (($F | 0) === $F) {
			let $I;
			if ($F < 0) {
				$I = $F < 0;
			} else if ($F === 0) {
				$I = 1 / $F < 0;
			} else {
				$I = false;
			}
			let $J, $K;
			if ($I) {
				$J = -$F;
				$K = 64;
			} else {
				$J = $F;
				$K = 0;
			}
			if ($J < 32) {
				$KEY_7 = Number($H) + 1;
				$G[$KEY_7] = ($K | $J) ^ $E();
			} else if ($J < 4096) {
				let $L = Number($H) + 1;
				$G[$L] = ($K | ($J & 31 | 32)) ^ $E();
				$G[$L + 1] = $J >> 5 ^ $E();
				$KEY_7 = $L + 1;
			} else if ($J < 524288) {
				let $M = Number($H) + 1;
				$G[$M] = ($K | ($J & 31 | 32)) ^ $E();
				$G[$M + 1] = ($J >> 5 & 127 | 128) ^ $E();
				$G[$M + 2] = $J >>> 12 ^ $E();
				$KEY_7 = $M + 2;
			} else if ($J < 67108864) {
				let $N = Number($H) + 1;
				$G[$N] = ($K | ($J & 31 | 32)) ^ $E();
				$KEY_7 = $N + 1;
				$G[$KEY_7] = ($J >> 5 & 127 | 128) ^ $E();
				$G[++$KEY_7] = ($J >>> 12 & 127 | 128) ^ $E();
				$G[++$KEY_7] = $J >>> 19 ^ $E();
			} else {
				let $O = Number($H) + 1;
				$G[$O] = ($K | ($J & 31 | 32)) ^ $E();
				$G[$O + 1] = ($J >> 5 & 127 | 128) ^ $E();
				$G[$O + 2] = ($J >>> 12 & 127 | 128) ^ $E();
				$G[$O + 3] = ($J >>> 19 & 127 | 128) ^ $E();
				$G[$O + 4] = $J >>> 26 ^ $E();
				$KEY_7 = $O + 4;
			}
		} else {
			$KEY_7 = Number($H) + 1;
			$G[$KEY_7] = $E() ^ 128;
			let $P;
			if ($KEY_12) {
				$KEY_12[0] = $F;
				$P = new $KEY_6($KEY_12.buffer);
			} else if ($F !== $F) {
				$P = $KEY_4;
			} else {
				const $Q = $F < 0 || $F === 0 && 1 / $F < 0;
				let $R = Math.abs($F);
				let $S, $T;
				if ($R >= Math.pow(2, -1022)) {
					let $KEY_3 = Math.min(Math.floor(Math.log($R) / Math.LN2), 1023);
					$R = $R / Math.pow(2, $KEY_3);
					let $U = $R < 1;
					$KEY_3 = $U ? $KEY_3 - 1 : $KEY_3;
					$U = $U ? $R * 2 : $R;
					let $V = $U >= 2;
					$U = $V ? $U / 2 : $U;
					let $KEY_14 = Math.pow(2, 52);
					let $W = Math.floor($U * $KEY_14);
					$U = $U * $KEY_14 - $W < .5 ? $W : $U * $KEY_14 - $W > .5 ? $W + 1 : $W % 2 ? $W + 1 : $W;
					$V = ($V ? $KEY_3 + 1 : $KEY_3) + 1023;
					let $X = ($U - $KEY_14) / $KEY_14 >= 1;
					$T = $X ? $V + 1 : $V;
					if ($T > 2046) {
						$S = 0;
						$T = 2047;
					} else {
						$S = $X ? 0 : $U - $KEY_14;
					}
				} else {
					$R = $R / Math.pow(2, -1074);
					let $Y = Math.floor($R);
					$S = $R - $Y < .5 ? $Y : $R - $Y > .5 ? $Y + 1 : $Y % 2 ? $Y + 1 : $Y;
					$T = 0;
				}
				let $Z = [];
				let k = 51;
				for (; k >= 0; k--) {
					$Z.unshift(!!($S & 1));
					$S = Math.floor($S / 2);
				}
				let i = 11;
				for (; i > 0; i--) {
					$Z.unshift(!!($T & 1));
					$T = Math.floor($T / 2);
				}
				$Z.unshift($Q);
				$P = [];
				let j = 56;
				for (; j >= 0; j = j - 8) {
					let $AA = [$P, $P.push];
					$AA[1].call($AA[0], Number($Z[j]) << 7 | Number($Z[j + 1]) << 6 | Number($Z[j + 2]) << 5 | Number($Z[j + 3]) << 4 | Number($Z[j + 4]) << 3 | Number($Z[j + 5]) << 2 | Number($Z[j + 6]) << 1 | Number($Z[j + 7]));
				}
			}
			let $AB = $G;
			let i_2 = 0;
			for (; i_2 < 8; i_2++) {
				$AB[++$KEY_7] = $P[i_2] ^ $E();
			}
		}
		return $KEY_7;
	}"#;

fn witness() -> String {
    [
        "function main(){let $KEY_23=0,$KEY_223=ArrayBuffer,$KEY_311=[];",
        WITNESS_BODY,
        "return $THREAD_6_6494;}main();",
    ]
    .concat()
}

fn aliased_position() -> String {
    [
        "function main(){",
        ALIASED_POSITION_BODY,
        "return $THREAD_5_111700;}main();",
    ]
    .concat()
}

fn split_position() -> String {
    [
        "function main(){",
        SPLIT_POSITION_BODY,
        "return $THREAD_7_102903;}main();",
    ]
    .concat()
}

fn base_plus() -> String {
    [
        "function main(){",
        BASE_PLUS_BODY,
        "return $THREAD_0_111797;}main();",
    ]
    .concat()
}

fn claims(source: &str) -> usize {
    name_threads(source, true)
        .unwrap()
        .report
        .function_roles
        .iter()
        .filter(|role| {
            role.candidates.iter().any(|candidate| candidate == NAME)
                && role.evaluators.iter().any(|evaluator| evaluator == EVALUATOR)
        })
        .count()
}

fn applied(source: &str) -> usize {
    name_threads(source, true)
        .unwrap()
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

#[test]
fn witness_number_writer_is_claimed_and_renamed() {
    let source = witness();
    assert_eq!(claims(&source), 1);
    assert_eq!(applied(&source), 1);
    let renamed = name_threads(&source, true).unwrap();
    assert!(renamed.source.contains("function writeNumber("));
    assert!(!renamed.source.contains("function $THREAD_6_6494("));
    // Replaying the reported edits reproduces the transformed source.
    let mut replay = source.clone();
    let mut edits = renamed.report.edits.clone();
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.start));
    for edit in edits {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, renamed.source);
}

#[test]
fn vendor_and_local_spelling_variants_are_claimed() {
    for source in [aliased_position(), split_position(), base_plus()] {
        assert_eq!(claims(&source), 1, "{source}");
        assert_eq!(applied(&source), 1, "{source}");
    }
    // Renamed thread grammar and renamed captured locals keep the same proof.
    let renamed = witness()
        .replace("$THREAD_6_6494", "$THREAD_71_887766_2")
        .replace("$KEY_23", "$SCRATCH_ONE")
        .replace("$KEY_223", "$SCRATCH_TWO")
        .replace("$KEY_311", "$SCRATCH_THREE")
        .replace("$KEY_2", "$POSITION")
        .replace("$BW", "$MASK_FN")
        .replace("$BX", "$VALUE")
        .replace("$BY", "$BUFFER")
        .replace("$BZ", "$OFFSET")
        .replace("$CA", "$SIGN_TEST")
        .replace("$CB", "$SIGN_CODE")
        .replace("$CC", "$MAGNITUDE")
        .replace("$CD", "$BYTES")
        .replace("$CE", "$ABSOLUTE")
        .replace("$CF", "$MANTISSA")
        .replace("$CG", "$EXPONENT")
        .replace("$CH", "$EXP_ADJUSTED")
        .replace("$CI", "$ROUND_CARRY")
        .replace("$CJ", "$TRUNCATED")
        .replace("$CK", "$BITS");
    assert_eq!(claims(&renamed), 1, "{renamed}");
    assert_eq!(applied(&renamed), 1, "{renamed}");
}

#[test]
fn for_versus_while_final_loop_is_claimed() {
    let source = aliased_position()
        .replace("for (; i_2 < 8; i_2++) {", "while (i_2 < 8) {")
        .replace(
            "$CD[++$BU] = $CC[i_2] ^ $BQ();",
            "$CD[++$BU] = $CC[i_2] ^ $BQ();i_2++;",
        );
    assert!(source.contains("while (i_2 < 8)"));
    assert_eq!(claims(&source), 1, "{source}");
    assert_eq!(applied(&source), 1, "{source}");
}

#[test]
fn meaningful_operation_and_receiver_negatives_are_refused() {
    let witness = witness();
    let aliased_position = aliased_position();
    for source in [
        // operation: shift amount, threshold, continuation mask, tag mask.
        witness.replace("$CC >> 5", "$CC >> 6"),
        witness.replace("$CC < 32", "$CC < 33"),
        witness.replace("& 127 | 128", "& 127 | 129"),
        witness.replace("& 31 | 32", "& 31 | 33"),
        // operation: the byte store is no longer masked by XOR.
        witness.replace("^ $BW()", "| $BW()"),
        witness.replace("$BW() ^ 128", "$BW() ^ 129"),
        // operation: the sign tag is not the codec's 64.
        witness.replace("$CB = 64;", "$CB = 65;"),
        // receiver: the buffer is a private array, not formal slot 2.
        witness
            .replace("let $KEY_2;", "let $KEY_2;let $BUF2=[];")
            .replace("$BY[", "$BUF2["),
        // receiver: the keystream callback is called with an argument.
        witness.replace("$BW()", "$BW(1)"),
        // receiver: a local callback replaces the mask formal.
        witness
            .replace("let $KEY_2;", "let $KEY_2;let $MASK=function(){return 0;};")
            .replace("$BW()", "$MASK()"),
        // float decoder spelling changed: exponent 52 -> 53.
        aliased_position.replace("Math.pow(2, 52)", "Math.pow(2, 53)"),
    ] {
        assert_eq!(claims(&source), 0, "accepted negative:\n{source}");
    }
}

#[test]
fn shadow_and_nested_callback_negatives_are_refused() {
    // A same-named local holds a copy of the value, so the integer test and
    // the sign branch no longer read the proven formal symbol.
    let shadowed = witness()
        .replace(
            "function $THREAD_6_6494($BW, $BX, $BY, $BZ) {",
            "function $THREAD_6_6494($BW, $VALUE, $BY, $BZ) {let $BX=$VALUE;",
        );
    assert_eq!(claims(&shadowed), 0, "{shadowed}");

    // The encoder shape nested inside an owned arrow is a decoy: the arrow has
    // no thread identity and the outer function is a factory.
    let nested = [
        "function main(){let $KEY_23=0,$KEY_223=ArrayBuffer,$KEY_311=[];",
        "function $THREAD_6_6494($BW,$BX,$BY,$BZ){let $inner=()=>{",
        WITNESS_BODY.trim_start_matches("function $THREAD_6_6494($BW, $BX, $BY, $BZ) {"),
        // The representative body already ends with the brace that closes the
        // arrow body; only the factory `return` and the outer brace remain.
        ";return $inner;}return $THREAD_6_6494;}main();",
    ]
    .concat();
    assert_eq!(claims(&nested), 0, "{nested}");

    // The two-parameter float serializer helper is not this family.
    let float_helper = r#"function main(){
function $THREAD_9_123($BQ, $BR) {
let $KEY_7 = $BQ < 0;
let $KEY_8 = Math.abs($BQ);
let $KEY_5 = $KEY_8 / Math.pow(2, -1074);
let $KEY_2 = Math.floor($KEY_5);
let $CE = [];
$CE.unshift($KEY_7);
$BR.push($KEY_2);
}
return $THREAD_9_123;}main();"#;
    assert_eq!(claims(float_helper), 0, "{float_helper}");

    // A Math.pow ladder without the byte-buffer encoder is not the family.
    let modpow_like = r#"function main(){
function $THREAD_7_24052($BQ, $BR, $BS, $BT) {
let $KEY_2 = Math.pow(2, -1074);
let $KEY_5 = Math.floor($BQ * Math.pow(2, 52));
let $KEY_6 = $KEY_5 % 2 ? $KEY_5 + 1 : $KEY_5;
return $KEY_6;
}
return $THREAD_7_24052;}main();"#;
    assert_eq!(claims(modpow_like), 0, "{modpow_like}");
}

#[test]
fn two_instances_per_seed_are_both_claimed() {
    let second = WITNESS_BODY.replace("$THREAD_6_6494", "$THREAD_8_74386");
    let source = [
        "function main(){let $KEY_23=0,$KEY_223=ArrayBuffer,$KEY_311=[];",
        WITNESS_BODY,
        "return $THREAD_6_6494;}",
        second.as_str(),
        "main();",
    ]
    .concat();
    assert_eq!(claims(&source), 2, "{source}");
    assert_eq!(applied(&source), 2, "{source}");
    let renamed = name_threads(&source, true).unwrap().source;
    assert_eq!(renamed.matches("function writeNumber(").count(), 2);
}

#[test]
fn prior_readable_name_is_retained_and_a_local_collision_is_not_applied() {
    let prior = witness().replace("$THREAD_6_6494", "writeNumber");
    let report = name_threads(&prior, true).unwrap();
    assert_eq!(report.source, prior);
    assert!(report.report.function_roles.iter().any(|role| {
        role.current_name.as_deref() == Some("writeNumber")
            && role.evaluators.iter().any(|evaluator| evaluator == EVALUATOR)
    }));

    let collided = witness().replace(
        "function main(){",
        "function main(){let writeNumber=42;consume(writeNumber);",
    );
    let report = name_threads(&collided, true).unwrap();
    let outcome = report
        .report
        .outcomes
        .iter()
        .find(|outcome| outcome.binding.name == "$THREAD_6_6494")
        .unwrap();
    assert_eq!(outcome.proposed_name.as_deref(), Some(NAME));
    assert!(!outcome.applied);
    assert_eq!(outcome.reason.as_deref(), Some("NameCollision"));
}

#[test]
fn class_methods_empty_stubs_and_arrows_never_gain_the_role() {
    let source = r#"function main(){
class Holder { $THREAD_6_6494($BW, $BX, $BY, $BZ) { return $KEY_2; } }
function $THREAD_9_999() {}
let $arrow = ($BW, $BX, $BY, $BZ) => $BX;
return [Holder, $arrow];}main();"#;
    let result = name_threads(source, true).unwrap();
    assert!(!result.report.function_roles.iter().any(|role| role
        .evaluators
        .iter()
        .any(|evaluator| evaluator == EVALUATOR)));
    assert!(result.source.contains("class Holder"));
}

// Mixed continuation-index variants encode the same four-threshold varint ladder and
// IEEE-754 eight-byte path. Declarations can own multiple position locals, with upper-tier
// final bytes written through `pos = Number(prev) + 1; buf[pos]`. The alternate branch can
// build bytes through an assigned `[array, array.push]` pair and alias the final-loop
// array.

const MIXED_INDEX_BODY: &str = r#"function $THREAD_6_22985($AO, $AP, $AQ, $AR) {
			let $KEY_0, $KEY_5;
			if (($AP | 0) === $AP) {
				let $AS;
				if ($AP < 0) {
					$AS = $AP < 0;
				} else if ($AP === 0) {
					$AS = 1 / $AP < 0;
				} else {
					$AS = false;
				}
				let $AT, $AU;
				if ($AS) {
					$AT = -$AP;
					$AU = 64;
				} else {
					$AT = $AP;
					$AU = 0;
				}
				if ($AT < 32) {
					$KEY_0 = Number($AR) + 1;
					$AQ[$KEY_0] = ($AU | $AT) ^ $AO();
				} else if ($AT < 4096) {
					$KEY_0 = Number($AR) + 1;
					$AQ[$KEY_0] = ($AU | ($AT & 31 | 32)) ^ $AO();
					$AQ[++$KEY_0] = $AT >> 5 ^ $AO();
				} else if ($AT < 524288) {
					$KEY_5 = Number($AR) + 1;
					$AQ[$KEY_5] = ($AU | ($AT & 31 | 32)) ^ $AO();
					$AQ[++$KEY_5] = ($AT >> 5 & 127 | 128) ^ $AO();
					$KEY_0 = Number($KEY_5) + 1;
					$AQ[$KEY_0] = $AT >>> 12 ^ $AO();
				} else if ($AT < 67108864) {
					$KEY_5 = Number($AR) + 1;
					$AQ[$KEY_5] = ($AU | ($AT & 31 | 32)) ^ $AO();
					$AQ[++$KEY_5] = ($AT >> 5 & 127 | 128) ^ $AO();
					$AQ[++$KEY_5] = ($AT >>> 12 & 127 | 128) ^ $AO();
					$KEY_0 = Number($KEY_5) + 1;
					$AQ[$KEY_0] = $AT >>> 19 ^ $AO();
				} else {
					$KEY_5 = Number($AR) + 1;
					$AQ[$KEY_5] = ($AU | ($AT & 31 | 32)) ^ $AO();
					$AQ[++$KEY_5] = ($AT >> 5 & 127 | 128) ^ $AO();
					$AQ[++$KEY_5] = ($AT >>> 12 & 127 | 128) ^ $AO();
					$AQ[++$KEY_5] = ($AT >>> 19 & 127 | 128) ^ $AO();
					$KEY_0 = Number($KEY_5) + 1;
					$AQ[$KEY_0] = $AT >>> 26 ^ $AO();
				}
			} else {
				$KEY_5 = $AR;
				$KEY_0 = Number($AR) + 1;
				$AQ[$KEY_0] = $AO() ^ 128;
				let $AV;
				if ($KEY_379) {
					$KEY_379[0] = $AP;
					$AV = new $KEY_399($KEY_379.buffer);
				} else if ($AP !== $AP) {
					$AV = $KEY_343;
				} else {
					let $KEY_6 = $AP < 0 || $AP === 0 && 1 / $AP < 0;
					let $AW = Math.abs($AP);
					let $AX, $AY;
					if ($AW >= Math.pow(2, -1022)) {
						$AX = Math.min(Math.floor(Math.log($AW) / Math.LN2), 1023);
						$AW = $AW / Math.pow(2, $AX);
						let $AZ = $AW < 1;
						$AX = $AZ ? $AX - 1 : $AX;
						$AZ = $AZ ? $AW * 2 : $AW;
						let $BA = $AZ >= 2;
						$AX = $BA ? $AX + 1 : $AX;
						$BA = $BA ? $AZ / 2 : $AZ;
						let $KEY_7 = Math.pow(2, 52);
						let $BB = Math.floor($BA * $KEY_7);
						if (!($BA * $KEY_7 - $BB < .5)) if ($BA * $KEY_7 - $BB > .5) {
							++$BB;
						} else {
							$BB = [$BB, $BB % 2];
							$BB = $BB[1] ? $BB[0] + 1 : $BB[0];
						}
						$AY = ($BB - $KEY_7) / $KEY_7 >= 1;
						$AX = $AY ? $AX + 1024 : $AX + 1023;
						$AY = $AY ? 0 : $BB - $KEY_7;
						if ($AX > 2046) {
							$AX = 2047;
							$AY = 0;
						}
					} else {
						$AW = $AW / Math.pow(2, -1074);
						$AY = Math.floor($AW);
						if (!($AW - $AY < .5)) if ($AW - $AY > .5) {
							++$AY;
						} else {
							$AY = [$AY, $AY % 2];
							$AY = $AY[1] ? $AY[0] + 1 : $AY[0];
						}
						$AX = 0;
					}
					let $BC = [];
					let k = 51;
					for (; k >= 0; k--) {
						$BC.unshift(!!($AY & 1));
						$AY = Math.floor($AY / 2);
					}
					let i = 11;
					for (; i > 0; i--) {
						$BC.unshift(!!($AX & 1));
						$AX = Math.floor($AX / 2);
					}
					$BC.unshift($KEY_6);
					$AV = [];
					let j = 56;
					for (; j >= 0; j = j - 8) {
						$AV.push(Number($BC[j]) << 7 | Number($BC[j + 1]) << 6 | Number($BC[j + 2]) << 5 | Number($BC[j + 3]) << 4 | Number($BC[j + 4]) << 3 | Number($BC[j + 5]) << 2 | Number($BC[j + 6]) << 1 | Number($BC[j + 7]));
					}
				}
				let i_2 = 0;
				for (; i_2 < 8; i_2++) {
					$AQ[++$KEY_0] = $AV[i_2] ^ $AO();
				}
			}
			return $KEY_0;
		}"#;

const PUSH_PAIR_BODY: &str = r#"function $THREAD_8_98775($BR, $BS, $BT, $BU) {
			let $KEY_4;
			if (($BS | 0) === $BS) {
				let $BV;
				if ($BS < 0) {
					$BV = $BS < 0;
				} else if ($BS === 0) {
					$BV = 1 / $BS < 0;
				} else {
					$BV = $BS === 0;
				}
				let $BW, $BX;
				if ($BV) {
					$BW = 64;
					$BX = -$BS;
				} else {
					$BW = 0;
					$BX = $BS;
				}
				if ($BX < 32) {
					$KEY_4 = Number($BU) + 1;
					$BT[$KEY_4] = ($BW | $BX) ^ $BR();
				} else if ($BX < 4096) {
					$KEY_4 = Number($BU) + 1;
					$BT[$KEY_4] = ($BW | ($BX & 31 | 32)) ^ $BR();
					$BT[++$KEY_4] = $BX >> 5 ^ $BR();
				} else if ($BX < 524288) {
					$KEY_4 = Number($BU) + 1;
					$BT[$KEY_4] = ($BW | ($BX & 31 | 32)) ^ $BR();
					$BT[++$KEY_4] = ($BX >> 5 & 127 | 128) ^ $BR();
					$BT[++$KEY_4] = $BX >>> 12 ^ $BR();
				} else if ($BX < 67108864) {
					$KEY_4 = Number($BU) + 1;
					$BT[$KEY_4] = ($BW | ($BX & 31 | 32)) ^ $BR();
					$BT[++$KEY_4] = ($BX >> 5 & 127 | 128) ^ $BR();
					$BT[++$KEY_4] = ($BX >>> 12 & 127 | 128) ^ $BR();
					$BT[++$KEY_4] = $BX >>> 19 ^ $BR();
				} else {
					$KEY_4 = Number($BU) + 1;
					$BT[$KEY_4] = ($BW | ($BX & 31 | 32)) ^ $BR();
					$BT[++$KEY_4] = ($BX >> 5 & 127 | 128) ^ $BR();
					$BT[++$KEY_4] = ($BX >>> 12 & 127 | 128) ^ $BR();
					$BT[++$KEY_4] = ($BX >>> 19 & 127 | 128) ^ $BR();
					$BT[++$KEY_4] = $BX >>> 26 ^ $BR();
				}
			} else {
				$KEY_4 = $BU;
				$KEY_4 = Number($BU) + 1;
				$BT[$KEY_4] = $BR() ^ 128;
				let $BY;
				if ($KEY_256) {
					$KEY_256[0] = $BS;
					$BY = new $KEY_154($KEY_256.buffer);
				} else if ($BS !== $BS) {
					$BY = $KEY_36;
				} else {
					let $KEY_1 = $BS < 0 || $BS === 0 && 1 / $BS < 0;
					let $BZ = Math.abs($BS);
					let $CA, $CB;
					if ($BZ >= Math.pow(2, -1022)) {
						let $KEY_9 = Math.min(Math.floor(Math.log($BZ) / Math.LN2), 1023);
						$BZ = $BZ / Math.pow(2, $KEY_9);
						if ($BZ < 1) {
							$BZ = $BZ * 2;
							--$KEY_9;
						}
						if ($BZ >= 2) {
							++$KEY_9;
							$BZ = $BZ / 2;
						}
						let $KEY_0 = Math.pow(2, 52);
						$BZ = $BZ * $KEY_0;
						let $CC = Math.floor($BZ);
						if (!($BZ - $CC < .5)) if ($BZ - $CC > .5) {
							++$CC;
						} else {
							$CC = [$CC, $CC % 2];
							$CC = $CC[1] ? $CC[0] + 1 : $CC[0];
						}
						$KEY_9 += 1023;
						$CB = ($CC - $KEY_0) / $KEY_0 >= 1;
						$KEY_0 = $CB ? 0 : $CC - $KEY_0;
						$CB = $CB ? $KEY_9 + 1 : $KEY_9;
						if ($CB > 2046) {
							$CA = 0;
							$CB = 2047;
						} else {
							$CA = $KEY_0;
						}
					} else {
						$BZ = $BZ / Math.pow(2, -1074);
						$CA = Math.floor($BZ);
						if (!($BZ - $CA < .5)) if ($BZ - $CA > .5) {
							++$CA;
						} else {
							$CA = [$CA, $CA % 2];
							$CA = $CA[1] ? $CA[0] + 1 : $CA[0];
						}
						$CB = 0;
					}
					let $CD = [];
					let k = 51;
					for (; k >= 0; k--) {
						$CD.unshift(!!($CA & 1));
						$CA = Math.floor($CA / 2);
					}
					let i = 11;
					for (; i > 0; i--) {
						$CD.unshift(!!($CB & 1));
						$CB = Math.floor($CB / 2);
					}
					$CD.unshift($KEY_1);
					let $CE = [];
					let j = 56;
					let $CF, $CG;
					for (;; j = $CG - 8) {
						$CF = $CE;
						$CG = j;
						if (!(j >= 0)) break;
						$CF = [$CF, $CF.push];
						$CF[1].call($CF[0], Number($CD[$CG]) << 7 | Number($CD[$CG + 1]) << 6 | Number($CD[$CG + 2]) << 5 | Number($CD[$CG + 3]) << 4 | Number($CD[$CG + 4]) << 3 | Number($CD[$CG + 5]) << 2 | Number($CD[$CG + 6]) << 1 | Number($CD[$CG + 7]));
					}
					$BY = $CF;
				}
				let i_2 = 0;
				for (; i_2 < 8; i_2++) {
					$BT[++$KEY_4] = $BY[i_2] ^ $BR();
				}
			}
			return $KEY_4;
		}"#;

const FINAL_ARRAY_ALIAS_BODY: &str = r#"function $THREAD_6_32264($BU, $BV, $BW, $BX) {
			let $KEY_5;
			if (($BV | 0) === $BV) {
				let $BY;
				if ($BV < 0) {
					$BY = $BV < 0;
				} else if ($BV === 0) {
					$BY = 1 / $BV < 0;
				} else {
					$BY = $BV === 0;
				}
				let $BZ, $CA;
				if ($BY) {
					$BZ = 64;
					$CA = -$BV;
				} else {
					$BZ = 0;
					$CA = $BV;
				}
				if ($CA < 32) {
					$KEY_5 = Number($BX) + 1;
					$BW[$KEY_5] = ($BZ | $CA) ^ $BU();
				} else if ($CA < 4096) {
					$KEY_5 = Number($BX) + 1;
					$BW[$KEY_5] = ($BZ | ($CA & 31 | 32)) ^ $BU();
					$BW[++$KEY_5] = $CA >> 5 ^ $BU();
				} else if ($CA < 524288) {
					$KEY_5 = Number($BX) + 1;
					$BW[$KEY_5] = ($BZ | ($CA & 31 | 32)) ^ $BU();
					$BW[++$KEY_5] = ($CA >> 5 & 127 | 128) ^ $BU();
					$BW[++$KEY_5] = $CA >>> 12 ^ $BU();
				} else if ($CA < 67108864) {
					$KEY_5 = Number($BX) + 1;
					$BW[$KEY_5] = ($BZ | ($CA & 31 | 32)) ^ $BU();
					$BW[++$KEY_5] = ($CA >> 5 & 127 | 128) ^ $BU();
					$BW[++$KEY_5] = ($CA >>> 12 & 127 | 128) ^ $BU();
					$BW[++$KEY_5] = $CA >>> 19 ^ $BU();
				} else {
					$KEY_5 = Number($BX) + 1;
					$BW[$KEY_5] = ($BZ | ($CA & 31 | 32)) ^ $BU();
					$BW[++$KEY_5] = ($CA >> 5 & 127 | 128) ^ $BU();
					$BW[++$KEY_5] = ($CA >>> 12 & 127 | 128) ^ $BU();
					$BW[++$KEY_5] = ($CA >>> 19 & 127 | 128) ^ $BU();
					$BW[++$KEY_5] = $CA >>> 26 ^ $BU();
				}
			} else {
				$KEY_5 = $BX;
				let $KEY_4 = Number($BX) + 1;
				$BW[$KEY_4] = $BU() ^ 128;
				let $CB;
				if ($KEY_211) {
					$KEY_211[0] = $BV;
					$CB = new $KEY_240($KEY_211.buffer);
				} else if ($BV !== $BV) {
					$CB = $KEY_317;
				} else {
					let $KEY_9 = $BV < 0 || $BV === 0 && 1 / $BV < 0;
					let $CC = Math.abs($BV);
					let $CD;
					if ($CC >= Math.pow(2, -1022)) {
						$CD = Math.min(Math.floor(Math.log($CC) / Math.LN2), 1023);
						$CC = $CC / Math.pow(2, $CD);
						let $CE = $CC < 1;
						$CD = $CE ? $CD - 1 : $CD;
						$CE = $CE ? $CC * 2 : $CC;
						let $CF = $CE >= 2;
						$CD = $CF ? $CD + 1 : $CD;
						$CF = $CF ? $CE / 2 : $CE;
						let $CG = Math.pow(2, 52);
						let $KEY_1 = Math.floor($CF * $CG);
						let $CH = (($CF * $CG - $KEY_1 < .5 ? $KEY_1 : $CF * $CG - $KEY_1 > .5 ? $KEY_1 + 1 : $KEY_1 % 2 ? $KEY_1 + 1 : $KEY_1) - $CG) / $CG >= 1;
						$CD = $CH ? $CD + 1024 : $CD + 1023;
						if ($CD > 2046) {
							$CD = 2047;
							$CC = 0;
						} else {
							$CC = $CH ? 0 : ($CF * $CG - $KEY_1 < .5 ? $KEY_1 : $CF * $CG - $KEY_1 > .5 ? $KEY_1 + 1 : $KEY_1 % 2 ? $KEY_1 + 1 : $KEY_1) - $CG;
						}
					} else {
						$CC = $CC / Math.pow(2, -1074);
						let $CI = Math.floor($CC);
						$CD = 0;
						$CC = $CC - $CI < .5 ? $CI : $CC - $CI > .5 ? $CI + 1 : $CI % 2 ? $CI + 1 : $CI;
					}
					let $CJ = [];
					let k = 51;
					for (; k >= 0; k--) {
						$CJ.unshift(!!($CC & 1));
						$CC = Math.floor($CC / 2);
					}
					let i = 11;
					for (; i > 0; i--) {
						$CJ.unshift(!!($CD & 1));
						$CD = Math.floor($CD / 2);
					}
					$CJ.unshift($KEY_9);
					let $CK = [];
					let j = 56;
					let $CL, $CM;
					for (;; j = $CM - 8) {
						$CL = $CK;
						$CM = j;
						if (!(j >= 0)) break;
						$CL = [$CL, $CL.push];
						$CL[1].call($CL[0], Number($CJ[$CM]) << 7 | Number($CJ[$CM + 1]) << 6 | Number($CJ[$CM + 2]) << 5 | Number($CJ[$CM + 3]) << 4 | Number($CJ[$CM + 4]) << 3 | Number($CJ[$CM + 5]) << 2 | Number($CJ[$CM + 6]) << 1 | Number($CJ[$CM + 7]));
					}
					$CB = $CL;
				}
				let i_2 = 0;
				for (; i_2 < 8; i_2++) {
					$BW[++$KEY_4] = $CB[i_2] ^ $BU();
				}
				$KEY_5 = $KEY_4;
			}
			return $KEY_5;
		}"#;

fn mixed_index() -> String {
    [
        "function main(){let $KEY_379=0,$KEY_399=ArrayBuffer,$KEY_343=[];",
        MIXED_INDEX_BODY,
        "return $THREAD_6_22985;}main();",
    ]
    .concat()
}

fn push_pair() -> String {
    [
        "function main(){let $KEY_256=0,$KEY_154=ArrayBuffer,$KEY_36=[];",
        PUSH_PAIR_BODY,
        "return $THREAD_8_98775;}main();",
    ]
    .concat()
}

fn final_array_alias() -> String {
    [
        "function main(){let $KEY_211=0,$KEY_240=ArrayBuffer,$KEY_317=[];",
        FINAL_ARRAY_ALIAS_BODY,
        "return $THREAD_6_32264;}main();",
    ]
    .concat()
}

#[test]
fn mixed_continuation_index_and_alias_variants_are_claimed() {
    for source in [mixed_index(), push_pair(), final_array_alias()] {
        assert_eq!(claims(&source), 1, "{source}");
        assert_eq!(applied(&source), 1, "{source}");
    }
}

#[test]
fn renamed_mixed_index_variant_is_claimed() {
    let source = mixed_index()
        .replace("$THREAD_6_22985", "$THREAD_71_5150_3")
        .replace("$AO", "$MASK_FN")
        .replace("$AP", "$VALUE")
        .replace("$AQ", "$BUFFER")
        .replace("$AR", "$OFFSET")
        .replace("$AT", "$MAGNITUDE")
        .replace("$AU", "$SIGN_CODE")
        .replace("$KEY_0", "$POSITION")
        .replace("$KEY_5", "$SECOND_POSITION");
    assert_eq!(claims(&source), 1, "{source}");
    assert_eq!(applied(&source), 1, "{source}");
}

#[test]
fn const_sign_flag_keeps_the_same_immutability() {
    let source = push_pair().replace(
        "let $KEY_1 = $BS < 0 || $BS === 0 && 1 / $BS < 0;",
        "const $KEY_1 = $BS < 0 || $BS === 0 && 1 / $BS < 0;",
    );
    assert!(source.contains("const $KEY_1"));
    assert_eq!(claims(&source), 1, "{source}");
    assert_eq!(applied(&source), 1, "{source}");
}

#[test]
fn mixed_index_variant_edit_replay_matches_by_byte_offset() {
    let source = mixed_index();
    let renamed = name_threads(&source, true).unwrap();
    assert_eq!(renamed.source.matches("function writeNumber(").count(), 1);
    let mut replay = source.clone();
    let mut edits = renamed.report.edits.clone();
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.start));
    for edit in edits {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, renamed.source);
}

#[test]
fn mixed_index_and_alias_negatives_are_refused() {
    let mixed_index_source = mixed_index();
    let push_pair_source = push_pair();
    let array_alias_source = final_array_alias();
    for source in [
        // counter: the returned position is the position formal, not the
        // body-declared local.
        mixed_index_source.replace("return $KEY_0;", "return $AR;"),
        // counter: the position local is never seeded from the position formal.
        mixed_index_source.replace("Number($AR)", "$AR"),
        // operation: the third tier's final byte uses the wrong shift amount.
        mixed_index_source.replace("$AT >>> 12 ^ $AO()", "$AT >>> 13 ^ $AO()"),
        // stream: a continuation byte is no longer XORed with the keystream.
        mixed_index_source.replace(
            "$AQ[$KEY_5] = ($AU | ($AT & 31 | 32)) ^ $AO();",
            "$AQ[$KEY_5] = ($AU | ($AT & 31 | 32)) | $AO();",
        ),
        // receiver: the buffer is a private array, not formal slot 2.
        mixed_index_source
            .replace("let $KEY_0, $KEY_5;", "let $KEY_0, $KEY_5;let $BUF2=[];")
            .replace("$AQ[", "$BUF2["),
        // stream: the keystream callback is called with an argument.
        mixed_index_source.replace("$AO()", "$AO(1)"),
        // shadow: a same-named local copies the value formal.
        push_pair_source.replace(
            "function $THREAD_8_98775($BR, $BS, $BT, $BU) {",
            "function $THREAD_8_98775($BR, $VALUE, $BT, $BU) {let $BS=$VALUE;",
        ),
        // donor: the final loop reads the un-filled base array, not the pushed
        // alias.
        push_pair_source.replace("$BY = $CF;", "$BY = $CE;"),
        // donor/disconnect: the final loop reads an array that was never built
        // by the push pair.
        array_alias_source.replace("$CB = $CL;", "$CB = $CK;"),
        // operation: a wrong threshold in the mixed tier.
        array_alias_source.replace("$CA < 524288", "$CA < 524289"),
    ] {
        assert_eq!(claims(&source), 0, "accepted negative:\n{source}");
    }
}

#[test]
fn nested_mixed_index_donor_is_refused() {
    let nested = [
        "function main(){let $KEY_379=0,$KEY_399=ArrayBuffer,$KEY_343=[];",
        "function $THREAD_6_22985($AO,$AP,$AQ,$AR){let $inner=()=>{",
        MIXED_INDEX_BODY.trim_start_matches("function $THREAD_6_22985($AO, $AP, $AQ, $AR) {"),
        ";return $inner;}return $THREAD_6_22985;}main();",
    ]
    .concat();
    assert_eq!(claims(&nested), 0, "{nested}");
}


// Tuple continuation snapshot carries cursor and output through `holder = [cursor,
// buffer]`; the right side is read before holder is overwritten. Then `next = holder[0] +
// 1` and `holder[1][next] = payload ^ mask()`. The body is verbatim; the carrier supplies
// only ArrayBuffer and empty-array aliases. Direct/mixed-index siblings remain positive.
// Negatives isolate tuple dataflow errors.

const TUPLE_CURSOR_BODY: &str = r#"function $THREAD_1_96432($BZ, $CA, $CB, $CC) {
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

const MIXED_INDEX_NEIGHBOR_BODY: &str = r#"function $THREAD_3_94615($CB, $CC, $CD, $CE) {
					let $KEY_8;
					if (($CC | 0) === $CC) {
						let $CF;
						if ($CC < 0) {
							$CF = $CC < 0;
						} else if ($CC === 0) {
							$CF = 1 / $CC < 0;
						} else {
							$CF = $CC === 0;
						}
						let $CG, $CH;
						if ($CF) {
							$CG = 64;
							$CH = -$CC;
						} else {
							$CG = 0;
							$CH = $CC;
						}
						if ($CH < 32) {
							$KEY_8 = Number($CE) + 1;
							$CD[$KEY_8] = ($CG | $CH) ^ $CB();
						} else if ($CH < 4096) {
							let $CI = Number($CE) + 1;
							$CD[$CI] = ($CG | ($CH & 31 | 32)) ^ $CB();
							$KEY_8 = $CI + 1;
							$CD[$KEY_8] = $CH >> 5 ^ $CB();
						} else if ($CH < 524288) {
							let $CJ = Number($CE) + 1;
							$CD[$CJ] = ($CG | ($CH & 31 | 32)) ^ $CB();
							$CD[++$CJ] = ($CH >> 5 & 127 | 128) ^ $CB();
							$KEY_8 = $CJ + 1;
							$CD[$KEY_8] = $CH >>> 12 ^ $CB();
						} else if ($CH < 67108864) {
							$KEY_8 = Number($CE) + 1;
							$CD[$KEY_8] = ($CG | ($CH & 31 | 32)) ^ $CB();
							$CD[++$KEY_8] = ($CH >> 5 & 127 | 128) ^ $CB();
							$CD[++$KEY_8] = ($CH >>> 12 & 127 | 128) ^ $CB();
							$CD[++$KEY_8] = $CH >>> 19 ^ $CB();
						} else {
							let $CK = Number($CE) + 1;
							$CD[$CK] = ($CG | ($CH & 31 | 32)) ^ $CB();
							$CD[++$CK] = ($CH >> 5 & 127 | 128) ^ $CB();
							$CD[++$CK] = ($CH >>> 12 & 127 | 128) ^ $CB();
							$CD[++$CK] = ($CH >>> 19 & 127 | 128) ^ $CB();
							$KEY_8 = $CK + 1;
							$CD[$KEY_8] = $CH >>> 26 ^ $CB();
						}
					} else {
						$KEY_8 = Number($CE) + 1;
						$CD[$KEY_8] = $CB() ^ 128;
						let $CL, $CM;
						if ($KEY_50) {
							$KEY_50[0] = $CC;
							$CL = new $KEY_305($KEY_50.buffer);
							$CM = $CD;
						} else if ($CC !== $CC) {
							$CL = $KEY_175;
							$CM = $CD;
						} else {
							let $KEY_6 = $CC < 0 || $CC === 0 && 1 / $CC < 0;
							$CM = $CD;
							let $CN = Math.abs($CC);
							let $CO, $KEY_0;
							if ($CN >= Math.pow(2, -1022)) {
								let $CP = Math.min(Math.floor(Math.log($CN) / Math.LN2), 1023);
								$CN = $CN / Math.pow(2, $CP);
								let $CQ = $CN < 1;
								$CP = $CQ ? $CP - 1 : $CP;
								$CQ = $CQ ? $CN * 2 : $CN;
								let $CR = $CQ >= 2;
								$CQ = $CR ? $CQ / 2 : $CQ;
								$KEY_0 = Math.pow(2, 52);
								let $CS = Math.floor($CQ * $KEY_0);
								$CR = ($CR ? $CP + 1 : $CP) + 1023;
								let $CT = (($CQ * $KEY_0 - $CS < .5 ? $CS : $CQ * $KEY_0 - $CS > .5 ? $CS + 1 : $CS % 2 ? $CS + 1 : $CS) - $KEY_0) / $KEY_0 >= 1;
								$CO = $CT ? $CR + 1 : $CR;
								if ($CO > 2046) {
									$CO = 2047;
									$KEY_0 = 0;
								} else {
									$KEY_0 = $CT ? 0 : ($CQ * $KEY_0 - $CS < .5 ? $CS : $CQ * $KEY_0 - $CS > .5 ? $CS + 1 : $CS % 2 ? $CS + 1 : $CS) - $KEY_0;
								}
							} else {
								$CN = $CN / Math.pow(2, -1074);
								let $CU = Math.floor($CN);
								$CO = 0;
								$KEY_0 = $CN - $CU < .5 ? $CU : $CN - $CU > .5 ? $CU + 1 : $CU % 2 ? $CU + 1 : $CU;
							}
							let $CV = [];
							let k = 51;
							for (; k >= 0; k--) {
								$CV.unshift(!!($KEY_0 & 1));
								$KEY_0 = Math.floor($KEY_0 / 2);
							}
							let i = 11;
							for (; i > 0; i--) {
								$CV.unshift(!!($CO & 1));
								$CO = Math.floor($CO / 2);
							}
							$CV.unshift($KEY_6);
							let $CW = [];
							let j = 56;
							let $CX, $CY;
							for (;; j = $CY - 8) {
								$CX = $CW;
								$CY = j;
								if (!(j >= 0)) break;
								$CX.push(Number($CV[$CY]) << 7 | Number($CV[$CY + 1]) << 6 | Number($CV[$CY + 2]) << 5 | Number($CV[$CY + 3]) << 4 | Number($CV[$CY + 4]) << 3 | Number($CV[$CY + 5]) << 2 | Number($CV[$CY + 6]) << 1 | Number($CV[$CY + 7]));
							}
							$CL = $CX;
						}
						let i_2 = 0;
						for (; i_2 < 8; i_2++) {
							$CM[++$KEY_8] = $CL[i_2] ^ $CB();
						}
					}
					return $KEY_8;
				}"#;

fn tuple_cursor() -> String {
    [
        "function main(){let $KEY_187=0,$KEY_202=ArrayBuffer,$KEY_289=[];",
        TUPLE_CURSOR_BODY,
        "return $THREAD_1_96432;}main();",
    ]
    .concat()
}

fn tuple_cursor_alpha() -> String {
    tuple_cursor()
        .replace("$THREAD_1_96432", "$THREAD_71_5150_3")
        .replace("$KEY_187", "$SCRATCH_HOLDER_BUFFER")
        .replace("$KEY_202", "$SCRATCH_ARRAY_BUFFER")
        .replace("$KEY_289", "$SCRATCH_EMPTY_ARRAY")
        .replace("$KEY_3", "$MANTISSA_PART")
        .replace("$KEY_5", "$ROUNDED_MANTISSA")
        .replace("$KEY_9", "$POW_TWO_52")
        .replace("$BZ", "$MASK_FN")
        .replace("$CA", "$VALUE")
        .replace("$CB", "$BUFFER")
        .replace("$CC", "$OFFSET")
        .replace("$CD", "$POSITION")
        .replace("$CE", "$SIGN_TEST")
        .replace("$CF", "$SIGN_CODE")
        .replace("$CG", "$MAGNITUDE")
        .replace("$CH", "$HOLDER")
        .replace("$CI", "$LOOP_TWO")
        .replace("$CJ", "$LOOP_FOUR")
        .replace("$CK", "$LOOP_FOUR_B")
        .replace("$CL", "$LOOP_FIVE")
        .replace("$CM", "$LOOP_FIVE_B")
        .replace("$CN", "$BYTES")
        .replace("$CO", "$SIGN_FLAG")
        .replace("$CP", "$ABSOLUTE")
        .replace("$CQ", "$EXPONENT")
        .replace("$CR", "$EXP_SMALL")
        .replace("$CS", "$EXP_OVER_TWO")
        .replace("$CT", "$ROUND_CARRY")
        .replace("$CU", "$BITS")
        .replace("$CV", "$EXP_COUNTER")
        .replace("$CW", "$BIT_HOLDER")
}

fn mixed_index_neighbor() -> String {
    [
        "function main(){let $KEY_50=0,$KEY_305=ArrayBuffer,$KEY_175=[];",
        MIXED_INDEX_NEIGHBOR_BODY,
        "return $THREAD_3_94615;}main();",
    ]
    .concat()
}

#[test]
fn tuple_snapshot_positive_is_claimed_after_a_nonvacuous_base_check() {
    // BASE first: the already-accepted direct/mixed proof must still claim, so
    // the expectations below cannot pass vacuously.
    assert_eq!(claims(&witness()), 1);
    assert_eq!(claims(&mixed_index()), 1);

    let source = tuple_cursor();
    assert_eq!(claims(&source), 1, "{source}");
    assert_eq!(applied(&source), 1, "{source}");
    let renamed = name_threads(&source, true).unwrap();
    assert!(renamed.source.contains("function writeNumber("));
    assert!(!renamed.source.contains("function $THREAD_1_96432("));

    // Nearest applied neighbour: the same role in the direct/mixed spelling.
    let neighbor = mixed_index_neighbor();
    assert_eq!(claims(&neighbor), 1, "{neighbor}");
    assert_eq!(applied(&neighbor), 1, "{neighbor}");

    // Replaying reported edits reproduces the transformed source.
    let mut replay = source.clone();
    let mut edits = renamed.report.edits.clone();
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.start));
    for edit in edits {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, renamed.source);
}

#[test]
fn tuple_snapshot_alpha_rename_is_claimed() {
    let source = tuple_cursor_alpha();
    assert!(source.contains("$HOLDER = [$HOLDER, $BUFFER];"));
    assert_eq!(claims(&source), 1, "{source}");
    assert_eq!(applied(&source), 1, "{source}");
}

#[test]
fn tuple_snapshot_dataflow_misses_are_refused() {
    let source = tuple_cursor();
    // swapped holder fields: field 0 is the buffer, field 1 is the cursor.
    let swapped = source.replace("$CH = [$CH, $CB];", "$CH = [$CB, $CH];");
    assert_eq!(claims(&swapped), 0, "{swapped}");
    // foreign output array: field 1 is a body-local array that is not slot 2.
    let foreign = source
        .replace(
            "let $CH = Number($CC) + 1;",
            "let $BUF2=[];let $CH = Number($CC) + 1;",
        )
        .replace("$CH = [$CH, $CB];", "$CH = [$CH, $BUF2];");
    assert_eq!(claims(&foreign), 0, "{foreign}");
    // wrong cursor: the cursor is read from field 1 (the array), not field 0.
    let wrong = source.replace("$CD = $CH[0] + 1;", "$CD = $CH[1] + 1;");
    assert_eq!(claims(&wrong), 0, "{wrong}");
    // reassigned holder: a second tuple snapshot follows the owned use.
    let reassigned = source.replace(
        "\t\t\t\t\t\t\t$CH[1][$CD] = $CG >> 5 ^ $BZ();",
        "\t\t\t\t\t\t\t$CH[1][$CD] = $CG >> 5 ^ $BZ();\n\t\t\t\t\t\t\t$CH = [$CH, $CB];",
    );
    assert_eq!(claims(&reassigned), 0, "{reassigned}");
    // misordered read: the cursor step runs before the tuple snapshot.
    let misordered = source.replace(
        "\t\t\t\t\t\t\t$CH = [$CH, $CB];\n\t\t\t\t\t\t\t$CD = $CH[0] + 1;",
        "\t\t\t\t\t\t\t$CD = $CH[0] + 1;\n\t\t\t\t\t\t\t$CH = [$CH, $CB];",
    );
    assert_eq!(claims(&misordered), 0, "{misordered}");
    // intervening write: the holder is overwritten between snapshot and step.
    let intervening = source.replace(
        "\t\t\t\t\t\t\t$CH = [$CH, $CB];\n\t\t\t\t\t\t\t$CD = $CH[0] + 1;",
        "\t\t\t\t\t\t\t$CH = [$CH, $CB];\n\t\t\t\t\t\t\t$CH = $CB;\n\t\t\t\t\t\t\t$CD = $CH[0] + 1;",
    );
    assert_eq!(claims(&intervening), 0, "{intervening}");
}
