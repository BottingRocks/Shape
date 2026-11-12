//! Focused, portable coverage for the storage-removal seeded frame emitter.
//!
//! The holder-cursor callback has captured key, removal flag, old value, error flag,
//! auxiliary flag, and observer declared before it. Its actual seed and writer factories
//! return their own nested children; the formal reaches the child through a one-hop alias,
//! read as data by the seed child and invoked by the writer. The surrounding parent
//! registers the callback for storage events, while the owned-body proof stays
//! registration-free.
//!
//! ALPHA respells callbacks, captured slots, and factories; READABLE uses human local
//! names. HOLDER varies reseed/tag literals and null-guard equality while retaining the
//! ordered frame. These distinguish scope identity from identifier and literal lookup.
//!
//! Each negative asserts its mutation anchor, requires parsing, and requires refusal. Dead
//! locals, never-called children, empty tests, discarded arguments, overwritten aliases,
//! and captured-global seeding cannot substitute for live formal dataflow.
//!
//! The direct variant shares the writer binding with the cursor, invokes writer(old,
//! header, 3), and uses `aux | 0 | removal << 1 | error << 2`. Its own factories and
//! storage registration remain intact.
//!
//! The fixed-slot variant appends shifted lanes at slot 4 before the null test, writes the
//! null tag at slot 5, and otherwise invokes the writer at cursor 4. The late-flags holder
//! variant retains a separate cursor and appends `flag | 0 | error << 1 | second << 2`
//! after the branch, reading the captured removal flag directly in the low lane. Each uses
//! its own real factories and parent registration.
use sha2::{Digest, Sha256};
use shape::naming::name_threads;

const NAME: &str = "emitStorageRemovalSignal";

const BODY: &str = r#"function $THREAD_2_111500($BV) {
						try {
							if ($BV.key === null || $BV.key === $KEY_39 && $BV.newValue === null) {
								$KEY_15 = true;
								$KEY_6 = $BV.oldValue ?? null;
								let $BW = Math.random() * 4294967296 | 0;
								let $BX = [$BW >>> 24, $BW >> 16 & 255, $BW >> 8 & 255, $BW & 255];
								$BW = $KEY_282($BW ^ 585606060);
								let $BY = $KEY_327($BW);
								let $BZ = $KEY_15;
								let $CA = $KEY_6;
								let $CB;
								if ($CA == null) {
									$BX[4] = $BW() ^ 130;
									$CB = 4;
								} else {
									$BY = [$BY, $CA, $BX];
									$CB = $BY[0]($BY[1], $BY[2], 3);
								}
								$BX[$CB + 1] = ($BZ | 0 | $KEY_8 << 1 | $KEY_2 << 2) ^ $BW();
								$KEY_0.next([[], $BX]);
							}
						} catch (e) {
							$KEY_8 = true;
							let $CC = Math.random() * 4294967296;
							let $CD = [($CC | 0) >>> 24, ($CC | 0) >> 16 & 255, ($CC | 0) >> 8 & 255, ($CC | 0) & 255];
							$CC = $KEY_282(($CC | 0) ^ 585606060);
							let $CE = $KEY_327($CC);
							let $CF = $KEY_15;
							let $CG = $KEY_6;
							let $CH;
							if ($CG == null) {
								$CD[4] = $CC() ^ 130;
								$CH = 4;
							} else {
								$CE = [$CE, $CG, $CD];
								$CH = $CE[0]($CE[1], $CE[2], 3);
							}
							$CD[$CH + 1] = ($CF | 0 | $KEY_8 << 1 | $KEY_2 << 2) ^ $CC();
							$KEY_0.next([[], $CD]);
						}
					}"#;

const POSITIVE: &str = r#"function main(){
let $KEY_39 = 0;
let $KEY_15 = false;
let $KEY_6 = null;
let $KEY_8 = false;
let $KEY_2 = false;
let $KEY_0 = { next(value){ return value; } };
let $KEY_232 = 0;
function $KEY_323($k, $o, $b, $p) { return $p; }
function $KEY_218($c, $i) { return 0; }
function $THREAD_5_99534($BH) {
    let $KEY_0, $KEY_1, $KEY_2, $KEY_4;
    function $THREAD_1_68323() {
        let $BI = $KEY_1;
        $KEY_1 = ($BI + 1) % 4;
        if (($BI + 1) % 4 === 0) {
            $KEY_0 = $KEY_2 + $KEY_4;
            let $BJ = $KEY_4;
            $KEY_4 = $BJ ^ $KEY_2;
            $KEY_2 = $BJ ^ $KEY_2 ^ ($KEY_2 << 26 | $KEY_2 >>> 6) ^ $KEY_4 << 9;
            $KEY_4 = $KEY_4 << 13 | $KEY_4 >>> 19;
            return $KEY_0 & 255;
        }
        if (($BI + 1) % 4 === 1) {
            return $KEY_0 >> 8 & 255;
        }
        if (($BI + 1) % 4 === 2) {
            return $KEY_0 >> 16 & 255;
        }
        if (($BI + 1) % 4 === 3) {
            return $KEY_0 >>> 24 & 255;
        } else {
            return void 0;
        }
    }
    $KEY_2 = $KEY_232;
    $KEY_4 = $BH;
    $KEY_1 = -1;
    $THREAD_1_68323();
    $THREAD_1_68323();
    $THREAD_1_68323();
    $THREAD_1_68323();
    return $THREAD_1_68323;
}
function $THREAD_4_126358($BH) {
    let $KEY_2 = $BH;
    let $KEY_0 = Object.create(null);
    let $KEY_1 = 0;
    function $THREAD_7_93919($BI, $BJ, $BK) {
        let $KEY_3;
        if ($KEY_0[$BI] != null) {
            $KEY_3 = $BK;
            $KEY_3 = Number($BK) + 1;
            $BJ[$KEY_3] = $KEY_2() ^ 129;
            $KEY_3 = $KEY_323($KEY_2, $KEY_0[$BI], $BJ, $KEY_3);
        } else {
            let $BL = $BI.length;
            let $BM = 0;
            let $BN = $BK;
            while ($BM < $BL) {
                let $BO = $KEY_218($BI[$BM], 0);
                if ($BO === 0) {
                    $BJ[++$BN] = $KEY_2() ^ 192;
                    $BO = 128;
                } else if (!($BO < 128)) if ($BO < 2048) {
                    $BJ[++$BN] = ($BO >> 6 | 192) ^ $KEY_2();
                    $BO = $BO & 63 | 128;
                } else {
                    let $BP = $BO >= 55296 && $BO <= 57343 && $BO < 56320 && $BM + 1 < $BL ? $KEY_218($BI[$BM + 1], 0) : null;
                    if ($BP == null || $BP < 56320 || $BP > 57343) {
                        $BJ[++$BN] = ($BO >> 12 | 224) ^ $KEY_2();
                        $BJ[++$BN] = ($BO >> 6 & 63 | 128) ^ $KEY_2();
                        $BO = $BO & 63 | 128;
                    } else {
                        ++$BM;
                        $BJ[++$BN] = (65536 + ($BO - 55296 << 10) + ($BP - 56320) >> 18 | 240) ^ $KEY_2();
                        $BJ[++$BN] = (65536 + ($BO - 55296 << 10) + ($BP - 56320) >> 12 & 63 | 128) ^ $KEY_2();
                        $BJ[++$BN] = (65536 + ($BO - 55296 << 10) + ($BP - 56320) >> 6 & 63 | 128) ^ $KEY_2();
                        $BO = 65536 + ($BO - 55296 << 10) + ($BP - 56320) & 63 | 128;
                    }
                }
                $BJ[++$BN] = $BO ^ $KEY_2();
                ++$BM;
            }
            $KEY_3 = Number($BN) + 1;
            $BJ[$KEY_3] = $KEY_2();
            let $BQ = $KEY_0;
            let $BR = Number($KEY_1);
            $KEY_1 = $BR + 1;
            $BQ[$BI] = $BR;
        }
        return $KEY_3;
    }
    return $THREAD_7_93919;
}
function $THREAD_4_126359($BH) {
function $THREAD_7_93920($BI, $BJ, $BK) { return $BJ; }
return $THREAD_7_93920;
}
let $KEY_282 = $THREAD_5_99534;
let $KEY_327 = $THREAD_4_126358;
let $KEY_330 = $THREAD_4_126359;
function $THREAD_2_111500($BV) {
						try {
							if ($BV.key === null || $BV.key === $KEY_39 && $BV.newValue === null) {
								$KEY_15 = true;
								$KEY_6 = $BV.oldValue ?? null;
								let $BW = Math.random() * 4294967296 | 0;
								let $BX = [$BW >>> 24, $BW >> 16 & 255, $BW >> 8 & 255, $BW & 255];
								$BW = $KEY_282($BW ^ 585606060);
								let $BY = $KEY_327($BW);
								let $BZ = $KEY_15;
								let $CA = $KEY_6;
								let $CB;
								if ($CA == null) {
									$BX[4] = $BW() ^ 130;
									$CB = 4;
								} else {
									$BY = [$BY, $CA, $BX];
									$CB = $BY[0]($BY[1], $BY[2], 3);
								}
								$BX[$CB + 1] = ($BZ | 0 | $KEY_8 << 1 | $KEY_2 << 2) ^ $BW();
								$KEY_0.next([[], $BX]);
							}
						} catch (e) {
							$KEY_8 = true;
							let $CC = Math.random() * 4294967296;
							let $CD = [($CC | 0) >>> 24, ($CC | 0) >> 16 & 255, ($CC | 0) >> 8 & 255, ($CC | 0) & 255];
							$CC = $KEY_282(($CC | 0) ^ 585606060);
							let $CE = $KEY_327($CC);
							let $CF = $KEY_15;
							let $CG = $KEY_6;
							let $CH;
							if ($CG == null) {
								$CD[4] = $CC() ^ 130;
								$CH = 4;
							} else {
								$CE = [$CE, $CG, $CD];
								$CH = $CE[0]($CE[1], $CE[2], 3);
							}
							$CD[$CH + 1] = ($CF | 0 | $KEY_8 << 1 | $KEY_2 << 2) ^ $CC();
							$KEY_0.next([[], $CD]);
						}
					}
}
main();"#;

const ALPHA: &str = r#"function main(){
let $KEY_900 = 0;
let $KEY_901 = false;
let $KEY_902 = null;
let $KEY_903 = false;
let $KEY_904 = false;
let $KEY_905 = { next(value){ return value; } };
let $KEY_232 = 0;
function $KEY_323($k, $o, $b, $p) { return $p; }
function $KEY_218($c, $i) { return 0; }
function $THREAD_80_1($BH) {
    let $KEY_0, $KEY_1, $KEY_2, $KEY_4;
    function $THREAD_80_2() {
        let $BI = $KEY_1;
        $KEY_1 = ($BI + 1) % 4;
        if (($BI + 1) % 4 === 0) {
            $KEY_0 = $KEY_2 + $KEY_4;
            let $BJ = $KEY_4;
            $KEY_4 = $BJ ^ $KEY_2;
            $KEY_2 = $BJ ^ $KEY_2 ^ ($KEY_2 << 26 | $KEY_2 >>> 6) ^ $KEY_4 << 9;
            $KEY_4 = $KEY_4 << 13 | $KEY_4 >>> 19;
            return $KEY_0 & 255;
        }
        if (($BI + 1) % 4 === 1) {
            return $KEY_0 >> 8 & 255;
        }
        if (($BI + 1) % 4 === 2) {
            return $KEY_0 >> 16 & 255;
        }
        if (($BI + 1) % 4 === 3) {
            return $KEY_0 >>> 24 & 255;
        } else {
            return void 0;
        }
    }
    $KEY_2 = $KEY_232;
    $KEY_4 = $BH;
    $KEY_1 = -1;
    $THREAD_80_2();
    $THREAD_80_2();
    $THREAD_80_2();
    $THREAD_80_2();
    return $THREAD_80_2;
}
function $THREAD_80_3($BH) {
    let $KEY_2 = $BH;
    let $KEY_0 = Object.create(null);
    let $KEY_1 = 0;
    function $THREAD_80_4($BI, $BJ, $BK) {
        let $KEY_3;
        if ($KEY_0[$BI] != null) {
            $KEY_3 = $BK;
            $KEY_3 = Number($BK) + 1;
            $BJ[$KEY_3] = $KEY_2() ^ 129;
            $KEY_3 = $KEY_323($KEY_2, $KEY_0[$BI], $BJ, $KEY_3);
        } else {
            let $BL = $BI.length;
            let $BM = 0;
            let $BN = $BK;
            while ($BM < $BL) {
                let $BO = $KEY_218($BI[$BM], 0);
                if ($BO === 0) {
                    $BJ[++$BN] = $KEY_2() ^ 192;
                    $BO = 128;
                } else if (!($BO < 128)) if ($BO < 2048) {
                    $BJ[++$BN] = ($BO >> 6 | 192) ^ $KEY_2();
                    $BO = $BO & 63 | 128;
                } else {
                    let $BP = $BO >= 55296 && $BO <= 57343 && $BO < 56320 && $BM + 1 < $BL ? $KEY_218($BI[$BM + 1], 0) : null;
                    if ($BP == null || $BP < 56320 || $BP > 57343) {
                        $BJ[++$BN] = ($BO >> 12 | 224) ^ $KEY_2();
                        $BJ[++$BN] = ($BO >> 6 & 63 | 128) ^ $KEY_2();
                        $BO = $BO & 63 | 128;
                    } else {
                        ++$BM;
                        $BJ[++$BN] = (65536 + ($BO - 55296 << 10) + ($BP - 56320) >> 18 | 240) ^ $KEY_2();
                        $BJ[++$BN] = (65536 + ($BO - 55296 << 10) + ($BP - 56320) >> 12 & 63 | 128) ^ $KEY_2();
                        $BJ[++$BN] = (65536 + ($BO - 55296 << 10) + ($BP - 56320) >> 6 & 63 | 128) ^ $KEY_2();
                        $BO = 65536 + ($BO - 55296 << 10) + ($BP - 56320) & 63 | 128;
                    }
                }
                $BJ[++$BN] = $BO ^ $KEY_2();
                ++$BM;
            }
            $KEY_3 = Number($BN) + 1;
            $BJ[$KEY_3] = $KEY_2();
            let $BQ = $KEY_0;
            let $BR = Number($KEY_1);
            $KEY_1 = $BR + 1;
            $BQ[$BI] = $BR;
        }
        return $KEY_3;
    }
    return $THREAD_80_4;
}
function $THREAD_80_5($BH) {
function $THREAD_80_6($BI, $BJ, $BK) { return $BJ; }
return $THREAD_80_6;
}
let $KEY_906 = $THREAD_80_1;
let $KEY_907 = $THREAD_80_3;
let $KEY_908 = $THREAD_80_5;
function $THREAD_91_2($ZA) {
						try {
							if ($ZA.key === null || $ZA.key === $KEY_900 && $ZA.newValue === null) {
								$KEY_901 = true;
								$KEY_902 = $ZA.oldValue ?? null;
								let $ZB = Math.random() * 4294967296 | 0;
								let $ZC = [$ZB >>> 24, $ZB >> 16 & 255, $ZB >> 8 & 255, $ZB & 255];
								$ZB = $KEY_906($ZB ^ 585606060);
								let $ZD = $KEY_907($ZB);
								let $ZE = $KEY_901;
								let $ZF = $KEY_902;
								let $ZG;
								if ($ZF == null) {
									$ZC[4] = $ZB() ^ 130;
									$ZG = 4;
								} else {
									$ZD = [$ZD, $ZF, $ZC];
									$ZG = $ZD[0]($ZD[1], $ZD[2], 3);
								}
								$ZC[$ZG + 1] = ($ZE | 0 | $KEY_903 << 1 | $KEY_904 << 2) ^ $ZB();
								$KEY_905.next([[], $ZC]);
							}
						} catch (e) {
							$KEY_903 = true;
							let $ZH = Math.random() * 4294967296;
							let $ZI = [($ZH | 0) >>> 24, ($ZH | 0) >> 16 & 255, ($ZH | 0) >> 8 & 255, ($ZH | 0) & 255];
							$ZH = $KEY_906(($ZH | 0) ^ 585606060);
							let $ZJ = $KEY_907($ZH);
							let $ZK = $KEY_901;
							let $ZL = $KEY_902;
							let $ZM;
							if ($ZL == null) {
								$ZI[4] = $ZH() ^ 130;
								$ZM = 4;
							} else {
								$ZJ = [$ZJ, $ZL, $ZI];
								$ZM = $ZJ[0]($ZJ[1], $ZJ[2], 3);
							}
							$ZI[$ZM + 1] = ($ZK | 0 | $KEY_903 << 1 | $KEY_904 << 2) ^ $ZH();
							$KEY_905.next([[], $ZI]);
						}
					}
}
main();"#;

const READABLE: &str = r#"function main(){
let $KEY_39 = 0;
let $KEY_15 = false;
let $KEY_6 = null;
let $KEY_8 = false;
let $KEY_2 = false;
let $KEY_0 = { next(value){ return value; } };
let $KEY_232 = 0;
function $KEY_323($k, $o, $b, $p) { return $p; }
function $KEY_218($c, $i) { return 0; }
function $THREAD_5_99534($BH) {
    let $KEY_0, $KEY_1, $KEY_2, $KEY_4;
    function $THREAD_1_68323() {
        let $BI = $KEY_1;
        $KEY_1 = ($BI + 1) % 4;
        if (($BI + 1) % 4 === 0) {
            $KEY_0 = $KEY_2 + $KEY_4;
            let $BJ = $KEY_4;
            $KEY_4 = $BJ ^ $KEY_2;
            $KEY_2 = $BJ ^ $KEY_2 ^ ($KEY_2 << 26 | $KEY_2 >>> 6) ^ $KEY_4 << 9;
            $KEY_4 = $KEY_4 << 13 | $KEY_4 >>> 19;
            return $KEY_0 & 255;
        }
        if (($BI + 1) % 4 === 1) {
            return $KEY_0 >> 8 & 255;
        }
        if (($BI + 1) % 4 === 2) {
            return $KEY_0 >> 16 & 255;
        }
        if (($BI + 1) % 4 === 3) {
            return $KEY_0 >>> 24 & 255;
        } else {
            return void 0;
        }
    }
    $KEY_2 = $KEY_232;
    $KEY_4 = $BH;
    $KEY_1 = -1;
    $THREAD_1_68323();
    $THREAD_1_68323();
    $THREAD_1_68323();
    $THREAD_1_68323();
    return $THREAD_1_68323;
}
function $THREAD_4_126358($BH) {
    let $KEY_2 = $BH;
    let $KEY_0 = Object.create(null);
    let $KEY_1 = 0;
    function $THREAD_7_93919($BI, $BJ, $BK) {
        let $KEY_3;
        if ($KEY_0[$BI] != null) {
            $KEY_3 = $BK;
            $KEY_3 = Number($BK) + 1;
            $BJ[$KEY_3] = $KEY_2() ^ 129;
            $KEY_3 = $KEY_323($KEY_2, $KEY_0[$BI], $BJ, $KEY_3);
        } else {
            let $BL = $BI.length;
            let $BM = 0;
            let $BN = $BK;
            while ($BM < $BL) {
                let $BO = $KEY_218($BI[$BM], 0);
                if ($BO === 0) {
                    $BJ[++$BN] = $KEY_2() ^ 192;
                    $BO = 128;
                } else if (!($BO < 128)) if ($BO < 2048) {
                    $BJ[++$BN] = ($BO >> 6 | 192) ^ $KEY_2();
                    $BO = $BO & 63 | 128;
                } else {
                    let $BP = $BO >= 55296 && $BO <= 57343 && $BO < 56320 && $BM + 1 < $BL ? $KEY_218($BI[$BM + 1], 0) : null;
                    if ($BP == null || $BP < 56320 || $BP > 57343) {
                        $BJ[++$BN] = ($BO >> 12 | 224) ^ $KEY_2();
                        $BJ[++$BN] = ($BO >> 6 & 63 | 128) ^ $KEY_2();
                        $BO = $BO & 63 | 128;
                    } else {
                        ++$BM;
                        $BJ[++$BN] = (65536 + ($BO - 55296 << 10) + ($BP - 56320) >> 18 | 240) ^ $KEY_2();
                        $BJ[++$BN] = (65536 + ($BO - 55296 << 10) + ($BP - 56320) >> 12 & 63 | 128) ^ $KEY_2();
                        $BJ[++$BN] = (65536 + ($BO - 55296 << 10) + ($BP - 56320) >> 6 & 63 | 128) ^ $KEY_2();
                        $BO = 65536 + ($BO - 55296 << 10) + ($BP - 56320) & 63 | 128;
                    }
                }
                $BJ[++$BN] = $BO ^ $KEY_2();
                ++$BM;
            }
            $KEY_3 = Number($BN) + 1;
            $BJ[$KEY_3] = $KEY_2();
            let $BQ = $KEY_0;
            let $BR = Number($KEY_1);
            $KEY_1 = $BR + 1;
            $BQ[$BI] = $BR;
        }
        return $KEY_3;
    }
    return $THREAD_7_93919;
}
function $THREAD_4_126359($BH) {
function $THREAD_7_93920($BI, $BJ, $BK) { return $BJ; }
return $THREAD_7_93920;
}
let $KEY_282 = $THREAD_5_99534;
let $KEY_327 = $THREAD_4_126358;
let $KEY_330 = $THREAD_4_126359;
function $THREAD_2_111500(event) {
						try {
							if (event.key === null || event.key === $KEY_39 && event.newValue === null) {
								$KEY_15 = true;
								$KEY_6 = event.oldValue ?? null;
								let nonce = Math.random() * 4294967296 | 0;
								let header = [nonce >>> 24, nonce >> 16 & 255, nonce >> 8 & 255, nonce & 255];
								nonce = $KEY_282(nonce ^ 585606060);
								let writer = $KEY_327(nonce);
								let removalAlias = $KEY_15;
								let oldAlias = $KEY_6;
								let cursor;
								if (oldAlias == null) {
									header[4] = nonce() ^ 130;
									cursor = 4;
								} else {
									writer = [writer, oldAlias, header];
									cursor = writer[0](writer[1], writer[2], 3);
								}
								header[cursor + 1] = (removalAlias | 0 | $KEY_8 << 1 | $KEY_2 << 2) ^ nonce();
								$KEY_0.next([[], header]);
							}
						} catch (e) {
							$KEY_8 = true;
							let catchNonce = Math.random() * 4294967296;
							let catchHeader = [(catchNonce | 0) >>> 24, (catchNonce | 0) >> 16 & 255, (catchNonce | 0) >> 8 & 255, (catchNonce | 0) & 255];
							catchNonce = $KEY_282((catchNonce | 0) ^ 585606060);
							let catchWriter = $KEY_327(catchNonce);
							let catchRemoval = $KEY_15;
							let catchOld = $KEY_6;
							let catchCursor;
							if (catchOld == null) {
								catchHeader[4] = catchNonce() ^ 130;
								catchCursor = 4;
							} else {
								catchWriter = [catchWriter, catchOld, catchHeader];
								catchCursor = catchWriter[0](catchWriter[1], catchWriter[2], 3);
							}
							catchHeader[catchCursor + 1] = (catchRemoval | 0 | $KEY_8 << 1 | $KEY_2 << 2) ^ catchNonce();
							$KEY_0.next([[], catchHeader]);
						}
					}
}
main();"#;

const HOLDER: &str = r#"function main(){
let $KEY_39 = 0;
let $KEY_15 = false;
let $KEY_6 = null;
let $KEY_8 = false;
let $KEY_2 = false;
let $KEY_0 = { next(value){ return value; } };
let $KEY_232 = 0;
function $KEY_323($k, $o, $b, $p) { return $p; }
function $KEY_218($c, $i) { return 0; }
function $THREAD_5_99534($BH) {
    let $KEY_0, $KEY_1, $KEY_2, $KEY_4;
    function $THREAD_1_68323() {
        let $BI = $KEY_1;
        $KEY_1 = ($BI + 1) % 4;
        if (($BI + 1) % 4 === 0) {
            $KEY_0 = $KEY_2 + $KEY_4;
            let $BJ = $KEY_4;
            $KEY_4 = $BJ ^ $KEY_2;
            $KEY_2 = $BJ ^ $KEY_2 ^ ($KEY_2 << 26 | $KEY_2 >>> 6) ^ $KEY_4 << 9;
            $KEY_4 = $KEY_4 << 13 | $KEY_4 >>> 19;
            return $KEY_0 & 255;
        }
        if (($BI + 1) % 4 === 1) {
            return $KEY_0 >> 8 & 255;
        }
        if (($BI + 1) % 4 === 2) {
            return $KEY_0 >> 16 & 255;
        }
        if (($BI + 1) % 4 === 3) {
            return $KEY_0 >>> 24 & 255;
        } else {
            return void 0;
        }
    }
    $KEY_2 = $KEY_232;
    $KEY_4 = $BH;
    $KEY_1 = -1;
    $THREAD_1_68323();
    $THREAD_1_68323();
    $THREAD_1_68323();
    $THREAD_1_68323();
    return $THREAD_1_68323;
}
function $THREAD_4_126358($BH) {
    let $KEY_2 = $BH;
    let $KEY_0 = Object.create(null);
    let $KEY_1 = 0;
    function $THREAD_7_93919($BI, $BJ, $BK) {
        let $KEY_3;
        if ($KEY_0[$BI] != null) {
            $KEY_3 = $BK;
            $KEY_3 = Number($BK) + 1;
            $BJ[$KEY_3] = $KEY_2() ^ 129;
            $KEY_3 = $KEY_323($KEY_2, $KEY_0[$BI], $BJ, $KEY_3);
        } else {
            let $BL = $BI.length;
            let $BM = 0;
            let $BN = $BK;
            while ($BM < $BL) {
                let $BO = $KEY_218($BI[$BM], 0);
                if ($BO === 0) {
                    $BJ[++$BN] = $KEY_2() ^ 192;
                    $BO = 128;
                } else if (!($BO < 128)) if ($BO < 2048) {
                    $BJ[++$BN] = ($BO >> 6 | 192) ^ $KEY_2();
                    $BO = $BO & 63 | 128;
                } else {
                    let $BP = $BO >= 55296 && $BO <= 57343 && $BO < 56320 && $BM + 1 < $BL ? $KEY_218($BI[$BM + 1], 0) : null;
                    if ($BP == null || $BP < 56320 || $BP > 57343) {
                        $BJ[++$BN] = ($BO >> 12 | 224) ^ $KEY_2();
                        $BJ[++$BN] = ($BO >> 6 & 63 | 128) ^ $KEY_2();
                        $BO = $BO & 63 | 128;
                    } else {
                        ++$BM;
                        $BJ[++$BN] = (65536 + ($BO - 55296 << 10) + ($BP - 56320) >> 18 | 240) ^ $KEY_2();
                        $BJ[++$BN] = (65536 + ($BO - 55296 << 10) + ($BP - 56320) >> 12 & 63 | 128) ^ $KEY_2();
                        $BJ[++$BN] = (65536 + ($BO - 55296 << 10) + ($BP - 56320) >> 6 & 63 | 128) ^ $KEY_2();
                        $BO = 65536 + ($BO - 55296 << 10) + ($BP - 56320) & 63 | 128;
                    }
                }
                $BJ[++$BN] = $BO ^ $KEY_2();
                ++$BM;
            }
            $KEY_3 = Number($BN) + 1;
            $BJ[$KEY_3] = $KEY_2();
            let $BQ = $KEY_0;
            let $BR = Number($KEY_1);
            $KEY_1 = $BR + 1;
            $BQ[$BI] = $BR;
        }
        return $KEY_3;
    }
    return $THREAD_7_93919;
}
function $THREAD_4_126359($BH) {
function $THREAD_7_93920($BI, $BJ, $BK) { return $BJ; }
return $THREAD_7_93920;
}
let $KEY_282 = $THREAD_5_99534;
let $KEY_327 = $THREAD_4_126358;
let $KEY_330 = $THREAD_4_126359;
function $THREAD_2_111500($BV) {
						try {
							if ($BV.key == null || $BV.key === $KEY_39 && $BV.newValue == null) {
								$KEY_15 = true;
								$KEY_6 = $BV.oldValue ?? null;
								let $BW = Math.random() * 4294967296 | 0;
								let $BX = [$BW >>> 24, $BW >> 16 & 255, $BW >> 8 & 255, $BW & 255];
								$BW = $KEY_282($BW ^ 777777777);
								let $BY = $KEY_327($BW);
								let $BZ = $KEY_15;
								let $CA = $KEY_6;
								let $CB;
								if ($CA == null) {
									$BX[4] = $BW() ^ 129;
									$CB = 4;
								} else {
									$BY = [$BY, $CA, $BX];
									$CB = $BY[0]($BY[1], $BY[2], 3);
								}
								$BX[$CB + 1] = ($BZ | 0 | $KEY_8 << 1 | $KEY_2 << 2) ^ $BW();
								$KEY_0.next([[], $BX]);
							}
						} catch (e) {
							$KEY_8 = true;
							let $CC = Math.random() * 4294967296;
							let $CD = [($CC | 0) >>> 24, ($CC | 0) >> 16 & 255, ($CC | 0) >> 8 & 255, ($CC | 0) & 255];
							$CC = $KEY_282(($CC | 0) ^ 777777777);
							let $CE = $KEY_327($CC);
							let $CF = $KEY_15;
							let $CG = $KEY_6;
							let $CH;
							if ($CG == null) {
								$CD[4] = $CC() ^ 129;
								$CH = 4;
							} else {
								$CE = [$CE, $CG, $CD];
								$CH = $CE[0]($CE[1], $CE[2], 3);
							}
							$CD[$CH + 1] = ($CF | 0 | $KEY_8 << 1 | $KEY_2 << 2) ^ $CC();
							$KEY_0.next([[], $CD]);
						}
					}
}
main();"#;

const NEIGHBOR: &str = r#"function main(){
let $KEY_0 = { next(value){ return value; } };
let $KEY_232 = 0;
function $KEY_323($k, $o, $b, $p) { return $p; }
function $KEY_218($c, $i) { return 0; }
function $THREAD_5_99534($BH) {
function $THREAD_1_68323() { return $BH & 255; }
return $THREAD_1_68323;
}
function $THREAD_4_126358($BH) {
function $THREAD_7_93919($BI, $BJ, $BK) { return $BK; }
return $THREAD_7_93919;
}
let $KEY_282 = $THREAD_5_99534;
let $KEY_327 = $THREAD_4_126358;
function $THREAD_9_1($BV) {
if ($BV != null) {
let $BW = Math.random() * 4294967296;
let $BX = [$BW >>> 24, $BW >> 16 & 255, $BW >> 8 & 255, $BW & 255];
let $BY = $KEY_282($BW ^ 1);
let $BZ = $KEY_327($BY);
$KEY_0.next([[], $BX]);
}
$KEY_0.complete();
}
}
main();"#;

fn sha(bytes: impl AsRef<[u8]>) -> String {
    Sha256::digest(bytes.as_ref())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn named(source: &str, target: &str, name: &str) -> bool {
    name_threads(source, true).is_ok_and(|report| {
        report.report.outcomes.iter().any(|outcome| {
            outcome.applied
                && outcome.binding.name == target
                && outcome.proposed_name.as_deref() == Some(name)
        })
    })
}

/// Compact outcome trace used only in assertion messages so a refusal shows
/// whether the family declined or another evaluator claimed the binding.
fn outcome_trace(source: &str, target: &str) -> String {
    let Ok(report) = name_threads(source, true) else {
        return format!("{target}: analysis error");
    };
    let rows: Vec<String> = report
        .report
        .outcomes
        .iter()
        .filter(|outcome| outcome.binding.name == target)
        .map(|outcome| {
            format!(
                "applied={} proposed={:?} evaluator={:?} reason={:?}",
                outcome.applied, outcome.proposed_name, outcome.evaluator, outcome.reason
            )
        })
        .collect();
    format!("{target}: [{}]", rows.join("; "))
}

/// A negative source must still parse and analyze; only the naming proof is
/// expected to fail. This keeps refusal tests from passing vacuously.
fn refused(source: &str, target: &str, name: &str) {
    let result = name_threads(source, true).unwrap_or_else(|error| {
        panic!("negative source must parse and analyze: {error}\n{source}")
    });
    let outcome = result
        .report
        .outcomes
        .iter()
        .find(|outcome| outcome.binding.name == target)
        .unwrap_or_else(|| panic!("target binding {target} missing from outcomes:\n{source}"));
    assert!(
        !(outcome.applied && outcome.proposed_name.as_deref() == Some(name)),
        "accepted {target} as {name}:\n{source}"
    );
}

fn refuses(template: &str, target: &str, name: &str, changes: &[(&str, &str)]) {
    for &(from, to) in changes {
        assert!(template.contains(from), "missing replacement {from}");
        refused(&template.replace(from, to), target, name);
    }
}

#[test]
fn holder_cursor_storage_removal_callback_is_named() {
    assert_eq!(BODY.len(), 1360, "witness body must be the exact 1360-byte slice");
    assert_eq!(
        sha(BODY.as_bytes()),
        "90d88d4e9b0e70b374fa4fee7df9990794aa97703a9387a64aad533ad346e71b",
        "witness body SHA-256 must match the sealed computed-gate slice"
    );
    assert!(POSITIVE.contains(BODY), "positive must embed the verbatim body");
    assert!(
        named(POSITIVE, "$THREAD_2_111500", NAME),
        "actual storage-removal callback was not named:\n{}",
        outcome_trace(POSITIVE, "$THREAD_2_111500")
    );
}

#[test]
fn alpha_renamed_spelling_keeps_the_proof() {
    assert!(
        named(ALPHA, "$THREAD_91_2", NAME),
        "alpha spelling was not named:\n{}",
        outcome_trace(ALPHA, "$THREAD_91_2")
    );
}

#[test]
fn readable_spellings_keep_the_proof() {
    assert!(
        named(READABLE, "$THREAD_2_111500", NAME),
        "readable spelling was not named:\n{}",
        outcome_trace(READABLE, "$THREAD_2_111500")
    );
}

#[test]
fn holder_and_streaming_variants_keep_the_proof() {
    assert!(
        named(HOLDER, "$THREAD_2_111500", NAME),
        "holder/literal variant was not named:\n{}",
        outcome_trace(HOLDER, "$THREAD_2_111500")
    );
    // The family is the streaming-update variant: the witness never completes.
    assert!(
        !POSITIVE.contains("complete"),
        "the accepted witness must be the no-complete streaming variant"
    );
}

#[test]
fn the_rule_needs_the_storage_guard_and_old_value() {
    refuses(
        POSITIVE,
        "$THREAD_2_111500",
        NAME,
        &[
            ("$BV.key === null", "$BV.key !== null"),
            ("$BV.key === $KEY_39", "$BV.key === $BV.oldValue"),
            ("$BV.key === $KEY_39", "$BV.key === $BV.key"),
            ("$BV.newValue === null", "$BV.newValue !== null"),
            ("$BV.oldValue ?? null", "$BV.newValue ?? null"),
        ],
    );
}

#[test]
fn the_rule_needs_the_nonce_and_header_dataflow() {
    refuses(
        POSITIVE,
        "$THREAD_2_111500",
        NAME,
        &[
            ("Math.random() * 4294967296 | 0", "Math.random() * 4294967295 | 0"),
            ("$BW >>> 24", "$BW >>> 16"),
            ("$BW ^ 585606060", "$BW ^ 585606061"),
            ("Math.random() * 4294967296;", "Math.random() * 4294967295;"),
            ("$CC | 0", "$CC >> 0"),
            ("$BX[4] = $BW() ^ 130;", "$BX[3] = $BW() ^ 130;"),
            ("$BX[$CB + 1] =", "$BX[$CB + 2] ="),
            ("($CC | 0) >>> 24", "($CC | 0) >>> 8"),
        ],
    );
}

#[test]
fn the_rule_needs_the_source_backed_writer_and_holder() {
    refuses(
        POSITIVE,
        "$THREAD_2_111500",
        NAME,
        &[
            ("let $BY = $KEY_327($BW);", "let $BY = $KEY_327($KEY_2);"),
            ("$BY = [$BY, $CA, $BX];", "$BY = [$BY, $BX, $CA];"),
            ("$BY[0]($BY[1], $BY[2], 3)", "$BY[0]($BY[1], $BY[2], 2)"),
            ("let $CE = $KEY_327($CC);", "let $CE = $KEY_330($CC);"),
        ],
    );
}

#[test]
fn the_rule_needs_the_same_captured_observer_flags_and_cursor() {
    refuses(
        POSITIVE,
        "$THREAD_2_111500",
        NAME,
        &[
            ("$KEY_0.next([[], $CD]);", "$KEY_2.next([[], $CD]);"),
            ("$KEY_0.next([[], $BX]);", "$KEY_15.next([[], $BX]);"),
            ("$KEY_15 = true;", "$KEY_15 = false;"),
            ("$KEY_8 = true;", "$KEY_8 = false;"),
            ("($BZ | 0 | $KEY_8 << 1 | $KEY_2 << 2)", "($BZ | 0 | $KEY_6 << 1 | $KEY_2 << 2)"),
            ("($BZ | 0 | $KEY_8 << 1 | $KEY_2 << 2)", "($BZ | 0 | $KEY_8 << 1 | $KEY_6 << 2)"),
            ("($CF | 0 | $KEY_8 << 1 | $KEY_2 << 2)", "($CF | 0 | $KEY_6 << 1 | $KEY_2 << 2)"),
            ("$KEY_8 = true;", "$KEY_2 = true;"),
            ("$CB = 4;", "$CB = 5;"),
            ("$BY[1], $BY[2], 3", "$BY[1], $BY[2], 4"),
            ("$CD[4] = $CC() ^ 130;", "$CD[5] = $CC() ^ 130;"),
        ],
    );
}

#[test]
fn borrowed_factories_and_unused_donors_are_not_claimed() {
    // A frame whose catch borrows an unused donor writer factory is not the
    // same ordered frame, and the unused donor itself is never claimed.
    refuses(
        POSITIVE,
        "$THREAD_2_111500",
        NAME,
        &[
            ("let $CE = $KEY_327($CC);", "let $CE = $KEY_330($CC);"),
            ("let $BY = $KEY_327($BW);", "let $BY = $KEY_330($BW);"),
        ],
    );
    refused(POSITIVE, "$THREAD_4_126359", NAME);
    refused(POSITIVE, "$THREAD_7_93920", NAME);
}

#[test]
fn streaming_update_must_not_complete() {
    refuses(
        POSITIVE,
        "$THREAD_2_111500",
        NAME,
        &[
            ("$KEY_0.next([[], $CD]);", "$KEY_0.next([[], $CD]);\n$KEY_0.complete();"),
            ("$KEY_0.next([[], $BX]);", "$KEY_0.next([[], $BX]);\n$KEY_0.complete();"),
        ],
    );
}

#[test]
fn neighbor_seeded_emitter_is_not_routed_here() {
    // The neighbour shares the header/nonce/observer vocabulary and completes,
    // so it must stay with the seeded-emitter family, not this storage one.
    assert!(NEIGHBOR.contains("complete"));
    refused(NEIGHBOR, "$THREAD_9_1", NAME);
}

/// Swap both factories at both call sites in both frames while preserving captured aliases.
/// The seed site receives the writer factory and the writer site receives the seed factory.
fn swap_factory_sites(source: &str) -> String {
    source
        .replace(
            "$BW = $KEY_282($BW ^ 585606060);",
            "$BW = $KEY_327($BW ^ 585606060);",
        )
        .replace("let $BY = $KEY_327($BW);", "let $BY = $KEY_282($BW);")
        .replace(
            "$CC = $KEY_282(($CC | 0) ^ 585606060);",
            "$CC = $KEY_327(($CC | 0) ^ 585606060);",
        )
        .replace("let $CE = $KEY_327($CC);", "let $CE = $KEY_282($CC);")
}

#[test]
fn the_seed_factory_must_really_use_its_formal() {
    // An own child that ignores the factory formal is a nested no-op, not a
    // seeded stream. The old last-return-only proof accepted exactly this.
    assert!(POSITIVE.contains("$KEY_4 = $BH;"));
    let ignored = POSITIVE.replace("$KEY_4 = $BH;", "$KEY_4 = $KEY_232;");
    assert_ne!(ignored, POSITIVE);
    assert!(ignored.contains("$KEY_4 = $KEY_232;"));
    refused(&ignored, "$THREAD_2_111500", NAME);
}

#[test]
fn a_dead_seed_mention_is_not_a_live_dependency() {
    // The child mentions `$BH;` as a dead expression statement while the
    // keystream state is seeded from the captured global `$KEY_232`.
    assert!(POSITIVE.contains("        let $BI = $KEY_1;"));
    assert!(POSITIVE.contains("$KEY_4 = $BH;"));
    let decoy = POSITIVE
        .replace(
            "        let $BI = $KEY_1;",
            "        $BH;\n        let $BI = $KEY_1;",
        )
        .replace("$KEY_4 = $BH;", "$KEY_4 = $KEY_232;");
    assert!(decoy.contains("$BH;\n        let $BI = $KEY_1;"));
    assert!(!decoy.contains("$KEY_4 = $BH;"));
    refused(&decoy, "$THREAD_2_111500", NAME);
}

#[test]
fn a_borrowed_global_child_is_not_an_owned_factory_return() {
    // The factory returns a child declared outside its own body, so it cannot
    // have closed over the factory formal.
    let mut borrowed = POSITIVE.replacen(
        "function main(){\n",
        "function main(){\nfunction $THREAD_GLOBAL_CHILD() { return 1; }\n",
        1,
    );
    assert!(borrowed.contains("function $THREAD_GLOBAL_CHILD() { return 1; }"));
    assert!(borrowed.contains("    return $THREAD_1_68323;"));
    borrowed = borrowed.replace(
        "    return $THREAD_1_68323;",
        "    return $THREAD_GLOBAL_CHILD;",
    );
    assert!(borrowed.contains("    return $THREAD_GLOBAL_CHILD;"));
    refused(&borrowed, "$THREAD_2_111500", NAME);
}

#[test]
fn swapping_both_factory_sites_is_refused() {
    // The seed site needs the child reading its seed as data; the writer site needs the child
    // invoking it. Swapping the call sites fails both live-formal-use checks even though both
    // helpers are owned.
    let swapped = swap_factory_sites(POSITIVE);
    assert_ne!(swapped, POSITIVE);
    assert!(swapped.contains("$BW = $KEY_327($BW ^ 585606060);"));
    assert!(swapped.contains("let $BY = $KEY_282($BW);"));
    assert!(swapped.contains("$CC = $KEY_327(($CC | 0) ^ 585606060);"));
    assert!(swapped.contains("let $CE = $KEY_282($CC);"));
    refused(&swapped, "$THREAD_2_111500", NAME);
}

#[test]
fn foreign_factories_at_both_sites_are_refused() {
    // Two distinct foreign factories own a child but never feed that child the
    // factory formal; replacing both real factories must decline.
    let mut foreign = POSITIVE.replacen(
        "function $THREAD_4_126359($BH) {",
        "function $THREAD_4_126360($BH) {\nfunction $THREAD_7_93921($BI, $BJ, $BK) { return $BI; }\nreturn $THREAD_7_93921;\n}\nfunction $THREAD_4_126359($BH) {",
        1,
    );
    assert!(foreign.contains("function $THREAD_4_126360($BH) {"));
    foreign = foreign
        .replace(
            "let $KEY_282 = $THREAD_5_99534;",
            "let $KEY_282 = $THREAD_4_126359;",
        )
        .replace(
            "let $KEY_327 = $THREAD_4_126358;",
            "let $KEY_327 = $THREAD_4_126360;",
        );
    assert!(foreign.contains("let $KEY_282 = $THREAD_4_126359;"));
    assert!(foreign.contains("let $KEY_327 = $THREAD_4_126360;"));
    refused(&foreign, "$THREAD_2_111500", NAME);
}

/// Seed helper state from captured global $KEY_232 instead of formal $BH, then insert decoy
/// into the returned child. Formal mentions that do not feed returned-byte state must not
/// rescue the proof.
fn global_seeded_with(decoy: &str) -> String {
    assert!(POSITIVE.contains("    $KEY_4 = $BH;"));
    assert!(POSITIVE.contains("        let $BI = $KEY_1;"));
    POSITIVE
        .replace("    $KEY_4 = $BH;", "    $KEY_4 = $KEY_232;")
        .replace(
            "        let $BI = $KEY_1;",
            &format!("{decoy}\n        let $BI = $KEY_1;"),
        )
}

#[test]
fn a_dead_local_seed_capture_is_not_a_live_dependency() {
    // The child initializes an unread local from the formal while the real
    // keystream state is seeded from the captured global `$KEY_232`.  An
    // initializer read of the formal is a dead local, not a consumed seed.
    let dead = global_seeded_with("        let $BDEAD = $BH;");
    assert!(dead.contains("let $BDEAD = $BH;"));
    assert!(!dead.contains("    $KEY_4 = $BH;"));
    refused(&dead, "$THREAD_2_111500", NAME);
}

#[test]
fn a_never_called_nested_child_reference_is_not_a_live_dependency() {
    // The formal is only read by a never-called nested function inside the
    // returned child, so the nested donor must not count as a seed capture.
    let nested = global_seeded_with("        function $THREAD_DEAD() { return $BH; }");
    assert!(nested.contains("function $THREAD_DEAD() { return $BH; }"));
    assert!(!nested.contains("    $KEY_4 = $BH;"));
    refused(&nested, "$THREAD_2_111500", NAME);
}

#[test]
fn an_empty_control_test_is_not_a_live_dependency() {
    // A control-flow test alone is not a consumed seed value: the guarded branch
    // is empty and the real state still comes from `$KEY_232`.
    let empty = global_seeded_with("        if ($BH) {}");
    assert!(empty.contains("if ($BH) {}"));
    refused(&empty, "$THREAD_2_111500", NAME);
}

#[test]
fn a_discarded_call_argument_is_not_a_live_dependency() {
    // The formal is only an argument of a call whose result is discarded, so the
    // argument must not count as a consumed seed value.
    let discarded = global_seeded_with("        $KEY_218($BH, 0);");
    assert!(discarded.contains("$KEY_218($BH, 0);"));
    refused(&discarded, "$THREAD_2_111500", NAME);
}

#[test]
fn a_dead_overwritten_alias_is_not_a_live_dependency() {
    // The factory routes the formal into `$KEY_4` and then overwrites it with
    // the captured global before the child runs, so the alias value is dead and
    // the real state is the global.
    assert!(POSITIVE.contains("    $KEY_4 = $BH;"));
    let overwritten = POSITIVE.replace(
        "    $KEY_4 = $BH;",
        "    $KEY_4 = $BH;\n    $KEY_4 = $KEY_232;",
    );
    assert!(overwritten.contains("    $KEY_4 = $BH;\n    $KEY_4 = $KEY_232;"));
    refused(&overwritten, "$THREAD_2_111500", NAME);
}

// Direct storage frame with a shared writer/cursor binding.

/// The writer binding also holds the cursor and is invoked directly. Flag lanes are `aux |
/// 0 | removal << 1 | error << 2`.
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

/// The parent declares the captured key, removal flag, old value, error flag, auxiliary
/// flag, and observer before the callback. The actual seed and writer factories retain
/// their bodies; referenced utility helpers are captured-global stand-ins, as in the holder
/// fixture.
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

/// The real parent registration: the callback is aliased and then registered as a
/// `storage` listener, which is the family's provenance.
const DIRECT_CURSOR_SUFFIX: &str = r#"
let $KEY_5 = $THREAD_2_125597;
let $KEY_8 = { global: { addEventListener(){ return void 0; } } };
$KEY_8.global.addEventListener("storage", $KEY_5);
}
main();"#;

fn direct_cursor_positive() -> String {
    format!("{}{}{}", DIRECT_CURSOR_PREFIX, BODY_DIRECT_CURSOR, DIRECT_CURSOR_SUFFIX)
}

fn identifier_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

/// Token-boundary replacement so `$KEY_16` does not rewrite `$KEY_166` and
/// `$KEY_14` does not rewrite `$KEY_145`.
fn rename_identifiers(source: &str, pairs: &[(&str, &str)]) -> String {
    let mut text = source.to_owned();
    for &(from, to) in pairs {
        let mut out = String::with_capacity(text.len());
        let bytes = text.as_bytes();
        let mut cursor = 0;
        while let Some(offset) = text[cursor..].find(from) {
            let start = cursor + offset;
            let end = start + from.len();
            let left = start == 0 || !identifier_word_byte(bytes[start - 1]);
            let right = end == bytes.len() || !identifier_word_byte(bytes[end]);
            out.push_str(&text[cursor..start]);
            if left && right {
                out.push_str(to);
            } else {
                out.push_str(&text[start..end]);
            }
            cursor = end;
        }
        out.push_str(&text[cursor..]);
        text = out;
    }
    text
}

/// Respell the direct callback and all captured slots consistently. It must still be
/// claimed as emitStorageRemovalSignal.
fn direct_cursor_alpha() -> String {
    rename_identifiers(
        &direct_cursor_positive(),
        &[
            ("$THREAD_2_125597", "$THREAD_91_2"),
            ("$CB", "$ZA"),
            ("$KEY_14", "$KEY_900"),
            ("$KEY_28", "$KEY_901"),
            ("$KEY_16", "$KEY_902"),
            ("$KEY_20", "$KEY_903"),
            ("$KEY_9", "$KEY_904"),
            ("$KEY_17", "$KEY_905"),
            ("$KEY_310", "$KEY_906"),
            ("$KEY_145", "$KEY_907"),
            ("$THREAD_9_7577", "$THREAD_80_1"),
            ("$THREAD_1_58232", "$THREAD_80_3"),
            ("$CC", "$ZB"),
            ("$CD", "$ZC"),
            ("$CE", "$ZD"),
            ("$CF", "$ZE"),
            ("$CG", "$ZF"),
            ("$CH", "$ZG"),
            ("$CI", "$ZH"),
            ("$CJ", "$ZI"),
        ],
    )
}

/// Seed the direct-frame factory state from captured global $KEY_284 rather than formal $BR
/// and insert decoy into the returned child. The callback body stays untouched.
fn direct_cursor_global_seeded_with(decoy: &str) -> String {
    let positive = direct_cursor_positive();
    assert!(positive.contains("    $KEY_4 = $BR;"));
    assert!(positive.contains("        let $BS = $KEY_2;"));
    positive
        .replace("    $KEY_4 = $BR;", "    $KEY_4 = $KEY_284;")
        .replace(
            "        let $BS = $KEY_2;",
            &format!("{decoy}\n        let $BS = $KEY_2;"),
        )
}

#[test]
fn direct_cursor_storage_removal_callback_is_named() {
    assert_eq!(
        BODY_DIRECT_CURSOR.len(),
        1165,
        "direct/cursor-shared witness body must be the exact 1165-byte slice"
    );
    assert_eq!(
        sha(BODY_DIRECT_CURSOR.as_bytes()),
        "11672b0c224d635d970898557887f8f4acf0c1ee42f18f40572df4c37e13b609",
        "direct/cursor-shared witness body SHA-256 must match the sealed slice"
    );
    let positive = direct_cursor_positive();
    assert!(positive.contains(BODY_DIRECT_CURSOR), "positive must embed the verbatim body");
    assert!(positive.contains("addEventListener(\"storage\", $KEY_5);"));
    assert!(
        named(&positive, "$THREAD_2_125597", NAME),
        "direct/cursor-shared callback was not named:\n{}",
        outcome_trace(&positive, "$THREAD_2_125597")
    );
    // The family is the streaming-update variant: the witness never completes.
    assert!(
        !positive.contains("complete"),
        "the accepted direct/cursor-shared witness must be the no-complete streaming variant"
    );
}

#[test]
fn direct_cursor_alpha_renamed_spelling_keeps_the_proof() {
    let alpha = direct_cursor_alpha();
    assert!(alpha.contains("$THREAD_91_2"));
    assert!(!alpha.contains("$THREAD_2_125597"));
    assert!(alpha.contains("$ZB = $KEY_906(($ZB | 0) ^ 636298419);"));
    assert!(alpha.contains("$ZD = $ZD($ZE, $ZC, 3);"));
    assert!(
        named(&alpha, "$THREAD_91_2", NAME),
        "direct/cursor-shared alpha spelling was not named:\n{}",
        outcome_trace(&alpha, "$THREAD_91_2")
    );
}

#[test]
fn direct_cursor_rule_needs_the_storage_guard_and_old_value() {
    let positive = direct_cursor_positive();
    refuses(
        &positive,
        "$THREAD_2_125597",
        NAME,
        &[
            ("$CB.key === null", "$CB.key !== null"),
            ("$CB.key === $KEY_14", "$CB.key === $CB.oldValue"),
            ("$CB.key === $KEY_14", "$CB.key === $CB.key"),
            ("$CB.newValue === null", "$CB.newValue !== null"),
            ("$CB.oldValue ?? null", "$CB.newValue ?? null"),
        ],
    );
}

#[test]
fn direct_cursor_rule_needs_the_nonce_and_header_dataflow() {
    let positive = direct_cursor_positive();
    refuses(
        &positive,
        "$THREAD_2_125597",
        NAME,
        &[
            ("Math.random() * 4294967296;", "Math.random() * 4294967295;"),
            ("($CC | 0) >>> 24", "($CC | 0) >>> 16"),
            ("($CC | 0) >> 16 & 255", "($CC | 0) >> 8 & 255"),
            ("($CC | 0) ^ 636298419", "($CC | 0) ^ 636298420"),
            ("$CD[4] = $CC() ^ 130;", "$CD[3] = $CC() ^ 130;"),
            ("($CG | 0) >>> 24", "($CG | 0) >>> 8"),
        ],
    );
}

#[test]
fn direct_cursor_rule_needs_the_shared_writer_cursor() {
    let positive = direct_cursor_positive();
    refuses(
        &positive,
        "$THREAD_2_125597",
        NAME,
        &[
            ("let $CE = $KEY_145($CC);", "let $CE = $KEY_145($KEY_9);"),
            ("let $CE = $KEY_145($CC);", "let $CE = $KEY_310($CC);"),
            ("$CE = 4;", "$CE = 5;"),
            ("$CE = $CE($CF, $CD, 3);", "$CE = $CE($CD, $CF, 3);"),
            ("$CE = $CE($CF, $CD, 3);", "$CE = $CE($CF, $CD, 2);"),
            ("$CD[$CE + 1] =", "$CD[$CE + 2] ="),
        ],
    );
}

#[test]
fn existing_direct_cursor_explanation_remains_byte_exact() {
    let result = name_threads(&direct_cursor_positive(), true).expect("fixture must parse and build Model");
    let outcome = result
        .report
        .outcomes
        .iter()
        .find(|outcome| outcome.binding.name == "$THREAD_2_125597")
        .expect("existing direct/cursor-shared binding");
    assert!(outcome.applied);
    assert_eq!(outcome.proposed_name.as_deref(), Some(NAME));
    assert_eq!(outcome.explanation.len(), 1);
    // Original committed evaluator detail, including every space and byte.
    assert_eq!(
        sha(outcome.explanation[0].as_bytes()),
        "aff2b31bb4acee0f4ae280859c3d3b7d4d876554073c891025bc51b17323fa75"
    );
}

#[test]
fn direct_cursor_rule_needs_the_permuted_flag_lanes_and_observer() {
    let positive = direct_cursor_positive();
    refuses(
        &positive,
        "$THREAD_2_125597",
        NAME,
        &[
            (
                "($KEY_9 | 0 | $KEY_28 << 1 | $KEY_20 << 2)",
                "($KEY_28 | 0 | $KEY_9 << 1 | $KEY_20 << 2)",
            ),
            (
                "($KEY_9 | 0 | $KEY_28 << 1 | $KEY_20 << 2)",
                "($KEY_9 | 0 | $KEY_28 << 1 | $KEY_28 << 2)",
            ),
            (
                "($KEY_9 | 0 | $KEY_28 << 1 | $KEY_20 << 2)",
                "($KEY_9 | 0 | $KEY_20 << 1 | $KEY_28 << 2)",
            ),
            ("$KEY_17.next([[], $CD]);", "$KEY_16.next([[], $CD]);"),
            ("$KEY_17.next([[], $CH]);", "$KEY_16.next([[], $CH]);"),
            ("$KEY_17.next([[], $CD]);", "$KEY_20.next([[], $CD]);"),
        ],
    );
}

#[test]
fn direct_cursor_rule_needs_the_null_marker_and_direct_branch() {
    let positive = direct_cursor_positive();
    refuses(
        &positive,
        "$THREAD_2_125597",
        NAME,
        &[
            ("if ($CF == null) {", "if ($CF != null) {"),
            ("if ($CF == null) {", "if ($CD == null) {"),
            ("$CD[4] = $CC() ^ 130;", "$CD[4] = $CC() ^ 129;"),
            ("$CH[4] = $CG() ^ 130;", "$CH[4] = $CG() ^ 131;"),
        ],
    );
}

#[test]
fn direct_cursor_streaming_update_must_not_complete() {
    let positive = direct_cursor_positive();
    refuses(
        &positive,
        "$THREAD_2_125597",
        NAME,
        &[
            (
                "$KEY_17.next([[], $CD]);",
                "$KEY_17.next([[], $CD]);\n\t\t\t\t\t\t$KEY_17.complete();",
            ),
            (
                "$KEY_17.next([[], $CH]);",
                "$KEY_17.next([[], $CH]);\n\t\t\t\t\t$KEY_17.complete();",
            ),
        ],
    );
}

#[test]
fn direct_cursor_catch_must_reexamine_the_same_frame() {
    let positive = direct_cursor_positive();
    refuses(
        &positive,
        "$THREAD_2_125597",
        NAME,
        &[
            ("$KEY_20 = true;", "$KEY_9 = true;"),
            (
                "$CG = $KEY_310(($CG | 0) ^ 636298419);",
                "$CG = $KEY_310(($CG | 0) ^ 636298420);",
            ),
            ("$CH[4] = $CG() ^ 130;", "$CH[4] = $CG() ^ 129;"),
            ("let $CI = $KEY_145($CG);", "let $CI = $KEY_145($KEY_9);"),
            ("$CH[$CI + 1] =", "$CH[$CI + 2] ="),
        ],
    );
}

#[test]
fn direct_cursor_factory_helpers_must_really_use_the_formal() {
    let positive = direct_cursor_positive();
    // An own child that ignores the factory formal is a nested no-op.
    assert!(positive.contains("    $KEY_4 = $BR;"));
    let ignored = positive.replace("    $KEY_4 = $BR;", "    $KEY_4 = $KEY_284;");
    assert!(ignored.contains("    $KEY_4 = $KEY_284;"));
    assert!(!ignored.contains("    $KEY_4 = $BR;"));
    refused(&ignored, "$THREAD_2_125597", NAME);

    // The formal read in a dead local, a never-called nested function, an empty
    // control test or a discarded call argument is not a consumed seed value.
    for decoy in [
        "        let $BDEAD = $BR;",
        "        function $THREAD_DEAD() { return $BR; }",
        "        if ($BR) {}",
        "        $KEY_166($BR, 0);",
    ] {
        let source = direct_cursor_global_seeded_with(decoy);
        assert!(source.contains(decoy));
        assert!(!source.contains("    $KEY_4 = $BR;"));
        refused(&source, "$THREAD_2_125597", NAME);
    }

    // A dead-overwritten factory alias: the formal write is replaced before the
    // returned child runs, so the formal is not a live carrier.
    let overwritten = positive.replace(
        "    $KEY_4 = $BR;",
        "    $KEY_4 = $BR;\n    $KEY_4 = $KEY_284;",
    );
    assert!(overwritten.contains("    $KEY_4 = $BR;\n    $KEY_4 = $KEY_284;"));
    refused(&overwritten, "$THREAD_2_125597", NAME);
}

#[test]
fn direct_cursor_writer_factory_must_really_use_its_formal() {
    let positive = direct_cursor_positive();
    // The writer factory routes the formal into `$KEY_0`, which the child invokes
    // into the output bytes.  Seeding that alias from the captured global instead
    // removes the only live formal use.
    assert!(positive.contains("    let $KEY_0 = $BR;"));
    let ignored = positive.replace("    let $KEY_0 = $BR;", "    let $KEY_0 = $KEY_284;");
    assert!(ignored.contains("    let $KEY_0 = $KEY_284;"));
    refused(&ignored, "$THREAD_2_125597", NAME);
}

// Fixed-slot storage frame with flags written before the null branch.

/// Shifted lanes are appended at literal slot 4 before the null test. The null branch
/// writes the tag at slot 5; the other branch invokes the writer at literal cursor 4.
const BODY_FIXED_SLOT: &str = r#"function $THREAD_1_115684($BW) {
						try {
							if ($BW.key === null || $BW.key === $KEY_12 && $BW.newValue === null) {
								$KEY_14 = true;
								$KEY_6 = $BW.oldValue ?? null;
								let $BX = Math.random() * 4294967296 | 0;
								let $BY = [$BX >>> 24, $BX >> 16 & 255, $BX >> 8 & 255, $BX & 255];
								$BX = $KEY_165($BX ^ 3349629762);
								let $BZ = $KEY_429($BX);
								$BY[4] = ($KEY_7 | 0 | $KEY_14 << 1 | $KEY_16 << 2) ^ $BX();
								let $CA = $KEY_6;
								if ($CA == null) {
									$BY[5] = $BX() ^ 130;
								} else {
									$BZ($CA, $BY, 4);
								}
								$KEY_10.next([[], $BY]);
							}
						} catch (e) {
							$KEY_16 = true;
							let $CB = Math.random() * 4294967296;
							let $CC = [($CB | 0) >>> 24, ($CB | 0) >> 16 & 255, ($CB | 0) >> 8 & 255, ($CB | 0) & 255];
							$CB = $KEY_165(($CB | 0) ^ 3349629762);
							let $CD = $KEY_429($CB);
							$CC[4] = ($KEY_7 | 0 | $KEY_14 << 1 | $KEY_16 << 2) ^ $CB();
							let $CE = $KEY_6;
							if ($CE == null) {
								$CC[5] = $CB() ^ 130;
							} else {
								$CD($CE, $CC, 4);
							}
							$KEY_10.next([[], $CC]);
						}
					}"#;

/// The fixed-slot seed factory body.
const FIXED_SLOT_SEED: &str = r#"function $THREAD_5_94032($BQ) {
					let $KEY_0, $KEY_1, $KEY_2, $KEY_3;
					function $THREAD_0_103890() {
						let $BR = $KEY_2;
						$KEY_2 = ($BR + 1) % 4;
						if (($BR + 1) % 4 === 0) {
							$KEY_3 = $KEY_1 + $KEY_0;
							let $BS = $KEY_0;
							$KEY_0 = $BS ^ $KEY_1;
							$KEY_1 = $BS ^ $KEY_1 ^ ($KEY_1 << 26 | $KEY_1 >>> 6) ^ $KEY_0 << 9;
							$KEY_0 = $KEY_0 << 13 | $KEY_0 >>> 19;
							return $KEY_3 & 255;
						}
						if (($BR + 1) % 4 === 1) {
							return $KEY_3 >> 8 & 255;
						}
						if (($BR + 1) % 4 === 2) {
							return $KEY_3 >> 16 & 255;
						}
						if (($BR + 1) % 4 === 3) {
							return $KEY_3 >>> 24 & 255;
						} else {
							return void 0;
						}
					}
					$KEY_1 = $KEY_325;
					$KEY_0 = $BQ;
					$KEY_2 = -1;
					$THREAD_0_103890();
					$THREAD_0_103890();
					$THREAD_0_103890();
					$THREAD_0_103890();
					return $THREAD_0_103890;
				}"#;

/// The fixed-slot writer factory body.
const FIXED_SLOT_WRITER: &str = r#"function $THREAD_5_62575($BQ) {
					let $KEY_1 = $BQ;
					let $KEY_2;
					let $KEY_0 = Object.create(null);
					$KEY_2 = 0;
					function $THREAD_5_15372($BR, $BS, $BT) {
						let $KEY_12;
						if ($KEY_0[$BR] != null) {
							$KEY_12 = $BT;
							$KEY_12 = Number($BT) + 1;
							$BS[$KEY_12] = $KEY_1() ^ 129;
							$KEY_12 = $KEY_143($KEY_1, $KEY_0[$BR], $BS, $KEY_12);
						} else {
							let $BU = $BR.length;
							let $BV = 0;
							let $BW = $BT;
							while ($BV < $BU) {
								let $BX = $KEY_128($BR[$BV], 0);
								if ($BX === 0) {
									$BS[++$BW] = $KEY_1() ^ 192;
									$BX = 128;
								} else if (!($BX < 128)) if ($BX < 2048) {
									$BS[++$BW] = ($BX >> 6 | 192) ^ $KEY_1();
									$BX = $BX & 63 | 128;
								} else {
									const $BY = $BX >= 55296 && $BX <= 57343 && $BX < 56320 && $BV + 1 < $BU ? $KEY_128($BR[$BV + 1], 0) : null;
									if ($BY == null || $BY < 56320 || $BY > 57343) {
										$BS[++$BW] = ($BX >> 12 | 224) ^ $KEY_1();
										$BS[++$BW] = ($BX >> 6 & 63 | 128) ^ $KEY_1();
										$BX = $BX & 63 | 128;
									} else {
										++$BV;
										$BS[++$BW] = (65536 + ($BX - 55296 << 10) + ($BY - 56320) >> 18 | 240) ^ $KEY_1();
										$BS[++$BW] = (65536 + ($BX - 55296 << 10) + ($BY - 56320) >> 12 & 63 | 128) ^ $KEY_1();
										$BS[++$BW] = (65536 + ($BX - 55296 << 10) + ($BY - 56320) >> 6 & 63 | 128) ^ $KEY_1();
										$BX = 65536 + ($BX - 55296 << 10) + ($BY - 56320) & 63 | 128;
									}
								}
								$BS[++$BW] = $BX ^ $KEY_1();
								++$BV;
							}
							$KEY_12 = Number($BW) + 1;
							$BS[$KEY_12] = $KEY_1();
							let $BZ = $KEY_0;
							let $CA = Number($KEY_2);
							$KEY_2 = $CA + 1;
							$BZ[$BR] = $CA;
						}
						return $KEY_12;
					}
					return $THREAD_5_15372;
				}"#;

/// Enclosing fixture mirroring the real `$THREAD_8_41852` parent: the captured
/// key (`$KEY_12`), removal flag (`$KEY_14`), old value (`$KEY_6`), auxiliary
/// flag (`$KEY_7`), error flag (`$KEY_16`) and observer (`$KEY_10`) are declared
/// before the callback, and the seed/writer factories are the real
/// `$THREAD_5_94032` / `$THREAD_5_62575` bodies from the same file.  Their
/// referenced `$KEY_325` / `$KEY_143` / `$KEY_128` helpers are captured-global
/// stand-ins exactly as the earlier fixtures use their own helpers.
fn fixed_slot_prefix() -> String {
    let mut source = String::from("function main(){\n");
    source.push_str(" let $KEY_12 = \"persistent\";\n");
    source.push_str(" let $KEY_14 = false;\n");
    source.push_str(" let $KEY_6 = null;\n");
    source.push_str(" let $KEY_7 = false;\n");
    source.push_str(" let $KEY_16 = false;\n");
    source.push_str(" let $KEY_10 = { next(value){ return value; } };\n");
    source.push_str(" let $KEY_325 = 0;\n");
    source.push_str(" function $KEY_143($k, $o, $b, $p) { return $p; }\n");
    source.push_str(" function $KEY_128($c, $i) { return 0; }\n");
    source.push_str(FIXED_SLOT_SEED);
    source.push('\n');
    source.push_str(FIXED_SLOT_WRITER);
    source.push('\n');
    source.push_str(" let $KEY_165 = $THREAD_5_94032;\n");
    source.push_str(" let $KEY_429 = $THREAD_5_62575;\n");
    source
}

/// The real parent registration: the callback is aliased and then registered as
/// a `storage` listener, which is the family's provenance.
const FIXED_SLOT_SUFFIX: &str = r#"
 let $KEY_24 = $THREAD_1_115684;
 let $KEY_15 = { global: { addEventListener(){ return void 0; } } };
 $KEY_15.global.addEventListener("storage", $KEY_24);
}
main();"#;

fn fixed_slot_positive() -> String {
    format!("{}{}{}", fixed_slot_prefix(), BODY_FIXED_SLOT, FIXED_SLOT_SUFFIX)
}

/// Respell the fixed-slot callback and all captured slots consistently. It must still be
/// claimed as emitStorageRemovalSignal.
fn fixed_slot_alpha() -> String {
    rename_identifiers(
        &fixed_slot_positive(),
        &[
            ("$THREAD_1_115684", "$THREAD_71_1"),
            ("$THREAD_5_94032", "$THREAD_70_1"),
            ("$THREAD_5_62575", "$THREAD_70_2"),
            ("$KEY_12", "$KEY_912"),
            ("$KEY_14", "$KEY_914"),
            ("$KEY_6", "$KEY_906"),
            ("$KEY_7", "$KEY_907"),
            ("$KEY_16", "$KEY_916"),
            ("$KEY_10", "$KEY_910"),
            ("$KEY_165", "$KEY_965"),
            ("$KEY_429", "$KEY_942"),
            ("$KEY_325", "$KEY_925"),
            ("$KEY_143", "$KEY_943"),
            ("$KEY_128", "$KEY_928"),
            ("$KEY_24", "$KEY_924"),
            ("$KEY_15", "$KEY_915"),
            ("$BW", "$ZW"),
            ("$BX", "$ZX"),
            ("$BY", "$ZY"),
            ("$BZ", "$ZZ"),
            ("$CA", "$ZA"),
            ("$CB", "$ZB"),
            ("$CC", "$ZC"),
            ("$CD", "$ZD"),
            ("$CE", "$ZE"),
        ],
    )
}

/// Seed the fixed-slot factory state from captured global $KEY_325 rather than formal $BQ
/// and insert decoy into the returned child. The callback stays untouched.
fn fixed_slot_global_seeded_with(decoy: &str) -> String {
    let positive = fixed_slot_positive();
    assert!(positive.contains("$KEY_0 = $BQ;"));
    assert!(positive.contains("let $BR = $KEY_2;"));
    positive
        .replace("$KEY_0 = $BQ;", "$KEY_0 = $KEY_325;")
        .replace(
            "let $BR = $KEY_2;",
            &format!("{decoy}\n\t\t\t\t\t\tlet $BR = $KEY_2;"),
        )
}

#[test]
fn actual_fixed_slot_fixed_slot_storage_removal_callback_is_named() {
    assert_eq!(BODY_FIXED_SLOT.len(), 1149, "fixed-slot witness body must be the exact 1149-byte slice");
    assert_eq!(
        sha(BODY_FIXED_SLOT.as_bytes()),
        "28cfb02d5302cec53ab3cdba6b539bbb2614cf1bb0b99f7098cb539008dd465d",
        "fixed-slot witness body SHA-256 must match the sealed slice"
    );
    assert_eq!(FIXED_SLOT_SEED.len(), 906, "fixed-slot seed donor must be the exact 906-byte slice");
    assert_eq!(
        sha(FIXED_SLOT_SEED.as_bytes()),
        "502c4ee365419377d6d7945a8056dddeba44745d52b1412a023547f4ec8e1966",
        "fixed-slot seed donor SHA-256 must match the cached factory"
    );
    assert_eq!(FIXED_SLOT_WRITER.len(), 1808, "fixed-slot writer donor must be the exact 1808-byte slice");
    assert_eq!(
        sha(FIXED_SLOT_WRITER.as_bytes()),
        "64ef83384a7c2af9bc28c93fb66f04e04f14754746982a47a4cc2de6b85e085e",
        "fixed-slot writer donor SHA-256 must match the cached factory"
    );
    let positive = fixed_slot_positive();
    assert!(positive.contains(BODY_FIXED_SLOT), "positive must embed the verbatim body");
    assert!(positive.contains(FIXED_SLOT_SEED), "positive must embed the real seed donor");
    assert!(positive.contains(FIXED_SLOT_WRITER), "positive must embed the real writer donor");
    assert!(positive.contains("addEventListener(\"storage\", $KEY_24);"));
    assert!(
        named(&positive, "$THREAD_1_115684", NAME),
        "fixed-slot callback was not named:\n{}",
        outcome_trace(&positive, "$THREAD_1_115684")
    );
    // The family is the streaming-update variant: the witness never completes.
    assert!(
        !positive.contains("complete"),
        "the accepted fixed-slot witness must be the no-complete streaming variant"
    );
}

#[test]
fn fixed_slot_alpha_renamed_spelling_keeps_the_proof() {
    let alpha = fixed_slot_alpha();
    assert!(alpha.contains("$THREAD_71_1"));
    assert!(!alpha.contains("$THREAD_1_115684"));
    assert!(alpha.contains("$ZX = $KEY_965($ZX ^ 3349629762);"));
    assert!(alpha.contains("$ZZ($ZA, $ZY, 4);"));
    assert!(
        named(&alpha, "$THREAD_71_1", NAME),
        "fixed-slot alpha spelling was not named:\n{}",
        outcome_trace(&alpha, "$THREAD_71_1")
    );
}

#[test]
fn fixed_slot_rule_needs_the_storage_guard_and_old_value() {
    let positive = fixed_slot_positive();
    refuses(
        &positive,
        "$THREAD_1_115684",
        NAME,
        &[
            ("$BW.key === null", "$BW.key !== null"),
            ("$BW.key === $KEY_12", "$BW.key === $BW.oldValue"),
            ("$BW.key === $KEY_12", "$BW.key === $BW.key"),
            ("$BW.newValue === null", "$BW.newValue !== null"),
            ("$BW.oldValue ?? null", "$BW.newValue ?? null"),
        ],
    );
}

#[test]
fn fixed_slot_rule_needs_the_nonce_and_header_dataflow() {
    let positive = fixed_slot_positive();
    refuses(
        &positive,
        "$THREAD_1_115684",
        NAME,
        &[
            ("Math.random() * 4294967296 | 0;", "Math.random() * 4294967295 | 0;"),
            ("$BX >>> 24", "$BX >>> 16"),
            ("$BX >> 16 & 255", "$BX >> 8 & 255"),
            ("$BX = $KEY_165($BX ^ 3349629762);", "$BX = $KEY_165($BX ^ 3349629761);"),
            ("$CC[4] =", "$CC[3] ="),
            ("($CB | 0) >>> 24", "($CB | 0) >>> 8"),
        ],
    );
}

#[test]
fn fixed_slot_rule_needs_the_fixed_slots_and_writer_cursor() {
    let positive = fixed_slot_positive();
    refuses(
        &positive,
        "$THREAD_1_115684",
        NAME,
        &[
            (
                "$BY[4] = ($KEY_7 | 0 | $KEY_14 << 1 | $KEY_16 << 2) ^ $BX();",
                "$BY[3] = ($KEY_7 | 0 | $KEY_14 << 1 | $KEY_16 << 2) ^ $BX();",
            ),
            ("$BY[5] = $BX() ^ 130;", "$BY[6] = $BX() ^ 130;"),
            ("$BZ($CA, $BY, 4);", "$BZ($CA, $BY, 3);"),
            ("$BZ($CA, $BY, 4);", "$BZ($BY, $CA, 4);"),
            ("let $BZ = $KEY_429($BX);", "let $BZ = $KEY_429($KEY_7);"),
            ("let $BZ = $KEY_429($BX);", "let $BZ = $KEY_165($BX);"),
        ],
    );
}

#[test]
fn fixed_slot_rule_needs_the_flag_lanes_and_observer() {
    let positive = fixed_slot_positive();
    refuses(
        &positive,
        "$THREAD_1_115684",
        NAME,
        &[
            (
                "($KEY_7 | 0 | $KEY_14 << 1 | $KEY_16 << 2)",
                "($KEY_14 | 0 | $KEY_7 << 1 | $KEY_16 << 2)",
            ),
            (
                "($KEY_7 | 0 | $KEY_14 << 1 | $KEY_16 << 2)",
                "($KEY_7 | 0 | $KEY_16 << 1 | $KEY_14 << 2)",
            ),
            (
                "($KEY_7 | 0 | $KEY_14 << 1 | $KEY_16 << 2)",
                "($KEY_7 | 0 | $KEY_14 << 1 | $KEY_7 << 2)",
            ),
            ("$KEY_10.next([[], $BY]);", "$KEY_6.next([[], $BY]);"),
            ("$KEY_10.next([[], $CC]);", "$KEY_6.next([[], $CC]);"),
        ],
    );
}

#[test]
fn fixed_slot_streaming_update_must_not_complete() {
    let positive = fixed_slot_positive();
    refuses(
        &positive,
        "$THREAD_1_115684",
        NAME,
        &[
            (
                "$KEY_10.next([[], $BY]);",
                "$KEY_10.next([[], $BY]);\n\t\t\t\t\t\t\t$KEY_10.complete();",
            ),
            (
                "$KEY_10.next([[], $CC]);",
                "$KEY_10.next([[], $CC]);\n\t\t\t\t\t\t$KEY_10.complete();",
            ),
        ],
    );
}

#[test]
fn fixed_slot_catch_must_reexamine_the_same_frame() {
    let positive = fixed_slot_positive();
    refuses(
        &positive,
        "$THREAD_1_115684",
        NAME,
        &[
            ("$KEY_16 = true;", "$KEY_7 = true;"),
            (
                "$CB = $KEY_165(($CB | 0) ^ 3349629762);",
                "$CB = $KEY_165(($CB | 0) ^ 3349629761);",
            ),
            ("$CC[5] = $CB() ^ 130;", "$CC[5] = $CB() ^ 129;"),
            ("let $CD = $KEY_429($CB);", "let $CD = $KEY_429($KEY_7);"),
            ("$CC[4] =", "$CC[3] ="),
        ],
    );
}

#[test]
fn fixed_slot_factory_helpers_must_really_use_the_formal() {
    let positive = fixed_slot_positive();
    // An own child that ignores the factory formal is a nested no-op.
    assert!(positive.contains("$KEY_0 = $BQ;"));
    let ignored = positive.replace("$KEY_0 = $BQ;", "$KEY_0 = $KEY_325;");
    assert!(ignored.contains("$KEY_0 = $KEY_325;"));
    assert!(!ignored.contains("$KEY_0 = $BQ;"));
    refused(&ignored, "$THREAD_1_115684", NAME);

    // The formal read in a dead local, a never-called nested function, an empty
    // control test or a discarded call argument is not a consumed seed value.
    for decoy in [
        "        let $BDEAD = $BQ;",
        "        function $THREAD_DEAD() { return $BQ; }",
        "        if ($BQ) {}",
        "        $KEY_128($BQ, 0);",
    ] {
        let source = fixed_slot_global_seeded_with(decoy);
        assert!(source.contains(decoy));
        assert!(!source.contains("$KEY_0 = $BQ;"));
        refused(&source, "$THREAD_1_115684", NAME);
    }

    // A dead-overwritten factory alias: the formal write is replaced before the
    // returned child runs, so the formal is not a live carrier.
    let overwritten = positive.replace(
        "$KEY_0 = $BQ;",
        "$KEY_0 = $BQ;\n\t\t\t\t\t$KEY_0 = $KEY_325;",
    );
    assert!(overwritten.contains("$KEY_0 = $BQ;\n\t\t\t\t\t$KEY_0 = $KEY_325;"));
    refused(&overwritten, "$THREAD_1_115684", NAME);
}

#[test]
fn fixed_slot_writer_factory_must_really_use_its_formal() {
    let positive = fixed_slot_positive();
    // The writer factory routes the formal into `$KEY_1`, which the child invokes
    // into the output bytes.  Seeding that alias from the captured global instead
    // removes the only live formal use.
    assert!(positive.contains("let $KEY_1 = $BQ;"));
    let ignored = positive.replace("let $KEY_1 = $BQ;", "let $KEY_1 = $KEY_325;");
    assert!(ignored.contains("let $KEY_1 = $KEY_325;"));
    refused(&ignored, "$THREAD_1_115684", NAME);
}

// Holder storage frame with a separate cursor and late flag writes.

/// The holder tail keeps a separate cursor and no flag alias. It appends `flag | 0 | error
/// << 1 | second << 2` at header[cursor + 1] after the branch, reading the captured removal
/// flag directly in the low lane.
const BODY_LATE_FLAGS: &str = r#"function $THREAD_8_24419($CD) {
						try {
							if ($CD.key === null || $CD.key === $KEY_51 && $CD.newValue === null) {
								$KEY_0 = true;
								$KEY_95 = $CD.oldValue ?? null;
								let $CE = Math.random() * 4294967296;
								let $CF = [($CE | 0) >>> 24, ($CE | 0) >> 16 & 255, ($CE | 0) >> 8 & 255, ($CE | 0) & 255];
								$CE = $KEY_312(($CE | 0) ^ 3413877106);
								let $CG = $KEY_158($CE);
								let $CH = $KEY_95;
								let $CI;
								if ($CH == null) {
									$CF[4] = $CE() ^ 130;
									$CI = 4;
								} else {
									$CG = [$CG, $CH, $CF];
									$CI = $CG[0]($CG[1], $CG[2], 3);
								}
								$CF[$CI + 1] = ($KEY_0 | 0 | $KEY_46 << 1 | $KEY_23 << 2) ^ $CE();
								$KEY_67.next([[], $CF]);
							}
						} catch (e) {
							$KEY_46 = true;
							let $CJ = Math.random() * 4294967296 | 0;
							let $CK = [$CJ >>> 24, $CJ >> 16 & 255, $CJ >> 8 & 255, $CJ & 255];
							$CJ = $KEY_312($CJ ^ 3413877106);
							let $CL = $KEY_158($CJ);
							let $CM = $KEY_95;
							let $CN;
							if ($CM == null) {
								$CK[4] = $CJ() ^ 130;
								$CN = 4;
							} else {
								$CL = [$CL, $CM, $CK];
								$CN = $CL[0]($CL[1], $CL[2], 3);
							}
							$CK[$CN + 1] = ($KEY_0 | 0 | $KEY_46 << 1 | $KEY_23 << 2) ^ $CJ();
							$KEY_67.next([[], $CK]);
						}
					}"#;

/// The late-flags seed factory body.
const LATE_FLAGS_SEED: &str = r#"function $THREAD_5_139977($BR) {
					let $KEY_0, $KEY_1, $KEY_2, $KEY_3;
					function $THREAD_1_30211() {
						let $BS = $KEY_2;
						$KEY_2 = ($BS + 1) % 4;
						if (($BS + 1) % 4 === 0) {
							$KEY_0 = $KEY_1 + $KEY_3;
							let $BT = $KEY_3;
							$KEY_3 = $BT ^ $KEY_1;
							$KEY_1 = $BT ^ $KEY_1 ^ ($KEY_1 << 26 | $KEY_1 >>> 6) ^ $KEY_3 << 9;
							$KEY_3 = $KEY_3 << 13 | $KEY_3 >>> 19;
							return $KEY_0 & 255;
						}
						if (($BS + 1) % 4 === 1) {
							return $KEY_0 >> 8 & 255;
						}
						if (($BS + 1) % 4 === 2) {
							return $KEY_0 >> 16 & 255;
						}
						if (($BS + 1) % 4 === 3) {
							return $KEY_0 >>> 24 & 255;
						} else {
							return void 0;
						}
					}
					$KEY_1 = $KEY_175;
					$KEY_3 = $BR;
					$KEY_2 = -1;
					$THREAD_1_30211();
					$THREAD_1_30211();
					$THREAD_1_30211();
					$THREAD_1_30211();
					return $THREAD_1_30211;
				}"#;

/// The late-flags writer factory body.
const LATE_FLAGS_WRITER: &str = r#"function $THREAD_4_1132($BR) {
					let $KEY_0 = $BR;
					let $KEY_1, $KEY_2;
					$KEY_2 = Object.create(null);
					$KEY_1 = 0;
					function $THREAD_5_18669($BS, $BT, $BU) {
						let $KEY_10;
						if ($KEY_2[$BS] != null) {
							$KEY_10 = $BU;
							$KEY_10 = Number($BU) + 1;
							$BT[$KEY_10] = $KEY_0() ^ 129;
							$KEY_10 = $KEY_17($KEY_0, $KEY_2[$BS], $BT, $KEY_10);
						} else {
							let $BV = $BS.length;
							let $BW = 0;
							let $BX = $BU;
							while ($BW < $BV) {
								let $BY = $KEY_207($BS[$BW], 0);
								let $BZ;
								if ($BY === 0) {
									$BT[++$BX] = $KEY_0() ^ 192;
									$BY = $KEY_0();
									$BZ = 128;
								} else {
									if (!($BY < 128)) if ($BY < 2048) {
										$BT[++$BX] = ($BY >> 6 | 192) ^ $KEY_0();
										$BY = $BY & 63 | 128;
									} else {
										const $CA = $BY >= 55296 && $BY <= 57343 && $BY < 56320 && $BW + 1 < $BV ? $KEY_207($BS[$BW + 1], 0) : null;
										if ($CA == null || $CA < 56320 || $CA > 57343) {
											$BT[++$BX] = ($BY >> 12 | 224) ^ $KEY_0();
											$BT[++$BX] = ($BY >> 6 & 63 | 128) ^ $KEY_0();
											$BY = $BY & 63 | 128;
										} else {
											++$BW;
											$BT[++$BX] = (65536 + ($BY - 55296 << 10) + ($CA - 56320) >> 18 | 240) ^ $KEY_0();
											$BT[++$BX] = (65536 + ($BY - 55296 << 10) + ($CA - 56320) >> 12 & 63 | 128) ^ $KEY_0();
											$BT[++$BX] = (65536 + ($BY - 55296 << 10) + ($CA - 56320) >> 6 & 63 | 128) ^ $KEY_0();
											$BY = 65536 + ($BY - 55296 << 10) + ($CA - 56320) & 63 | 128;
										}
									}
									$BZ = $KEY_0();
								}
								$BT[++$BX] = $BY ^ $BZ;
								++$BW;
							}
							$KEY_10 = Number($BX) + 1;
							$BT[$KEY_10] = $KEY_0();
							let $CB = $KEY_2;
							let $CC = Number($KEY_1);
							$KEY_1 = $CC + 1;
							$CB[$BS] = $CC;
						}
						return $KEY_10;
					}
					return $THREAD_5_18669;
				}"#;

/// Enclosing fixture mirroring the real `$THREAD_1_121852` parent: the captured
/// key (`$KEY_51`), removal flag (`$KEY_0`), old value (`$KEY_95`), error flag
/// (`$KEY_46`), auxiliary flag (`$KEY_23`) and observer (`$KEY_67`) are declared
/// before the callback, and the seed/writer factories are the real
/// `$THREAD_5_139977` / `$THREAD_4_1132` bodies from the same file.  Their
/// referenced `$KEY_175` / `$KEY_17` / `$KEY_207` helpers are captured-global
/// stand-ins exactly as the earlier fixtures use their own helpers.
fn late_flags_prefix() -> String {
    let mut source = String::from("function main(){\n");
    source.push_str(" let $KEY_51 = \"persistent\";\n");
    source.push_str(" let $KEY_0 = false;\n");
    source.push_str(" let $KEY_95 = null;\n");
    source.push_str(" let $KEY_46 = false;\n");
    source.push_str(" let $KEY_23 = false;\n");
    source.push_str(" let $KEY_67 = { next(value){ return value; } };\n");
    source.push_str(" let $KEY_175 = 0;\n");
    source.push_str(" function $KEY_17($k, $o, $b, $p) { return $p; }\n");
    source.push_str(" function $KEY_207($c, $i) { return 0; }\n");
    source.push_str(LATE_FLAGS_SEED);
    source.push('\n');
    source.push_str(LATE_FLAGS_WRITER);
    source.push('\n');
    source.push_str(" let $KEY_312 = $THREAD_5_139977;\n");
    source.push_str(" let $KEY_158 = $THREAD_4_1132;\n");
    source
}

/// The real parent registration: the callback is aliased and then registered as
/// a `storage` listener, which is the family's provenance.
const LATE_FLAGS_SUFFIX: &str = r#"
 let $KEY_70 = $THREAD_8_24419;
 let $KEY_103 = { global: { addEventListener(){ return void 0; } } };
 $KEY_103.global.addEventListener("storage", $KEY_70);
}
main();"#;

fn late_flags_positive() -> String {
    format!("{}{}{}", late_flags_prefix(), BODY_LATE_FLAGS, LATE_FLAGS_SUFFIX)
}

/// Respell the late-flags callback and captured slots consistently. It must still be
/// claimed as emitStorageRemovalSignal.
fn late_flags_alpha() -> String {
    rename_identifiers(
        &late_flags_positive(),
        &[
            ("$THREAD_8_24419", "$THREAD_81_1"),
            ("$THREAD_5_139977", "$THREAD_80_1"),
            ("$THREAD_4_1132", "$THREAD_80_2"),
            ("$KEY_51", "$KEY_951"),
            ("$KEY_0", "$KEY_900"),
            ("$KEY_95", "$KEY_995"),
            ("$KEY_46", "$KEY_946"),
            ("$KEY_23", "$KEY_923"),
            ("$KEY_67", "$KEY_967"),
            ("$KEY_312", "$KEY_932"),
            ("$KEY_158", "$KEY_958"),
            ("$KEY_175", "$KEY_975"),
            ("$KEY_17", "$KEY_917"),
            ("$KEY_207", "$KEY_927"),
            ("$KEY_70", "$KEY_970"),
            ("$KEY_103", "$KEY_903"),
            ("$CD", "$ZY"),
            ("$CE", "$ZX"),
            ("$CF", "$ZW"),
            ("$CG", "$ZV"),
            ("$CH", "$ZU"),
            ("$CI", "$ZT"),
            ("$CJ", "$ZS"),
            ("$CK", "$ZR"),
            ("$CL", "$ZQ"),
            ("$CM", "$ZP"),
            ("$CN", "$ZO"),
        ],
    )
}

/// Seed the late-flags factory state from captured global $KEY_175 rather than formal $BR
/// and insert decoy into the returned child. The callback stays untouched.
fn late_flags_global_seeded_with(decoy: &str) -> String {
    let positive = late_flags_positive();
    assert!(positive.contains("$KEY_3 = $BR;"));
    assert!(positive.contains("let $BS = $KEY_2;"));
    positive
        .replace("$KEY_3 = $BR;", "$KEY_3 = $KEY_175;")
        .replace(
            "let $BS = $KEY_2;",
            &format!("{decoy}\n\t\t\t\t\t\tlet $BS = $KEY_2;"),
        )
}

#[test]
fn actual_late_flags_storage_removal_callback_is_named() {
    assert_eq!(BODY_LATE_FLAGS.len(), 1323, "more witness body must be the exact 1323-byte slice");
    assert_eq!(
        sha(BODY_LATE_FLAGS.as_bytes()),
        "926a92917356b214aa1959f6d71ad7a090ae1ddf81bda86dacfa2bd2e0e2c891",
        "late-flags witness body SHA-256 must match the sealed slice"
    );
    assert_eq!(LATE_FLAGS_SEED.len(), 901, "more seed donor must be the exact 901-byte slice");
    assert_eq!(
        sha(LATE_FLAGS_SEED.as_bytes()),
        "76cceaca8b9be5c01992c75e9f5a545c02bd54e1542ed2887ec59139d2e327ef",
        "more seed donor SHA-256 must match the cached factory"
    );
    assert_eq!(LATE_FLAGS_WRITER.len(), 1909, "more writer donor must be the exact 1909-byte slice");
    assert_eq!(
        sha(LATE_FLAGS_WRITER.as_bytes()),
        "65cacf7b156153069fb28566500b460d5e2d0a957f44694a49d566087623dfa8",
        "more writer donor SHA-256 must match the cached factory"
    );
    let positive = late_flags_positive();
    assert!(positive.contains(BODY_LATE_FLAGS), "positive must embed the verbatim body");
    assert!(positive.contains(LATE_FLAGS_SEED), "positive must embed the real seed donor");
    assert!(positive.contains(LATE_FLAGS_WRITER), "positive must embed the real writer donor");
    assert!(positive.contains("addEventListener(\"storage\", $KEY_70);"));
    assert!(
        named(&positive, "$THREAD_8_24419", NAME),
        "late-flags holder callback was not named:\n{}",
        outcome_trace(&positive, "$THREAD_8_24419")
    );
    // The family is the streaming-update variant: the witness never completes.
    assert!(
        !positive.contains("complete"),
        "the accepted more witness must be the no-complete streaming variant"
    );
}

#[test]
fn late_flags_alpha_renamed_spelling_keeps_the_proof() {
    let alpha = late_flags_alpha();
    assert!(alpha.contains("$THREAD_81_1"));
    assert!(!alpha.contains("$THREAD_8_24419"));
    assert!(alpha.contains("$ZS = $KEY_932($ZS ^ 3413877106);"));
    assert!(alpha.contains("$ZT = $ZV[0]($ZV[1], $ZV[2], 3);"));
    assert!(
        named(&alpha, "$THREAD_81_1", NAME),
        "more alpha spelling was not named:\n{}",
        outcome_trace(&alpha, "$THREAD_81_1")
    );
}

#[test]
fn late_flags_rule_needs_the_storage_guard_and_old_value() {
    let positive = late_flags_positive();
    refuses(
        &positive,
        "$THREAD_8_24419",
        NAME,
        &[
            ("$CD.key === null", "$CD.key !== null"),
            ("$CD.key === $KEY_51", "$CD.key === $CD.oldValue"),
            ("$CD.key === $KEY_51", "$CD.key === $CD.key"),
            ("$CD.newValue === null", "$CD.newValue !== null"),
            ("$CD.oldValue ?? null", "$CD.newValue ?? null"),
        ],
    );
}

#[test]
fn late_flags_rule_needs_the_nonce_and_header_dataflow() {
    let positive = late_flags_positive();
    refuses(
        &positive,
        "$THREAD_8_24419",
        NAME,
        &[
            ("Math.random() * 4294967296;", "Math.random() * 4294967295;"),
            ("Math.random() * 4294967296 | 0;", "Math.random() * 4294967295 | 0;"),
            ("($CE | 0) >>> 24", "($CE | 0) >>> 16"),
            ("($CE | 0) >> 16 & 255", "($CE | 0) >> 8 & 255"),
            ("$CE = $KEY_312(($CE | 0) ^ 3413877106);", "$CE = $KEY_312(($CE | 0) ^ 3413877105);"),
            ("$CJ >>> 24", "$CJ >>> 8"),
        ],
    );
}

#[test]
fn late_flags_rule_needs_the_holder_cursor_and_late_flags() {
    let positive = late_flags_positive();
    refuses(
        &positive,
        "$THREAD_8_24419",
        NAME,
        &[
            ("let $CG = $KEY_158($CE);", "let $CG = $KEY_158($KEY_23);"),
            ("let $CG = $KEY_158($CE);", "let $CG = $KEY_312($CE);"),
            ("$CI = 4;", "$CI = 5;"),
            ("$CG = [$CG, $CH, $CF];", "$CG = [$CG, $CF, $CH];"),
            ("$CI = $CG[0]($CG[1], $CG[2], 3);", "$CI = $CG[1]($CG[0], $CG[2], 3);"),
            ("$CF[$CI + 1] =", "$CF[$CI + 2] ="),
            ("let $CH = $KEY_95;", "let $CH = $KEY_51;"),
        ],
    );
}

#[test]
fn late_flags_rule_needs_the_captured_removal_flag_and_observer() {
    let positive = late_flags_positive();
    refuses(
        &positive,
        "$THREAD_8_24419",
        NAME,
        &[
            (
                "($KEY_0 | 0 | $KEY_46 << 1 | $KEY_23 << 2)",
                "($KEY_46 | 0 | $KEY_0 << 1 | $KEY_23 << 2)",
            ),
            (
                "($KEY_0 | 0 | $KEY_46 << 1 | $KEY_23 << 2)",
                "($KEY_0 | 0 | $KEY_23 << 1 | $KEY_46 << 2)",
            ),
            (
                "($KEY_0 | 0 | $KEY_46 << 1 | $KEY_23 << 2)",
                "($KEY_0 | 0 | $KEY_46 << 1 | $KEY_0 << 2)",
            ),
            ("$KEY_67.next([[], $CF]);", "$KEY_51.next([[], $CF]);"),
            ("$KEY_67.next([[], $CK]);", "$KEY_95.next([[], $CK]);"),
        ],
    );
}

#[test]
fn late_flags_streaming_update_must_not_complete() {
    let positive = late_flags_positive();
    refuses(
        &positive,
        "$THREAD_8_24419",
        NAME,
        &[
            (
                "$KEY_67.next([[], $CF]);",
                "$KEY_67.next([[], $CF]);\n\t\t\t\t\t\t\t$KEY_67.complete();",
            ),
            (
                "$KEY_67.next([[], $CK]);",
                "$KEY_67.next([[], $CK]);\n\t\t\t\t\t\t$KEY_67.complete();",
            ),
        ],
    );
}

#[test]
fn late_flags_catch_must_reexamine_the_same_frame() {
    let positive = late_flags_positive();
    refuses(
        &positive,
        "$THREAD_8_24419",
        NAME,
        &[
            ("$KEY_46 = true;", "$KEY_23 = true;"),
            (
                "$CJ = $KEY_312($CJ ^ 3413877106);",
                "$CJ = $KEY_312($CJ ^ 3413877105);",
            ),
            ("$CK[4] = $CJ() ^ 130;", "$CK[4] = $CJ() ^ 129;"),
            ("let $CL = $KEY_158($CJ);", "let $CL = $KEY_158($KEY_23);"),
            ("$CK[$CN + 1] =", "$CK[$CN + 2] ="),
        ],
    );
}

#[test]
fn late_flags_factory_helpers_must_really_use_the_formal() {
    let positive = late_flags_positive();
    // An own child that ignores the factory formal is a nested no-op.
    assert!(positive.contains("$KEY_3 = $BR;"));
    let ignored = positive.replace("$KEY_3 = $BR;", "$KEY_3 = $KEY_175;");
    assert!(ignored.contains("$KEY_3 = $KEY_175;"));
    assert!(!ignored.contains("$KEY_3 = $BR;"));
    refused(&ignored, "$THREAD_8_24419", NAME);

    // The formal read in a dead local, a never-called nested function, an empty
    // control test or a discarded call argument is not a consumed seed value.
    for decoy in [
        "        let $BDEAD = $BR;",
        "        function $THREAD_DEAD() { return $BR; }",
        "        if ($BR) {}",
        "        $KEY_207($BR, 0);",
    ] {
        let source = late_flags_global_seeded_with(decoy);
        assert!(source.contains(decoy));
        assert!(!source.contains("$KEY_3 = $BR;"));
        refused(&source, "$THREAD_8_24419", NAME);
    }

    // A dead-overwritten factory alias: the formal write is replaced before the
    // returned child runs, so the formal is not a live carrier.
    let overwritten = positive.replace(
        "$KEY_3 = $BR;",
        "$KEY_3 = $BR;\n\t\t\t\t\t$KEY_3 = $KEY_175;",
    );
    assert!(overwritten.contains("$KEY_3 = $BR;\n\t\t\t\t\t$KEY_3 = $KEY_175;"));
    refused(&overwritten, "$THREAD_8_24419", NAME);
}

#[test]
fn late_flags_writer_factory_must_really_use_its_formal() {
    let positive = late_flags_positive();
    // The writer factory routes the formal into `$KEY_0`, which the child invokes
    // into the output bytes.  Seeding that alias from the captured global instead
    // removes the only live formal use.
    assert!(positive.contains("let $KEY_0 = $BR;"));
    let ignored = positive.replace("let $KEY_0 = $BR;", "let $KEY_0 = $KEY_175;");
    assert!(ignored.contains("let $KEY_0 = $KEY_175;"));
    refused(&ignored, "$THREAD_8_24419", NAME);
}

fn iso_storage_checked(source: &str, target: &str, accepted: bool) {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::unambiguous()).parse();
    assert!(!parsed.panicked && parsed.errors.is_empty(), "{:?}", parsed.errors);
    let result = name_threads(source, true).expect("ISO fixture must analyze");
    assert!(result.report.outcomes.iter().any(|o| o.binding.name == target), "missing {target}");
    let claimed = result.report.function_roles.iter().any(|r| {
        r.current_name.as_deref() == Some(target)
            && r.evaluators.iter().any(|e| e == "thread_posthost_storage_removal_emitter")
            && r.candidates.iter().any(|n| n == NAME)
    });
    assert_eq!(claimed, accepted, "family proof for {target}\n{source}");
    assert_eq!(result.report.outcomes.iter().any(|o| {
        o.binding.name == target && o.applied && o.proposed_name.as_deref() == Some(NAME)
    }), accepted, "applied role for {target}");
    assert_eq!(result.report.outcomes.iter().any(|o| {
        o.binding.name == target
            && o.evaluator.as_deref() == Some("thread_posthost_storage_removal_emitter")
    }), accepted, "target evaluator for {target}");
}

fn iso_storage_replace(from: &str, to: &str) -> String {
    assert_eq!(POSITIVE.matches(from).count(), 1, "mutation anchor {from}");
    POSITIVE.replace(from, to)
}

#[test]
fn factory_exit_storage_factory_alternate_exits_are_refused() {
    for child in ["$THREAD_1_68323", "$THREAD_7_93919"] {
        let anchor = format!("return {child};");
        for exit in [
            "if (true) return null;",
            "if ($BH) return function () { return 0; };",
            "{ if ($BH) { return void 0; } }",
        ] {
            let source = iso_storage_replace(&anchor, &format!("{exit} {anchor}"));
            iso_storage_checked(&source, "$THREAD_2_111500", false);
        }
    }
}

#[test]
fn factory_exit_storage_nested_returns_are_not_factory_exits() {
    let source = iso_storage_replace("return $THREAD_1_68323;",
        "function isoNestedExit() { return null; } return $THREAD_1_68323;");
    iso_storage_checked(&source, "$THREAD_2_111500", true);
}

#[test]
fn child_carrier_child_carrier_reset_is_refused_in_all_storage_forms() {
    // The first mutation resets the child carrier directly; the others retain the direct,
    // fixed-slot, and late-flags callbacks.
    for (source, target, anchor, reset) in [
        (POSITIVE.to_owned(), "$THREAD_2_111500", "let $BI = $KEY_1;", "$KEY_4 = 0;"),
        (direct_cursor_positive(), "$THREAD_2_125597", "let $BS = $KEY_2;", "$KEY_4 = 0;"),
        (fixed_slot_positive(), "$THREAD_1_115684", "let $BR = $KEY_2;", "$KEY_0 = 0;"),
        (late_flags_positive(), "$THREAD_8_24419", "let $BS = $KEY_2;", "$KEY_3 = 0;"),
    ] {
        assert_eq!(source.matches(anchor).count(), 1);
        iso_storage_checked(&source.replace(anchor, &format!("{reset}\n{anchor}")), target, false);
    }
}

#[test]
fn child_carrier_child_alias_kills_order_and_branch_joins_are_refused() {
    let anchor = "$KEY_0 = $KEY_2 + $KEY_4;";
    for replacement in [
        "let isoAlias = $KEY_4; isoAlias = 0; $KEY_0 = $KEY_2 + isoAlias;",
        "let isoAlias = 0; $KEY_0 = $KEY_2 + isoAlias; isoAlias = $KEY_4;",
        "let isoAlias = 0; if (false) { isoAlias = $KEY_4; } $KEY_0 = $KEY_2 + isoAlias;",
        "let isoAlias = 0; function isoDonor() { isoAlias = $KEY_4; } $KEY_0 = $KEY_2 + isoAlias;",
        "let isoAlias = 0; if ($BI) { isoAlias = $KEY_4; } $KEY_0 = $KEY_2 + isoAlias;",
        "let isoAlias = $KEY_4; if ($BI) { isoAlias = 0; } else { isoAlias = 0; } $KEY_0 = $KEY_2 + isoAlias;",
        "$KEY_0 = $KEY_2 + $KEY_4; $KEY_0 = 0;",
    ] {
        iso_storage_checked(&iso_storage_replace(anchor, replacement), "$THREAD_2_111500", false);
    }
}

#[test]
fn child_carrier_live_child_aliases_and_branch_joins_remain_supported() {
    for replacement in [
        "let isoAlias = $KEY_4; $KEY_0 = $KEY_2 + isoAlias;",
        "let isoAlias = 0; isoAlias = $KEY_4; $KEY_0 = $KEY_2 + isoAlias;",
        "let isoAlias; if ($BI) { isoAlias = $KEY_4; } else { isoAlias = $KEY_4; } $KEY_0 = $KEY_2 + isoAlias;",
        "function isoDonor() { $KEY_4 = 0; } $KEY_0 = $KEY_2 + $KEY_4;",
        "if (false) { $KEY_4 = 0; } $KEY_0 = $KEY_2 + $KEY_4;",
    ] {
        iso_storage_checked(&iso_storage_replace("$KEY_0 = $KEY_2 + $KEY_4;", replacement), "$THREAD_2_111500", true);
    }
}

#[test]
fn child_carrier_returned_writer_carrier_reset_is_refused() {
    let source = iso_storage_replace("function $THREAD_7_93919($BI, $BJ, $BK) {",
        "function $THREAD_7_93919($BI, $BJ, $BK) { $KEY_2 = function () { return 0; };");
    iso_storage_checked(&source, "$THREAD_2_111500", false);
}

#[test]
fn reaching_seed_unreachable_literal_seed_returns_cannot_rescue_carrier_reset() {
    // An unreachable literal seed return must not rescue the reset carrier in the supported
    // source.
    for test in [
        "0", "0.0", "0x0", "-0", "+0", "false", "null", "\"\"", "!true", "!!0", "void 0",
    ] {
        let replacement = format!(
            "if ({test}) return $KEY_4 & 255; $KEY_4 = 0; let $BI = $KEY_1;"
        );
        let source = iso_storage_replace("let $BI = $KEY_1;", &replacement);
        assert!(source.contains(&replacement), "missing exact mutant {test}");
        iso_storage_checked(&source, "$THREAD_2_111500", false);
    }
    // A dead alternate cannot supply a witness either. The reachable arm
    // kills the carrier before the real four-way stream body executes.
    for test in ["true", "1", "-1", "+1", "\"seed\"", "!0", "!!1"] {
        let replacement = format!(
            "if ({test}) {{ $KEY_4 = 0; }} else {{ return $KEY_4 & 255; }} let $BI = $KEY_1;"
        );
        let source = iso_storage_replace("let $BI = $KEY_1;", &replacement);
        assert!(source.contains(&replacement), "missing exact mutant {test}");
        iso_storage_checked(&source, "$THREAD_2_111500", false);
    }
}

#[test]
fn reaching_seed_arithmetic_dead_seed_return_cannot_rescue_carrier_reset() {
    // An arithmetic dead seed return must refuse with a clean parse and the original callback
    // still present.
    let replacement = "if (1 - 1) return $KEY_4 & 255; $KEY_4 = 0; let $BI = $KEY_1;";
    let source = iso_storage_replace("let $BI = $KEY_1;", replacement);
    assert!(source.contains(replacement));
    iso_storage_checked(&source, "$THREAD_2_111500", false);
}

#[test]
fn reaching_seed_each_return_requires_its_own_reaching_seed() {
    for prefix in [
        "if (1 === 2) return $KEY_4 & 255; $KEY_4 = 0;",
        "if ((2 * 3) - 6) return $KEY_4 & 255; $KEY_4 = 0;",
        "if ($KEY_1) return $KEY_4 & 255; $KEY_4 = 0;",
        "if ($KEY_1) { return $KEY_4 & 255; } else { $KEY_4 = 0; }",
        "if (1 === 2) { return $KEY_4 & 255; } else { $KEY_4 = 0; }",
        "if (1 === 2) { $KEY_4 = 0; } else { return 0; }",
        "if ($KEY_1) return 0;",
        "if ($KEY_1) return;",
        "if ($KEY_1) return void 0;",
        "if ($KEY_1) return $KEY_232 & 255;",
        "return 0;",
        "return $KEY_232 & 255;",
        "while (false) { return $KEY_4 & 255; } $KEY_4 = 0;",
    ] {
        let replacement = format!("{prefix} let $BI = $KEY_1;");
        let source = iso_storage_replace("let $BI = $KEY_1;", &replacement);
        assert!(source.contains(&replacement), "missing exact mutant {prefix}");
        iso_storage_checked(&source, "$THREAD_2_111500", false);
    }
    // A good update return cannot hide a killed or unseeded cached lane.
    for replacement in [
        "return $KEY_232 >> 8 & 255;",
        "$KEY_0 = 0; return $KEY_0 >> 8 & 255;",
        "return 0;",
    ] {
        iso_storage_checked(
            &iso_storage_replace("return $KEY_0 >> 8 & 255;", replacement),
            "$THREAD_2_111500",
            false,
        );
    }
}

#[test]
fn reaching_seed_each_seeded_return_and_both_branch_returns_remain_supported() {
    for prefix in [
        "if ($KEY_1) return $KEY_4 & 255;",
        "if (1 === 2) return $KEY_4 & 255;",
        "if ($KEY_1) { return $KEY_4 & 255; } else { return $KEY_4 >> 8 & 255; }",
        "if ($KEY_1) { let isoByte = $KEY_4; return isoByte & 255; } else { return $KEY_4 & 255; }",
        "if (false) return 0;",
        "function isoDeadReturn() { return 0; }",
    ] {
        let replacement = format!("{prefix} let $BI = $KEY_1;");
        iso_storage_checked(
            &iso_storage_replace("let $BI = $KEY_1;", &replacement),
            "$THREAD_2_111500",
            true,
        );
    }
}

#[test]
fn reaching_seed_live_literal_arms_and_real_storage_streams_remain_supported() {
    for test in [
        "0", "-0", "+0", "false", "null", "\"\"", "!true", "!!0", "void 0",
    ] {
        let replacement = format!("if ({test}) {{ $KEY_4 = 0; }} let $BI = $KEY_1;");
        iso_storage_checked(
            &iso_storage_replace("let $BI = $KEY_1;", &replacement),
            "$THREAD_2_111500",
            true,
        );
    }
    for test in ["true", "1", "-1", "+1", "\"seed\"", "!0", "!!1"] {
        let replacement = format!(
            "if ({test}) {{}} else {{ $KEY_4 = 0; }} let $BI = $KEY_1;"
        );
        iso_storage_checked(
            &iso_storage_replace("let $BI = $KEY_1;", &replacement),
            "$THREAD_2_111500",
            true,
        );
    }
    for (source, target) in [
        (POSITIVE.to_owned(), "$THREAD_2_111500"),
        (direct_cursor_positive(), "$THREAD_2_125597"),
        (fixed_slot_positive(), "$THREAD_1_115684"),
        (late_flags_positive(), "$THREAD_8_24419"),
    ] {
        iso_storage_checked(&source, target, true);
    }
}
